//! The Automitron's turn: `DoAutomitronAiTurn` (`1098:01e0`), its
//! designs (`EnsureISShdefs`, `1098:1938`), its war test
//! (`FPotentISWarFleet`, `1098:012e`) and the scouts' targeting it alone
//! calls (`IdTargetScout`, `1090:61de`), with the shared routines
//! transcribed in [`super::turindrone`] and [`super::robotoid`] reused.
//!
//! The shape is the TurinDrone's — research, merges by slot mask, the
//! armada potencies from the year, `CheckAiShdefStatus` over the slot
//! ranges, `SplitOutShdefs`, the designs, a pass over the planets'
//! queues, a pass over the fleets, then `HandleBasicAiTasks` and
//! `FillProductionQueue` — with its own slots: **Scouts in slot 0**, a
//! Medium Freighter carrying a colonisation module in slot 1, B-17 and
//! B-52 bombers in 2 and 3, Medium and Super Freighter haulers in 4 and
//! 5, a Privateer mine layer in 6, Battleships in 9 (and 10), whatever
//! the player began with in 11 and 12, and a Destroyer in 14. Its scouts
//! hunt: an armed scout fleet chases the nearest enemy fleet within 180
//! light years before it scouts. See `docs/formulas/ai.md`, *The
//! Automitron's turn*.

use std::collections::BTreeSet;

use crate::ai::colonise::{nearest_colonisable, Mark};
use crate::ai::dispatch::ideal_warp;
use crate::ai::parts::{create_design, Fitting};
use crate::ai::personality::Profile;
use crate::ai::robotoid::{is_attack_fleet, is_transport};
use crate::ai::turindrone::{
    armada_dest, basic_tasks, check_status, ensure_research, fill_production_queues,
    install_design, lay_leg, marks, merge_all, mineral_worth, move_to_nearest_starbase,
    split_out_designs, target_armada_as, target_freighter, validate_starbase_history,
    valued_planets, ArmadaRule, Report, Valued,
};
use crate::ai::AiPersonality;
use crate::fleet::grobj;
use crate::movement::Point;
use crate::parts::Builder;
use crate::production::QueueItem;
use crate::rng::Rng;
use crate::GameState;

/// The scouts' slot.
pub const SCOUT_SLOT: u8 = 0;
/// The colony freighters' slot.
pub const COLONY_SLOT: u8 = 1;
/// The mine layers' slot.
pub const LAYER_SLOT: u8 = 6;
/// A planet's queue is only filled when it has a starbase and more than
/// this many hundreds of colonists (`1098:04a4`: the population word
/// against `0x5db`).
pub const QUEUE_MIN_POP: i32 = 1500;
/// How many colonists a colony freighter is loaded with: `XferAiSupply(…,
/// 3, 0x96)`, fifteen thousand people.
pub const COLONISTS_ABOARD: i32 = 150;
/// How far `IdTargetScout` chases an enemy fleet: 180 light years
/// (`0x7e90` squared distance).
pub const CHASE_RANGE_SQUARED: i64 = 0x7e90;

/// The fittings, from `1098:0078` by the byte offsets at `1098:0064`;
/// each byte is a part class ([`crate::ai::parts::PART_CLASSES`]) per hull
/// slot.
pub mod fitting {
    use super::Fitting;
    /// Offset 0: the Medium Freighter (1) colony ship of slot 1 — a
    /// ramscoop, a colonisation module, a shield.
    pub const COLONY_FREIGHTER: Fitting = &[30, 31, 10];
    /// Offset 1: the Scout (4) of slot 0 — a ramscoop, a scanner, a beam.
    pub const SCOUT: Fitting = &[30, 26, 4];
    /// Offset 4: the Destroyer (6) of slot 14.
    pub const DESTROYER: Fitting = &[30, 0, 0, 13, 9, 18, 11];
    /// Offsets 10 to 13: the Battleship (9) of slot 9, one drawn at random.
    pub const BATTLESHIPS: [Fitting; 4] = [
        &[8, 12, 10, 6, 3, 5, 3, 7, 9, 20, 20],
        &[8, 12, 10, 0, 0, 0, 0, 0, 9, 19, 11],
        &[8, 12, 10, 6, 3, 4, 2, 7, 17, 20, 20],
        &[8, 12, 10, 1, 1, 1, 1, 1, 17, 19, 11],
    ];
    /// Offset 14: the Medium Freighter hauler of slot 4.
    pub const MEDIUM_FREIGHTER: Fitting = &[8, 16, 10];
    /// Offset 15: the B-17 (17) of slot 2.
    pub const B17: Fitting = &[8, 21, 23, 12];
    /// Offset 16: the B-52 (19) of slot 3.
    pub const B52: Fitting = &[8, 21, 23, 23, 23, 12, 10];
    /// Offset 17: the Privateer (11) mine layer of slot 6.
    pub const MINE_LAYER: Fitting = &[30, 10, 12, 25, 32];
    /// Offset 18: the Super Freighter (3) hauler of slot 5.
    pub const SUPER_FREIGHTER: Fitting = &[8, 16, 10, 19];
}

/// The armada potencies for the year (`1098:0280`): three from turn 131
/// rising by one every twenty years to fifty, and half that; six from
/// turn 116 rising by one every twenty-two years to twelve, and half that
/// less one, at most three.
#[must_use]
pub fn potency(turn: i16) -> [u8; 4] {
    let a = if turn > 130 { 3 + (turn - 120) / 20 } else { 3 }.min(50);
    let c = if turn > 115 { 6 + (turn - 100) / 22 } else { 6 }.min(12);
    let d = (c / 2 - 1).min(3);
    [
        u8::try_from(a).unwrap_or(50),
        u8::try_from(a / 2).unwrap_or(25),
        u8::try_from(c).unwrap_or(12),
        u8::try_from(d).unwrap_or(3),
    ]
}

/// `FPotentISWarFleet(fleet, 2)`: the fleet's weight of war is its ships
/// in slots 11 and 12 plus twice those in 9 and 10, and it is potent when
/// that reaches the first potency.
#[must_use]
pub fn is_potent_war_fleet(fleet: &crate::fleet::Fleet, potency: &[u8; 4]) -> bool {
    let weight: i32 = fleet
        .stacks
        .iter()
        .map(|s| match s.design {
            11 | 12 => s.count,
            9 | 10 => s.count * 2,
            _ => 0,
        })
        .sum();
    weight >= i32::from(potency[0])
}

/// `EnsureISShdefs` (`1098:1938`): the designs the personality wants in
/// its slots, each made when the slot is retired (or, for the scout and
/// colony slots, when no ship of it is left) and the tech allows
/// (`rgTech` 1 to 5 = Weapons, Propulsion, Construction, Electronics,
/// Biotechnology).
fn ensure_designs(state: &mut GameState, player: usize, rng: &mut Rng, report: &mut Report) {
    let me = i16::try_from(player).unwrap_or(-1);
    let levels = state.players[player].research.levels;
    let above = |field: usize, n: u8| levels[field] > n;
    let retired = |state: &GameState, slot: usize| {
        state
            .designs
            .get(player)
            .and_then(|d| d.get(slot))
            .is_none_or(|d| d.obsolete || d.hull().is_none())
    };
    let exists = |state: &GameState, slot: u8| {
        state
            .fleets
            .iter()
            .filter(|f| f.owner == me)
            .any(|f| f.stacks.iter().any(|s| s.design == slot && s.count > 0))
    };
    fn draw(
        state: &mut GameState,
        player: usize,
        slot: u8,
        hull: i16,
        fitting: Fitting,
        rng: &mut Rng,
        report: &mut Report,
    ) -> bool {
        let who = Builder::player(&state.players[player]);
        let Some(design) = create_design(hull, fitting, &who) else {
            return false;
        };
        install_design(state, player, slot, design, rng, report);
        true
    }

    // The haulers: a Medium Freighter at Propulsion 5, a Super Freighter
    // at 7.
    if retired(state, 4) && above(2, 4) {
        draw(state, player, 4, 1, fitting::MEDIUM_FREIGHTER, rng, report);
    }
    if retired(state, 5) && above(2, 6) {
        draw(state, player, 5, 3, fitting::SUPER_FREIGHTER, rng, report);
    }
    // The Destroyer of slot 14, four tries at its one fitting.
    if retired(state, 14) && above(1, 4) && above(4, 5) && above(3, 3) && above(2, 4) {
        for _ in 0..4 {
            rng.random(1);
            if draw(state, player, 14, 6, fitting::DESTROYER, rng, report) {
                break;
            }
        }
    }
    // The colony freighter and the scout: redrawn whenever retired or
    // none is left, the live design retired first.
    for (slot, hull, fit) in [
        (COLONY_SLOT, 1, fitting::COLONY_FREIGHTER),
        (SCOUT_SLOT, 4, fitting::SCOUT),
    ] {
        if retired(state, usize::from(slot)) || !exists(state, slot) {
            if !retired(state, usize::from(slot)) {
                state.designs[player][usize::from(slot)].obsolete = true;
            }
            draw(state, player, slot, hull, fit, rng, report);
        }
    }
    // The mine layer: Construction 4, Propulsion 5, Biotechnology 6.
    if retired(state, 6) && above(3, 3) && above(2, 4) && above(5, 5) {
        draw(state, player, 6, 11, fitting::MINE_LAYER, rng, report);
    }
    // The bombers: a B-17 at Weapons 8, Electronics 7, Construction 6,
    // Propulsion 7; a B-52 at 11, 12, 15 and 9.
    if retired(state, 2) && above(1, 7) && above(4, 6) && above(3, 5) && above(2, 6) {
        draw(state, player, 2, 17, fitting::B17, rng, report);
    }
    if retired(state, 3) && above(1, 10) && above(4, 11) && above(3, 14) && above(2, 8) {
        draw(state, player, 3, 19, fitting::B52, rng, report);
    }
    // The Battleship of slot 9: Weapons 5, Electronics 6, Construction
    // 13, Propulsion 7, five tries at one of four fittings.
    if retired(state, 9) && above(1, 4) && above(4, 5) && above(3, 12) && above(2, 6) {
        for _ in 0..5 {
            let pick = usize::try_from(rng.random(4)).unwrap_or(0).min(3);
            if draw(state, player, 9, 9, fitting::BATTLESHIPS[pick], rng, report) {
                break;
            }
        }
    }
}

/// The Automitron's turn.
#[allow(clippy::too_many_lines)]
pub fn turn(state: &mut GameState, player: usize, rng: &mut Rng, profile: &Profile) -> Report {
    let mut report = Report::default();
    let Some(me) = i16::try_from(player).ok() else {
        return report;
    };
    if state.players.get(player).is_none_or(|p| p.dead) {
        return report;
    }
    let turn = state.turn;

    let seen = crate::visibility::view(state, player).planets;
    state.players[player].explored.extend(seen);
    let explored = state.players[player].explored.clone();

    // `IroEnsureAi(vrgbAutomitronRes, 18, &ishdefSBLatest, pct)`.
    report.research = ensure_research(state, player, profile.plan, profile.research_pct(turn));
    validate_starbase_history(state, player, me);
    let history_count =
        i32::try_from(state.players[player].starbase_history.len()).unwrap_or(i32::MAX);

    // `MergeAllShdefs`: the Destroyers (`0x4000`) from turn 31; from turn
    // 51 also the bombers with the Battleships and the starting designs
    // (`0x1e0c`: slots 2, 3, 9 to 12) and the mine layers (`0x40`).
    if turn > 50 {
        for mask in [0x1e0cu16, 0x0040, 0x4000] {
            merge_all(state, me, mask, &mut report);
        }
    } else if turn > 30 {
        merge_all(state, me, 0x4000, &mut report);
    }
    let potency = potency(turn);
    state.ai_armada_potency = potency;

    // `CheckAiShdefStatus` over the slot ranges.
    let recycle: i16 = if turn < 120 {
        50
    } else if turn < 200 {
        70
    } else {
        100
    };
    let mut old = [false; 16];
    let elevens = check_status(state, player, me, 11, 12, recycle, &mut old);
    let haulers = check_status(state, player, me, 4, 5, recycle, &mut old);
    let bombers = check_status(state, player, me, 2, 3, recycle, &mut old);
    let battleships = check_status(state, player, me, 9, 10, recycle, &mut old);
    if turn > 60 {
        split_out_designs(state, player, me, &old, &mut report);
    }
    ensure_designs(state, player, rng, &mut report);

    // The planets we could settle (`1098:0420`): unowned, scanned, with a
    // positive opt value.
    let mut marks = marks(state, player, me, &explored, AiPersonality::Automitron);
    let colonisable = {
        let race = &state.players[player].race;
        let levels = state.players[player].research.levels;
        state
            .planets
            .iter()
            .filter(|p| p.owner.is_none() && explored.contains(&p.id))
            .filter(|p| {
                let reach = crate::terraform::optimal_env(p, race, levels);
                crate::ai::colonise::pct_planet_opt_value(p, race, reach) > 0
            })
            .count()
    };
    // The other players' planets worth settling (`vlpbAiPlanet[+3]`).
    let mut valued = valued_planets(state, player, me);

    // --- The planet pass: the queue at every own planet with a starbase
    // and people enough that has no ship queued.
    let planet_count = i32::try_from(state.planets.len()).unwrap_or(0);
    let owned =
        i32::try_from(state.planets.iter().filter(|p| p.owner == Some(me)).count()).unwrap_or(0);
    let race = state.players[player].race.clone();
    let levels = state.players[player].research.levels;
    let ships_of = |state: &GameState, slot: u8| -> i32 {
        state
            .fleets
            .iter()
            .filter(|f| f.owner == me)
            .flat_map(|f| f.stacks.iter())
            .filter(|s| s.design == slot)
            .map(|s| s.count)
            .sum()
    };
    let colony_live = state.designs[player]
        .get(usize::from(COLONY_SLOT))
        .is_some_and(|d| d.hull().is_some() && !d.obsolete);
    let layer_live = state.designs[player]
        .get(usize::from(LAYER_SLOT))
        .is_some_and(|d| d.hull().is_some() && !d.obsolete);
    for index in 0..state.planets.len() {
        let planet = &state.planets[index];
        if planet.owner != Some(me) {
            continue;
        }
        // An own planet hostile to us as it stands is passed over.
        if crate::hab::pct_planet_desirability(planet, &race) < 0 {
            continue;
        }
        if !planet.starbase || planet.pop <= QUEUE_MIN_POP {
            continue;
        }
        if planet.queue.iter().any(|q| q.ship) {
            continue;
        }
        let id = planet.id;
        let mut added: Vec<(u8, i32)> = Vec::new();
        // Haulers (newest of 4, 5), from Propulsion 5: while fewer than the
        // larger of a tenth of the planets owned and twice the starbase
        // history's entries, or under ten sevenths of that one roll in
        // four.
        let want = (history_count * 2).max(owned / 10);
        if levels[2] > 4 {
            if let Some(latest) = haulers.latest {
                let count = haulers.count;
                if count < want || (count < want * 10 / 7 && rng.random(4) == 0) {
                    added.push((latest, 1));
                }
            }
        }
        // A colony freighter, from turn 11, while somewhere is left to
        // settle and no ship of the design exists.
        if colonisable > 0 && colony_live && ships_of(state, COLONY_SLOT) == 0 && turn > 10 {
            added.push((COLONY_SLOT, 1));
        }
        // Mine layers, three at a time, one roll in three: the fleet of
        // them here under ten (under seventeen one roll in eight), and a
        // roll of `2 × count + 1` coming up zero.
        if layer_live && rng.random(3) == 0 {
            let here = state
                .fleets
                .iter()
                .find(|f| {
                    f.owner == me
                        && f.orbiting == Some(u16::try_from(id).unwrap_or(u16::MAX))
                        && f.stacks
                            .iter()
                            .any(|s| s.design == LAYER_SLOT && s.count > 0)
                })
                .map_or(0, |f| {
                    f.stacks
                        .iter()
                        .filter(|s| s.design == LAYER_SLOT)
                        .map(|s| s.count)
                        .sum::<i32>()
                });
            if (here < 10 || (here < 17 && rng.random(8) == 0))
                && rng.random(i16::try_from(here * 2 + 1).unwrap_or(i16::MAX)) == 0
            {
                added.push((LAYER_SLOT, 3));
            }
        }
        // Bombers (newest of 2, 3), four at a time, where a potent war
        // fleet of ours here already holds the third potency of them —
        // and nothing more this year.
        let mut finished = false;
        if let Some(latest) = bombers.latest {
            let potent = state
                .fleets
                .iter()
                .find(|f| {
                    f.owner == me
                        && f.orbiting == Some(u16::try_from(id).unwrap_or(u16::MAX))
                        && is_potent_war_fleet(f, &potency)
                })
                .map(|f| {
                    f.stacks
                        .iter()
                        .filter(|s| s.design == 2 || s.design == 3)
                        .map(|s| s.count)
                        .sum::<i32>()
                });
            if potent.is_some_and(|aboard| aboard >= i32::from(potency[2])) {
                added.push((latest, 4));
                finished = true;
            }
        }
        // Then the classes paid for out of what is left, up to five of
        // each while fewer than the limit exist, stopping at the first
        // that cannot be paid: the newest of slots 11 and 12 to
        // `planets/12 + 8`, the Battleships to `planets/24 + 4`.
        if !finished {
            for (status, limit) in [
                (&elevens, planet_count / 12 + 8),
                (&battleships, planet_count / 24 + 4),
            ] {
                let Some(latest) = status.latest else {
                    continue;
                };
                let existing = i64::from(ships_of(state, latest));
                if existing >= i64::from(limit) {
                    continue;
                }
                let mut left = crate::ai::production::resources_available(
                    &state.planets[index],
                    &race,
                    state.players[player].research_pct,
                    i16::from(levels[0]),
                );
                let committed =
                    crate::ai::production::queue_cost(&state.planets[index].queue, &race);
                let mut paid = true;
                for (have, spent) in left.iter_mut().zip(committed.iter()) {
                    *have -= spent;
                    if *have < 0 {
                        paid = false;
                    }
                }
                if !paid {
                    break;
                }
                let who = Builder::player(&state.players[player]);
                let Some(cost) = state.designs[player]
                    .get(usize::from(latest))
                    .and_then(|d| d.true_cost(&who))
                else {
                    continue;
                };
                let mut n = 0;
                for _ in 0..5 {
                    let each = [
                        cost.minerals[0],
                        cost.minerals[1],
                        cost.minerals[2],
                        cost.resources,
                    ];
                    let mut ok = true;
                    for (have, spent) in left.iter_mut().zip(each.iter()) {
                        *have -= spent;
                        if *have < 0 {
                            ok = false;
                        }
                    }
                    if !ok {
                        paid = false;
                        break;
                    }
                    n += 1;
                }
                if n > 0 {
                    added.push((latest, n));
                }
                if !paid {
                    break;
                }
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

    // --- The first fleet pass: `drop_pass`, with `FIsAiAttack` listing
    // the attack fleets.
    let attack_fleets = drop_pass(
        state,
        player,
        me,
        &mut valued,
        &[],
        &|state, player, fleet| is_attack_fleet(state, player, fleet),
        &mut report,
    );

    // --- The second fleet pass: every fleet of ours.
    let positions: Vec<(i16, Point)> = state
        .planets
        .iter()
        .filter_map(|p| p.position.map(|at| (p.id, at)))
        .collect();
    let position_of = |id: i16| positions.iter().find(|(p, _)| *p == id).map(|(_, at)| *at);
    let worth = mineral_worth(state, &explored, marks.len());
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let scout_engine_no_scoop = designs
        .first()
        .and_then(|d| d.slots.first())
        .is_some_and(|s| s.category == crate::components::slot::ENGINE && s.item < 10);
    let first_starbase = state
        .planets
        .iter()
        .find(|p| p.owner == Some(me) && p.starbase)
        .map(|p| p.id);
    let ready = |fleet: &crate::fleet::Fleet| {
        let count = |slot: u8| -> i32 {
            fleet
                .stacks
                .iter()
                .filter(|s| s.design == slot)
                .map(|s| s.count)
                .sum()
        };
        !((count(2) < 2 && count(3) < 2) || (count(9) < 3 && count(10) < 3))
    };
    for index in 0..state.fleets.len() {
        let fleet = state.fleets[index].clone();
        if fleet.owner != me || fleet.is_empty() {
            continue;
        }
        let fleet_id = fleet.id;
        let count = |slot: u8| -> i32 {
            fleet
                .stacks
                .iter()
                .filter(|s| s.design == slot)
                .map(|s| s.count)
                .sum()
        };
        let out_of_fuel = scout_engine_no_scoop && fleet.cargo.fuel < 2;
        let scrap = |state: &mut GameState, report: &mut Report| {
            let f = &mut state.fleets[index];
            f.waypoints.truncate(1);
            f.waypoints[0].task = stars_formats::task::SCRAP;
            f.waypoints[0].task_data = Vec::new();
            f.waypoints[0].transport = None;
            report.scrapped.push(fleet_id);
        };
        if fleet.waypoints.len() > 1 {
            // Under way: a fleet out of fuel, when the scout design's
            // engine is no scoop, is scrapped.
            if out_of_fuel {
                scrap(state, &mut report);
            }
            continue;
        }
        // Mine layers with no task: lay mines, five years' worth.
        if count(LAYER_SLOT) > 0 {
            let f = &mut state.fleets[index];
            if f.waypoints[0].task == stars_formats::task::NONE {
                f.waypoints[0].task = stars_formats::task::LAY_MINES;
                f.waypoints[0].task_data = vec![5, 0];
                report.laying.push(fleet_id);
            }
            continue;
        }
        // Colony freighters.
        if count(COLONY_SLOT) > 0 {
            // Only the Medium Freighter design colonises; anything else in
            // the slot is broken up.
            if designs
                .get(usize::from(COLONY_SLOT))
                .is_none_or(|d| d.hull_id != 1)
            {
                scrap(state, &mut report);
                continue;
            }
            let orbiting = fleet
                .orbiting
                .and_then(|p| i16::try_from(p).ok())
                .and_then(|id| state.planets.iter().position(|p| p.id == id));
            let at_own_populous = orbiting
                .is_some_and(|p| state.planets[p].owner == Some(me) && state.planets[p].pop >= 200);
            if !at_own_populous && fleet.cargo.colonists == 0 {
                // Empty, away from a populous own planet: home to an own
                // starbase (unless the design's engine is the cheapest),
                // else scrapped; at an own starbase it waits.
                let at_own_starbase = orbiting.is_some_and(|p| {
                    state.planets[p].owner == Some(me) && state.planets[p].starbase
                });
                if at_own_starbase {
                    continue;
                }
                let engine_beyond_first = designs
                    .get(usize::from(COLONY_SLOT))
                    .and_then(|d| d.slots.first())
                    .is_some_and(|s| s.item > 1);
                if engine_beyond_first && move_to_nearest_starbase(state, me, index, false) {
                    continue;
                }
                scrap(state, &mut report);
                continue;
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
            let from = fleet.position;
            let target = nearest_colonisable((i32::from(from.x), i32::from(from.y)), &candidates);
            // Colonists from the own planet it sits at.
            if let Some(p) = orbiting.filter(|&p| state.planets[p].owner == Some(me)) {
                let room =
                    state.fleets[index].cargo_capacity(&designs) - state.fleets[index].cargo.mass();
                let take = COLONISTS_ABOARD.min(state.planets[p].pop).min(room).max(0);
                state.planets[p].pop -= take;
                state.fleets[index].cargo.colonists += take;
            }
            if let Some((target, at)) = target.and_then(|t| position_of(t).map(|at| (t, at))) {
                let stacks: Vec<(&crate::design::ShipDesign, i32)> = state.fleets[index]
                    .stacks
                    .iter()
                    .filter_map(|s| designs.get(usize::from(s.design)).map(|d| (d, s.count)))
                    .collect();
                let warp = ideal_warp(&stacks, false);
                lay_leg(
                    &mut state.fleets[index],
                    at,
                    target,
                    stars_formats::task::COLONIZE,
                    warp,
                );
                if let Some(mark) = usize::try_from(target).ok().and_then(|i| marks.get_mut(i)) {
                    *mark = Mark::Claimed;
                }
                report.colonising.push((fleet_id, target));
            }
            continue;
        }
        // Haulers: `IdTargetFreighter` from the starbase-history planet,
        // once there is an own starbase — with none the rest of the fleets
        // are left alone.
        if is_transport(state, player, &fleet) {
            let Some(first_starbase) = first_starbase else {
                break;
            };
            let home = state.players[player]
                .starbase_history
                .iter()
                .find(|e| e.fleets.contains(&fleet_id))
                .map_or(first_starbase, |e| e.planet);
            if let Some(to) = target_freighter(state, player, me, index, &worth, home, &valued, rng)
            {
                report.hauling.push((fleet_id, to));
            }
            continue;
        }
        // Armadas — bombers aboard: by the armada rule, at warp 4.
        if count(2) > 0 || count(3) > 0 {
            let rule = ArmadaRule {
                bombers: &[2, 3],
                ready: &ready,
                warp: Some(4),
            };
            if let Some(to) = target_armada_as(state, player, me, index, &rule, rng) {
                report.attacking.push((fleet_id, to));
            }
            continue;
        }
        // Scouts: a scout out of fuel is scrapped; the rest hunt and
        // scout by `IdTargetScout`.
        if count(SCOUT_SLOT) > 0 {
            if out_of_fuel {
                scrap(state, &mut report);
                continue;
            }
            if let Some(to) =
                target_scout(state, player, me, index, &attack_fleets, &mut marks, rng)
            {
                report.scouted.push((fleet_id, to));
            }
        }
    }

    basic_tasks(
        state,
        player,
        me,
        &worth,
        AiPersonality::Automitron,
        rng,
        &mut report,
    );
    fill_production_queues(
        state,
        player,
        me,
        AiPersonality::Automitron,
        rng,
        &mut report,
    );
    report
}

/// The first fleet pass the Automitron and the Rototill share
/// (`1098:0d5d`–`1098:1075`, `1098:2308`–`1098:2402`): the attack fleets
/// listed by `is_attack`, and stale orders cut. A colony ship (slot 1)
/// or a transport (`FIsAiTransport`) whose destination — the planet it
/// sits at with no leg, else its next waypoint's planet — is somebody
/// else's has its orders blown away, unless that planet is one worth
/// settling (`vlpbAiPlanet[+3]`) and colonists are aboard (and its owner
/// is not Alternate Reality): then they are dropped there, a Transport
/// order unloading them all, and the planet claimed. A colony ship bound
/// for somebody else's planet not so marked has its orders blown away
/// too. Fleets carrying a design in `skip_with_orders` that have orders
/// are the caller's business.
pub(crate) fn drop_pass(
    state: &mut GameState,
    player: usize,
    me: i16,
    valued: &mut Valued,
    skip_with_orders: &[u8],
    is_attack: &dyn Fn(&GameState, usize, &crate::fleet::Fleet) -> bool,
    report: &mut Report,
) -> Vec<u16> {
    let positions: Vec<(i16, Point)> = state
        .planets
        .iter()
        .filter_map(|p| p.position.map(|at| (p.id, at)))
        .collect();
    let position_of = |id: i16| positions.iter().find(|(p, _)| *p == id).map(|(_, at)| *at);
    let mut attack_fleets: Vec<u16> = Vec::new();
    for index in 0..state.fleets.len() {
        let fleet = state.fleets[index].clone();
        if fleet.owner != me || fleet.is_empty() {
            continue;
        }
        if is_attack(state, player, &fleet) {
            attack_fleets.push(fleet.id);
        }
        let skipped = fleet
            .stacks
            .iter()
            .any(|s| skip_with_orders.contains(&s.design) && s.count > 0)
            && !fleet.waypoints.is_empty()
            && fleet.waypoints.len() > 1;
        if skipped {
            continue;
        }
        let transport = is_transport(state, player, &fleet);
        let colony = fleet
            .stacks
            .iter()
            .any(|s| s.design == COLONY_SLOT && s.count > 0);
        if !transport && !colony {
            continue;
        }
        let has_leg = fleet.waypoints.len() > 1;
        let dest = if !has_leg {
            fleet.orbiting.and_then(|p| i16::try_from(p).ok())
        } else if fleet.waypoints[1].target_class == grobj::PLANET {
            fleet.waypoints[1]
                .target
                .and_then(|t| i16::try_from(t).ok())
        } else {
            None
        };
        let Some(dest) = dest else {
            continue;
        };
        let planet = state.planets.iter().find(|p| p.id == dest).cloned();
        let theirs = planet
            .as_ref()
            .is_none_or(|p| p.owner.is_some_and(|o| o != me));
        // A colony freighter bound for their planet is only looked at
        // when the planet is worth settling.
        if !theirs || (!transport && !valued.planets.contains(&dest)) {
            continue;
        }
        let drop = valued.planets.contains(&dest)
            && fleet.cargo.colonists > 0
            && planet.as_ref().is_some_and(|p| {
                p.owner
                    .and_then(|o| usize::try_from(o).ok())
                    .and_then(|o| state.players.get(o))
                    .is_some_and(|p| p.race.prt() != Some(crate::race::Prt::Ar))
            });
        if drop {
            use stars_formats::{ItemAction, TransportTask, XferAction};
            let mut items = [ItemAction {
                quantity: 0,
                action: XferAction::None,
            }; 5];
            items[3] = ItemAction {
                quantity: 0,
                action: XferAction::UnloadAll,
            };
            // The order goes on the leg to the planet, or on the current
            // waypoint when the fleet stands there.
            let f = &mut state.fleets[index];
            let at = if has_leg && f.waypoints[1].target == u16::try_from(dest).ok() {
                1
            } else if !has_leg {
                0
            } else {
                let at = position_of(dest).unwrap_or(f.position);
                f.waypoints.truncate(2);
                f.waypoints.push(crate::fleet::Waypoint {
                    position: at,
                    target: u16::try_from(dest).ok(),
                    target_class: grobj::PLANET,
                    warp: 1,
                    task: stars_formats::task::NONE,
                    transport: None,
                    task_data: Vec::new(),
                });
                2
            };
            let w = &mut f.waypoints[at];
            w.task = stars_formats::task::TRANSPORT;
            w.transport = Some(TransportTask { items });
            w.task_data = Vec::new();
            valued.claimed.insert(dest);
            report.dropping.push((fleet.id, dest));
        } else {
            let f = &mut state.fleets[index];
            f.waypoints.truncate(1);
            f.waypoints[0].task = stars_formats::task::NONE;
            f.waypoints[0].task_data = Vec::new();
            f.waypoints[0].transport = None;
            report.cleaned.push(fleet.id);
        }
    }
    attack_fleets
}

/// `IdTargetScout` (`1090:61de`): where a scout fleet goes.
///
/// An armed fleet (`FFleetMightHaveTeeth`) first looks for the nearest of
/// the other players' fleets — a computer player's passed over when the
/// computer players band together — that another attack fleet of ours is
/// not already chasing (one that is, one roll in three): within 180 light
/// years it is the target; further off, a fleet with half its fuel or
/// less goes home to a starbase, and otherwise the nearest planet to that
/// fleet that no attack fleet of ours is bound for is the target. Failing
/// all that, and for an unarmed fleet: the nearest planet still unknown
/// (`IdNearestUnknownPlanet`, `1090:15a4`); with none left, the best
/// enemy planet by the armada score from the home planet, or one at
/// random. The planet is claimed (`vlpbAiPlanet[+15] = 4`) and the leg
/// laid at the ideal warp. Wormholes (`plpthWorm`) are not kept.
///
/// Answers the target's id — a planet's, or the fleet word of a fleet —
/// when a leg was laid.
pub(crate) fn target_scout(
    state: &mut GameState,
    player: usize,
    me: i16,
    index: usize,
    attack_fleets: &[u16],
    marks: &mut [Mark],
    rng: &mut Rng,
) -> Option<i16> {
    let fleet = state.fleets[index].clone();
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let from = fleet.position;
    let d2 = |a: Point, b: Point| {
        let dx = i64::from(a.x) - i64::from(b.x);
        let dy = i64::from(a.y) - i64::from(b.y);
        dx * dx + dy * dy
    };
    let orbiting = fleet.orbiting.and_then(|p| i16::try_from(p).ok());
    let mut target: Option<(Point, Option<u16>, u8)> = None;
    if fleet.is_armed(&designs) {
        let chased = |state: &GameState, word: u16| {
            state.fleets.iter().any(|f| {
                f.owner == me
                    && f.id != fleet.id
                    && attack_fleets.contains(&f.id)
                    && f.waypoints.len() > 1
                    && f.waypoints[1].target_class == grobj::FLEET
                    && f.waypoints[1].target == Some(word)
            })
        };
        let mut best: Option<(i64, u16, Point)> = None;
        for pass in 0..2 {
            let bands = state.ais_band && pass == 0;
            for other in state
                .fleets
                .iter()
                .filter(|f| f.owner != me && !f.is_empty())
            {
                let computer = usize::try_from(other.owner)
                    .ok()
                    .and_then(|o| state.players.get(o))
                    .is_some_and(|p| p.control.is_computer());
                if bands && computer {
                    continue;
                }
                let word = (u16::try_from(other.owner).unwrap_or(0) << 9) | (other.id & 0x1ff);
                if chased(state, word) && rng.random(3) == 0 {
                    continue;
                }
                let d = d2(other.position, from);
                if best.is_none_or(|(b, _, _)| d < b) {
                    best = Some((d, word, other.position));
                }
            }
            if best.is_some() || !bands {
                break;
            }
        }
        match best {
            Some((d, word, at)) if d < CHASE_RANGE_SQUARED => {
                target = Some((at, Some(word), grobj::FLEET));
            }
            Some((_, _, at)) => {
                let capacity = fleet.fuel_capacity(&designs);
                if fleet.cargo.fuel <= capacity / 2
                    && move_to_nearest_starbase(state, me, index, false)
                {
                    return None;
                }
                let claimed: BTreeSet<u16> = state
                    .fleets
                    .iter()
                    .filter(|f| f.owner == me && f.id != fleet.id && attack_fleets.contains(&f.id))
                    .filter(|f| {
                        f.waypoints.len() > 1 && f.waypoints[1].target_class == grobj::PLANET
                    })
                    .filter_map(|f| f.waypoints[1].target)
                    .collect();
                let mut nearest: Option<(i64, i16, Point)> = None;
                for planet in &state.planets {
                    let Some(p) = planet.position else { continue };
                    if claimed.contains(&u16::try_from(planet.id).unwrap_or(u16::MAX)) {
                        continue;
                    }
                    let d = d2(p, at);
                    if nearest.is_none_or(|(b, _, _)| d < b) {
                        nearest = Some((d, planet.id, p));
                    }
                }
                if let Some((_, id, p)) = nearest.filter(|(_, id, _)| Some(*id) != orbiting) {
                    target = Some((p, u16::try_from(id).ok(), grobj::PLANET));
                }
            }
            None => {}
        }
    }
    let target = match target {
        Some(t) => t,
        None => {
            // The nearest planet still unknown.
            let mut best: Option<(i64, i16, Point)> = None;
            for planet in &state.planets {
                let Some(p) = planet.position else { continue };
                let unknown = usize::try_from(planet.id)
                    .ok()
                    .and_then(|i| marks.get(i))
                    .is_some_and(|m| *m == Mark::Unknown);
                if !unknown {
                    continue;
                }
                let d = d2(p, from);
                if best.is_none_or(|(b, _, _)| d < b) {
                    best = Some((d, planet.id, p));
                }
            }
            let (id, p) = match best {
                Some((_, id, p)) => (id, p),
                None => {
                    // Nothing unknown: the best enemy planet by the
                    // armada score from home, else one at random.
                    // `rgplr[].idPlanetHome`: the homeworld, read here as
                    // the homeworld we still hold.
                    let home = state
                        .planets
                        .iter()
                        .find(|p| p.homeworld && p.owner == Some(me))
                        .and_then(|p| p.position.map(|at| (p.id, at)));
                    let from_home = home.and_then(|(h, at)| {
                        armada_dest(state, player, me, h, at, &BTreeSet::new(), rng)
                    });
                    match from_home {
                        Some(found) => found,
                        None => {
                            let n = i16::try_from(state.planets.len()).unwrap_or(1).max(1);
                            let i = usize::try_from(rng.random(n))
                                .unwrap_or(0)
                                .min(state.planets.len().saturating_sub(1));
                            let planet = state.planets.get(i)?;
                            (planet.id, planet.position?)
                        }
                    }
                }
            };
            (p, u16::try_from(id).ok(), grobj::PLANET)
        }
    };
    let (at, word, class) = target;
    if class == grobj::PLANET {
        if let Some(mark) = word.map(usize::from).and_then(|i| marks.get_mut(i)) {
            *mark = Mark::Claimed;
        }
        if word.and_then(|w| i16::try_from(w).ok()) == orbiting {
            return None;
        }
    }
    let stacks: Vec<(&crate::design::ShipDesign, i32)> = fleet
        .stacks
        .iter()
        .filter_map(|s| designs.get(usize::from(s.design)).map(|d| (d, s.count)))
        .collect();
    let warp = ideal_warp(&stacks, false);
    let f = &mut state.fleets[index];
    f.waypoints.truncate(1);
    f.waypoints[0].task = stars_formats::task::NONE;
    f.waypoints[0].task_data = Vec::new();
    f.waypoints.push(crate::fleet::Waypoint {
        position: at,
        target: word,
        target_class: class,
        warp,
        task: stars_formats::task::NONE,
        transport: None,
        task_data: Vec::new(),
    });
    f.warp = Some(warp);
    word.and_then(|w| i16::try_from(w).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fittings_fit_their_hulls() {
        let slots = |hull: usize| {
            crate::components::HULLS[hull]
                .slots
                .iter()
                .take_while(|s| s.allowed != 0)
                .count()
        };
        assert_eq!(fitting::COLONY_FREIGHTER.len(), slots(1));
        assert_eq!(fitting::MEDIUM_FREIGHTER.len(), slots(1));
        assert_eq!(fitting::SCOUT.len(), slots(4));
        assert_eq!(fitting::DESTROYER.len(), slots(6));
        for f in &fitting::BATTLESHIPS {
            assert_eq!(f.len(), slots(9));
        }
        assert_eq!(fitting::B17.len(), slots(17));
        assert_eq!(fitting::B52.len(), slots(19));
        assert_eq!(fitting::MINE_LAYER.len(), slots(11));
        assert_eq!(fitting::SUPER_FREIGHTER.len(), slots(3));
    }

    #[test]
    fn the_potencies_follow_the_years() {
        assert_eq!(potency(0), [3, 1, 6, 2]);
        assert_eq!(potency(130), [3, 1, 7, 2]);
        assert_eq!(potency(140), [4, 2, 7, 2]);
        assert_eq!(potency(300), [12, 6, 12, 3]);
    }
}
