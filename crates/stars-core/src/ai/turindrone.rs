//! The TurinDrone's turn — `DoTurinDroneAiTurn` (`1088:3670`), the
//! personality the tutorial's Berserkers use.
//!
//! The routine's shape is written up in `docs/formulas/ai.md`, *The
//! TurinDrone turn*. This is the transcription, **begun** with the parts the
//! tutorial's first years turn on and marked where it stops: the year-0
//! scouts, scouting to planets the player has not seen, and colony ships to
//! the nearest planet worth settling. The designs used are whatever the
//! player holds in the slots the personality keeps for the roles — the
//! fittings `EnsureTurinDroneShdefs` would build are not read out yet — and
//! the war fleets, the haulers, the mine layers and the rest of the queue
//! pass are not here. Nothing in it is verified against a corpus turn.

use std::collections::BTreeSet;

use crate::ai::colonise::{mark_planet, nearest_colonisable, pct_planet_opt_value, Mark};
use crate::ai::dispatch::ideal_warp;
use crate::ai::AiPersonality;
use crate::components::slot;
use crate::fleet::{grobj, Waypoint};
use crate::movement::Point;
use crate::parts::Builder;
use crate::production::QueueItem;
use crate::rng::Rng;
use crate::GameState;

/// The design slot the personality keeps its scouts in.
pub const SCOUT_SLOT: u8 = 0;
/// The design slot it keeps its colony ships in.
pub const COLONY_SLOT: u8 = 1;
/// How many colonists a colony ship is loaded with: `XferAiSupply(..., 3,
/// 0x19)`, twenty-five kT.
pub const COLONISTS_ABOARD: i32 = 25;
/// A planet's queue is only filled when it has a starbase and this many kT
/// of colonists (`rgwtMin[3] < 200` is skipped).
pub const QUEUE_MIN_POP: i32 = 200;

/// What the turn did, for a test to look at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Fleets given a scouting leg, by fleet id, and where to.
    pub scouted: Vec<(u16, i16)>,
    /// Fleets sent to settle, by fleet id, and where.
    pub colonising: Vec<(u16, i16)>,
    /// Ships queued, as `(planet, design slot, count)`.
    pub queued: Vec<(i16, u8, i32)>,
    /// Designs made this turn, as `(slot, hull)`.
    pub designed: Vec<(u8, i16)>,
}

/// Run the personality's turn for `player`, as the host does before the
/// year is generated. The orders it gives are written straight into the
/// state, which is what replaying the AI's log would do.
pub fn turn(state: &mut GameState, player: usize, rng: &mut Rng) -> Report {
    let mut report = Report::default();
    let Some(me) = i16::try_from(player).ok() else {
        return report;
    };
    if state.players.get(player).is_none_or(|p| p.dead) {
        return report;
    }

    // What the player has seen so far: `det & 0xff > 2` on a planet record
    // is a planet that has been scanned, and the AI keeps that knowledge
    // from year to year as its file does.
    let seen = crate::visibility::view(state, player).planets;
    state.players[player].explored.extend(seen);
    let explored = state.players[player].explored.clone();

    // `EnsureTurinDroneShdefs`: the designs the personality wants in its
    // slots, made when the tech allows.
    ensure_designs(state, player, rng, &mut report);

    let marks = marks(state, player, me, &explored);
    let anywhere_to_settle = marks.contains(&Mark::Colonisable);

    // --- The planet pass: the queue at every planet with a starbase and
    // people enough. Only the year-0 scouts and the colony ships so far.
    let colony_design = colony_design(state, player);
    let colony_ships: i32 = state
        .fleets
        .iter()
        .filter(|f| f.owner == me)
        .flat_map(|f| f.stacks.iter())
        .filter(|s| Some(s.design) == colony_design)
        .map(|s| s.count)
        .sum();
    let scout_design = state
        .designs
        .get(player)
        .and_then(|d| d.get(usize::from(SCOUT_SLOT)))
        .filter(|d| d.hull().is_some() && !d.obsolete)
        .map(|_| SCOUT_SLOT);
    let planet_count = i32::try_from(state.planets.len()).unwrap_or(0);
    for index in 0..state.planets.len() {
        let planet = &state.planets[index];
        if planet.owner != Some(me) || !planet.starbase || planet.pop < QUEUE_MIN_POP {
            continue;
        }
        // A ship already in the queue and nothing more is added.
        if planet.queue.iter().any(|q| q.ship) {
            continue;
        }
        let id = planet.id;
        let mut added: Vec<(u8, i32)> = Vec::new();
        if state.turn == 0 {
            // One scout per thirty planets, per hundred past a hundred and
            // ninety: `for (n = cPlanMax; n > 0; n -= n < 191 ? 30 : 100)`.
            if let Some(scout) = scout_design {
                let mut left = planet_count;
                let mut scouts = 0;
                while left > 0 {
                    scouts += 1;
                    left -= if left < 191 { 30 } else { 100 };
                }
                added.push((scout, scouts));
            }
        }
        if anywhere_to_settle && colony_ships < 2 {
            if let Some(design) = colony_design {
                added.push((design, 4));
            }
        }
        for (design, count) in added {
            for _ in 0..count {
                state.planets[index].queue.push(QueueItem {
                    count: 1,
                    item: u16::from(design),
                    ship: true,
                    completion: 0,
                });
            }
            report.queued.push((id, design, count));
        }
    }

    // --- The fleet pass: every fleet of ours with no orders.
    let positions: Vec<(i16, Point)> = state
        .planets
        .iter()
        .filter_map(|p| p.position.map(|at| (p.id, at)))
        .collect();
    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        if fleet.owner != me || fleet.waypoints.len() > 1 || fleet.is_empty() {
            continue;
        }
        let designs = state.designs.get(player).cloned().unwrap_or_default();
        let is_colony = fleet
            .stacks
            .iter()
            .any(|s| Some(s.design) == colony_design && s.count > 0);
        let is_starbase = fleet.stacks.iter().any(|s| {
            designs
                .get(usize::from(s.design))
                .is_some_and(crate::design::ShipDesign::is_starbase)
        });
        if is_starbase {
            continue;
        }
        let stacks: Vec<(&crate::design::ShipDesign, i32)> = fleet
            .stacks
            .iter()
            .filter_map(|s| designs.get(usize::from(s.design)).map(|d| (d, s.count)))
            .collect();
        let warp = ideal_warp(&stacks, false);
        let from = fleet.position;
        let fleet_id = fleet.id;

        if is_colony {
            // Colonists first, at an own planet: `XferAiSupply(planet,
            // fleet, colonists, 25)`, as much as the planet has.
            if let Some(planet) = fleet
                .orbiting
                .and_then(|p| i16::try_from(p).ok())
                .and_then(|p| state.planets.iter().position(|q| q.id == p))
                .filter(|&p| state.planets[p].owner == Some(me))
            {
                let want = (COLONISTS_ABOARD - state.fleets[index].cargo.colonists).max(0);
                let take = want.min(state.planets[planet].pop);
                state.planets[planet].pop -= take;
                state.fleets[index].cargo.colonists += take;
            }
            let candidates: Vec<(i16, (i32, i32), Mark)> = positions
                .iter()
                .map(|(id, at)| {
                    let mark = usize::try_from(*id)
                        .ok()
                        .and_then(|i| marks.get(i).copied())
                        .unwrap_or(Mark::Unknown);
                    (*id, (i32::from(at.x), i32::from(at.y)), mark)
                })
                .collect();
            let Some(target) =
                nearest_colonisable((i32::from(from.x), i32::from(from.y)), &candidates)
            else {
                continue;
            };
            if let Some(at) = positions
                .iter()
                .find(|(id, _)| *id == target)
                .map(|(_, at)| *at)
            {
                lay_leg(
                    &mut state.fleets[index],
                    at,
                    target,
                    stars_formats::task::COLONIZE,
                    warp,
                );
                report.colonising.push((fleet_id, target));
            }
            continue;
        }

        // The scouts and the destroyers (slots 0, 10 and 11) scout; every
        // other idle fleet — the starting miner and freighter among them —
        // is left where it is until its part of the pass is written.
        let is_scout = fleet
            .stacks
            .iter()
            .all(|s| matches!(s.design, SCOUT_SLOT | 10 | 11));
        if !is_scout {
            continue;
        }
        // `IdNearestUnknownPlanet`: the nearest planet still marked unknown
        // that no other fleet of ours is bound for; failing that, a planet
        // at random.
        let bound_for: BTreeSet<i16> = state
            .fleets
            .iter()
            .filter(|f| f.owner == me && f.waypoints.len() > 1)
            .filter_map(|f| f.waypoints[1].target)
            .filter_map(|t| i16::try_from(t).ok())
            .collect();
        let mut best: Option<(i64, i16, Point)> = None;
        for (id, at) in &positions {
            let mark = usize::try_from(*id)
                .ok()
                .and_then(|i| marks.get(i).copied())
                .unwrap_or(Mark::Unknown);
            if mark != Mark::Unknown || bound_for.contains(id) {
                continue;
            }
            let dx = i64::from(at.x) - i64::from(from.x);
            let dy = i64::from(at.y) - i64::from(from.y);
            let d2 = dx * dx + dy * dy;
            if best.is_none_or(|(b, _, _)| d2 < b) {
                best = Some((d2, *id, *at));
            }
        }
        let target = match best {
            Some((_, id, at)) => Some((id, at)),
            None => {
                let pick = usize::try_from(rng.random(i16::try_from(positions.len()).unwrap_or(1)))
                    .unwrap_or(0);
                positions.get(pick).copied()
            }
        };
        if let Some((id, at)) = target {
            if fleet.orbiting.and_then(|p| i16::try_from(p).ok()) == Some(id) {
                continue;
            }
            lay_leg(
                &mut state.fleets[index],
                at,
                id,
                stars_formats::task::NONE,
                warp,
            );
            report.scouted.push((fleet_id, id));
        }
    }
    report
}

/// The mark on every planet, by id.
fn marks(state: &GameState, player: usize, me: i16, explored: &BTreeSet<i16>) -> Vec<Mark> {
    let personality = Some(AiPersonality::TurinDrone);
    let race = &state.players[player].race;
    let count = state.planets.len().max(
        state
            .planets
            .iter()
            .map(|p| usize::try_from(p.id).unwrap_or(0) + 1)
            .max()
            .unwrap_or(0),
    );
    let mut marks = vec![crate::ai::colonise::unknown_mark(personality); count];
    for planet in &state.planets {
        let Ok(at) = usize::try_from(planet.id) else {
            continue;
        };
        if !explored.contains(&planet.id) && planet.owner != Some(me) {
            continue;
        }
        let value = pct_planet_opt_value(planet, race, planet.env);
        marks[at] = mark_planet(planet, me, personality, value);
    }
    // A planet one of our colony fleets is already bound for is claimed.
    for fleet in state.fleets.iter().filter(|f| f.owner == me) {
        if let Some(next) = fleet.waypoints.get(1) {
            if next.task == stars_formats::task::COLONIZE && next.target_class == grobj::PLANET {
                if let Some(at) = next.target.map(usize::from) {
                    if let Some(mark) = marks.get_mut(at) {
                        *mark = Mark::Claimed;
                    }
                }
            }
        }
    }
    marks
}

/// The player's colony-ship design: the personality's slot, while it holds
/// a live design carrying a colonisation module.
fn colony_design(state: &GameState, player: usize) -> Option<u8> {
    let designs = state.designs.get(player)?;
    let has_module = |d: &crate::design::ShipDesign| {
        !d.obsolete
            && d.slots
                .iter()
                .any(|s| s.category == slot::SPECIAL_M && s.item == 0 && s.count > 0)
    };
    designs
        .get(usize::from(COLONY_SLOT))
        .is_some_and(&has_module)
        .then_some(COLONY_SLOT)
}

/// `FMoveAiFleet` with a fresh order: the fleet's route becomes its own
/// position and this one leg.
fn lay_leg(fleet: &mut crate::fleet::Fleet, at: Point, planet: i16, task: u8, warp: u8) {
    fleet.waypoints.truncate(1);
    if fleet.waypoints.is_empty() {
        fleet.waypoints.push(Waypoint {
            position: fleet.position,
            target: fleet.orbiting,
            target_class: grobj::PLANET,
            warp: 0,
            task: stars_formats::task::NONE,
            transport: None,
            task_data: Vec::new(),
        });
    }
    fleet.waypoints.push(Waypoint {
        position: at,
        target: u16::try_from(planet).ok(),
        target_class: grobj::PLANET,
        warp,
        task,
        transport: None,
        task_data: Vec::new(),
    });
    fleet.warp = Some(warp);
}

/// `EnsureTurinDroneShdefs` (`1088:58ba`): fill the slots the personality
/// keeps for each role, each when its tech is reached and the slot is
/// empty, obsolete, or (for the scout, colony ship, miner and mine layer)
/// holds a design no ship of which exists. The scout, colony ship and
/// miner slots have their old design **retired first**, whether or not
/// the new one can be made — which leaves a young TurinDrone with no
/// scout design at all once its last starting scout is gone, until it
/// reaches Construction 6 for the Frigate. The fittings are
/// [`crate::ai::parts::fitting`].
///
/// The tech thresholds are the routine's own comparisons, `tech[f] > n`,
/// in the order Energy, Weapons, Propulsion, Construction, Electronics,
/// Biotechnology.
fn ensure_designs(state: &mut GameState, player: usize, rng: &mut Rng, report: &mut Report) {
    use crate::ai::parts::{create_design, fitting, pick_name};

    let me = i16::try_from(player).unwrap_or(-1);
    let levels = state.players[player].research.levels;
    let above = |field: usize, n: u8| levels[field] > n;
    let exists = |state: &GameState, slot: u8| -> bool {
        state
            .fleets
            .iter()
            .filter(|f| f.owner == me)
            .any(|f| f.stacks.iter().any(|s| s.design == slot && s.count > 0))
    };
    let slot_state = |state: &GameState, slot: u8| -> (bool, bool) {
        let design = state
            .designs
            .get(player)
            .and_then(|d| d.get(usize::from(slot)));
        // An empty slot carries the retired bit in the original, which is
        // how "free" is told.
        let present = design.is_some_and(|d| d.hull().is_some());
        let obsolete = design.is_none_or(|d| d.obsolete || d.hull().is_none());
        (present, obsolete)
    };

    // (slot, hull, fittings to try in turn, needs, retire the old first,
    //  make when no ship of it exists)
    struct Want {
        slot: u8,
        hull: i16,
        fittings: Vec<crate::ai::parts::Fitting>,
        needs: bool,
        retire_first: bool,
        when_none_exist: bool,
    }
    let battleship_pick = usize::try_from(rng.random(4)).unwrap_or(0);
    let wants = [
        Want {
            slot: 8,
            hull: 12,
            fittings: vec![fitting::ROGUE],
            needs: above(1, 4) && above(3, 7),
            retire_first: false,
            when_none_exist: false,
        },
        Want {
            slot: 9,
            hull: 13,
            fittings: vec![fitting::GALLEON],
            needs: above(1, 6) && above(3, 10),
            retire_first: false,
            when_none_exist: false,
        },
        Want {
            slot: 10,
            hull: 6,
            fittings: vec![fitting::DESTROYER],
            needs: above(0, 4) && above(4, 4) && above(3, 3) && above(1, 4),
            retire_first: false,
            when_none_exist: false,
        },
        Want {
            slot: 1,
            hull: 15,
            fittings: vec![fitting::COLONY_SHIP],
            needs: true,
            retire_first: true,
            when_none_exist: true,
        },
        Want {
            slot: 0,
            hull: 5,
            fittings: vec![fitting::SCOUT],
            needs: true,
            retire_first: true,
            when_none_exist: true,
        },
        Want {
            slot: 2,
            hull: 22,
            fittings: vec![fitting::MINER],
            needs: above(3, 6) && above(4, 3),
            retire_first: true,
            when_none_exist: true,
        },
        Want {
            slot: 12,
            hull: 11,
            fittings: vec![fitting::MINE_LAYER],
            needs: above(3, 3) && above(5, 3),
            retire_first: false,
            when_none_exist: true,
        },
        Want {
            slot: 13,
            hull: 18,
            fittings: vec![fitting::STEALTH_BOMBER],
            needs: above(0, 7) && above(4, 6) && above(3, 5),
            retire_first: false,
            when_none_exist: false,
        },
        Want {
            slot: 14,
            hull: 18,
            fittings: vec![fitting::STEALTH_BOMBER],
            needs: above(0, 10) && above(4, 11) && above(3, 14) && above(1, 8),
            retire_first: false,
            when_none_exist: false,
        },
        Want {
            slot: 4,
            hull: 9,
            // One of the four at random, then the others if it fails.
            fittings: (0..4)
                .map(|i| fitting::BATTLESHIPS[(battleship_pick + i) % 4])
                .collect(),
            needs: above(0, 4) && above(4, 5) && above(3, 12) && above(1, 6),
            retire_first: false,
            when_none_exist: false,
        },
        Want {
            slot: 15,
            hull: 12,
            fittings: vec![fitting::ROGUE],
            needs: above(0, 4) && above(4, 5) && above(3, 12) && above(1, 6),
            retire_first: false,
            when_none_exist: false,
        },
    ];

    for want in wants {
        let (present, obsolete) = slot_state(state, want.slot);
        let wanted = obsolete || (want.when_none_exist && !exists(state, want.slot));
        if !wanted || !want.needs {
            continue;
        }
        // The colony-ship slot keeps a Privateer it may already hold.
        if want.slot == 1 && present && !obsolete && state.designs[player][1].hull_id == 11 {
            continue;
        }
        if want.retire_first && present && !obsolete {
            state.designs[player][usize::from(want.slot)].obsolete = true;
        }
        let who = Builder::player(&state.players[player]);
        let Some(mut design) = want
            .fittings
            .iter()
            .find_map(|f| create_design(want.hull, f, &who))
        else {
            continue;
        };
        let taken: Vec<String> = state
            .designs
            .get(player)
            .map(|d| {
                d.iter()
                    .filter(|d| !d.obsolete)
                    .map(|d| d.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        design.name = pick_name(want.hull, &taken, rng);
        design.picture = crate::components::HULLS
            .get(usize::try_from(want.hull).unwrap_or(0))
            .and_then(|h| u8::try_from(h.picture).ok())
            .unwrap_or(0);
        let designs = &mut state.designs[player];
        let at = usize::from(want.slot);
        while designs.len() <= at {
            designs.push(crate::design::ShipDesign {
                hull_id: -1,
                slots: Vec::new(),
                name: String::new(),
                picture: 0,
                stored_armor: 0,
                obsolete: true,
                designed: 0,
                built: 0,
            });
        }
        designs[at] = design;
        report.designed.push((want.slot, want.hull));
    }
}
