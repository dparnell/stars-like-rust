//! Race advantage points: what the race wizard charges for a race, and what a
//! leftover balance buys on its homeworld.
//!
//! A race is designed by spending a fixed budget on habitability, growth rate,
//! economy, primary and lesser traits, and research costs. The balance left
//! over is not wasted: [`crate::newgame`] spends it at the start of the game on
//! extra surface minerals, richer mineral concentrations, or extra mines,
//! factories or defences, according to the race's `rsUseLeftover` setting.
//!
//! **Only a person's balance is ever asked for.** A computer player's homeworld
//! is stocked with the full fifty points whatever its race costs, so the
//! built-in opponents — several of which this function prices well below zero —
//! never go through it. That leaves the stock Humanoid's 25, confirmed twice
//! over by the turn-0 fixture, as the one independent check there is; see the
//! open question in `docs/formulas/new-game.md`.
//!
//! Sources: `CAdvantagePoints` (`10e0:444c`) and `LInnateRaceHabitability`
//! (`10e0:4cb2`), cross-checked against `race.c` in the reconstructed NB09
//! sources. `MANUAL.PDF` describes the budget in player terms (p. 3-2, "you
//! have 1650 advantage points to spend") but gives none of the coefficients,
//! so every number below comes from the binary.

use crate::hab::pct_planet_desirability;
use crate::planet::Planet;
use crate::race::{lrt, Prt, Race, RaceStat};

/// Points charged for each primary racial trait, indexed by [`Prt`]
/// (`rgRacePrimaryTrait`, `1078:b58c`).
const PRIMARY_TRAIT_POINTS: [i32; 10] = [40, 95, 45, 10, -100, -150, 120, 180, 90, -66];

/// Points charged for each of the fourteen lesser racial traits, indexed by the
/// trait's bit position (`rgRaceAdvDisPts`).
///
/// A negative entry is an advantage (it costs points); a positive entry is a
/// disadvantage (it refunds them).
const LRT_POINTS: [i32; 14] = [
    -235, -25, -159, -201, 40, -240, -155, 160, 240, 255, 325, 180, 70, 30,
];

/// Refund for making research fields more expensive, indexed by how many net
/// steps of "expensive" the race took (`rgRaceDisEnvPts`).
const EXPENSIVE_RESEARCH_POINTS: [i32; 6] = [150, 330, 540, 780, 1050, 1380];

/// The largest growth rate the wizard allows (`pctIdealGrowthMax`).
const MAX_IDEAL_GROWTH: i16 = 20;

/// Index of the first per-field research-cost byte in `Race::attrs`.
const TECH_BONUS_FIRST: usize = RaceStat::TechBonus1 as usize;
/// Index of the last per-field research-cost byte.
const TECH_BONUS_LAST: usize = TECH_BONUS_FIRST + 5;

/// The race's habitability integrated over the whole environment cube, at
/// three levels of terraforming.
///
/// This is the term that makes a wide, well-centred race expensive: it samples
/// desirability on an 11x11x11 grid across each axis's habitable band (a single
/// mid-band sample on an immune axis), squares each reading, weights the three
/// terraforming levels 7/5/6, and scales each axis by the width of the band it
/// covered. Terraforming levels are 0%, 5% and 15% — 8% and 17% for a race with
/// Total Terraforming.
///
/// Source: `LInnateRaceHabitability` (`10e0:4cb2`).
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn innate_race_habitability(race: &Race) -> i32 {
    let mut innate = 0.0f64;
    let has_tt = race.has_lrt(lrt::TT);
    let mut planet = Planet::unowned(0);

    for terra in 0..3 {
        let pct_terra: i16 = match terra {
            0 => 0,
            1 => {
                if has_tt {
                    8
                } else {
                    5
                }
            }
            _ => {
                if has_tt {
                    17
                } else {
                    15
                }
            }
        };

        // Per axis: where the sampling starts, how wide it runs, how many
        // samples. An immune axis is sampled once, in the middle.
        let mut base = [0i16; 3];
        let mut inc = [0i16; 3];
        let mut steps = [0i16; 3];
        for i in 0..3 {
            if race.is_immune(i) {
                base[i] = 50;
                inc[i] = 11;
                steps[i] = 1;
            } else {
                base[i] = (i16::from(race.env_min[i]) - pct_terra).max(0);
                let high = (i16::from(race.env_max[i]) + pct_terra).min(100);
                inc[i] = high - base[i];
                steps[i] = 11;
            }
        }

        // How far each axis had to be terraformed to reach the sample point;
        // the total is charged against the reading below.
        let mut delta = [0i16; 3];
        // Move the sample point toward the race's centre by up to `pct_terra`,
        // exactly as the original does, and remember what that cost.
        let sample = |axis: usize, mut try_value: i16, delta: &mut [i16; 3]| -> i16 {
            if terra != 0 && !race.is_immune(axis) {
                let centre = i16::from(race.env_center[axis]);
                let mut d = centre - try_value;
                if d.abs() <= pct_terra {
                    d = 0;
                } else if d < 0 {
                    d += pct_terra;
                } else {
                    d -= pct_terra;
                }
                delta[axis] = d;
                try_value = centre - d;
            }
            try_value
        };

        let mut terra_sum = 0.0f64;
        for i in 0..steps[0] {
            let mut value = if i == 0 || steps[0] <= 1 {
                base[0]
            } else {
                i * inc[0] / (steps[0] - 1) + base[0]
            };
            value = sample(0, value, &mut delta);
            planet.env[0] = value as i8;

            let mut i_sum = 0.0f64;
            for j in 0..steps[1] {
                let mut value = if j == 0 || steps[1] <= 1 {
                    base[1]
                } else {
                    j * inc[1] / (steps[1] - 1) + base[1]
                };
                value = sample(1, value, &mut delta);
                planet.env[1] = value as i8;

                let mut acc: u32 = 0;
                for k in 0..steps[2] {
                    let mut value = if k == 0 || steps[2] <= 1 {
                        base[2]
                    } else {
                        k * inc[2] / (steps[2] - 1) + base[2]
                    };
                    value = sample(2, value, &mut delta);
                    planet.env[2] = value as i8;

                    let mut desire = i32::from(pct_planet_desirability(&planet, race));
                    let delta_sum = delta[0] + delta[1] + delta[2];
                    if delta_sum > pct_terra {
                        desire -= i32::from(delta_sum - pct_terra);
                        desire = desire.max(0);
                    }
                    // The original squares in 32-bit unsigned and lets it wrap.
                    let square = (desire as u32).wrapping_mul(desire as u32);
                    let weight = match terra {
                        0 => 7,
                        1 => 5,
                        _ => 6,
                    };
                    acc = acc.wrapping_add(square.wrapping_mul(weight));
                }

                let scaled = if race.is_immune(2) {
                    (acc.wrapping_mul(11)) as i32
                } else {
                    ((acc.wrapping_mul(inc[2] as u16 as u32)) as i32) / 100
                };
                i_sum += f64::from(scaled);
            }

            if race.is_immune(1) {
                i_sum *= 11.0;
            } else {
                i_sum = i_sum * f64::from(inc[1]) / 100.0;
            }
            terra_sum += i_sum;
        }

        if race.is_immune(0) {
            terra_sum *= 11.0;
        } else {
            terra_sum = terra_sum * f64::from(inc[0]) / 100.0;
        }
        innate += terra_sum;
    }

    (innate / 10.0 + 0.5).floor() as i32
}

/// How many advantage points the race has **left over** after paying for
/// itself.
///
/// The wizard's budget is 1650 points (`550 * 3`, divided by three at the end,
/// so the player sees 550-point units). A race that spends exactly its budget
/// scores 0; a cheap race scores positive, and the new-game code spends up to
/// 50 of those on its homeworld.
///
/// Source: `CAdvantagePoints` (`10e0:444c`).
#[must_use]
pub fn advantage_points(race: &Race) -> i16 {
    let mut points: i32 = 550 * 3;
    let major = race.prt();
    let stat = |s: RaceStat| i32::from(race.stat(s));
    let attr = |i: usize| i32::from(race.attrs[i]);

    // Habitability: the wider and better-centred the race, the more it costs.
    let mut innate = i64::from(innate_race_habitability(race)) / 2000;

    // Growth rate. Below 6% refunds heavily; the value is then remapped onto
    // the scale the habitability term is charged against.
    let growth = i32::from(race.pct_ideal_growth).clamp(1, i32::from(MAX_IDEAL_GROWTH));
    let mut spread = growth;
    if spread > 5 {
        if spread > 13 {
            spread = if spread < 20 {
                5 + 8 * 2 + (spread - 13) * 3
            } else {
                5 + 8 * 2 + 18 + 6
            };
        } else {
            points += match spread {
                6 => 1200 * 3,
                7 => 750 * 3,
                8 => 200 * 3,
                9 => 75 * 3,
                _ => 0,
            };
            spread = 5 + (spread - 5) * 2;
        }
    } else {
        points += 1400 * 3 * (6 - spread);
    }
    innate = innate * i64::from(spread) / 24;
    points -= innate as i32;

    // Off-centre habitability bands are cheaper; two or more immunities get a
    // small further discount.
    let mut immunities = 0;
    for i in 0..3 {
        if race.is_immune(i) {
            immunities += 1;
        } else {
            points += (i32::from(race.env_center[i]) - 50).abs() * 4;
        }
    }
    if immunities > 1 {
        points -= 150;
    }

    // A race that both operates and runs many factories pays for the
    // combination, scaled by its growth rate.
    {
        let mut operate = stat(RaceStat::FactOperate);
        let mut produce = stat(RaceStat::FactProd);
        if operate > 10 || produce > 10 {
            operate = (operate - 9).max(1);
            produce = (produce - 9).max(1);
            produce *= if major == Some(Prt::He) { 3 } else { 2 };
            let cost = operate * produce * growth;
            points -= if immunities >= 2 { cost / 2 } else { cost / 9 };
        }
    }

    // Colonists per resource.
    let res_gen = stat(RaceStat::ResGen).min(25);
    if res_gen <= 7 {
        points -= 2400;
    } else if res_gen == 8 {
        points -= 1260;
    } else if res_gen == 9 {
        points -= 600;
    } else if res_gen > 10 {
        points += 120 * (res_gen - 10);
    }

    if major == Some(Prt::Ar) {
        // Alternate Reality has no planetary factories or mines to price.
        points += 210;
    } else {
        // Factory efficiency.
        let d = [
            10 - stat(RaceStat::FactProd),
            10 - stat(RaceStat::FactBuild),
            10 - stat(RaceStat::FactOperate),
        ];
        let mut cur = if d[0] > 0 { d[0] * 100 } else { d[0] * 121 };
        cur -= if d[1] < 0 {
            d[1] * 55
        } else {
            d[1] * d[1] * 60
        };
        cur += if d[2] > 0 { d[2] * 40 } else { d[2] * 35 };
        if cur > 700 {
            cur = 700 + (cur - 700) / 3;
        }
        if d[2] <= -7 {
            if d[2] >= -11 {
                cur -= 30 * (-6 - d[2]);
            } else if d[2] >= -14 {
                cur -= 75 * 3 + 45 * (-12 - d[2]);
            } else {
                cur -= 120 * 3;
            }
        }
        if d[0] <= -3 {
            cur -= (-2 - d[0]) * 20 * 3;
        }
        points += cur;

        if race.has_lrt(lrt::CHEAP_FACT) {
            points -= 175;
        }

        // Mine efficiency.
        let d = [
            10 - stat(RaceStat::MineProd),
            3 - stat(RaceStat::MineBuild),
            10 - stat(RaceStat::MineOperate),
        ];
        let mut cur = if d[0] > 0 { d[0] * 100 } else { d[0] * 169 };
        cur -= if d[1] <= 0 { -80 + d[1] * 65 } else { 360 };
        cur += if d[2] > 0 { d[2] * 40 } else { d[2] * 35 };
        points += cur;
    }

    if let Some(prt) = major {
        points -= PRIMARY_TRAIT_POINTS[prt as usize];
    }

    // Lesser racial traits, with a surcharge for taking many of them and for
    // an unbalanced mix.
    let (mut good, mut bad) = (0, 0);
    for (bit, cost) in LRT_POINTS.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        if race.has_lrt(bit as u32) {
            if *cost < 0 {
                bad += 1;
            } else {
                good += 1;
            }
            points += *cost;
        }
    }
    if bad + good > 4 {
        points -= ((bad + good) * 10) * (bad + good - 4);
    }
    if good - bad > 3 {
        points -= 60 * (good - bad - 3);
    }
    if bad - good > 3 {
        points -= 40 * (bad - good - 3);
    }

    // No Advanced Scanners is worth much more to some primary traits.
    if race.has_lrt(lrt::NO_ADV_SCANNER) {
        points -= match major {
            Some(Prt::Pp) => 280,
            Some(Prt::Ss) => 200,
            Some(Prt::Joat) => 40,
            _ => 0,
        };
    }

    // Research costs: `1` is normal, `2` is cheap, `0` is expensive.
    let net: i32 = (TECH_BONUS_FIRST..=TECH_BONUS_LAST)
        .map(|i| attr(i) - 1)
        .sum();
    if net > 0 {
        points -= net * net * 130;
        if net == 6 {
            points += (36 - 25) * 130;
        } else if net == 5 {
            points += (25 - 21) * 130;
        }
    } else if net < 0 {
        let index = usize::try_from(-net - 1).unwrap_or(0);
        points += EXPENSIVE_RESEARCH_POINTS
            .get(index)
            .copied()
            .unwrap_or_else(|| EXPENSIVE_RESEARCH_POINTS[EXPENSIVE_RESEARCH_POINTS.len() - 1]);
        if -net > 4 && stat(RaceStat::ResGen) < 10 {
            points -= 190;
        }
    }

    if race.has_lrt(lrt::TECH3) {
        points -= 180;
    }
    if major == Some(Prt::Ar) && attr(TECH_BONUS_FIRST) == 2 {
        points -= 100;
    }

    #[allow(clippy::cast_possible_truncation)]
    {
        (points / 3) as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stock Humanoid, whose leftover balance the turn-0 fixture pins at 25.
    #[test]
    fn the_humanoid_has_twenty_five_points_left() {
        assert_eq!(advantage_points(&Race::humanoid()), 25);
    }

    /// Widening the habitable range costs points; narrowing it refunds them.
    #[test]
    fn a_wider_race_is_more_expensive() {
        let narrow = {
            let mut race = Race::humanoid();
            race.env_min = [40, 40, 40];
            race.env_max = [60, 60, 60];
            race
        };
        let wide = {
            let mut race = Race::humanoid();
            race.env_min = [1, 1, 1];
            race.env_max = [99, 99, 99];
            race
        };
        assert!(
            advantage_points(&narrow) > advantage_points(&Race::humanoid()),
            "a narrow race should be cheaper than the baseline"
        );
        assert!(
            advantage_points(&wide) < advantage_points(&Race::humanoid()),
            "a wide race should be dearer than the baseline"
        );
    }

    /// Immunity on every axis is the most expensive habitability there is, and
    /// the habitability integral notices.
    #[test]
    fn immunity_costs_the_most() {
        let mut immune = Race::humanoid();
        immune.env_center = [0, 0, 0];
        immune.env_min = [0, 0, 0];
        immune.env_max = [-1, -1, -1];
        assert!(innate_race_habitability(&immune) > innate_race_habitability(&Race::humanoid()));
        assert!(advantage_points(&immune) < advantage_points(&Race::humanoid()));
    }

    /// A faster-growing race is dearer; below 6% the refund is large.
    #[test]
    fn growth_rate_dominates_the_budget() {
        let slow = {
            let mut race = Race::humanoid();
            race.pct_ideal_growth = 5;
            race
        };
        let fast = {
            let mut race = Race::humanoid();
            race.pct_ideal_growth = 20;
            race
        };
        assert!(advantage_points(&slow) > advantage_points(&fast) + 1000);
    }

    /// The habitability integral does not depend on the primary trait, only on
    /// the habitable bands, the growth-independent geometry and Total
    /// Terraforming.
    #[test]
    fn total_terraforming_widens_the_habitability_integral() {
        let plain = Race::humanoid();
        let mut tt = Race::humanoid();
        tt.lrt_bits |= 1 << lrt::TT;
        assert!(innate_race_habitability(&tt) > innate_race_habitability(&plain));
    }
}
