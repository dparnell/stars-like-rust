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
/// An inhabited planet always produces at least 1. Returns `None` for
/// Alternate Reality races, whose output depends on the starbase.
#[must_use]
pub fn resources_at_planet(planet: &Planet, race: &Race) -> Option<i16> {
    if planet.pop == 0 {
        return Some(0);
    }
    if race.is_ar() {
        return None;
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
