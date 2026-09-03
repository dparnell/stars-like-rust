//! Resource output, and the population limits on operable mines and factories.
//!
//! Source: `CResourcesAtPlanet` (`1048:788e`), `CMaxMines` (`1048:7248`),
//! `CMaxOperableMines` (`1048:7304`), `CMaxFactories` (`1048:755c`) and
//! `CMaxOperableFactories` (`1048:7618`) in `stars.2.7j.exe`, cross-checked
//! against the reconstructed NB09 C (`planet.c`) and `MANUAL.PDF` pp. 6-2..6-3.
//! Full derivation in `docs/formulas/resources.md`.

use crate::hab::calc_planet_max_pop;
use crate::planet::Planet;
use crate::population::chg_pop_from_planet;
use crate::race::{Race, RaceStat};

/// The most mines this planet could ever operate, set by its maximum
/// population rather than its current one. Never less than 10.
#[must_use]
pub fn max_mines(planet: &Planet, race: &Race) -> i16 {
    if race.is_ar() {
        return 0;
    }
    let Some(max_pop) = calc_planet_max_pop(planet, race) else {
        return 0;
    };
    let count = i64::from(max_pop) * i64::from(race.stat(RaceStat::MineOperate)) / 100;
    i16::try_from(count.max(10)).unwrap_or(i16::MAX)
}

/// The most mines the planet's **current** population can staff.
///
/// With `next_year` the population is advanced first, which is how the
/// production screen shows what will be operable after growth.
#[must_use]
pub fn max_operable_mines(planet: &Planet, race: &Race, next_year: bool) -> i16 {
    if race.is_ar() {
        return 0;
    }
    let by_rule = max_mines(planet, race);
    let mut pop = i64::from(planet.pop);
    if next_year {
        if let Some(change) = chg_pop_from_planet(planet, race) {
            pop += i64::from(change.delta);
        }
    }
    let by_pop = pop * i64::from(race.stat(RaceStat::MineOperate)) / 100;
    let count = i16::try_from(by_pop).unwrap_or(i16::MAX).min(by_rule);
    count.max(1)
}

/// The most factories this planet could ever operate. Never less than 10.
#[must_use]
pub fn max_factories(planet: &Planet, race: &Race) -> i16 {
    if race.is_ar() {
        return 0;
    }
    let Some(max_pop) = calc_planet_max_pop(planet, race) else {
        return 0;
    };
    let count = i64::from(max_pop) * i64::from(race.stat(RaceStat::FactOperate)) / 100;
    i16::try_from(count.max(10)).unwrap_or(i16::MAX)
}

/// The most factories the planet's current population can staff.
#[must_use]
pub fn max_operable_factories(planet: &Planet, race: &Race, next_year: bool) -> i16 {
    if race.is_ar() {
        return 0;
    }
    let by_rule = max_factories(planet, race);
    let mut pop = i64::from(planet.pop);
    if next_year {
        if let Some(change) = chg_pop_from_planet(planet, race) {
            pop += i64::from(change.delta);
        }
    }
    let by_pop = pop * i64::from(race.stat(RaceStat::FactOperate)) / 100;
    let count = i16::try_from(by_pop).unwrap_or(i16::MAX).min(by_rule);
    count.max(1)
}

/// The factories actually running on the planet this year.
#[must_use]
pub fn factories_operating(planet: &Planet, race: &Race) -> i16 {
    if planet.owner.is_none() || race.is_ar() {
        return 0;
    }
    planet
        .factories
        .min(max_operable_factories(planet, race, false))
}

/// The resources a planet generates in a year.
///
/// Colonists contribute `pop / colonists_per_resource`, and each operating
/// factory contributes `factory_output / 10` (rounded up in aggregate).
/// Population beyond capacity is only half as productive, and population past
/// 300% of capacity does no useful work at all (`MANUAL.PDF` p. 6-3).
///
/// An inhabited planet always produces at least 1.
///
/// Alternate Reality races do not use factories at all; `energy_tech` is their
/// energy technology level, which drives the separate formula in
/// [`ar_resources_at_planet`] and is ignored for every other race.
///
/// Returns `None` only when the planet's maximum population cannot be
/// determined.
#[must_use]
pub fn resources_at_planet(planet: &Planet, race: &Race, energy_tech: i16) -> Option<i16> {
    if planet.pop == 0 {
        return Some(0);
    }
    if race.is_ar() {
        // The overcrowding clamp below needs a maximum population, which for
        // an AR race comes from its starbase hull and is not modelled yet.
        // Skipping it only matters for a planet holding more than its
        // starbase supports.
        return Some(ar_resources_at_planet(
            planet,
            race,
            i64::from(planet.pop),
            energy_tech,
        ));
    }

    let max_pop = calc_planet_max_pop(planet, race)?;

    // Overcrowding: colonists between 100% and 300% of capacity work at half
    // efficiency, and anything past 300% not at all.
    let mut pop = i64::from(planet.pop);
    let cap = i64::from(max_pop);
    if pop > cap {
        pop = cap + (pop - cap) / 2;
        if pop > 2 * cap {
            pop = 2 * cap;
        }
    }

    let per_resource = i64::from(race.stat(RaceStat::ResGen));
    let mut resources = if per_resource == 0 {
        0
    } else {
        pop / per_resource
    };

    let factories = i64::from(
        planet
            .factories
            .min(max_operable_factories(planet, race, false)),
    );
    let output = i64::from(race.stat(RaceStat::FactProd));
    resources += (factories * output + 9) / 10;

    Some(i16::try_from(resources.max(1)).unwrap_or(i16::MAX))
}

/// The energy technology level below which an Alternate Reality planet is
/// costed as if it had one (`1048:79bf`).
const AR_MIN_ENERGY: i64 = 1;

/// The habitability value below which an Alternate Reality planet is costed as
/// if it were 25% (`1048:79cd` compares against `0x19`).
const AR_MIN_DESIRABILITY: i64 = 25;

/// What one planet yields for an Alternate Reality race.
///
/// Source: the `rsMajorAdv == 8` branch of `CResourcesAtPlanet`
/// (`1048:7990`). AR races have no factories; a planet's output instead grows
/// with the square root of its population scaled by energy technology, and is
/// then weighted by how habitable the planet is:
///
/// ```text
/// floor(sqrt(pop * energy / colonists_per_resource) * desirability / 10 + 0.999)
/// ```
///
/// The trailing `0.999` (the double at `1120:1d3e`) makes the truncation a
/// round *up*. `pop` is the overcrowding-adjusted population the caller has
/// already computed, in units of 100 colonists.
#[must_use]
pub fn ar_resources_at_planet(planet: &Planet, race: &Race, pop: i64, energy_tech: i16) -> i16 {
    let energy = i64::from(energy_tech).max(AR_MIN_ENERGY);
    let desirability =
        i64::from(crate::hab::pct_planet_desirability(planet, race)).max(AR_MIN_DESIRABILITY);
    let per_resource = i64::from(race.stat(RaceStat::ResGen)).max(1);

    // The original does this division and the square root in the x87 unit, so
    // it is a real division rather than an integer one.
    #[allow(clippy::cast_precision_loss)]
    let scaled = (pop as f64) * (energy as f64) / (per_resource as f64);
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let resources = (scaled.sqrt() * (desirability as f64) / 10.0 + 0.999) as i64;

    i16::try_from(resources.max(1)).unwrap_or(i16::MAX)
}
