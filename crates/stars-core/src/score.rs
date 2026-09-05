//! The player scoreboard.
//!
//! Every year the host works out a score for each player and ranks them by it.
//! The number is what the score screen shows and what the victory conditions
//! are measured against; the block it goes into is decoded in
//! [`stars_formats::score`], and the formula here is `CalcPlayerScore`
//! (`1038:58a6`).
//!
//! See `docs/formulas/scores.md`.

use crate::design::ShipDesign;
use crate::GameState;

/// What a player scores, and the counts the scoreboard shows alongside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerScore {
    /// The score itself.
    pub score: i32,
    /// Rank, `1` for the leader. Filled in by [`scores`].
    pub rank: u16,
    /// Resources the player's planets produce in a year.
    pub resources: i32,
    /// Planets owned.
    pub planets: i32,
    /// Starbases with a dock; an Orbital Fort does not count.
    pub starbases: i32,
    /// Ships with no offensive power.
    pub unarmed_ships: i32,
    /// Armed ships under [`CAPITAL_POWER`] power.
    pub escort_ships: i32,
    /// Armed ships at or over [`CAPITAL_POWER`].
    pub capital_ships: i32,
    /// The six tech levels added together.
    pub tech_levels: i32,
}

/// The power at which a design stops being an escort and becomes a capital
/// ship (`1038:5b4a`).
pub const CAPITAL_POWER: i32 = 2000;

/// A design's combat power, from `LComputePower` (`1038:0b32`).
///
/// Beams count `damage × count × (range + 3) / 4`, a **sapper** a third of
/// that; torpedoes `damage × count × (range - 2) / 2`; bombs
/// `(colonists killed + installations destroyed) × count × 2`. Capacitors then
/// multiply the beam total — each one raises a running thousand by its own
/// percentage, and the result, divided by ten and capped at 255, is applied to
/// the beams as a percentage.
///
/// The original adds one more term, `beams × (speed - 4) / 10`, where the speed
/// is the design's battle speed from `SpdOfShip`. That routine is not recovered
/// yet, so this leaves the term out; a design close to [`CAPITAL_POWER`] could
/// therefore be classified one step low.
#[must_use]
pub fn design_power(design: &ShipDesign) -> i32 {
    use crate::components::{slot, BEAMS, BOMBS, SPECIALS_E, TORPEDOES};

    let mut beams: i64 = 0;
    let mut torpedoes: i64 = 0;
    let mut bombs: i64 = 0;
    let mut capacitors: i64 = 1000;

    for fitted in &design.slots {
        if fitted.count == 0 {
            continue;
        }
        let count = i64::from(fitted.count);
        let item = usize::from(fitted.item);
        match fitted.category {
            c if c == slot::BEAM => {
                let Some(beam) = BEAMS.get(item) else {
                    continue;
                };
                let mut power = i64::from(beam.dp) * count * (i64::from(beam.range_max) + 3) / 4;
                if beam.abilities & 1 != 0 {
                    // A sapper hurts shields, not hulls; it counts for a third.
                    power /= 3;
                }
                beams += power;
            }
            c if c == slot::TORPEDO => {
                let Some(torpedo) = TORPEDOES.get(item) else {
                    continue;
                };
                torpedoes += i64::from(torpedo.dp) * count * (i64::from(torpedo.range_max) - 2) / 2;
            }
            c if c == slot::BOMB => {
                let Some(bomb) = BOMBS.get(item) else {
                    continue;
                };
                bombs +=
                    (i64::from(bomb.colonist_damage) + i64::from(bomb.building_damage)) * count * 2;
            }
            c if c == slot::SPECIAL_E => {
                // Items 12 and 13 are the Energy and Flux Capacitors.
                if item != 12 && item != 13 {
                    continue;
                }
                let Some(part) = SPECIALS_E.get(item) else {
                    continue;
                };
                for _ in 0..count {
                    capacitors = capacitors * (100 + i64::from(part.ability)) / 100;
                }
            }
            _ => {}
        }
    }

    if capacitors != 1000 {
        let scale = (capacitors / 10).min(255);
        beams = beams * scale / 100;
    }
    i32::try_from(bombs + beams + torpedoes).unwrap_or(i32::MAX)
}

/// Which of the three classes a design's ships count as.
///
/// `1038:5b14`: no power at all is unarmed, under [`CAPITAL_POWER`] an escort,
/// and the rest are capital ships.
#[must_use]
pub fn ship_class(power: i32) -> usize {
    if power <= 0 {
        0
    } else if power < CAPITAL_POWER {
        1
    } else {
        2
    }
}

/// Pack a number the way a score block does (`WPackLong`, `1038:4ba2`).
///
/// Thirteen bits of value and three of a shift: while the number does not fit
/// in thirteen bits it is halved twice and the shift counted, so a large count
/// is stored roughly rather than exactly.
#[must_use]
pub fn pack(value: i32) -> u16 {
    let mut value = value.max(0);
    let mut shift = 0u16;
    while value >= 0x2000 && shift < 7 {
        value >>= 2;
        shift += 1;
    }
    (shift << 13) | u16::try_from(value & 0x1FFF).unwrap_or(0)
}

/// Undo [`pack`].
#[must_use]
pub fn unpack(packed: u16) -> i32 {
    i32::from(packed & 0x1FFF) << (2 * (packed >> 13))
}

/// Work out one player's score.
///
/// `CalcPlayerScore` (`1038:58a6`), in its own order:
///
/// * **planets** give a point per hundred thousand colonists, six at most —
///   nothing for the planet itself;
/// * **starbases** give three apiece, but only those whose hull has a dock, so
///   an Orbital Fort is worth nothing;
/// * **resources** give a point per thirty;
/// * **tech levels** rise: a point a level to 3, two a level to 6, three to 9
///   and four beyond, less the offsets that keep the scale continuous. A dead
///   player scores none of it;
/// * **ships** are counted by class and **capped by the number of planets** —
///   a huge fleet over a small empire scores as though it were small. Unarmed
///   ships are worth half a point, escorts two, and capital ships
///   `8 × ships × planets / (ships + planets)`.
#[must_use]
pub fn score_for(state: &GameState, player: usize) -> PlayerScore {
    let Some(record) = state.players.get(player) else {
        return PlayerScore::default();
    };
    let owner = i16::try_from(player).unwrap_or(-1);
    let mut out = PlayerScore::default();
    let mut score: i64 = 0;

    // Every planet the player owns counts, including any this file describes
    // only in part: the original counts from its own planet records, and a
    // player always has full records of their own. A planet this engine could
    // not load fully still counts toward the total and the ship caps.
    for planet in state.planets.iter().chain(state.known_planets.iter()) {
        if planet.owner != Some(owner) {
            continue;
        }
        out.planets += 1;
        // Population is stored in hundreds, so this is a point per hundred
        // thousand colonists.
        score += i64::from((planet.pop + 999) / 1000).min(6);
        if planet.starbase && starbase_has_dock(state, player, planet) {
            out.starbases += 1;
        }
        out.resources += i32::from(
            crate::resources::resources_at_planet(
                planet,
                &record.race,
                record.research.levels[0].into(),
            )
            .unwrap_or(0),
        );
    }
    score += i64::from(out.resources) / 30;
    score += i64::from(out.starbases) * 3;

    if !record.dead {
        for level in record.research.levels {
            let level = i64::from(level);
            out.tech_levels += i32::try_from(level).unwrap_or(0);
            score += match level {
                l if l < 4 => l,
                l if l < 7 => l * 2 - 3,
                l if l < 10 => l * 3 - 9,
                l => l * 4 - 18,
            };
        }
    }

    // Every design is classified once, then the ships are counted.
    let designs = state.designs.get(player);
    let classes: Vec<Option<usize>> = (0..16)
        .map(|slot| {
            designs
                .and_then(|d| d.get(slot))
                .filter(|d| d.hull_id >= 0)
                .map(|d| ship_class(design_power(d)))
        })
        .collect();
    let mut counts = [0i64; 3];
    for fleet in &state.fleets {
        if fleet.owner != owner {
            continue;
        }
        for stack in &fleet.stacks {
            if stack.count <= 0 {
                continue;
            }
            if let Some(Some(class)) = classes.get(usize::from(stack.design)) {
                counts[*class] += i64::from(stack.count);
            }
        }
    }
    out.unarmed_ships = i32::try_from(counts[0]).unwrap_or(i32::MAX);
    out.escort_ships = i32::try_from(counts[1]).unwrap_or(i32::MAX);
    out.capital_ships = i32::try_from(counts[2]).unwrap_or(i32::MAX);

    let planets = i64::from(out.planets);
    score += counts[0].min(planets) / 2;
    score += counts[1].min(planets) * 2;
    if counts[2] > 0 {
        score += counts[2] * 8 * planets / (counts[2] + planets);
    }

    out.score = i32::try_from(score).unwrap_or(i32::MAX);
    out
}

/// Whether a planet's starbase has a dock, which is what makes it worth three
/// points: `CalcPlayerScore` looks the starbase design's hull up and asks
/// whether it can hold cargo, which an Orbital Fort cannot.
fn starbase_has_dock(state: &GameState, player: usize, planet: &crate::planet::Planet) -> bool {
    let slot = usize::from(planet.starbase_design.unwrap_or(0))
        + usize::from(crate::startup::FIRST_STARBASE_SLOT);
    state
        .designs
        .get(player)
        .and_then(|designs| designs.get(slot))
        .and_then(ShipDesign::hull)
        .is_some_and(|hull| hull.cargo_max > 0)
}

/// Score every player and rank them.
///
/// `UpdatePlayerScores` (`10b8:6258`) ranks by score alone: a player's rank is
/// one plus the number of players who scored higher, so a tie shares a rank.
#[must_use]
pub fn scores(state: &GameState) -> Vec<PlayerScore> {
    let mut out: Vec<PlayerScore> = (0..state.players.len())
        .map(|player| score_for(state, player))
        .collect();
    for index in 0..out.len() {
        let mine = out[index].score;
        let better = out.iter().filter(|other| other.score > mine).count();
        out[index].rank = u16::try_from(better + 1).unwrap_or(u16::MAX);
    }
    out
}
