//! Population growth and death.
//!
//! Source: `ChgPopFromPlanet` (`1038:7082`) and `PctTrueMaxGrowth`
//! (`10e0:65d4`) in `stars.2.7j.exe`, cross-checked against the reconstructed
//! NB09 C (`util.c`, `race.c`) and `MANUAL.PDF` pp. 6-3..6-4. Full derivation
//! in `docs/formulas/population.md`.
//!
//! Population is stored in units of 100 colonists, but the growth arithmetic
//! is carried out at 1/100th of that unit and the remainder is kept in the
//! planet's [`Planet::delta_pop`] accumulator, so that a colony growing by less
//! than one stored unit a year still grows.

use crate::hab::{calc_planet_max_pop_with_starbase, pct_planet_desirability};
use crate::planet::Planet;
use crate::race::{Prt, Race};

/// The result of one year of population change on a planet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopChange {
    /// Change in population, in units of 100 colonists. Negative on a hostile
    /// or overcrowded planet.
    pub delta: i32,
    /// The planet's new fractional-population accumulator.
    pub delta_accum: u8,
}

/// The race's true maximum growth rate, as a percentage per year.
///
/// Hyper Expansion races grow at twice their displayed rate
/// (`MANUAL.PDF` p. 6-3).
#[must_use]
pub fn pct_true_max_growth(race: &Race) -> i16 {
    let ideal = i16::from(race.pct_ideal_growth);
    if race.prt() == Some(Prt::He) {
        ideal * 2
    } else {
        ideal
    }
}

/// Compute one year of population change for a planet, without applying it.
///
/// Returns `None` for an unowned or empty planet, and for Alternate Reality
/// races (whose maximum population depends on the starbase hull — see
/// [`chg_pop_from_planet_with_starbase`]).
///
/// Use [`apply_pop_change`] to write the result back.
#[must_use]
pub fn chg_pop_from_planet(planet: &Planet, race: &Race) -> Option<PopChange> {
    chg_pop_from_planet_with_starbase(planet, race, None)
}

/// [`chg_pop_from_planet`], told the hull of the planet's starbase so an
/// Alternate Reality race's maximum is known.
///
/// `ChgPopFromPlanet` (`1038:7096`) grows an AR planet by the same rule as
/// any other against `CalcPlanetMaxPop`'s figure; a maximum of nothing
/// (no starbase) leaves the growth uncrowded, as the original's `max / 4 <
/// pop` test followed by a division by that maximum would not survive.
#[must_use]
pub fn chg_pop_from_planet_with_starbase(
    planet: &Planet,
    race: &Race,
    starbase_hull: Option<i16>,
) -> Option<PopChange> {
    let pop_old = planet.pop;
    if planet.owner.is_none() || pop_old == 0 {
        return None;
    }

    let hab = pct_planet_desirability(planet, race);
    let mut delta_cur = i32::from(planet.delta_pop);
    let pop_inc = if hab < 0 {
        // ---- hostile planet: colonists die ----
        // A habitability of -9 kills 0.9% a year: pop * 9 / 10 / 100.
        let deaths_100 = {
            let d = i64::from(pop_old) * i64::from(-hab) / 10;
            if d <= 1 {
                1
            } else {
                d
            }
        };

        let mut whole = (deaths_100 / 100) as i32;
        let mut frac = (deaths_100 % 100) as i32;
        if whole == 0 && frac == 0 {
            frac = 1;
        }

        delta_cur -= frac;
        if delta_cur < 0 {
            whole += 1;
            delta_cur += 100;
        }
        -whole
    } else {
        // ---- growth ----
        let max_pop = calc_planet_max_pop_with_starbase(planet, race, starbase_hull)?;
        // Growth is the race's maximum rate scaled by habitability, so the
        // units here are hundredths of a percent.
        let mut pct_grow = i32::from(pct_true_max_growth(race)) * i32::from(hab);

        if max_pop != 0 && pop_old > max_pop / 4 {
            {
                // Past a quarter of capacity growth begins to plateau
                // (`MANUAL.PDF` p. 6-3).
                let pct_full = ((i64::from(pop_old) * 1000) / i64::from(max_pop)) as i32;

                if pop_old >= max_pop {
                    // At or over capacity. Within 10 units of it nothing
                    // happens at all; beyond that colonists die, reaching 12%
                    // a year at 400% of capacity.
                    if pop_old < max_pop + 10 {
                        return Some(PopChange {
                            delta: 0,
                            delta_accum: planet.delta_pop,
                        });
                    }
                    let pct_retard = (99 - pct_full / 10).max(-300);
                    pct_grow = pct_retard << 2;
                } else {
                    // Crowding: growth is scaled by ((1000 - fullness)/750)^2,
                    // which is 1.0 at 25% of capacity and 0 at capacity.
                    let retard = i64::from(1000 - pct_full);
                    let retard_sq = retard * retard;
                    pct_grow = if pct_grow < 1000 {
                        (i64::from(pct_grow) * retard_sq / 562_500) as i32
                    } else {
                        // The original divides first to stay inside 32 bits.
                        let t = i64::from(pct_grow / 10) * retard_sq / 562_500;
                        (t * 10) as i32
                    };
                }
            }
        }

        // The original computes this twice, preferring the more precise form
        // whenever it cannot overflow.
        let coarse = i64::from(pop_old) * i64::from(pct_grow / 100);
        let growth_100 = if coarse < 10_000_000 {
            i64::from(pop_old) * i64::from(pct_grow) / 100
        } else {
            coarse
        };

        let mut whole = (growth_100 / 100) as i32;
        let mut frac = (growth_100 % 100) as i32;
        if whole == 0 && frac == 0 {
            frac = 1;
        }

        delta_cur += frac;
        if delta_cur >= 100 {
            whole += 1;
            delta_cur -= 100;
        } else if delta_cur < 0 {
            whole -= 1;
            delta_cur += 100;
        }
        whole
    };

    Some(PopChange {
        delta: pop_inc,
        delta_accum: u8::try_from(delta_cur.rem_euclid(256)).unwrap_or(0),
    })
}

/// Apply a computed population change to a planet.
pub fn apply_pop_change(planet: &mut Planet, change: PopChange) {
    planet.delta_pop = change.delta_accum;
    planet.pop += change.delta;
}

/// Advance a planet's population by one year, returning the change applied.
pub fn update_population(planet: &mut Planet, race: &Race) -> Option<PopChange> {
    update_population_with_starbase(planet, race, None)
}

/// [`update_population`], told the hull of the planet's starbase.
pub fn update_population_with_starbase(
    planet: &mut Planet,
    race: &Race,
    starbase_hull: Option<i16>,
) -> Option<PopChange> {
    let change = chg_pop_from_planet_with_starbase(planet, race, starbase_hull)?;
    apply_pop_change(planet, change);
    Some(change)
}
