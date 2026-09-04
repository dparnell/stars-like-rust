//! Mining: how much of each mineral a planet's mines extract in a year, and
//! how fast that depletes the concentration underneath.
//!
//! Source: `EstMineralsMined` (`1028:5362`) in `stars.2.7j.exe`, cross-checked
//! against the reconstructed NB09 C (`mine.c`) and `MANUAL.PDF` pp. 13-2..13-3.
//! Full derivation in `docs/formulas/mining.md`.

use crate::planet::{Planet, MINERALS};
use crate::race::{Race, RaceStat};
use crate::resources::max_operable_mines;
use crate::rng::Rng;

/// Mine-years needed to drop a concentration of 1 by one point; the rate for
/// any other concentration is this divided by the concentration
/// (`MANUAL.PDF` p. 13-2).
const MINE_YEARS_PER_POINT: i32 = 12_500;

/// The number of a planet's mines that are actually staffed and operating.
///
/// Mines beyond what the population can operate sit idle.
#[must_use]
pub fn mines_operating(planet: &Planet, race: &Race) -> i16 {
    if planet.owner.is_none() {
        return 0;
    }
    if race.is_ar() {
        // Alternate Reality races mine from orbit: the planet itself operates
        // sqrt(population) mines.
        if planet.pop <= 0 {
            return 0;
        }
        return f64::from(planet.pop).sqrt() as i16;
    }
    planet.mines.min(max_operable_mines(planet, race, false))
}

/// How much of each mineral would be mined this year, in kT.
///
/// `mines` is the number of operating mines; pass `None` to use the planet's
/// own staffed mines. The mined quantity is
/// `mines * concentration * mine_efficiency / 10 / 100`, and the leftover
/// hundredths are resolved **probabilistically**: a remainder of 37 has a 37%
/// chance of yielding one more kT. That is the only RNG the planetary
/// economy consumes, and it is why `rng` is required to reproduce a turn.
///
/// Pass `rng: None` for the estimate the UI shows, which simply truncates.
#[must_use]
pub fn minerals_mined(
    planet: &Planet,
    race: &Race,
    mines: Option<i32>,
    rng: Option<&mut Rng>,
) -> [i32; MINERALS] {
    let remote = mines.is_some();
    let count = mines.unwrap_or_else(|| i32::from(mines_operating(planet, race)));

    // Remote (robot) miners always extract at efficiency 10; so do AR races.
    let efficiency = if remote || race.is_ar() {
        10
    } else {
        i32::from(race.stat(RaceStat::MineProd))
    };

    let mut out = [0i32; MINERALS];
    let mut rng = rng;
    for (i, slot) in out.iter_mut().enumerate() {
        let mut conc = i32::from(planet.min_conc[i]);
        // A home world never mines as if below 30, however depleted it is
        // (`MANUAL.PDF` p. 6-5).
        if conc < 30 && planet.homeworld && (!remote || race.is_ar()) {
            conc = 30;
        }

        let raw = count * conc;
        let scaled = if remote { raw } else { raw * efficiency / 10 };
        let mut quantity = scaled / 100;
        let remainder = scaled % 100;

        if remainder != 0 {
            if let Some(rng) = rng.as_deref_mut() {
                if i32::from(rng.random(100)) < remainder {
                    quantity += 1;
                }
            }
        }
        *slot = quantity;
    }
    out
}

/// Mine a planet for one year: add the minerals to the surface and deplete the
/// concentrations underneath.
///
/// Returns the quantities added, in kT.
pub fn mine_minerals(
    planet: &mut Planet,
    race: &Race,
    mines: Option<i32>,
    rng: &mut Rng,
) -> [i32; MINERALS] {
    let remote = mines.is_some();
    let count = mines.unwrap_or_else(|| i32::from(mines_operating(planet, race)));
    let mined = minerals_mined(planet, race, mines, Some(rng));

    for (i, &gain) in mined.iter().enumerate() {
        planet.surface_min[i] += gain;

        // Depletion is driven by *mine-years*, not by how much was actually
        // extracted, so an inefficient miner depletes a planet just as fast.
        let mut conc = i32::from(planet.min_conc[i]);
        if conc < 30 && planet.homeworld && (!remote || race.is_ar()) {
            conc = 30;
        }
        let mut decay_left = count * conc / 100;
        deplete(planet, i, &mut decay_left);
    }
    mined
}

/// Apply `decay_left` mine-years of depletion to one mineral.
fn deplete(planet: &mut Planet, i: usize, decay_left: &mut i32) {
    while *decay_left >= 1 && planet.min_conc[i] >= 2 {
        // A stored sub-level of 0 means a full 256/256ths remaining.
        let level = match planet.min_level[i] {
            0 => 256,
            l => i32::from(l),
        };
        // The threshold curve is flattened at the extremes: very rich planets
        // deplete no faster than concentration 100, and very poor ones no
        // faster than 10 (or 25 below concentration 25).
        let conc = match i32::from(planet.min_conc[i]) {
            c if c >= 101 => 100,
            c if c < 5 => 10,
            c if c < 25 => 25,
            c => c,
        };

        let threshold = MINE_YEARS_PER_POINT * level / 256 / conc;

        if *decay_left < threshold {
            // Not enough mining to lose a whole point: bank the progress.
            let per_point = MINE_YEARS_PER_POINT / conc;
            let mut new_level = if per_point == 0 {
                1
            } else {
                *decay_left * 256 / per_point
            };
            new_level = new_level.max(1);
            if new_level >= level {
                new_level = level - 1;
            }
            planet.min_level[i] = u8::try_from(new_level.clamp(0, 255)).unwrap_or(0);
            if new_level == 0 {
                planet.min_conc[i] -= 1;
            }
            return;
        }

        *decay_left -= threshold;
        planet.min_conc[i] -= 1;
        planet.min_level[i] = 0;
    }
}

/// The most mines one fleet can bring to bear from orbit.
///
/// Source: `CMineFromLpfl` clamps its total once it passes 3999.
pub const REMOTE_MINE_CAP: i32 = 4000;

/// How many mines a fleet operates on a planet it is orbiting.
///
/// Source: `CMineFromLpfl` (`1080:2600`). Each design in the fleet contributes
/// the sum of its mining slots — the number fitted in the slot times that
/// part's rating — multiplied by how many ships of that design are present.
/// The total is capped at [`REMOTE_MINE_CAP`].
///
/// A Robo-Midget Miner rates 5, so a ship with two of them mines as ten
/// planetary mines would, and a stack of twenty such ships as two hundred.
///
/// What those mines then extract is [`minerals_mined`] with an explicit count:
/// remote miners always work at efficiency 10 regardless of the race's mining
/// skill, and they do not benefit from the homeworld concentration floor.
#[must_use]
pub fn remote_mines(
    designs: &[crate::design::ShipDesign],
    stacks: &[crate::fleet::ShipStack],
) -> i32 {
    use crate::components::{slot, MINING};

    let mut total: i64 = 0;
    for stack in stacks {
        if stack.count <= 0 {
            continue;
        }
        let Some(design) = designs.get(usize::from(stack.design)) else {
            continue;
        };
        let per_ship: i64 = design
            .slots
            .iter()
            .filter(|s| s.category == slot::MINING)
            .map(|s| {
                let rating = MINING
                    .get(usize::from(s.item))
                    .map_or(0, |p| i64::from(p.ability));
                i64::from(s.count) * rating
            })
            .sum();
        total += per_ship * i64::from(stack.count);
    }
    i32::try_from(total.min(i64::from(REMOTE_MINE_CAP))).unwrap_or(REMOTE_MINE_CAP)
}

#[cfg(test)]
mod remote_tests {
    use super::*;
    use crate::components::slot;
    use crate::design::{DesignSlot, ShipDesign};
    use crate::fleet::ShipStack;

    fn miner(count: u8) -> ShipDesign {
        ShipDesign {
            name: String::new(),
            picture: 0,
            stored_armor: 0,
            hull_id: 0,
            slots: vec![DesignSlot {
                category: slot::MINING,
                item: 0, // Robo-Midget Miner, rated 5
                count,
            }],
        }
    }

    fn stack(design: u8, count: i32) -> ShipStack {
        ShipStack {
            design,
            count,
            damaged_pct: 0,
            damage_pct: 0,
        }
    }

    #[test]
    fn mines_scale_with_the_slot_and_the_stack() {
        let designs = vec![miner(2)];
        // Two miners rated 5 on each of twenty ships.
        assert_eq!(remote_mines(&designs, &[stack(0, 20)]), 200);
        assert_eq!(remote_mines(&designs, &[stack(0, 1)]), 10);
        assert_eq!(remote_mines(&designs, &[stack(0, 0)]), 0);
    }

    #[test]
    fn a_fleet_with_no_miners_operates_none() {
        let plain = ShipDesign {
            name: String::new(),
            picture: 0,
            stored_armor: 0,
            hull_id: 0,
            slots: vec![DesignSlot {
                category: slot::ENGINE,
                item: 1,
                count: 1,
            }],
        };
        assert_eq!(remote_mines(&[plain], &[stack(0, 50)]), 0);
    }

    #[test]
    fn the_total_is_capped() {
        let designs = vec![miner(4)];
        assert_eq!(remote_mines(&designs, &[stack(0, 10_000)]), REMOTE_MINE_CAP);
    }
}
