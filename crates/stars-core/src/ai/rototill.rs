//! The Rototill's turn: `DoRototillAiTurn` (`1098:1e22`), with the shared
//! routines reused from [`super::turindrone`], [`super::automitron`] and
//! [`super::robotoid`].
//!
//! The Rototill is the simplest of the five personalities with a middle
//! of their own: it hands `IroEnsureAi` no plan, its `EnsureCAShdefs`
//! (`1098:3020`) is an empty routine — it **designs nothing**, ever, and
//! builds only what the player began with — it merges nothing, rates no
//! armada and recycles no design. Its slots are the starting set's:
//! scouts in 0, colony ships in 1, remote miners in 7 and 8, bombers in
//! 13 and 14, and haulers wherever `FIsAiTransport` finds them. Its
//! scouts hunt by `IdTargetScout`, its miners dig by `FEnumCalcMinerDest`,
//! its colony ships settle with twenty-five hundred people, and a
//! bomber fleet sails by `FEnumCalcArmadaDest` once it holds two of
//! either bomber. See `docs/formulas/ai.md`, *The Rototill's turn*.

use crate::ai::automitron::{drop_pass, target_scout};
use crate::ai::colonise::{nearest_colonisable, Mark};
use crate::ai::dispatch::ideal_warp;
use crate::ai::personality::Profile;
use crate::ai::robotoid::is_transport;
use crate::ai::turindrone::{
    basic_tasks, ensure_research, fill_production_queues, is_attack_fleet, lay_leg, marks,
    mineral_worth, move_to_nearest_starbase, target_armada_as, target_freighter,
    validate_starbase_history, valued_planets, ArmadaRule, Report,
};
use crate::ai::AiPersonality;
use crate::movement::Point;
use crate::production::QueueItem;
use crate::rng::Rng;
use crate::GameState;

/// The scouts' slot.
pub const SCOUT_SLOT: u8 = 0;
/// The colony ships' slot.
pub const COLONY_SLOT: u8 = 1;
/// The remote miners' slots.
pub const MINER_SLOTS: [u8; 2] = [7, 8];
/// The bombers' slots.
pub const BOMBER_SLOTS: [u8; 2] = [13, 14];
/// How many colonists a colony ship is loaded with: `XferAiSupply(…, 3,
/// 0x19)`, twenty-five hundred people.
pub const COLONISTS_ABOARD: i32 = 25;
/// A planet's queue is only filled when it has a starbase and more than
/// this many hundreds of colonists (`1098:1fc2`: the population word
/// against 999).
pub const QUEUE_MIN_POP: i32 = 999;

/// The Rototill's turn.
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
    state.players[player].explored.extend(seen.into_keys());
    let explored = state.players[player].explored.clone();

    // `IroEnsureAi(0, 0, 0, pct)`: no plan, so the lowest field.
    // `EnsureCAShdefs` does nothing.
    report.research = ensure_research(state, player, profile.plan, profile.research_pct(turn));
    validate_starbase_history(state, player, me);

    // The planets we could settle (`1098:1ea6`): unowned, scanned, with a
    // positive opt value.
    let mut marks = marks(state, player, me, &explored, AiPersonality::Rototill);
    let colonisable = {
        let race = &state.players[player].race;
        let levels = state.players[player].research.levels;
        i64::try_from(
            state
                .planets
                .iter()
                .filter(|p| p.owner.is_none() && explored.contains(&p.id))
                .filter(|p| {
                    let reach = crate::terraform::optimal_env(p, race, levels);
                    crate::ai::colonise::pct_planet_opt_value(p, race, reach) > 0
                })
                .count(),
        )
        .unwrap_or(i64::MAX)
    };
    let mut valued = valued_planets(state, player, me);
    let mut worth = mineral_worth(state, &explored, marks.len());

    // --- The planet pass, in planet order: an own planet hostile as it
    // stands is passed over; one with a starbase and over 99,900 people
    // that has no ship queued builds two scouts in the first year, and
    // after that one colony ship — the first planet to, each year — while
    // none exist or fewer than the planets to settle, less one.
    let race = state.players[player].race.clone();
    let colony_ships: i64 = state
        .fleets
        .iter()
        .filter(|f| f.owner == me)
        .flat_map(|f| f.stacks.iter())
        .filter(|s| s.design == COLONY_SLOT)
        .map(|s| i64::from(s.count))
        .sum();
    let mut colony_queued = false;
    for index in 0..state.planets.len() {
        let planet = &state.planets[index];
        if planet.owner != Some(me) {
            continue;
        }
        if crate::hab::pct_planet_desirability(planet, &race) < 0 {
            continue;
        }
        if !planet.starbase || planet.pop <= QUEUE_MIN_POP {
            continue;
        }
        // A ship — or the first starbase design — already in the queue.
        if planet.queue.iter().any(|q| q.ship && q.item < 0x11) {
            continue;
        }
        let id = planet.id;
        let mut added: Vec<(u8, i32)> = Vec::new();
        if turn == 0 {
            added.push((SCOUT_SLOT, 2));
        } else if !colony_queued && (colony_ships == 0 || colony_ships + 1 < colonisable) {
            colony_queued = true;
            added.push((COLONY_SLOT, 1));
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

    // --- The first fleet pass: `drop_pass` with `FIsTurinDroneAiAttack`
    // listing the attack fleets, the miners with orders left to this
    // pass: in deep space with a leg, the leg's planet is claimed
    // (`vlpbAiPlanet[+1] |= 0x80`); at an unowned planet, that one; at
    // an owned planet the orders are blown away.
    let attack_fleets = drop_pass(
        state,
        player,
        me,
        &mut valued,
        &MINER_SLOTS,
        &|state, player, fleet| is_attack_fleet(state, player, fleet),
        &mut report,
    );
    for index in 0..state.fleets.len() {
        let fleet = state.fleets[index].clone();
        if fleet.owner != me || fleet.is_empty() || fleet.waypoints.len() < 2 {
            continue;
        }
        let has_miners = fleet
            .stacks
            .iter()
            .any(|s| MINER_SLOTS.contains(&s.design) && s.count > 0);
        if !has_miners {
            continue;
        }
        let claimed = match fleet.orbiting {
            None => fleet.waypoints[1]
                .target
                .and_then(|t| i16::try_from(t).ok()),
            Some(p) => {
                let owned = state
                    .planets
                    .iter()
                    .find(|q| q.id == i16::try_from(p).unwrap_or(-1))
                    .is_some_and(|q| q.owner.is_some());
                if owned {
                    let f = &mut state.fleets[index];
                    f.waypoints.truncate(1);
                    f.waypoints[0].task = stars_formats::task::NONE;
                    f.waypoints[0].task_data = Vec::new();
                    f.waypoints[0].transport = None;
                    report.cleaned.push(fleet.id);
                    continue;
                }
                i16::try_from(p).ok()
            }
        };
        if let Some(w) = claimed
            .and_then(|c| usize::try_from(c).ok())
            .and_then(|i| worth.get_mut(i))
        {
            *w |= 0x80;
        }
    }

    // --- The second fleet pass.
    let positions: Vec<(i16, Point)> = state
        .planets
        .iter()
        .filter_map(|p| p.position.map(|at| (p.id, at)))
        .collect();
    let position_of = |id: i16| positions.iter().find(|(p, _)| *p == id).map(|(_, at)| *at);
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let scout_engine_quick_jump = designs
        .first()
        .and_then(|d| d.slots.first())
        .is_some_and(|s| s.category == crate::components::slot::ENGINE && s.item == 1);
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
        !(count(13) < 2 && count(14) < 2)
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
        let has_miners = MINER_SLOTS.iter().any(|s| count(*s) > 0);
        let orbiting = fleet.orbiting.and_then(|p| i16::try_from(p).ok());
        let orbit_index = orbiting.and_then(|id| state.planets.iter().position(|p| p.id == id));
        let scrap = |state: &mut GameState, report: &mut Report| {
            let f = &mut state.fleets[index];
            f.waypoints.truncate(1);
            f.waypoints[0].task = stars_formats::task::SCRAP;
            f.waypoints[0].task_data = Vec::new();
            f.waypoints[0].transport = None;
            report.scrapped.push(fleet_id);
        };
        if has_miners {
            // Miners at a planet worth under four move to the best of the
            // rest (`FEnumCalcMinerDest`, `1088:5f32`: a claimed planet
            // passed over three times in four) and dig there, at warp 6.
            let Some(here) = orbiting else {
                continue;
            };
            let here_worth = usize::try_from(here)
                .ok()
                .and_then(|i| worth.get(i).copied())
                .unwrap_or(0);
            if here_worth >= 4 {
                continue;
            }
            let from = fleet.position;
            let mut best: Option<(u8, i64, i16, Point)> = None;
            for (id, at) in &positions {
                if *id == here {
                    continue;
                }
                let w = usize::try_from(*id)
                    .ok()
                    .and_then(|i| worth.get(i).copied())
                    .unwrap_or(0);
                let score = if w == 0 || (rng.random(100) > 0x18 && w & 0x80 != 0) {
                    0
                } else {
                    w
                };
                if score <= 1 {
                    continue;
                }
                let dx = i64::from(at.x) - i64::from(from.x);
                let dy = i64::from(at.y) - i64::from(from.y);
                let d2 = dx * dx + dy * dy;
                if best.is_none_or(|(s, d, _, _)| score > s || (score == s && d2 < d)) {
                    best = Some((score, d2, *id, *at));
                }
            }
            if let Some((_, _, target, at)) = best {
                lay_leg(
                    &mut state.fleets[index],
                    at,
                    target,
                    stars_formats::task::REMOTE_MINING,
                    6,
                );
                if let Some(w) = usize::try_from(target).ok().and_then(|i| worth.get_mut(i)) {
                    *w |= 0x80;
                }
                if let Some(w) = usize::try_from(here).ok().and_then(|i| worth.get_mut(i)) {
                    *w &= 0x80;
                }
                report.mining.push((fleet_id, target));
            }
            continue;
        }
        if fleet.waypoints.len() > 1 {
            continue;
        }
        if count(COLONY_SLOT) > 0 {
            let at_own_populous = orbit_index
                .is_some_and(|p| state.planets[p].owner == Some(me) && state.planets[p].pop >= 50);
            if !at_own_populous && fleet.cargo.colonists == 0 {
                // Empty, away from a populous own planet: home to an own
                // starbase (unless the design's engine is one of the two
                // cheapest), else scrapped; at an own starbase it waits.
                let at_own_starbase = orbit_index.is_some_and(|p| {
                    state.planets[p].owner == Some(me) && state.planets[p].starbase
                });
                if at_own_starbase {
                    continue;
                }
                let engine_beyond_second = designs
                    .get(usize::from(COLONY_SLOT))
                    .and_then(|d| d.slots.first())
                    .is_some_and(|s| s.item >= 2);
                if engine_beyond_second && move_to_nearest_starbase(state, me, index, false) {
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
            if let Some(p) = orbit_index.filter(|&p| state.planets[p].owner == Some(me)) {
                let room =
                    state.fleets[index].cargo_capacity(&designs) - state.fleets[index].cargo.mass();
                let take = COLONISTS_ABOARD.min(state.planets[p].pop).min(room).max(0);
                state.planets[p].pop -= take;
                state.fleets[index].cargo.colonists += take;
            }
            // With nowhere to go the routine would try a wormhole
            // (`FGotoWormholeAiFleet`), which is not kept.
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
        if BOMBER_SLOTS.iter().any(|s| count(*s) > 0) {
            let rule = ArmadaRule {
                bombers: &BOMBER_SLOTS,
                ready: &ready,
                warp: Some(4),
            };
            if let Some(to) = target_armada_as(state, player, me, index, &rule, rng) {
                report.attacking.push((fleet_id, to));
            }
            continue;
        }
        if count(SCOUT_SLOT) > 0 {
            // A scout on the Quick Jump 5 with under 2 mg of fuel is
            // scrapped; the rest hunt and scout.
            if scout_engine_quick_jump && fleet.cargo.fuel < 2 {
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
        AiPersonality::Rototill,
        rng,
        &mut report,
    );
    fill_production_queues(state, player, me, AiPersonality::Rototill, rng, &mut report);
    report
}
