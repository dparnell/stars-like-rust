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

/// The personality's research plan, `vrgbTurinDroneRes` — the thirty-one
/// bytes at `1088:3650` that `DoTurinDroneAiTurn` hands `IroEnsureAi`
/// (`1088:36a4`). Each is a field in its top three bits and a level in its
/// low five: Propulsion 2, Construction 4, Biotechnology 4, Energy 4,
/// Weapons 5, Propulsion 6, Construction 6, Weapons 8, Energy 6,
/// Electronics 6, Propulsion 9, Biotechnology 7, Construction 8,
/// Electronics 8, Biotechnology 5 (already passed by then — the table's
/// own quirk), Construction 9, Energy 7, Electronics 10, Weapons 10,
/// Propulsion 12, Construction 11, Energy 10, Weapons 12, Electronics 13,
/// Propulsion 16, Weapons 14, Construction 15, Electronics 14,
/// Biotechnology 10, Weapons 16, Energy 14.
pub const RESEARCH_PLAN: &[u8] = &[
    0x42, 0x64, 0xa4, 0x04, 0x25, 0x46, 0x66, 0x28, 0x06, 0x86, 0x49, 0xa7, 0x68, 0x88, 0xa5, 0x69,
    0x07, 0x8a, 0x2a, 0x4c, 0x6b, 0x0a, 0x2c, 0x8d, 0x50, 0x2e, 0x6f, 0x8e, 0xaa, 0x30, 0x0e,
];
/// The share of resources the personality puts into research: the `0xf`
/// pushed for `IroEnsureAi`'s `pct` at `1088:36a4`.
pub const RESEARCH_PCT: u8 = 15;
/// What `IroEnsureAi` returns once every level of the plan is reached
/// (`1090:425a`).
pub const PLAN_DONE: usize = 0x39e;

/// What the turn did, for a test to look at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Where the research plan stands: the index of the entry being worked
    /// toward, or [`PLAN_DONE`].
    pub research: usize,
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
    /// Defences queued by `FQueueAiDefenses`, as `(planet, count)`.
    pub defended: Vec<(i16, i32)>,
    /// Mass drivers aimed by `FAIFling`, as `(planet, target)`.
    pub flung: Vec<(i16, i16)>,
    /// Queues unblocked by `AddMinesToBlockedQueues`, as `(planet, mines)`
    /// — `0` mines is auto alchemy put in front instead.
    pub unblocked: Vec<(i16, i32)>,
    /// Fleets split by `SplitOutShdefs`, as `(fleet, new fleet)`.
    pub split: Vec<(u16, u16)>,
    /// Fleets whose stale orders the first pass cut, by fleet id.
    pub cleaned: Vec<u16>,
    /// Fleets sent to drop their colonists on somebody's planet and head
    /// for the nearest starbase, as `(fleet, planet)`.
    pub dropping: Vec<(u16, i16)>,
}

/// Run the personality's turn for `player`, as the host does before the
/// year is generated. The orders it gives are written straight into the
/// state, which is what replaying the AI's log would do.
pub fn turn(state: &mut GameState, player: usize, rng: &mut Rng) -> Report {
    turn_as(
        state,
        player,
        rng,
        &crate::ai::personality::Profile::of(AiPersonality::TurinDrone),
    )
}

/// `DoMaidAiTurn` (`1098:0000`), which is the shape every personality's
/// turn shares with nothing of its own between: `IroEnsureAi` with no plan,
/// `HandleBasicAiTasks` and `FillProductionQueue`. The Maid designs no
/// ships and sends no fleets; its planets build mines and factories, and
/// whatever `HandleBasicAiTasks` does for anyone — scanners, defences,
/// mineral packets, mines in front of blocked queues.
pub fn basic_turn(
    state: &mut GameState,
    player: usize,
    rng: &mut Rng,
    profile: &crate::ai::personality::Profile,
) -> Report {
    let mut report = Report::default();
    let Some(me) = i16::try_from(player).ok() else {
        return report;
    };
    if state.players.get(player).is_none_or(|p| p.dead) {
        return report;
    }
    let seen = crate::visibility::view(state, player).planets;
    state.players[player].explored.extend(seen);
    report.research = ensure_research(
        state,
        player,
        profile.plan,
        profile.research_pct(state.turn),
    );
    basic_tasks(
        state,
        player,
        me,
        &[],
        profile.personality,
        rng,
        &mut report,
    );
    fill_production_queues(state, player, me, profile.personality, rng, &mut report);
    report
}

/// The TurinDrone's turn run for a personality — its own, or one whose
/// middle is not yet transcribed and borrows this one under its own
/// research plan and share (`crate::ai::personality::Shape::StandIn`).
pub fn turn_as(
    state: &mut GameState,
    player: usize,
    rng: &mut Rng,
    profile: &crate::ai::personality::Profile,
) -> Report {
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

    // `IroEnsureAi(vrgbTurinDroneRes, 31, &ishdefSBLatest, 15)`: the field
    // under study, from the personality's plan.
    report.research = ensure_research(
        state,
        player,
        profile.plan,
        profile.research_pct(state.turn),
    );
    // `ValidateStarbaseHistory`, which `IroEnsureAi` runs for the
    // personality: the planets the haulers work from.
    validate_starbase_history(state, player, me);
    let history_count =
        i32::try_from(state.players[player].starbase_history.len()).unwrap_or(i32::MAX);

    // `MergeAllShdefs`, four times: the armada classes together (slots 4
    // to 7 and 13 to 15), the mine layers, the destroyers and the miners —
    // each fleet of ours at a place joining the first of its kind there.
    for mask in [0xe0f0u16, 0x1000, 0x0c00, 0x000c] {
        merge_all(state, me, mask, &mut report);
    }

    // `EnsureTurinDroneShdefs`: the designs the personality wants in its
    // slots, made when the tech allows.
    ensure_designs(state, player, rng, &mut report);

    let mut marks = marks(state, player, me, &explored);
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
    let mut old = [false; 16];
    let freighters = check_status(state, player, me, 6, 7, recycle, &mut old);
    let cruisers = check_status(state, player, me, 8, 9, recycle, &mut old);
    let bombers = check_status(state, player, me, 13, 14, recycle, &mut old);
    let battleships = check_status(state, player, me, 4, 5, recycle, &mut old);
    let _mine_layers = check_status(state, player, me, 12, 12, recycle, &mut old);
    let fifteens = check_status(state, player, me, 15, 15, recycle, &mut old);
    let _miners = if state.players[player].research.levels[3] < 7 {
        Status::default()
    } else {
        check_status(state, player, me, 2, 3, recycle, &mut old)
    };
    let destroyers = check_status(state, player, me, 10, 11, recycle, &mut old);
    // `SplitOutShdefs` from turn 61: the old designs into fleets of their
    // own.
    if state.turn > 60 {
        split_out_designs(state, player, me, &old, &mut report);
    }

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
        // A cruiser: Propulsion past 4 (`rgTech + 2`), and fewer than the
        // larger of a tenth of the planets owned and twice the starbase
        // history's entries, or under ten sevenths of that with one roll
        // in four.
        let want_cruisers = (owned / 10).max(history_count * 2);
        if levels[2] > 4 {
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
    // The first walk over the fleets: stale orders cut, colonists dropped
    // where they would be wanted, and the miners' planets claimed.
    let mut valued = valued_planets(state, player, me);
    first_pass(state, player, me, &mut valued, &mut worth, &mut report);
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
                // `vlpbAiPlanet[+15] = 4`: claimed, for the colony ships
                // after this one.
                if let Some(mark) = usize::try_from(target).ok().and_then(|i| marks.get_mut(i)) {
                    *mark = Mark::Claimed;
                }
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
            // Its home: the history entry it is listed under, else the
            // first own planet with a starbase — and with none of those
            // the routine leaves the rest of the fleets alone.
            let Some(first_starbase) = state
                .planets
                .iter()
                .find(|p| p.owner == Some(me) && p.starbase)
                .map(|p| p.id)
            else {
                break;
            };
            let home = state.players[player]
                .starbase_history
                .iter()
                .find(|e| e.fleets.contains(&fleet_id))
                .map_or(first_starbase, |e| e.planet);
            if let Some(target) =
                target_freighter(state, player, me, index, &worth, home, &valued, rng)
            {
                report.hauling.push((fleet_id, target));
            }
            continue;
        }
        // The scouts and the destroyers (slots 0, 10 and 11) scout. A
        // fleet of battleships (4, 5) or Rogues (15) with no bombers gets
        // nothing here — the routine's own ladder of `rgcsh` tests ends
        // at the mine layer — and waits to be merged with bombers
        // (`MergeAllShdefs(0xe0f0)`); `IdTargetArmada` is the Robotoid's.
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
    basic_tasks(
        state,
        player,
        me,
        &worth,
        profile.personality,
        rng,
        &mut report,
    );
    fill_production_queues(state, player, me, profile.personality, rng, &mut report);

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
        // `PctPlanetOptValue`: the planet as terraforming could leave it.
        let reach =
            crate::terraform::optimal_env(planet, race, state.players[player].research.levels);
        let value = pct_planet_opt_value(planet, race, reach);
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

/// `IroEnsureAi` (`1090:425a`): the research settings for the year. The
/// share goes to `pct`; a player at level 24 or more in every field stops
/// researching instead. The field is the one of the first entry of `plan`
/// whose level is not yet reached, and when that level is only one away
/// the *next* field is the following entry's, so the change is queued
/// rather than lost when the level comes. Past the end of the plan the
/// player studies whichever field is lowest (the first, on a tie), and the
/// routine answers [`PLAN_DONE`].
///
/// The starbase-design upkeep the routine also does (`EnsureAiStarbase-
/// Designs`, `IshdefAiSBLatest`, `ValidateStarbaseHistory`) is not here.
fn ensure_research(state: &mut GameState, player: usize, plan: &[u8], pct: u8) -> usize {
    use crate::research::NextField;

    let Some(p) = state.players.get_mut(player) else {
        return PLAN_DONE;
    };
    p.research_pct = pct;
    if p.research.levels.iter().all(|&l| l > 23) {
        p.research_pct = 0;
    }
    let decode = |entry: u8| -> (usize, u8) { (usize::from(entry >> 5).min(5), entry & 0x1f) };
    for (i, &entry) in plan.iter().enumerate() {
        let (field, level) = decode(entry);
        let have = p.research.levels[field];
        if have < level {
            p.research.current_field = field;
            if i + 1 < plan.len() && have + 1 == level {
                p.research.next_field = NextField::Field(decode(plan[i + 1]).0);
            }
            return i;
        }
    }
    let lowest = (0..6).min_by_key(|&f| p.research.levels[f]).unwrap_or(0);
    p.research.current_field = lowest;
    PLAN_DONE
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
/// The tech thresholds are the routine's own comparisons, `rgTech[f] > n`
/// — the bytes at `rgplr + 0x1b` to `0x1f`, `rgTech` starting at `0x1a`,
/// so fields 1 to 5: Weapons, Propulsion, Construction, Electronics,
/// Biotechnology. Energy is never asked about.
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
            needs: above(2, 4) && above(3, 7),
            retire_first: false,
            when_none_exist: false,
        },
        Want {
            slot: 9,
            hull: 13,
            fittings: vec![fitting::GALLEON],
            needs: above(2, 6) && above(3, 10),
            retire_first: false,
            when_none_exist: false,
        },
        Want {
            slot: 10,
            hull: 6,
            fittings: vec![fitting::DESTROYER],
            needs: above(1, 4) && above(4, 4) && above(3, 3) && above(2, 4),
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
            needs: above(1, 7) && above(4, 6) && above(3, 5),
            retire_first: false,
            when_none_exist: false,
        },
        Want {
            slot: 14,
            hull: 18,
            fittings: vec![fitting::STEALTH_BOMBER],
            needs: above(1, 10) && above(4, 11) && above(3, 14) && above(2, 8),
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
            needs: above(1, 4) && above(4, 5) && above(3, 12) && above(2, 6),
            retire_first: false,
            when_none_exist: false,
        },
        Want {
            slot: 15,
            hull: 12,
            fittings: vec![fitting::ROGUE],
            needs: above(1, 4) && above(4, 5) && above(3, 12) && above(2, 6),
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
/// years is retired if no ship of it exists, and marked in `old` for
/// [`split_out_designs`] otherwise.
#[allow(clippy::too_many_arguments)]
fn check_status(
    state: &mut GameState,
    player: usize,
    me: i16,
    from: u8,
    to: u8,
    recycle: i16,
    old: &mut [bool; 16],
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
        // otherwise marked for `SplitOutShdefs`.
        if state.turn - designed > recycle {
            if existing == 0 {
                state.designs[player][usize::from(slot)].obsolete = true;
            } else if let Some(mark) = old.get_mut(usize::from(slot)) {
                *mark = true;
            }
        }
    }
    status.count = i32::try_from(total.min(32_000)).unwrap_or(32_000);
    status
}

/// The first walk over every fleet of ours (`1088:4932`–`1088:4a70` and
/// the labels `LBlowAwayOrders` and `LCheckForColDrop`). The attack-fleet
/// chain it builds (`FIsTurinDroneAiAttack`) and the `det` bit 15 it
/// clears are not kept — the fleets are scanned where they are wanted.
/// For a fleet with no miners aboard, or whose last order has no task:
///
/// * a **colony ship** (slot 1, no freighters): its destination — the
///   planet it orbits with no orders, else its next waypoint's planet —
///   is looked at when somebody else's planet with a positive opt value
///   (`vlpbAiPlanet[+3]`) as below; a destination taken by another player
///   otherwise has the orders blown away;
/// * a **freighter** (slots 8, 9): with no destination, a next waypoint
///   on a bare point has the orders blown away; a destination that is
///   gone or another player's has them blown away too — unless it is a
///   valued planet of theirs, the fleet carries colonists, we are not
///   Alternate Reality and the planet has no starbase, in which case the
///   colonists are **dropped** there: at the planet, the current waypoint
///   gets a Transport task unloading all colonists (`0x1101`, the item
///   word `0x2000`), and the fleet then heads for the nearest starbase
///   (`FMoveToNearestStarbase`, at `0x1140`); away from it, the drop
///   order the routine writes into the next waypoint is overwritten by
///   that same move, so the fleet only heads for the starbase.
///
/// Blowing the orders away (`LBlowAwayOrders`) cuts them to the current
/// waypoint and clears its task (`ClearAiCurrentTask`).
///
/// A fleet **with miners** and a task on its last order: in deep space
/// with a next waypoint, that waypoint's task becomes Remote Mining and
/// its planet is claimed (`vlpbAiPlanet[+1] |= 0x80`); at a planet that
/// is unowned the planet is claimed, and at one that is owned the orders
/// are blown away.
fn first_pass(
    state: &mut GameState,
    player: usize,
    me: i16,
    valued: &mut Valued,
    worth: &mut [u8],
    report: &mut Report,
) {
    use crate::race::Prt;
    use stars_formats::{task, ItemAction, TransportTask, XferAction};

    let we_are_ar = state.players[player].race.prt() == Some(Prt::Ar);

    for index in 0..state.fleets.len() {
        let fleet = state.fleets[index].clone();
        if fleet.owner != me || fleet.is_empty() {
            continue;
        }
        let carries = |slot: u8| fleet.stacks.iter().any(|s| s.design == slot && s.count > 0);
        let has_orders = fleet.waypoints.len() > 1;
        let no_miners = !carries(2) && !carries(3);
        let last_task_none = fleet.waypoints.last().is_none_or(|w| w.task == task::NONE);
        let blow_away = |state: &mut GameState, report: &mut Report| {
            let fleet = &mut state.fleets[index];
            fleet.waypoints.truncate(1);
            if let Some(first) = fleet.waypoints.first_mut() {
                first.task = task::NONE;
            }
            fleet.warp = None;
            report.cleaned.push(fleet.id);
        };
        let planet_of = |id: i16| state.planets.iter().find(|p| p.id == id).cloned();

        if no_miners || last_task_none {
            let dest: Option<i16> = if !has_orders {
                fleet.orbiting.and_then(|p| i16::try_from(p).ok())
            } else if fleet.waypoints[1].target_class == grobj::PLANET {
                fleet.waypoints[1]
                    .target
                    .and_then(|p| i16::try_from(p).ok())
            } else {
                None
            };
            let freighter = carries(8) || carries(9);
            if !freighter {
                if !carries(1) {
                    continue;
                }
                let Some(dest) = dest else {
                    continue;
                };
                if !valued.planets.contains(&dest) {
                    if planet_of(dest).is_some_and(|p| p.owner.is_some_and(|o| o != me)) {
                        blow_away(state, report);
                    }
                    continue;
                }
            }
            // `LCheckForColDrop`.
            let Some(dest) = dest else {
                if has_orders && fleet.waypoints[1].target_class == grobj::POSITION {
                    blow_away(state, report);
                }
                continue;
            };
            let planet = planet_of(dest);
            let gone_or_theirs = planet
                .as_ref()
                .is_none_or(|p| p.owner.is_some_and(|o| o != me));
            if !gone_or_theirs {
                continue;
            }
            let drop = valued.planets.contains(&dest)
                && fleet.cargo.colonists > 0
                && !we_are_ar
                && planet.as_ref().is_some_and(|p| !p.starbase);
            if !drop {
                blow_away(state, report);
                continue;
            }
            let at_planet = !has_orders
                && fleet.waypoints.first().is_some_and(|w| {
                    w.target_class == grobj::PLANET && w.target == u16::try_from(dest).ok()
                });
            if at_planet {
                let mut items = [ItemAction {
                    quantity: 0,
                    action: XferAction::None,
                }; 5];
                items[3] = ItemAction {
                    quantity: 0,
                    action: XferAction::UnloadAll,
                };
                let first = &mut state.fleets[index].waypoints[0];
                first.task = task::TRANSPORT;
                first.transport = Some(TransportTask { items });
                first.task_data = Vec::new();
            }
            // `vlpbAiPlanet[+3] |= 0x80`: claimed, so no hauler goes for it
            // this turn.
            valued.claimed.insert(dest);
            report.dropping.push((fleet.id, dest));
            move_to_nearest_starbase(state, me, index, false);
            continue;
        }

        // Miners with a task on their last order.
        let claimed: Option<i16> = match fleet.orbiting {
            None => {
                if !has_orders {
                    continue;
                }
                state.fleets[index].waypoints[1].task = task::REMOTE_MINING;
                fleet.waypoints[1]
                    .target
                    .and_then(|p| i16::try_from(p).ok())
            }
            Some(here) => {
                let here = i16::try_from(here).ok();
                if here.and_then(planet_of).is_some_and(|p| p.owner.is_some()) {
                    blow_away(state, report);
                    continue;
                }
                here
            }
        };
        if let Some(w) = claimed
            .and_then(|id| usize::try_from(id).ok())
            .and_then(|i| worth.get_mut(i))
        {
            *w |= 0x80;
        }
    }
}

/// `vlpbAiPlanet[+3]`: the other players' planets with a positive opt
/// value (`PctPlanetOptValue`) for us, and which of them a fleet has
/// claimed this turn (the byte's `0x80`).
#[derive(Debug, Default)]
struct Valued {
    /// The planets, by id.
    planets: BTreeSet<i16>,
    /// Those a colonist drop is already bound for.
    claimed: BTreeSet<i16>,
}

/// The planet pass's `vlpbAiPlanet[+3]` marks.
fn valued_planets(state: &GameState, player: usize, me: i16) -> Valued {
    let race = &state.players[player].race;
    let levels = state.players[player].research.levels;
    Valued {
        planets: state
            .planets
            .iter()
            .filter(|p| p.owner.is_some_and(|o| o != me))
            .filter(|p| {
                let reach = crate::terraform::optimal_env(p, race, levels);
                pct_planet_opt_value(p, race, reach) > 0
            })
            .map(|p| p.id)
            .collect(),
        claimed: BTreeSet::new(),
    }
}

/// `ValidateStarbaseHistory` (`1090:4cf0`), which `IroEnsureAi` runs for
/// every personality but the Cybertron and the Macinti, from turn 20: the
/// **starbase history** in `vlpbAiData` — up to 64 entries of a planet and
/// the haulers (at most eight) assigned to it.
///
/// 1. Entries whose planet is no longer ours are dropped, and a count out
///    of range reset.
/// 2. Every own planet with a starbase not yet listed is added.
/// 3. Every own planet without one that has 8,000 people or more, mines
///    and factories both past nineteen, and minerals worth 7,000 kT —
///    each surface stock plus the square of its concentration over four
///    — is added too, unless (the Robotoid only) it lies within fifty
///    light years of a listed planet.
/// 4. Every own transport (`FIsAiTransport`: a hull from the Small
///    Freighter to the Super Freighter, or the Privateer, Rogue or
///    Galleon) listed nowhere is assigned to the nearest listed planet
///    with room.
/// 5. An entry with fewer than four haulers takes the last hauler of the
///    first entry with at least two more than it.
///
/// The original keeps the table in the player's history file; here it
/// lives with the [`crate::Player`] for the game in hand.
fn validate_starbase_history(state: &mut GameState, player: usize, me: i16) {
    use crate::ai::StarbaseHistoryEntry;

    let personality = match state.players[player].control {
        crate::ai::Control::Computer { personality, .. } => personality,
        crate::ai::Control::Human => None,
    };
    if matches!(
        personality,
        Some(AiPersonality::Cyber) | Some(AiPersonality::Macinti)
    ) {
        return;
    }
    if state.turn < 20 {
        return;
    }
    let mut history = std::mem::take(&mut state.players[player].starbase_history);
    history.truncate(64);

    // 1. Only planets still ours.
    history.retain(|e| {
        state
            .planets
            .iter()
            .any(|p| p.id == e.planet && p.owner == Some(me))
    });
    for entry in &mut history {
        entry.fleets.truncate(8);
    }

    // 2. Every starbase of ours.
    for planet in state
        .planets
        .iter()
        .filter(|p| p.owner == Some(me) && p.starbase)
    {
        if history.len() >= 64 {
            break;
        }
        if !history.iter().any(|e| e.planet == planet.id) {
            history.push(StarbaseHistoryEntry {
                planet: planet.id,
                fleets: Vec::new(),
            });
        }
    }

    // 3. Grown planets without one.
    for planet in state
        .planets
        .iter()
        .filter(|p| p.owner == Some(me) && !p.starbase)
    {
        if history.len() >= 64 {
            break;
        }
        if planet.pop < 80 || planet.mines <= 19 || planet.factories <= 19 {
            continue;
        }
        let minerals: i64 = (0..3)
            .map(|k| {
                let conc = i64::from(planet.min_conc[k]);
                i64::from(planet.surface_min[k]) + conc * conc / 4
            })
            .sum();
        if minerals < 7000 {
            continue;
        }
        if history.iter().any(|e| e.planet == planet.id) {
            continue;
        }
        if personality == Some(AiPersonality::Robotoid) {
            let near = history.iter().any(|e| {
                let (Some(a), Some(b)) = (
                    planet.position,
                    state
                        .planets
                        .iter()
                        .find(|p| p.id == e.planet)
                        .and_then(|p| p.position),
                ) else {
                    return false;
                };
                let dx = i64::from(a.x) - i64::from(b.x);
                let dy = i64::from(a.y) - i64::from(b.y);
                dx * dx + dy * dy < 2500
            });
            if near {
                continue;
            }
        }
        history.push(StarbaseHistoryEntry {
            planet: planet.id,
            fleets: Vec::new(),
        });
    }

    // 4. Every transport of ours listed nowhere goes to the nearest entry
    //    with room.
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let is_transport = |fleet: &crate::fleet::Fleet| {
        fleet.stacks.iter().any(|s| {
            s.count > 0
                && designs
                    .get(usize::from(s.design))
                    .is_some_and(|d| matches!(d.hull_id, 0..=3 | 11..=13))
        })
    };
    for fleet in state
        .fleets
        .iter()
        .filter(|f| f.owner == me && !f.is_empty())
    {
        if !is_transport(fleet) || history.iter().any(|e| e.fleets.contains(&fleet.id)) {
            continue;
        }
        let mut best: Option<(i64, usize)> = None;
        for (i, entry) in history.iter().enumerate() {
            if entry.fleets.len() >= 8 {
                continue;
            }
            let Some(at) = state
                .planets
                .iter()
                .find(|p| p.id == entry.planet)
                .and_then(|p| p.position)
            else {
                continue;
            };
            let dx = i64::from(at.x) - i64::from(fleet.position.x);
            let dy = i64::from(at.y) - i64::from(fleet.position.y);
            let d2 = dx * dx + dy * dy;
            if d2 < 10_000_000 && best.is_none_or(|(b, _)| d2 < b) {
                best = Some((d2, i));
            }
        }
        if let Some((_, i)) = best {
            history[i].fleets.push(fleet.id);
        }
    }

    // 5. Evening out.
    for i in 0..history.len() {
        if history[i].fleets.len() >= 4 {
            continue;
        }
        let want = history[i].fleets.len() + 2;
        if let Some(j) = history.iter().position(|e| e.fleets.len() >= want) {
            if let Some(fleet) = history[j].fleets.pop() {
                history[i].fleets.push(fleet);
            }
        }
    }
    state.players[player].starbase_history = history;
}

/// `FMoveToNearestStarbase` (`1090:6f7e`): a leg at `0x1140` — warp 4, no
/// task — to the nearest own planet with a starbase
/// (`IdplFindClosestStarbase`, `1090:6e04`; with `big_ones`, one of more
/// than 25,000 people), measured from the fleet's current waypoint. The
/// leg replaces whatever orders followed (`FMoveAiFleet` with `fAppend`
/// 0). Answers whether there was one.
fn move_to_nearest_starbase(state: &mut GameState, me: i16, index: usize, big_ones: bool) -> bool {
    let from = state.fleets[index]
        .waypoints
        .first()
        .map_or(state.fleets[index].position, |w| w.position);
    let mut best: Option<(i64, i16, Point)> = None;
    for planet in &state.planets {
        if planet.owner != Some(me) || !planet.starbase {
            continue;
        }
        if big_ones && planet.pop <= 250 {
            continue;
        }
        let Some(at) = planet.position else {
            continue;
        };
        let dx = i64::from(at.x) - i64::from(from.x);
        let dy = i64::from(at.y) - i64::from(from.y);
        let d2 = dx * dx + dy * dy;
        if d2 < 10_000_000 && best.is_none_or(|(b, _, _)| d2 < b) {
            best = Some((d2, planet.id, at));
        }
    }
    let Some((_, target, at)) = best else {
        return false;
    };
    let fleet = &mut state.fleets[index];
    if fleet.waypoints.first().is_some_and(|w| w.position == at) {
        // Already there: the orders shrink to this one, and the new order
        // — which has no task — is written over it.
        fleet.waypoints.truncate(1);
        fleet.waypoints[0].task = stars_formats::task::NONE;
        fleet.waypoints[0].transport = None;
        fleet.warp = None;
        return true;
    }
    lay_leg(fleet, at, target, stars_formats::task::NONE, 4);
    true
}

/// `SplitOutShdefs` (`1090:98d8`): while the player has fewer than 501
/// fleets, the first live fleet of theirs carrying both an old design
/// (marked by `CheckAiShdefStatus`) and a current one is split, the old
/// designs' ships going to a new fleet (`LpflNewSplit`, which copies the
/// orders) and the cargo following them (`FleetTransferCargoBalance`);
/// then the search starts over, until no such fleet is left.
fn split_out_designs(
    state: &mut GameState,
    player: usize,
    me: i16,
    old: &[bool; 16],
    report: &mut Report,
) {
    if !old.iter().any(|&o| o) {
        return;
    }
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    loop {
        if state.fleets.iter().filter(|f| f.owner == me).count() >= 501 {
            return;
        }
        let Some(index) = state.fleets.iter().position(|f| {
            f.owner == me
                && !f.is_empty()
                && f.stacks
                    .iter()
                    .any(|s| s.count > 0 && old.get(usize::from(s.design)) == Some(&true))
                && f.stacks
                    .iter()
                    .any(|s| s.count > 0 && old.get(usize::from(s.design)) == Some(&false))
        }) else {
            return;
        };
        let source = &state.fleets[index];
        let before = source.stacks.clone();
        let new_id = crate::turn::next_fleet_id(state, me);
        let mut split = crate::fleet::Fleet {
            id: new_id,
            owner: source.owner,
            position: source.position,
            orbiting: source.orbiting,
            stacks: Vec::new(),
            cargo: crate::fleet::Cargo::default(),
            battle_plan: source.battle_plan,
            warp: source.warp,
            waypoints: source.waypoints.clone(),
            name: None,
            repeat_orders: source.repeat_orders,
            direction: None,
        };
        let source = &mut state.fleets[index];
        let (moved, kept): (Vec<_>, Vec<_>) = source
            .stacks
            .drain(..)
            .partition(|s| old.get(usize::from(s.design)) == Some(&true));
        source.stacks = kept;
        split.stacks = moved;
        let source_id = source.id;
        crate::fleet::balance_cargo([source, &mut split], [&before, &[]], &designs);
        state.fleets.push(split);
        report.split.push((source_id, new_id));
    }
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

/// `IdTargetFreighter` (`1090:286c`): where a hauler goes next, and what
/// it moves when it gets there. `home` is the planet the hauler works
/// from — its starbase-history entry's, else the first with a starbase.
///
/// Home's **scarcity** is read first: its least-stocked mineral is the
/// *scarce* one, and the scarcity is 2 when that stock is under a quarter
/// of the next-least, 1 under a half, else 0. What a planet "has" of use
/// is then its stock of the scarce mineral alone at scarcity 2, or the
/// sum of the three with the other two halved at scarcity 1.
///
/// Every planet but the one we are at and those another hauler of the
/// same design is bound for is scored, the best score winning; distance
/// enters as `(d + 24) / 25`, at least 1:
///
/// * an unowned planet a miner of ours has claimed: its mineral worth
///   times 500 over the distance;
/// * home, when the hold is more than a third full: 25,000 when full,
///   else the fill times twenty over the distance;
/// * an own planet without a starbase and without a starbase at the head
///   of its queue: 25,000 when it is hostile to us and we are at home;
///   else what it has, over nine, as a share of the hold capped at the
///   room left, times a hundred over the distance;
/// * another player's planet, for the TurinDrone: only one worth
///   settling (`vlpbAiPlanet[+3]`) that no colonist drop has claimed this
///   turn (the byte's `0x80`), while we are at home, scored like an own
///   planet — the colonists aboard are dropped on it;
/// * salvage (`FSalvageTargetFreighter2`, `1090:395a`): a stationary
///   packet within 200 light years, scored like a planet on what it has;
///   one at our own position is emptied into the hold on the spot, and a
///   hold that is then full sends us home.
///
/// The orders (`0x1041`, Transport at warp 4, re-speeded afterwards): to
/// home, unload all three minerals; anywhere else, load all three — or,
/// when home is short, only the scarce one, unless the planet is owned
/// and holds less of it than the hold has room for, when all three are
/// loaded to 66 % (the scarce) and 33 % — and no task at all to salvage.
/// At home with 12,000 people or more, a thousand kT of colonists come
/// aboard for an owned planet with fewer people than home; and to an
/// owned planet other than home the colonists are unloaded.
#[allow(clippy::too_many_arguments)]
fn target_freighter(
    state: &mut GameState,
    player: usize,
    me: i16,
    index: usize,
    worth: &[u8],
    home: i16,
    valued: &Valued,
    rng: &mut Rng,
) -> Option<i16> {
    use stars_formats::{ItemAction, TransportTask, XferAction};

    let _ = rng;
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let home = state.planets.iter().find(|p| p.id == home)?.clone();
    let fleet = state.fleets[index].clone();
    let here = fleet.orbiting.and_then(|p| i16::try_from(p).ok());
    let at_home = here == Some(home.id);
    let capacity = fleet.cargo_capacity(&designs);
    if capacity <= 0 {
        return None;
    }
    let held: i32 = fleet.cargo.minerals.iter().sum::<i32>() + fleet.cargo.colonists;
    let free = (capacity - held).max(0);
    let fill = 100 - free * 100 / capacity;

    // Home's scarcity.
    let mut order: Vec<usize> = (0..3).collect();
    order.sort_by_key(|k| home.surface_min[*k]);
    let scarce = order[0];
    let least = i64::from(home.surface_min[order[0]]);
    let next = i64::from(home.surface_min[order[1]]);
    let scarcity = i32::from(least < next >> 1) + i32::from(least < next >> 2);
    let useful = |stock: [i64; 3]| -> i64 {
        if scarcity == 2 {
            stock[scarce]
        } else {
            (0..3)
                .map(|k| {
                    if scarcity == 0 || k == scarce {
                        stock[k]
                    } else {
                        stock[k] >> 1
                    }
                })
                .sum()
        }
    };

    // Planets another hauler of the same design is already bound for.
    let design = fleet.stacks.iter().find(|s| s.count > 0).map(|s| s.design);
    let taken: BTreeSet<u16> = state
        .fleets
        .iter()
        .filter(|f| f.owner == me && f.id != fleet.id && f.waypoints.len() > 1)
        .filter(|f| {
            f.stacks
                .iter()
                .any(|s| Some(s.design) == design && s.count > 0)
        })
        .filter(|f| f.waypoints[1].target_class == grobj::PLANET)
        .filter_map(|f| f.waypoints[1].target)
        .filter(|t| Some(*t) != u16::try_from(home.id).ok())
        .collect();

    let from = fleet.position;
    let distance = |at: Point| -> i64 {
        let dx = i64::from(at.x) - i64::from(from.x);
        let dy = i64::from(at.y) - i64::from(from.y);
        #[allow(clippy::cast_possible_truncation)]
        let d = ((dx * dx + dy * dy) as f64).sqrt() as i64;
        ((d + 24) / 25).max(1)
    };
    let race = state.players[player].race.clone();
    let mut best: Option<(i64, i16, Point)> = None;
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
        let score = if planet.owner.is_none() && worth_here & 0x80 != 0 {
            i64::from(worth_here & 0x7f) * 500 / distance(at)
        } else if planet.owner.is_none()
            || (planet.owner != Some(me)
                && !(valued.planets.contains(&planet.id)
                    && !valued.claimed.contains(&planet.id)
                    && at_home))
        {
            // Unowned and unclaimed, or somebody else's that is not worth
            // settling from home.
            continue;
        } else if planet.id == home.id {
            if fill <= 34 {
                continue;
            }
            if fill == 100 {
                25_000
            } else {
                i64::from(fill) * 20 / distance(at)
            }
        } else if planet.starbase
            || planet
                .queue
                .first()
                .is_some_and(|q| q.ship && q.item >= u16::from(crate::startup::FIRST_STARBASE_SLOT))
        {
            continue;
        } else if planet.owner == Some(me)
            && at_home
            && crate::hab::pct_planet_desirability(planet, &race) < 0
        {
            25_000
        } else {
            let have = useful([
                i64::from(planet.surface_min[0]),
                i64::from(planet.surface_min[1]),
                i64::from(planet.surface_min[2]),
            ]);
            if have <= 9 {
                continue;
            }
            let share = (have * 100 / i64::from(capacity)).min(i64::from(100 - fill));
            share * 100 / distance(at)
        };
        if score <= 0 {
            continue;
        }
        if best.is_none_or(|(b, _, _)| score > b) {
            best = Some((score, planet.id, at));
        }
    }

    // Salvage: a stationary packet.
    let mut salvage: Option<(u16, Point)> = None;
    let mut took = false;
    for p in 0..state.packets.len() {
        if state.packets[p].warp != 0 {
            continue;
        }
        let at = state.packets[p].position;
        if at == from {
            let take = |k: usize, state: &mut GameState| {
                let room = (capacity - state.fleets[index].cargo.mass()).max(0);
                let amount = i32::from(state.packets[p].minerals[k]).min(room);
                if amount > 0 {
                    state.packets[p].minerals[k] -= i16::try_from(amount).unwrap_or(0);
                    state.fleets[index].cargo.minerals[k] += amount;
                }
            };
            if scarcity != 0 {
                take(scarce, state);
            }
            if scarcity != 2 {
                for k in 0..3 {
                    take(k, state);
                }
            }
            took = true;
            continue;
        }
        let dx = (i64::from(at.x) - i64::from(from.x)).abs();
        let dy = (i64::from(at.y) - i64::from(from.y)).abs();
        if dx.max(dy) > 200 {
            continue;
        }
        let stock = state.packets[p].minerals.map(i64::from);
        let have = useful(stock);
        if have <= 9 {
            continue;
        }
        let share = (have * 100 / i64::from(capacity)).min(i64::from(100 - fill));
        let score = share * 100 / distance(at);
        if best.is_none_or(|(b, _, _)| score > b) {
            best = Some((score, -1, at));
            salvage = Some((state.packets[p].id, at));
        }
    }
    let held: i32 = state.fleets[index].cargo.mass();
    if took && capacity - held <= 0 {
        best = Some((25_000, home.id, home.position?));
        salvage = None;
    }
    let (_, target, at) = best?;

    let stacks: Vec<(&crate::design::ShipDesign, i32)> = fleet
        .stacks
        .iter()
        .filter_map(|s| designs.get(usize::from(s.design)).map(|d| (d, s.count)))
        .collect();
    let warp = ideal_warp(&stacks, false);
    if let Some((id, at)) = salvage {
        let fleet = &mut state.fleets[index];
        lay_leg(fleet, at, 0, stars_formats::task::NONE, warp);
        if let Some(leg) = fleet.waypoints.get_mut(1) {
            leg.target = Some(id);
            leg.target_class = grobj::THING;
        }
        return Some(-1);
    }

    let to_home = target == home.id;
    let target_planet = state.planets.iter().find(|p| p.id == target).cloned();
    let target_owned = target_planet.as_ref().is_some_and(|p| p.owner.is_some());
    let mineral = if to_home {
        XferAction::UnloadAll
    } else {
        XferAction::LoadAll
    };
    let mut items = [ItemAction {
        quantity: 0,
        action: XferAction::None,
    }; 5];
    for item in items.iter_mut().take(3) {
        item.action = mineral;
    }
    // The TurinDrone's colonists.
    if at_home && home.pop > 1200 && target_owned {
        let target_pop = target_planet.as_ref().map_or(0, |p| p.pop);
        if target_pop < home.pop {
            let room = (capacity - held).max(0);
            let take = 1000.min(room).min(home.pop);
            if let Some(h) = state.planets.iter_mut().find(|p| p.id == home.id) {
                h.pop -= take;
            }
            state.fleets[index].cargo.colonists += take;
        }
    }
    if target_owned && !to_home {
        items[3].action = XferAction::UnloadAll;
    }
    // Home's scarcity shapes the loading.
    if !to_home && scarcity != 0 {
        let their_scarce = target_planet.as_ref().map_or(0, |p| p.surface_min[scarce]);
        if scarcity != 2 && their_scarce < free && target_owned {
            for (k, item) in items.iter_mut().take(3).enumerate() {
                item.action = XferAction::FillPercent;
                item.quantity = if k == scarce { 66 } else { 33 };
            }
        } else {
            for (k, item) in items.iter_mut().take(3).enumerate() {
                if k != scarce {
                    item.action = XferAction::None;
                }
            }
        }
    }
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
///   upgrade by [`crate::ai::ships::upgrade_ai_starbase`], or a mass driver
///   aimed and packets queued by [`ai_fling`], or a scanner by
///   `FQueueAiScanner` — which never queues one, see [`queue_ai_scanner`] —
///   or defences by [`queue_ai_defenses`]; and when none of those wrote,
///   terraforming by [`crate::ai::production::queue_ai_terraforming`];
/// * `FixPlanetsUnderAttack`, which never runs in a tutorial game (flag
///   bit 3) and is not written;
/// * [`add_mines_to_blocked_queues`].
fn basic_tasks(
    state: &mut GameState,
    player: usize,
    me: i16,
    worth: &[u8],
    personality: AiPersonality,
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
        personality: Some(personality),
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
                // `FUpgradeAiStarbase`.
                let starbase_design = |index: u8| {
                    designs
                        .get(usize::from(crate::startup::FIRST_STARBASE_SLOT) + usize::from(index))
                        .filter(|d| d.hull().is_some() && !d.obsolete)
                };
                let inputs = crate::ai::ships::UpgradeInputs {
                    latest,
                    latest_orbital_fort: latest,
                    design_turn: i32::from(starbase_design(current).map_or(0, |d| d.designed)),
                    // The sideways move wants a live design two above.
                    sideways_design_free: starbase_design(current + 2).is_some(),
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
            if !written {
                written = ai_fling(state, player, me, index, &available, rng, report);
            }
            if !written {
                written = queue_ai_scanner(&state.planets[index]);
            }
            if !written {
                if let Some(count) = queue_ai_defenses(&state.planets[index], &race) {
                    state.planets[index].queue.push(QueueItem {
                        count,
                        item: crate::production::item::DEFENSE,
                        ship: false,
                        completion: 0,
                    });
                    report.defended.push((id, count));
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

    add_mines_to_blocked_queues(state, player, me, report);
}

/// `FQueueAiScanner` (`1090:90d6`): a planetary scanner for a planet without
/// one — which **never** queues anything. The routine looks through the
/// queue and then the production inventory for a planetary item numbered
/// 18 to 26 (`0x11 < iItem < 0x1b` at `1090:912c`–`1090:9199`), the ids
/// of the individual scanners, but `InitProduction` (`10d0:015e`) offers a
/// scanner only as the generic item 27, so the search always comes up
/// empty and the routine answers 0. Kept as the no-op it is, so the order
/// of the housekeeping reads as the original's.
fn queue_ai_scanner(planet: &crate::planet::Planet) -> bool {
    let _ = planet;
    false
}

/// `FQueueAiDefenses` (`1090:939a`): a planet of 160,000 people or more
/// (`rgwtMin[3] > 0x63f`) wants one defence per 8,000 (`pop / 80`); with
/// fewer than that, and no defences already in the queue, it queues as
/// many as the inventory offers (`CMaxDefenses` less those built), at most
/// four, at the back. The resources are not looked at.
fn queue_ai_defenses(planet: &crate::planet::Planet, race: &crate::race::Race) -> Option<i32> {
    use crate::production::item;
    if planet.pop < 1600 {
        return None;
    }
    let wanted = planet.pop / 80;
    if wanted <= i32::from(planet.defenses) {
        return None;
    }
    if planet
        .queue
        .iter()
        .any(|e| !e.ship && e.item == item::DEFENSE)
    {
        return None;
    }
    let room = i32::from(crate::resources::max_defenses(planet, race)) - i32::from(planet.defenses);
    if room <= 0 {
        return None;
    }
    Some(room.min(4))
}

/// `FAIFling` (`1090:7dd6`): a mass driver aimed at a neighbour, and packets
/// queued to throw at them. Only for a player of skill 2 or more (bits 10
/// to 12 of the player's `det`), never for the Cybertron, and not while a
/// packet (items 14 to 17) is already queued. The planet must have a
/// starbase whose driver flings at warp 10 or better (`IWarpMAFromLppl`),
/// more than 3,000 kT of minerals available between the three, and win a
/// one-in-four roll.
///
/// The target is drawn, one-in-*n* reservoir fashion, from the other
/// players' planets seen within the last two years within 84 light years
/// (a pair of drivers, 225; the squares at `1090:7dce`, tripled when a
/// surface mineral tops 12,500 kT) whose defences guess is under 14 or
/// whose population guess is under 750 — `uPopGuess` is a quarter of the
/// population, so under 300,000 people; the guess nibble is not kept here
/// — and whose owner is not Alternate Reality, nor Packet Physics if the
/// planet has a starbase. The driver is set to warp 13 (stored as 9) and
/// the packets follow: eighty germanium ones when 649 resources are to
/// hand and a two-in-three roll comes up; then thirty mixed at 3,001 /
/// 4,001 / 3,001 kT of ironium / boranium / germanium, fifteen at 1,501 /
/// 2,251 / 1,501, or otherwise, per mineral over 1,250 kT (boranium
/// 2,500), one packet of it per 200 kT over, one to twenty-five.
#[allow(clippy::too_many_arguments)]
fn ai_fling(
    state: &mut GameState,
    player: usize,
    me: i16,
    index: usize,
    available: &[i32; 4],
    rng: &mut Rng,
    report: &mut Report,
) -> bool {
    use crate::production::item;
    use crate::race::Prt;

    let planet = state.planets[index].clone();
    let control = state.players[player].control;
    let (personality, skill) = match control {
        crate::ai::Control::Computer {
            personality,
            skill_bits,
        } => (personality, skill_bits),
        crate::ai::Control::Human => (None, 0),
    };
    if personality == Some(AiPersonality::Cyber) {
        return false;
    }
    if planet
        .queue
        .iter()
        .any(|e| !e.ship && (item::PACKET_IRONIUM..=item::PACKET_MIXED).contains(&e.item))
    {
        return false;
    }
    if skill < 2 {
        return false;
    }
    if i64::from(available[0]) + i64::from(available[1]) + i64::from(available[2]) <= 3000 {
        return false;
    }
    if !planet.starbase {
        return false;
    }
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let driver = crate::production::mass_driver(&planet, &designs);
    if driver.warp < 10 || rng.random(4) != 0 {
        return false;
    }
    let Some(from) = planet.position else {
        return false;
    };
    let mut range2: i64 = if driver.paired { 50_625 } else { 7_056 };
    if planet.surface_min.iter().any(|&m| m > 12_500) {
        range2 *= 3;
    }
    let mut seen = 0i16;
    let mut target: Option<i16> = None;
    for other in &state.planets {
        let (Some(owner), Some(at)) = (other.owner, other.position) else {
            continue;
        };
        if owner == me {
            continue;
        }
        let Some(their_race) = usize::try_from(owner)
            .ok()
            .and_then(|o| state.players.get(o))
            .map(|p| &p.race)
        else {
            continue;
        };
        if other.pop / 4 >= 750 {
            continue;
        }
        if their_race.prt() == Some(Prt::Ar) {
            continue;
        }
        if other.starbase && their_race.prt() == Some(Prt::Pp) {
            continue;
        }
        let dx = i64::from(from.x) - i64::from(at.x);
        let dy = i64::from(from.y) - i64::from(at.y);
        if dx * dx + dy * dy > range2 {
            continue;
        }
        seen = seen.saturating_add(1);
        if rng.random(seen) == 0 {
            target = Some(other.id);
        }
    }
    let Some(target) = target else {
        return false;
    };
    let planet = &mut state.planets[index];
    planet.fling_dest = Some(target);
    planet.fling_warp = 13 - 4;
    let mut queue = |item: u16, count: i32| {
        planet.queue.push(QueueItem {
            count,
            item,
            ship: false,
            completion: 0,
        });
    };
    if available[3] >= 649 && rng.random(3) != 0 {
        queue(item::PACKET_GERMANIUM, 80);
    }
    let [ir, bo, ge, _] = *available;
    if ir >= 3001 && bo >= 4001 && ge >= 3001 {
        queue(item::PACKET_MIXED, 30);
    } else if ir >= 1501 && bo >= 2251 && ge >= 1501 {
        queue(item::PACKET_MIXED, 15);
    } else {
        for (i, &amount) in available.iter().take(3).enumerate() {
            let over = if i == 1 { 2500 } else { 1250 };
            if amount > over {
                let count = ((amount - over) / 200).clamp(1, 25);
                queue(item::PACKET_IRONIUM + u16::try_from(i).unwrap_or(0), count);
            }
        }
    }
    report.flung.push((planet.id, target));
    true
}

/// `AddMinesToBlockedQueues` (`1090:1792`): at every own planet whose
/// queue's first item is neither a mine, alchemy (auto or plain) nor
/// terraforming, and is not due next year (`PszProductionETA`; "as needed"
/// counts as 600 years), the planet's resources — less the research share
/// — over the years until it is due are compared with the item's resource
/// cost: when they would cover it, the item is waiting on minerals, and
/// mines go in front of it — as many as the resources buy at the race's
/// mine cost, capped at what the planet could operate over what it has —
/// or, when that is none, one auto alchemy.
///
/// The original then re-estimates (`1090:1b7c`–`1090:1c41`) and takes the
/// mines out again when the item's date has not come forward and the
/// mines themselves take longer than the item did, or trims their count
/// otherwise. Here the mines stay only when the item's date comes
/// forward; the trim is not written.
fn add_mines_to_blocked_queues(state: &mut GameState, player: usize, me: i16, report: &mut Report) {
    use crate::production::{eta, item, planetary_item_cost, QueueItem};
    use crate::race::RaceStat;

    let race = state.players[player].race.clone();
    let research_pct = state.players[player].research_pct;
    let designs = state.designs.get(player).cloned().unwrap_or_default();
    let energy = i16::from(state.players[player].research.levels[0]);
    let who = Builder::player(&state.players[player]);
    for index in 0..state.planets.len() {
        let planet = state.planets[index].clone();
        if planet.owner != Some(me) {
            continue;
        }
        let Some(head) = planet.queue.first().copied() else {
            continue;
        };
        if !head.ship
            && matches!(
                head.item,
                item::MINE | item::AUTO_ALCHEMY | item::ALCHEMY | item::TERRAFORM
            )
        {
            continue;
        }
        let due = eta(&planet, &who, research_pct, &designs, 0).first;
        if due == 1 {
            continue;
        }
        let due = i64::from(if due == -1 { 600 } else { due });
        let cost = if head.ship {
            designs
                .get(usize::from(head.item))
                .and_then(|d| d.true_cost(&who))
                .map(|c| c.resources)
        } else {
            planetary_item_cost(head.item, &race, state.tutorial).map(|c| c.resources)
        };
        let Some(cost) = cost else {
            continue;
        };
        let mut resources =
            i64::from(crate::resources::resources_at_planet(&planet, &race, energy).unwrap_or(0));
        if !planet.no_research {
            resources -= resources * i64::from(research_pct) / 100;
        }
        if i64::from(cost) > resources * (due - 1) {
            continue;
        }
        let room = (i64::from(crate::resources::max_operable_mines(&planet, &race, false))
            - i64::from(planet.mines))
        .max(0);
        let each = i64::from(race.stat(RaceStat::MineBuild)).max(1);
        let mines = room.min(resources / each);
        if mines < 1 {
            state.planets[index].queue.insert(
                0,
                QueueItem {
                    count: 1,
                    item: item::AUTO_ALCHEMY,
                    ship: false,
                    completion: 0,
                },
            );
            report.unblocked.push((planet.id, 0));
            continue;
        }
        let mines = i32::try_from(mines).unwrap_or(i32::MAX);
        state.planets[index].queue.insert(
            0,
            QueueItem {
                count: mines,
                item: item::MINE,
                ship: false,
                completion: 0,
            },
        );
        let after = eta(&state.planets[index], &who, research_pct, &designs, 1).first;
        let after = i64::from(if after == -1 { 600 } else { after });
        if due <= after {
            state.planets[index].queue.remove(0);
            continue;
        }
        report.unblocked.push((planet.id, mines));
    }
}

/// `FillProductionQueue` (`1090:9c1c`): `FFillProdMinesAndFactories` at
/// every own planet, mines at the front of the queue and factories at
/// the back.
fn fill_production_queues(
    state: &mut GameState,
    player: usize,
    me: i16,
    personality: AiPersonality,
    rng: &mut Rng,
    report: &mut Report,
) {
    let race = state.players[player].race.clone();
    let ctx = crate::ai::production::Context {
        personality: Some(personality),
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
