//! Habitability value and maximum population.
//!
//! These two functions sit underneath almost every other planetary formula:
//! growth, mine and factory caps and resource output are all expressed in terms
//! of them.
//!
//! Sources: `PctPlanetDesirability` (`1048:6e1e`) and `CalcPlanetMaxPop`
//! (`1048:7096`) in `stars.2.7j.exe`, cross-checked against the reconstructed
//! NB09 C (`planet.c`) and `MANUAL.PDF` pp. 6-2..6-3. Full derivation in
//! `docs/formulas/habitability.md`.

use crate::planet::Planet;
use crate::race::{lrt, Prt, Race};

/// The planet's habitability ("Value") for a race, as a percentage.
///
/// A positive result is the percentage of the race's optimum the planet
/// supports. A **negative** result is a hostile planet: `-9` means the planet
/// kills 0.9% of its colonists per year (`MANUAL.PDF` p. 6-3).
///
/// The three environment variables each contribute the square of their
/// "percent ideal", the mean of the three squares is square-rooted, and the
/// result is scaled down by a modifier that penalises being more than halfway
/// from the centre toward an edge.
#[must_use]
pub fn pct_planet_desirability(planet: &Planet, race: &Race) -> i16 {
    // Accumulates the squared percent-ideal of each in-range variable, 0..30000.
    let mut pct_pos: i32 = 0;
    // Accumulates the out-of-range distance of each hostile variable, capped
    // at 15 each.
    let mut pct_neg: i32 = 0;
    // Scaling factor in 1/10000ths, reduced by off-centre variables.
    let mut pct_mod: i32 = 10_000;

    for i in 0..3 {
        let value = i32::from(planet.env[i]);
        let center = i32::from(race.env_center[i]);
        let min = i32::from(race.env_min[i]);
        let max = i32::from(race.env_max[i]);

        // A negative upper bound marks the race immune to this variable; it
        // contributes a perfect 100^2 and cannot be off-centre.
        if max < 0 {
            pct_pos += 10_000;
            continue;
        }

        // Outside the habitable range: accumulate a penalty instead, capped at
        // 15 clicks per variable, and contribute nothing positive.
        if value < min || value > max {
            let delta = if value < min {
                min - value
            } else {
                value - max
            };
            pct_neg += delta.min(15);
            continue;
        }

        let absdiff = (value - center).abs();
        // `d` is the half-width on the side the planet sits, so a lopsided
        // habitable range is measured against the near edge.
        let (d, d_penalty) = if value < center {
            let d = center - min;
            (d, (center - value) * 2 - d)
        } else {
            let d = max - center;
            (d, (value - center) * 2 - d)
        };

        // A zero-width half-range is unreachable for a valid race (the wizard
        // cannot produce one, and the original would divide by zero); treat the
        // centre as ideal.
        let pct_var = if d == 0 { 0 } else { absdiff * 100 / d };
        let pct_ideal = 100 - pct_var;
        pct_pos += pct_ideal * pct_ideal;

        // Past the halfway point toward the edge, scale the whole result down.
        if d_penalty > 0 && d > 0 {
            let denom = d * 2;
            pct_mod = pct_mod * (denom - d_penalty) / denom;
        }
    }

    if pct_neg != 0 {
        // Hostile planets report the negative penalty and ignore pct_pos.
        return -(pct_neg as i16);
    }

    // sqrt of the mean of the three squares, biased by +0.9 before truncation
    // (the original loads 0.9 from `DS:0x1d36` and truncates on the x87 stack).
    let base = ((f64::from(pct_pos) / 3.0).sqrt() + 0.9) as i32;
    ((base * pct_mod) / 10_000) as i16
}

/// The maximum population a planet will support for a race, in units of 100
/// colonists.
///
/// An optimal (100%) planet supports 10_000 = 1,000,000 colonists. Planets
/// below 5% habitability are all treated as 5%, and Alternate Reality races
/// live on their starbase instead — for them this returns `None`, because the
/// answer depends on the starbase hull, which the ship-design layer owns.
#[must_use]
pub fn calc_planet_max_pop(planet: &Planet, race: &Race) -> Option<i32> {
    if race.is_ar() {
        // AR maximum population comes from `rglPopMac[hull]`; not modelled
        // until ship designs are (delivery Step 5).
        return None;
    }

    Some(max_pop_for_hab(pct_planet_desirability(planet, race), race))
}

/// The maximum population implied by a habitability percentage, in units of
/// 100 colonists.
///
/// Split out from [`calc_planet_max_pop`] so the racial modifiers can be
/// checked against the manual's worked examples directly, and so callers can
/// answer "what would this planet hold once terraformed?" without inventing a
/// planet to ask about.
#[must_use]
pub fn max_pop_for_hab(hab: i16, race: &Race) -> i32 {
    // Everything positive below 5% is treated as 5% (`MANUAL.PDF` p. 6-3).
    let mut max_pop = if hab < 5 { 500 } else { i32::from(hab) * 100 };

    match race.prt() {
        // Hyper Expansion: half the normal maximum (500,000 on an optimal world).
        Some(Prt::He) => max_pop -= max_pop / 2,
        // Jack of All Trades: 20% more (1,200,000 on an optimal world).
        Some(Prt::Joat) => max_pop += max_pop / 5,
        _ => {}
    }

    // Only Basic Remote Mining adds a further 10%, applied after the PRT
    // adjustment: a JoaT/OBRM race reaches 1,320,000 (`MANUAL.PDF` p. 6-3).
    if race.has_lrt(lrt::OBRM) {
        max_pop += max_pop / 10;
    }

    max_pop
}
