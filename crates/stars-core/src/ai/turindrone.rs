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
    /// Fleets given the Scrap task, by fleet id.
    pub scrapped: Vec<u16>,
    /// Miners sent to dig, by fleet id, and where.
    pub mining: Vec<(u16, i16)>,
    /// Mine layers told to lay, by fleet id.
    pub laying: Vec<u16>,
    /// Haulers sent, by fleet id, and where.
    pub hauling: Vec<(u16, i16)>,
    /// Fleets merged into another at the same place, as `(gone, into)`.
    pub merged: Vec<(u16, u16)>,
    /// Armadas sent, by fleet id, and where.
    pub attacking: Vec<(u16, i16)>,
    /// Starbases queued, by planet.
    pub starbases: Vec<i16>,
    /// Mines and factories queued by `FillProductionQueue`, as
    /// `(planet, mines, factories)`.
    pub filled: Vec<(i16, i32, i32)>,
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

    // `MergeAllShdefs`, four times: the armada classes together (slots 4
    // to 7 and 13 to 15), the mine layers, the destroyers and the miners —
    // each fleet of ours at a place joining the first of its kind there.
    for mask in [0xe0f0u16, 0x1000, 0x0c00, 0x000c] {
        merge_all(state, me, mask, &mut report);
    }

    // `EnsureTurinDroneShdefs`: the designs the personality wants in its
    // slots, made when the tech allows.
    ensure_designs(state, player, rng, &mut report);

    let marks = marks(state, player, me, &explored);
    let anywhere_to_settle = marks.contains(&Mark::Colonisable);

    // `CheckAiShdefStatus` over each slot range: how many ships of the
    // range exist, the newest design in it, and the recycling of old ones.
    let recycle: i16 = if state.turn < 120 {
        50
    } else if state.turn < 200 {
        70
    } else {
        100
    };
    let freighters = check_status(state, player, me, 6, 7, recycle);
    let cruisers = check_status(state, player, me, 8, 9, recycle);
    let bombers = check_status(state, player, me, 13, 14, recycle);
    let battleships = check_status(state, player, me, 4, 5, recycle);
    let _mine_layers = check_status(state, player, me, 12, 12, recycle);
    let fifteens = check_status(state, player, me, 15, 15, recycle);
    let _miners = if state.players[player].research.levels[3] < 7 {
        Status::default()
    } else {
        check_status(state, player, me, 2, 3, recycle)
    };
    let destroyers = check_status(state, player, me, 10, 11, recycle);

    // --- The planet pass: the queue at every planet with a starbase and
    // people enough.
    let colony_design = colony_design(state, player);
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
    let colony_ships = colony_design.map_or(0, |d| ships_of(state, d));
    let scout_design = state
        .designs
        .get(player)
        .and_then(|d| d.get(usize::from(SCOUT_SLOT)))
        .filter(|d| d.hull().is_some() && !d.obsolete)
        .cloned();
    let planet_count = i32::try_from(state.planets.len()).unwrap_or(0);
    let owned =
        i32::try_from(state.planets.iter().filter(|p| p.owner == Some(me)).count()).unwrap_or(0);
    let levels = state.players[player].research.levels;
    let potency = potency(state.turn);
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
            if scout_design.is_some() {
                let mut left = planet_count;
                let mut scouts = 0;
                while left > 0 {
                    scouts += 1;
                    left -= if left < 191 { 30 } else { 100 };
                }
                added.push((SCOUT_SLOT, scouts));
            }
        } else if let Some(scout) = scout_design.as_ref().filter(|d| d.hull_id == 5) {
            // A Frigate scout, while fewer than `min(cPlanMax/4, 32)` exist
            // and ten times the built count is under the existing count.
            let limit = (planet_count / 4).min(32);
            let existing = ships_of(state, SCOUT_SLOT);
            if existing < limit && i64::from(scout.built) * 10 < i64::from(existing) {
                added.push((SCOUT_SLOT, 1));
            }
        }
        // A cruiser: Weapons past 4, and fewer than the larger of a tenth of
        // the planets owned and twice the AI's own tally (which has nothing
        // in it yet), or under ten sevenths of that with one roll in four.
        let want_cruisers = (owned / 10).max(0);
        if levels[1] > 4 {
            if let Some(latest) = cruisers.latest {
                let count = cruisers.count;
                if count < want_cruisers || (count < want_cruisers * 10 / 7 && rng.random(4) == 0) {
                    added.push((latest, 1));
                }
            }
        }
        if anywhere_to_settle && colony_ships < 2 {
            if let Some(design) = colony_design {
                added.push((design, 4));
            }
        }
        // Mine layers: slot 12 live, one roll in three, the fleet of them
        // here under ten (under seventeen with one in eight), and a roll of
        // `2 × count + 1` coming up zero — three at a time.
        let layer_live = state.designs[player]
            .get(12)
            .is_some_and(|d| d.hull().is_some() && !d.obsolete);
        if layer_live && rng.random(3) == 0 {
            let here = state
                .fleets
                .iter()
                .find(|f| {
                    f.owner == me
                        && f.orbiting == Some(u16::try_from(id).unwrap_or(u16::MAX))
                        && f.stacks.iter().any(|s| s.design == 12 && s.count > 0)
                })
                .map_or(0, |f| {
                    f.stacks
                        .iter()
                        .filter(|s| s.design == 12)
                        .map(|s| s.count)
                        .sum::<i32>()
                });
            if (here < 10 || (here < 17 && rng.random(8) == 0))
                && rng.random(i16::try_from(here * 2 + 1).unwrap_or(i16::MAX)) == 0
            {
                added.push((12, 3));
            }
        }
        // A bomber when a war fleet here already holds `potency[2]` of them.
        if let Some(latest) = bombers.latest {
            let armada_here = state.fleets.iter().any(|f| {
                f.owner == me
                    && f.orbiting == Some(u16::try_from(id).unwrap_or(u16::MAX))
                    && is_attack_fleet(state, player, f)
                    && f.stacks
                        .iter()
                        .filter(|s| s.design == 13 || s.design == 14)
                        .map(|s| s.count)
                        .sum::<i32>()
                        >= i32::from(potency[2])
            });
            if armada_here {
                added.push((latest, 1));
            }
        }
        // Then the classes paid for out of what is left: up to five of each
        // while fewer than the limit exist, stopping at the first that
        // cannot be paid, and stopping the whole pass there.
        let mut paid = true;
        for (status, limit) in [
            (&battleships, planet_count / 24 + 4),
            (&freighters, planet_count / 12 + 8),
            (&destroyers, planet_count / 4 + 12),
            (&fifteens, planet_count / 12 + 8),
        ] {
            if !paid {
                break;
            }
            let Some(latest) = status.latest else {
                continue;
            };
            if status.count >= limit {
                continue;
            }
            let race = state.players[player].race.clone();
            let mut left = crate::ai::production::resources_available(
                &state.planets[index],
                &race,
                state.players[player].research_pct,
                i16::from(levels[0]),
            );
            let committed = crate::ai::production::queue_cost(&state.planets[index].queue, &race);
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
    // The mineral worth of every unowned scanned planet
    // (`vlpbAiPlanet[id*16 + 1]`): each concentration halved, capped at 75,
    // summed, capped at 127 — with the top bit for a planet one of our
    // miners is at or bound for.
    let mut worth: Vec<u8> = vec![0; marks.len()];
    for planet in &state.planets {
        let Ok(at) = usize::try_from(planet.id) else {
            continue;
        };
        if planet.owner.is_some() || !explored.contains(&planet.id) {
            continue;
        }
        let sum: u32 = planet
            .min_conc
            .iter()
            .map(|c| if *c < 0x43 { u32::from(*c) / 2 } else { 0x4b })
            .sum();
        worth[at] = u8::try_from(sum.min(0x7f)).unwrap_or(0x7f);
    }
    for fleet in state.fleets.iter().filter(|f| f.owner == me) {
        let is_miner = fleet
            .stacks
            .iter()
            .any(|s| (s.design == 2 || s.design == 3) && s.count > 0);
        if !is_miner {
            continue;
        }
        let claimed = if fleet.waypoints.len() > 1 {
            fleet.waypoints[1].target
        } else {
            fleet.orbiting
        };
        if let Some(at) = claimed.map(usize::from) {
            if let Some(w) = worth.get_mut(at) {
                *w |= 0x80;
            }
        }
    }
    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        if fleet.owner != me || fleet.is_empty() {
            continue;
        }
        let fleet_id = fleet.id;
        let carries = |slot: u8| fleet.stacks.iter().any(|s| s.design == slot && s.count > 0);
        let has_miners = carries(2) || carries(3);
        let has_orders = fleet.waypoints.len() > 1;

        // At year 0 the starting miners, and any cruiser standing idle, are
        // recycled: the Berserkers scrap their Potato Bugs.
        if state.turn == 0 && (has_miners || (!has_orders && (carries(8) || carries(9)))) {
            let fleet = &mut state.fleets[index];
            fleet.waypoints.truncate(1);
            if let Some(first) = fleet.waypoints.first_mut() {
                first.task = stars_formats::task::SCRAP;
            }
            report.scrapped.push(fleet_id);
            continue;
        }

        // Miners: at a planet worth less than four, move to the best of the
        // rest (`LpplFindBestEnum` over `FEnumCalcMinerDest`) and dig
        // there; a claimed planet is passed over three times in four.
        if has_miners {
            let Some(here) = fleet.orbiting.and_then(|p| i16::try_from(p).ok()) else {
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
                // `0x1163`: the Remote Mining task, at warp 6.
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
        if has_orders {
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

        // A mine layer on its own, with no task: lay mines for ever.
        let only_layers = fleet.stacks.iter().all(|s| s.design == 12);
        if only_layers && carries(12) {
            let fleet = &mut state.fleets[index];
            if fleet.waypoints.len() == 1 && fleet.waypoints[0].task == stars_formats::task::NONE {
                fleet.waypoints[0].task = stars_formats::task::LAY_MINES;
                fleet.waypoints[0].task_data = vec![5, 0];
                report.laying.push(fleet_id);
            }
            continue;
        }
        // An armada — bombers aboard (slots 13, 14): waits at an own
        // starbase until it holds `potency[2]` bombers and `potency[1]`
        // battleships, stays at a foreign planet unless an enemy warship
        // is there too, and otherwise goes for the best of the other
        // players' planets by `FEnumCalcArmadaDest`.
        if carries(13) || carries(14) {
            if let Some(target) = target_armada(state, player, me, index, &potency, rng) {
                report.attacking.push((fleet_id, target));
            }
            continue;
        }
        // The haulers (slots 8 and 9): `IdTargetFreighter`.
        if carries(8) || carries(9) {
            if let Some(target) = target_freighter(state, player, me, index, &worth, rng) {
                report.hauling.push((fleet_id, target));
            }
            continue;
        }
        // The scouts and the destroyers (slots 0, 10 and 11) scout; the
        // bombers (13, 14) wait for their part of the pass to be written.
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
    // `HandleBasicAiTasks`, then `FillProductionQueue`.
    basic_tasks(state, player, me, &worth, rng, &mut report);
    fill_production_queues(state, player, me, rng, &mut report);

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

/// What `CheckAiShdefStatus` reports of a slot range.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Status {
    /// Ships of the range's live designs, capped at 32,000.
    pub count: i32,
    /// The newest live design in the range, by the year it was made.
    pub latest: Option<u8>,
}

/// `CheckAiShdefStatus` (`1090:9b70`): count the ships of the live designs
/// in a slot range and find the newest; a design older than `recycle`
/// years is retired if no ship of it exists, and marked for
/// `SplitOutShdefs` otherwise (the split is not written yet).
fn check_status(
    state: &mut GameState,
    player: usize,
    me: i16,
    from: u8,
    to: u8,
    recycle: i16,
) -> Status {
    let mut status = Status::default();
    let mut total: i64 = 0;
    for slot in from..=to {
        let live = state.designs[player]
            .get(usize::from(slot))
            .is_some_and(|d| d.hull().is_some() && !d.obsolete);
        if !live {
            continue;
        }
        let existing: i64 = state
            .fleets
            .iter()
            .filter(|f| f.owner == me)
            .flat_map(|f| f.stacks.iter())
            .filter(|s| s.design == slot)
            .map(|s| i64::from(s.count))
            .sum();
        total += existing;
        let designed = state.designs[player][usize::from(slot)].designed;
        let newer = status
            .latest
            .is_none_or(|l| state.designs[player][usize::from(l)].designed < designed);
        if newer {
            status.latest = Some(slot);
        }
        // Past the recycling period: retired when none is left, and
        // otherwise marked for `SplitOutShdefs` (not written yet).
        if state.turn - designed > recycle && existing == 0 {
            state.designs[player][usize::from(slot)].obsolete = true;
        }
    }
    status.count = i32::try_from(total.min(32_000)).unwrap_or(32_000);
    status
}

/// The armada potencies for the year — `vrgAiArmadaPotency`, four figures
/// the personality sizes its fleets by.
fn potency(turn: i16) -> [u8; 4] {
    let a = if turn > 130 { 3 + (turn - 120) / 20 } else { 3 }.min(50);
    let c = if turn > 115 { 6 + (turn - 100) / 22 } else { 6 }.min(12);
    let d = if c / 2 - 1 < 4 { (c / 2 - 1).max(0) } else { 3 };
    [
        u8::try_from(a).unwrap_or(50),
        u8::try_from(a / 2).unwrap_or(25),
        u8::try_from(c).unwrap_or(12),
        u8::try_from(d).unwrap_or(3),
    ]
}

/// `FIsTurinDroneAiAttack`: any hull from the Destroyer to the Dreadnought
/// aboard — hull ids 4 to 10 in the original's numbering, the Scout and
/// Frigate included.
fn is_attack_fleet(state: &GameState, player: usize, fleet: &crate::fleet::Fleet) -> bool {
    fleet.stacks.iter().any(|s| {
        s.count > 0
            && state.designs[player]
                .get(usize::from(s.design))
                .is_some_and(|d| (4..=10).contains(&d.hull_id))
    })
}

/// `IdTargetFreighter` (`1090:2b2e`), the part of it a hauler needs: where
/// to go next, and what to move when it gets there. The freighter's home
/// is the personality's first planet with a starbase.
///
/// Every other planet is scored, and the best score wins with the nearer
/// planet breaking a tie; distance enters as `d/25 + 24` light years:
///
/// * an unowned planet one of our miners has claimed: its mineral worth
///   times 500, over the distance — go and collect what was dug;
/// * home, when the hold is more than a third full: 25,000 when full, else
///   the fill times twenty over the distance — bring it back;
/// * an own planet without a starbase that has no ship in its queue: when
///   its desirability is negative and we are at home, 25,000 — people are
///   wanted there; otherwise what it holds of the minerals home is short
///   of, as a share of the hold, capped at what the hold has room for, times
///   a hundred over the distance;
/// * a planet already another hauler's, or the one we are at: nothing.
///
/// The orders: out to a mined planet, load all three minerals; to an own
/// planet, unload all three and any colonists (with a thousand kT of
/// colonists taken aboard at home first when home has 1,200 kT or more and
/// the planet has fewer than home); back home, unload all. Salvage, the
/// drops onto enemy planets and the finer loading rules are not written.
fn target_freighter(
    state: &mut GameState,
    player: usize,
    me: i16,
    index: usize,
    worth: &[u8],
    rng: &mut Rng,
) -> Option<i16> {
    let _ = rng;
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let home = state
        .planets
        .iter()
        .find(|p| p.owner == Some(me) && p.starbase)?
        .clone();
    let fleet = state.fleets[index].clone();
    let here = fleet.orbiting.and_then(|p| i16::try_from(p).ok());
    let at_home = here == Some(home.id);
    let capacity = fleet.cargo_capacity(&designs);
    if capacity <= 0 {
        return None;
    }
    let held: i32 = fleet.cargo.minerals.iter().sum::<i32>() + fleet.cargo.colonists;
    let fill = 100 - (capacity - held) * 100 / capacity;
    // The minerals home is shortest of, by how much of each is on hand.
    let mut order: Vec<usize> = (0..3).collect();
    order.sort_by_key(|k| home.surface_min[*k]);
    let scarce = order[0];
    // Planets another hauler is already bound for.
    let taken: BTreeSet<u16> = state
        .fleets
        .iter()
        .filter(|f| f.owner == me && f.id != fleet.id && f.waypoints.len() > 1)
        .filter(|f| {
            f.stacks
                .iter()
                .any(|s| (s.design == 8 || s.design == 9) && s.count > 0)
        })
        .filter_map(|f| f.waypoints[1].target)
        .collect();

    let from = fleet.position;
    let distance = |at: Point| -> i64 {
        let dx = i64::from(at.x) - i64::from(from.x);
        let dy = i64::from(at.y) - i64::from(from.y);
        let d = ((dx * dx + dy * dy) as f64).sqrt() as i64;
        (d / 25 + 24).max(1)
    };
    let mut best: Option<(i64, i16, Point, bool)> = None;
    for planet in &state.planets {
        let Some(at) = planet.position else {
            continue;
        };
        if Some(planet.id) == here || taken.contains(&u16::try_from(planet.id).unwrap_or(u16::MAX))
        {
            continue;
        }
        let worth_here = usize::try_from(planet.id)
            .ok()
            .and_then(|i| worth.get(i).copied())
            .unwrap_or(0);
        let (score, load) = if planet.owner.is_none() && worth_here & 0x80 != 0 {
            (i64::from(worth_here & 0x7f) * 500 / distance(at), true)
        } else if planet.owner != Some(me) {
            continue;
        } else if planet.id == home.id {
            if fill <= 34 {
                continue;
            }
            if fill == 100 {
                (25_000, false)
            } else {
                (i64::from(fill) * 20 / distance(at), false)
            }
        } else if planet.starbase || planet.queue.iter().any(|q| q.ship) {
            continue;
        } else if at_home
            && crate::hab::pct_planet_desirability(planet, &state.players[player].race) < 0
        {
            (25_000, false)
        } else {
            // What it needs of the minerals home is short of: the sum of
            // its holdings of them, as a share of the hold.
            let have: i64 = order
                .iter()
                .take(2)
                .map(|k| i64::from(planet.surface_min[*k]))
                .sum();
            if have <= 9 {
                continue;
            }
            let share = (have * 100 / i64::from(capacity)).min(i64::from(100 - fill));
            (share * 100 / distance(at), false)
        };
        if score <= 0 {
            continue;
        }
        if best.is_none_or(|(b, _, _, _)| score > b) {
            best = Some((score, planet.id, at, load));
        }
    }
    let (_, target, at, load) = best?;

    // Colonists aboard at home for an own planet with fewer than home.
    let stacks: Vec<(&crate::design::ShipDesign, i32)> = fleet
        .stacks
        .iter()
        .filter_map(|s| designs.get(usize::from(s.design)).map(|d| (d, s.count)))
        .collect();
    let warp = ideal_warp(&stacks, false);
    let to_own = state
        .planets
        .iter()
        .find(|p| p.id == target)
        .is_some_and(|p| p.owner == Some(me));
    if at_home && to_own && target != home.id {
        let target_pop = state
            .planets
            .iter()
            .find(|p| p.id == target)
            .map_or(0, |p| p.pop);
        if home.pop >= 1200 && target_pop < home.pop {
            let room = (capacity - held).max(0);
            let take = 1000.min(room).min(home.pop);
            if let Some(h) = state.planets.iter_mut().find(|p| p.id == home.id) {
                h.pop -= take;
            }
            state.fleets[index].cargo.colonists += take;
        }
    }
    use stars_formats::{ItemAction, TransportTask, XferAction};
    let mineral = if load {
        XferAction::LoadAll
    } else {
        XferAction::UnloadAll
    };
    let mut items = [ItemAction {
        quantity: 0,
        action: XferAction::None,
    }; 5];
    for item in items.iter_mut().take(3) {
        item.action = mineral;
    }
    if to_own {
        items[3].action = XferAction::UnloadAll;
    }
    let _ = scarce;
    lay_leg(
        &mut state.fleets[index],
        at,
        target,
        stars_formats::task::TRANSPORT,
        warp,
    );
    if let Some(leg) = state.fleets[index].waypoints.get_mut(1) {
        leg.transport = Some(TransportTask { items });
        leg.task_data = TransportTask { items }.encode();
    }
    Some(target)
}

/// `MergeAllShdefs` (`1090:5a6c`): every fleet of ours carrying a design of
/// the slots in `mask` joins the first such fleet found at the same planet
/// and place; the joined fleet's ships and cargo pass to the survivor.
fn merge_all(state: &mut GameState, me: i16, mask: u16, report: &mut Report) {
    let mut survivors: Vec<(Option<u16>, Point, usize)> = Vec::new();
    let mut gone: Vec<usize> = Vec::new();
    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        if fleet.owner != me || fleet.is_empty() {
            continue;
        }
        let in_mask = fleet
            .stacks
            .iter()
            .any(|s| s.count > 0 && s.design < 16 && mask & (1 << s.design) != 0);
        if !in_mask {
            continue;
        }
        let key = (fleet.orbiting, fleet.position);
        if let Some((_, _, into)) = survivors.iter().find(|(o, p, _)| (*o, *p) == key).copied() {
            // `Merge2Fleets`: the ships move, and the cargo follows them.
            let before_into = state.fleets[into].stacks.clone();
            let before_gone = state.fleets[index].stacks.clone();
            let stacks = std::mem::take(&mut state.fleets[index].stacks);
            for stack in stacks {
                match state.fleets[into]
                    .stacks
                    .iter_mut()
                    .find(|s| s.design == stack.design)
                {
                    Some(s) => s.count += stack.count,
                    None => state.fleets[into].stacks.push(stack),
                }
            }
            let designs = state
                .designs
                .get(usize::try_from(me).unwrap_or(usize::MAX))
                .cloned()
                .unwrap_or_default();
            let (low, high) = (into.min(index), into.max(index));
            let (head, tail) = state.fleets.split_at_mut(high);
            let (a, b) = if into < index {
                (&mut head[low], &mut tail[0])
            } else {
                (&mut tail[0], &mut head[low])
            };
            crate::fleet::balance_cargo([a, b], [&before_into, &before_gone], &designs);
            report
                .merged
                .push((state.fleets[index].id, state.fleets[into].id));
            gone.push(index);
        } else if survivors.len() < 32 {
            survivors.push((key.0, key.1, index));
        }
    }
    if !gone.is_empty() {
        state.fleets.retain(|f| !f.is_empty());
    }
}

/// `FEnumCalcArmadaDest` (`1088:3286`) over every planet, from `base`:
/// a foreign planet's mark — 1, or 2 with a starbase — plus 7, 5, 4, 3, 2
/// or 1 for lying within 50, 100, 150, 200, 300 or 500 light years, a
/// claimed planet counting only one time in four; the best score wins, the
/// nearer breaking a tie, and a score of one is no target. With the
/// "computer players form alliances" option only human players' planets
/// are looked at first (`FEnumCalcArmadaHumanDest`, `1088:3406`).
fn target_armada(
    state: &mut GameState,
    player: usize,
    me: i16,
    index: usize,
    potency: &[u8; 4],
    rng: &mut Rng,
) -> Option<i16> {
    let fleet = state.fleets[index].clone();
    let here = fleet
        .orbiting
        .and_then(|p| i16::try_from(p).ok())
        .and_then(|id| state.planets.iter().find(|p| p.id == id).cloned());
    let base = match &here {
        None => state
            .planets
            .iter()
            .find(|p| p.owner == Some(me) && p.starbase)?
            .clone(),
        Some(planet) if planet.owner == Some(me) => {
            if planet.starbase {
                let bombers: i32 = fleet
                    .stacks
                    .iter()
                    .filter(|s| s.design == 13 || s.design == 14)
                    .map(|s| s.count)
                    .sum();
                let battleships: i32 = fleet
                    .stacks
                    .iter()
                    .filter(|s| s.design == 4 || s.design == 5)
                    .map(|s| s.count)
                    .sum();
                if bombers < i32::from(potency[2]) || battleships < i32::from(potency[1]) {
                    return None;
                }
            }
            planet.clone()
        }
        Some(planet) if planet.owner.is_some() => {
            // At somebody else's planet: stay, unless one of their warships
            // is here too.
            let contested = state.fleets.iter().any(|f| {
                f.owner != me
                    && f.position == fleet.position
                    && usize::try_from(f.owner).is_ok_and(|o| {
                        let hulls: Vec<(u8, i32)> = f
                            .stacks
                            .iter()
                            .filter_map(|s| {
                                state
                                    .designs
                                    .get(o)
                                    .and_then(|d| d.get(usize::from(s.design)))
                                    .and_then(|d| u8::try_from(d.hull_id).ok())
                                    .map(|h| (h, s.count))
                            })
                            .collect();
                        crate::ai::dispatch::is_attack_fleet_simple(&hulls)
                    })
            });
            if !contested {
                return None;
            }
            planet.clone()
        }
        Some(planet) => planet.clone(),
    };
    let from = base.position?;
    let explored = state.players[player].explored.clone();
    let claimed: BTreeSet<u16> = state
        .fleets
        .iter()
        .filter(|f| f.owner == me && f.id != fleet.id && f.waypoints.len() > 1)
        .filter(|f| {
            f.stacks
                .iter()
                .any(|s| (s.design == 13 || s.design == 14) && s.count > 0)
        })
        .filter_map(|f| f.waypoints[1].target)
        .collect();
    let human = |owner: i16| {
        usize::try_from(owner)
            .ok()
            .and_then(|o| state.players.get(o))
            .is_some_and(|p| !p.control.is_computer())
    };
    let pick = |only_humans: bool, rng: &mut Rng| -> Option<(i16, Point)> {
        let mut best: Option<(u8, i64, i16, Point)> = None;
        for planet in &state.planets {
            let (Some(owner), Some(at)) = (planet.owner, planet.position) else {
                continue;
            };
            if owner == me || planet.id == base.id || !explored.contains(&planet.id) {
                continue;
            }
            if only_humans && !human(owner) {
                continue;
            }
            let mut score: u8 = if planet.starbase { 2 } else { 1 };
            let dx = i64::from(at.x) - i64::from(from.x);
            let dy = i64::from(at.y) - i64::from(from.y);
            let d2 = dx * dx + dy * dy;
            score += match d2 {
                d if d < 2_500 => 7,
                d if d < 10_000 => 5,
                d if d < 22_500 => 4,
                d if d < 40_000 => 3,
                d if d < 90_000 => 2,
                d if d < 250_000 => 1,
                _ => 0,
            };
            if claimed.contains(&u16::try_from(planet.id).unwrap_or(u16::MAX)) && rng.random(4) != 0
            {
                continue;
            }
            if score <= 1 {
                continue;
            }
            if best.is_none_or(|(s, d, _, _)| score > s || (score == s && d2 < d)) {
                best = Some((score, d2, planet.id, at));
            }
        }
        best.map(|(_, _, id, at)| (id, at))
    };
    let target = if state.ais_band {
        pick(true, rng).or_else(|| pick(false, rng))
    } else {
        pick(false, rng)
    };
    let (target, at) = target?;
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let stacks: Vec<(&crate::design::ShipDesign, i32)> = fleet
        .stacks
        .iter()
        .filter_map(|s| designs.get(usize::from(s.design)).map(|d| (d, s.count)))
        .collect();
    let warp = ideal_warp(&stacks, false);
    lay_leg(
        &mut state.fleets[index],
        at,
        target,
        stars_formats::task::NONE,
        warp,
    );
    Some(target)
}

/// `HandleBasicAiTasks` (`1090:95a4`) and the two calls around it.
///
/// * `KeepFleetsMoving`: every fleet of ours with orders is re-speeded
///   (`SetAiFleetIdealSpeed`) — here to `IFindIdealWarp`'s warp;
/// * `QueueAiStarbases`: the newest starbase design queued at every planet
///   without one, by [`crate::ai::ships::queue_ai_starbase`];
/// * then for every own planet with 60 kT of people or more (or one marked
///   hostile), whose queue's minerals are already covered: a starbase
///   upgrade by [`crate::ai::ships::upgrade_ai_starbase`], else terraforming
///   by [`crate::ai::production::queue_ai_terraforming`]. `FAIFling`, the
///   scanner and the defences (`FQueueAiScanner`, `FQueueAiDefenses`) and
///   `AddMinesToBlockedQueues` are not written yet; `FixPlanetsUnderAttack`
///   never runs in a tutorial game (flag bit 3).
fn basic_tasks(
    state: &mut GameState,
    player: usize,
    me: i16,
    worth: &[u8],
    rng: &mut Rng,
    report: &mut Report,
) {
    let _ = worth;
    // KeepFleetsMoving.
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    for fleet in state.fleets.iter_mut().filter(|f| f.owner == me) {
        if fleet.waypoints.len() < 2 {
            continue;
        }
        let stacks: Vec<(&crate::design::ShipDesign, i32)> = fleet
            .stacks
            .iter()
            .filter_map(|s| designs.get(usize::from(s.design)).map(|d| (d, s.count)))
            .collect();
        let warp = ideal_warp(&stacks, false);
        if warp > 0 {
            fleet.waypoints[1].warp = warp;
            fleet.warp = Some(warp);
        }
    }

    // QueueAiStarbases: the newest starbase design, by the year it was made.
    let latest_starbase: Option<u8> = designs
        .iter()
        .enumerate()
        .skip(usize::from(crate::startup::FIRST_STARBASE_SLOT))
        .filter(|(_, d)| d.hull().is_some() && !d.obsolete)
        .max_by_key(|(_, d)| d.designed)
        .and_then(|(i, _)| u8::try_from(i - usize::from(crate::startup::FIRST_STARBASE_SLOT)).ok());
    let ctx = crate::ai::production::Context {
        personality: Some(AiPersonality::TurinDrone),
        research_pct: state.players[player].research_pct,
        tech: state.players[player].research.levels,
        turn: i32::from(state.turn),
        terraform_steps: 0,
        factories_cost_all_minerals: state.tutorial,
    };
    let race = state.players[player].race.clone();
    let owned: Vec<usize> = (0..state.planets.len())
        .filter(|&i| state.planets[i].owner == Some(me))
        .collect();
    for &index in &owned {
        let planet = &state.planets[index];
        if let Some(slot) = crate::ai::ships::queue_ai_starbase(
            planet,
            &ctx,
            latest_starbase,
            crate::ai::ships::PlanetTask::default(),
        ) {
            let id = planet.id;
            state.planets[index]
                .queue
                .push(crate::ai::ships::starbase_entry(slot));
            report.starbases.push(id);
        }
    }

    // The planets, in the AI's shuffled order.
    let ids: Vec<i16> = owned.iter().map(|&i| state.planets[i].id).collect();
    let order = crate::ai::planet_order(&ids, rng, true);
    for id in order {
        let Some(index) = state.planets.iter().position(|p| p.id == id) else {
            continue;
        };
        let planet = state.planets[index].clone();
        let hostile = crate::hab::pct_planet_desirability(&planet, &race) < 0;
        if planet.pop < 60 && !hostile {
            continue;
        }
        let available = crate::ai::production::resources_available(
            &planet,
            &race,
            ctx.research_pct,
            i16::from(ctx.tech[0]),
        );
        let committed = crate::ai::production::queue_cost(&planet.queue, &race);
        if (0..3).any(|k| available[k] < committed[k]) {
            continue;
        }
        let mut written = false;
        if !hostile {
            if let (Some(latest), Some(current)) = (latest_starbase, planet.starbase_design) {
                let current_design = designs
                    .get(usize::from(crate::startup::FIRST_STARBASE_SLOT) + usize::from(current));
                let inputs = crate::ai::ships::UpgradeInputs {
                    latest,
                    latest_orbital_fort: latest,
                    design_turn: i32::from(current_design.map_or(0, |d| d.designed)),
                    sideways_design_free: true,
                    ..crate::ai::ships::UpgradeInputs::default()
                };
                if let Some(slot) =
                    crate::ai::ships::upgrade_ai_starbase(&planet, &ctx, &inputs, rng)
                {
                    state.planets[index]
                        .queue
                        .push(crate::ai::ships::starbase_entry(slot));
                    report.starbases.push(id);
                    written = true;
                }
            }
        }
        if !written {
            let steps = crate::ai::production::queue_ai_terraforming(&planet, &race, &ctx);
            if steps > 0 {
                state.planets[index].queue.insert(
                    0,
                    QueueItem {
                        count: steps,
                        item: crate::production::item::TERRAFORM,
                        ship: false,
                        completion: 0,
                    },
                );
            }
        }
    }
}

/// `FillProductionQueue` (`1090:9c1c`): `FFillProdMinesAndFactories` at
/// every own planet, mines at the front of the queue and factories at
/// the back.
fn fill_production_queues(
    state: &mut GameState,
    player: usize,
    me: i16,
    rng: &mut Rng,
    report: &mut Report,
) {
    let race = state.players[player].race.clone();
    let ctx = crate::ai::production::Context {
        personality: Some(AiPersonality::TurinDrone),
        research_pct: state.players[player].research_pct,
        tech: state.players[player].research.levels,
        turn: i32::from(state.turn),
        terraform_steps: 0,
        factories_cost_all_minerals: state.tutorial,
    };
    let ids: Vec<i16> = state
        .planets
        .iter()
        .filter(|p| p.owner == Some(me))
        .map(|p| p.id)
        .collect();
    for id in crate::ai::planet_order(&ids, rng, true) {
        let Some(index) = state.planets.iter().position(|p| p.id == id) else {
            continue;
        };
        let decision = crate::ai::production::fill_prod_mines_and_factories(
            &state.planets[index],
            &race,
            &ctx,
        );
        if !decision.changed() {
            continue;
        }
        let (mines, factories) = (decision.mines, decision.factories);
        for entry in decision.entries() {
            if entry.item == crate::production::item::FACTORY
                || entry.item == crate::production::item::ALCHEMY
            {
                state.planets[index].queue.push(entry);
            } else {
                state.planets[index].queue.insert(0, entry);
            }
        }
        report.filled.push((id, mines, factories));
    }
}
