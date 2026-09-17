//! Turn generation: the order in which a year happens.
//!
//! Source: `FGenerateTurn` (`10b0:0000`) and `Produce` (`10b8:0000`) in
//! `stars.2.7j.exe`. Full derivation in `docs/formulas/turn-order.md`.
//!
//! The original's pipeline is:
//!
//! ```text
//! DoOrders(0) → UnmarkMineFields → MoveThings(0) → MoveFleets →
//! ThingDecay → BreedColonistsInTransit → Produce → MoveThings(1) →
//! FuelFleets → DoOrders(1) → SweepForMines → HealShips →
//! AutoTerraform → RemoteTerraforming → SpankTheCheaters →
//! ValidateWaypoints → UpdateGuesses → turn++ → UpdatePlayerScores
//! ```
//!
//! and `Produce` itself is:
//!
//! ```text
//! MineMinerals → (per planet: resources → research skim → build queue)
//!              → UpdatePopulations → UpdateResearchStatus → RandomEvents
//! ```
//!
//! [`generate_turn`] implements the steps whose formulas are recovered. The
//! steps that need fleets, orders, ship designs or the components table are
//! listed in [`TurnReport::skipped`] rather than silently omitted, so a caller
//! can never mistake a partial turn for a complete one. `RandomEvents`
//! (see [`crate::events`]) is skipped only when the game was set up
//! without random events.

use crate::fleet::Fleet;
use crate::mining::mine_minerals;
use crate::movement::{advance, distance, travel_this_year};
use crate::planet::Planet;
use crate::population::update_population;
use crate::production::{auto_build_cap, build_item, item, planetary_item_cost, COST_PARTS};
use crate::research::{add_research, Breakthrough};
use crate::rng::Rng;
use crate::{GameState, Player};

/// A step of the original turn pipeline that this crate does not yet
/// perform. Most of the list is history: movement, the things in space,
/// the build queue, combat and the scores are all in the turn now, and
/// only the two at the end are still reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkippedStep {
    /// Player orders: cargo transfers, waypoint changes, production edits —
    /// reported when no order log was replayed into the year.
    Orders,
    /// Fleet movement, fuel and mine-field traversal.
    FleetMovement,
    /// Mineral packets, wormholes and Mystery Trader movement and decay.
    Things,
    /// The production build queue (needs the components table).
    BuildQueue,
    /// Battle resolution.
    Combat,
    /// Random events (meteor strikes and the like) — skipped only when the
    /// game was set up without them.
    RandomEvents,
    /// Score calculation.
    Scores,
}

use crate::fleet::grobj::{POSITION as GROBJ_POSITION, THING as GROBJ_THING};

/// Whether a waypoint aimed at a `THING` names this particular object.
///
/// A thing's id in a waypoint is its **full** id: the nine-bit id, the owning
/// player, and in the top three bits the `ith` that says what kind of thing it
/// is. The models here keep the nine-bit id alone, so the kind has to be
/// checked against those top bits — without it, a fleet bound for wormhole 1
/// would be caught by anything else that happened to be object 1. Every one of
/// the 3,686 thing waypoints in the fixtures carries its kind: 3,346 name a
/// mineral packet and 340 a wormhole.
fn names_thing(target: Option<u16>, ith: u16, id: u16) -> bool {
    target.is_some_and(|t| t >> 13 == ith && t & 0x01FF == id)
}

/// `ith` for a minefield.
const ITH_MINEFIELD: u16 = 0;
/// `ith` for a wormhole.
const ITH_WORMHOLE: u16 = 2;
/// `ith` for the Mystery Trader.
const ITH_TRADER: u16 = 3;

/// What one generated turn did.
#[derive(Debug, Clone, Default)]
pub struct TurnReport {
    /// The year the game advanced to.
    pub year: i32,
    /// Minerals mined, per planet id, in kT.
    pub mined: Vec<(i16, [i32; 3])>,
    /// What each planet completed, as `(planet id, [(item id, count)])`.
    pub built: Vec<(i16, Vec<(u16, i32)>)>,
    /// Ships finished this year, as `(planet id, design slot, count)`. They are
    /// added to a fleet in orbit over the planet that built them.
    pub ships_built: Vec<(i16, u8, i32)>,
    /// Starbases finished this year, as `(planet id, design slot)` — the
    /// slot counted from 16, as the queue names it.
    pub starbases_built: Vec<(i16, u8)>,
    /// Fleets that moved, as `(fleet id, light years travelled)`.
    pub moved: Vec<(u16, i32)>,
    /// Population change per planet id, in units of 100 colonists.
    pub population: Vec<(i16, i32)>,
    /// Resources each player put into research.
    pub research_spending: Vec<i32>,
    /// Technology levels gained, per player.
    pub breakthroughs: Vec<Vec<Breakthrough>>,
    /// What each computer player's turn did, by player index.
    pub ai: Vec<(usize, crate::ai::turindrone::Report)>,
    /// The year's battles — see [`crate::combat::do_battles`].
    pub battles: Vec<crate::combat::Outcome>,
    /// The year's bombings — see [`crate::bombing::do_bombing`].
    pub bombings: Vec<crate::bombing::Bombing>,
    /// Planets `AutoTerraform` moved this year — Claim Adjusters only.
    pub terraformed: Vec<i16>,
    /// Planets remote terraforming moved, as `(planet id, clicks applied)`.
    pub remote_terraformed: Vec<(i16, i32)>,
    /// Recorded cargo transfers that moved something in this state.
    pub transfers: usize,
    /// Planets a colonist landing settled, invaded or defended.
    pub colonised: Vec<i16>,
    /// Waypoint tasks executed on arrival, as `(fleet id, task)`.
    pub tasks_done: Vec<(u16, u8)>,
    /// Planets mined from orbit, as `(planet id, minerals added)`.
    pub remote_mined: Vec<(i16, [i32; 3])>,
    /// Mines laid this year, as `(fleet id, kind, mines)`.
    pub mines_laid: Vec<(u16, u8, i32)>,
    /// Fleets that ran into a minefield, and what it cost them.
    pub mine_hits: Vec<(u16, crate::minefield::MineHit)>,
    /// Mines lost to decay, as `(field id, owner, mines)`.
    pub mines_decayed: Vec<(u16, i16, i32)>,
    /// Mines swept, as `(field id, owner, mines)`.
    pub mines_swept: Vec<(u16, i16, i32)>,
    /// Packets that landed, and what each did — see [`crate::packet::land`].
    pub packets_landed: Vec<crate::packet::Landing>,
    /// Packets thrown this year — see [`crate::packet::fling`].
    pub packets_flung: Vec<crate::packet::Flung>,
    /// Wormholes that jumped this year, by id.
    pub wormholes_moved: Vec<u16>,
    /// What became of each Mystery Trader, as `(trader id, event)`.
    pub trader_events: Vec<(u16, crate::wormhole::Event)>,
    /// Fleets that went through a wormhole, as `(fleet id, entered, left)`.
    pub wormhole_trips: Vec<(u16, u16, u16)>,
    /// Fleets that reached the Mystery Trader, and what came of it.
    pub trades: Vec<(u16, crate::wormhole::Gift)>,
    /// Trades a computer player made from a planet rather than a fleet, as
    /// `(planet id, gift)`. See [`crate::wormhole`].
    pub ai_trades: Vec<(i16, crate::wormhole::Gift)>,
    /// Interceptions a patrol ordered, as `(patrolling fleet, target fleet)`.
    pub patrols: Vec<(u16, u16)>,
    /// The scoreboard, one entry per player, after the year's events.
    pub scores: Vec<crate::score::PlayerScore>,
    /// Which victory conditions each player meets.
    pub victory: Vec<crate::victory::Met>,
    /// Players who have won, if the game is old enough for anyone to.
    pub winners: Vec<usize>,
    /// The year's random events — see [`crate::events`].
    pub events: crate::events::EventReport,
    /// Pipeline steps not performed, and therefore not reflected above.
    pub skipped: Vec<SkippedStep>,
}

/// Advance the game by one year.
///
/// The steps performed, in the original's order, are: mining, the per-planet
/// resource and research split, population update, the research advance, the
/// Claim Adjuster's free terraforming, and remote terraforming from orbit.
/// Everything else is reported in [`TurnReport::skipped`].
///
/// `rng` supplies the mining rounding draws; pass a generator seeded from the
/// game to reproduce a specific turn.
pub fn generate_turn(state: &mut GameState, rng: &mut Rng) -> TurnReport {
    generate_turn_with_orders(state, &TurnOrders::default(), rng)
}

/// The recorded orders a turn is generated from.
///
/// A `.x` file is a replay of what a player's client already did, so these are
/// applied verbatim rather than validated. Only the parts that move simulation
/// state are carried; settings changes belong to the file layer.
#[derive(Debug, Clone, Default)]
pub struct TurnOrders {
    /// Cargo transfers, in the order the file records them.
    pub cargo: Vec<stars_formats::CargoTransferRecord>,
    /// Messages the players wrote to one another, which the host delivers
    /// with the year's news.
    pub messages: Vec<stars_formats::PlayerMessage>,
}

/// Advance the game by one year, applying a set of recorded orders first.
///
/// `DoOrders(0)` runs before movement and production, which is why a transfer
/// can feed the same year's growth — see `docs/formulas/turn-order.md`.
pub fn generate_turn_with_orders(
    state: &mut GameState,
    orders: &TurnOrders,
    rng: &mut Rng,
) -> TurnReport {
    let mut report = TurnReport {
        skipped: vec![SkippedStep::Orders, SkippedStep::RandomEvents],
        research_spending: vec![0; state.players.len()],
        breakthroughs: vec![Vec::new(); state.players.len()],
        ..TurnReport::default()
    };

    // Last year's news is last year's. The original reloads the host file at
    // the head of `FGenerateTurn`, so nothing a previous year said survives
    // into this one; here it has to be said explicitly, and it has to be said
    // *first* — the Mystery Trader's news is the earliest thing a year sends.
    state.messages.clear();
    state.battles.clear();
    state.revealed_designs.clear();
    state.no_auto_track.clear();
    // The players' letters to one another go out with this year's news
    // (`FLoadLogFile` gathers them into `vlpmsgplrOut`, and
    // `WritePlayerMessages` puts each into the files of those it is for).
    state.player_messages = orders.messages.clone();
    for player in &mut state.players {
        player.learned_tech_this_year = false;
    }

    // --- DoAiTurn (`1088:0000`), for each computer player: the host plays
    // their turn before the year runs, as if they had submitted orders.
    // Each personality researches by its own plan and share, and every
    // personality's turn is transcribed (`ai::personality`).
    for player in 0..state.players.len() {
        let personality = match state.players[player].control {
            crate::ai::Control::Computer {
                personality: Some(personality),
                ..
            } => personality,
            _ => continue,
        };
        let profile = crate::ai::personality::Profile::of(personality);
        let did = match profile.shape {
            crate::ai::personality::Shape::Basic => {
                crate::ai::turindrone::basic_turn(state, player, rng, &profile)
            }
            crate::ai::personality::Shape::TurinDrone => {
                crate::ai::turindrone::turn_as(state, player, rng, &profile)
            }
            crate::ai::personality::Shape::Robotoid => {
                crate::ai::robotoid::turn(state, player, rng, &profile)
            }
            crate::ai::personality::Shape::Automitron => {
                crate::ai::automitron::turn(state, player, rng, &profile)
            }
            crate::ai::personality::Shape::Cyber => {
                crate::ai::cyber::turn(state, player, rng, &profile)
            }
            crate::ai::personality::Shape::Rototill => {
                crate::ai::rototill::turn(state, player, rng, &profile)
            }
            crate::ai::personality::Shape::Macinti => {
                crate::ai::macinti::turn(state, player, rng, &profile)
            }
        };
        report.ai.push((player, did));
    }

    // --- DoOrders(0): the recorded cargo transfers, before anything moves or
    // produces. A transfer applied here feeds this year's growth.
    if !orders.cargo.is_empty() {
        report.skipped.retain(|s| *s != SkippedStep::Orders);
        let (applied, drops) = crate::orders::apply_cargo_transfers(state, &orders.cargo);
        report.transfers = applied;
        // DropColonists: settle every landing together, so rival claims on one
        // planet are weighed against each other rather than one at a time.
        report.colonised = crate::orders::resolve_colonist_drops_with(state, &drops, Some(rng));
    }

    // --- DoOrders(0) -> SatisfyOrders(1), DropColonists, SatisfyOrders(2):
    // the tasks on the waypoint a fleet already stands at, before it
    // moves — a Transport set at the current planet unloads and then
    // loads, a Colonize colonises, a Scrap scraps — each pass's landings
    // settled before the next.
    for pass in [1u8, 2] {
        let (done, drops) = crate::orders::execute_tasks_pass(state, pass, &Default::default());
        report.tasks_done.extend(done);
        let settled = crate::orders::resolve_colonist_drops_with(state, &drops, Some(rng));
        report.colonised.extend(settled);
    }

    // --- UnmarkMineFields (`10b8:7638`): who can see each field *now*
    // starts the year empty; the scanners fill it in again at the end.
    for field in &mut state.minefields {
        field.visible_to = 0;
    }

    // --- MoveThings(0): the Mystery Trader crosses a year, and the packets
    // already in flight do too, before anything else happens.
    report.trader_events = move_traders(state, rng);
    report.packets_landed = move_packets(state, false, rng);

    // --- MoveFleets, which happens before Produce. Which fleets moved is
    // remembered for the tasks that want a fleet to have been **here all
    // turn** (`fHereAllTurn`): laying mines and remote mining.
    // Fleets that travelled this year, by `(owner, id)` — a fleet can be
    // pruned before this is read, so an index would not do.
    let mut moved_this_turn: std::collections::BTreeSet<(i16, u16)> =
        std::collections::BTreeSet::new();
    // A fleet whose next waypoint is another **fleet** chases it
    // (`10b0:426f`): it does not move in the first pass, and in the passes
    // after — up to ten — it flies toward wherever its quarry now is, all
    // of what it has left once the quarry has finished moving, a fifth of
    // it (`(left + used + 4) / 5`) while the quarry is still on the move,
    // so that two fleets chasing each other close on one another in steps.
    let mut chase: Vec<(usize, usize, i32, i32)> = Vec::new();
    let mut deferred: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    // Fleets that went through a stargate this year, by `(owner, id)`,
    // with where they left from: a fleet chasing one is left pointed at
    // that spot (`NoAutoTrackFleet`, `1080:…`), and none of them heals
    // this year (`fNoHeal`).
    let mut jumped_from: std::collections::BTreeMap<(i16, u16), crate::movement::Point> =
        std::collections::BTreeMap::new();
    let mut no_heal: std::collections::BTreeSet<(i16, u16)> = std::collections::BTreeSet::new();
    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        let Some(leg) = fleet.waypoints.get(1) else {
            continue;
        };
        if leg.target_class != crate::fleet::grobj::FLEET || leg.warp == 0 {
            continue;
        }
        let Some(word) = leg.target else {
            continue;
        };
        let (owner, id) = crate::orders::split_fleet_id(word);
        if let Some(quarry) = state
            .fleets
            .iter()
            .position(|f| f.owner == owner && f.id == id && !f.is_empty())
        {
            let allowance = crate::movement::travel_per_year(i16::from(leg.warp));
            chase.push((index, quarry, allowance, 0));
            deferred.insert(index);
        }
    }
    // The mishaps of the first pass: engines that will not start, colonists
    // who cannot take the acceleration, engines that cannot take warp 10.
    let (grounded, blown_up) = first_pass_mishaps(state, rng);
    let mut in_chase: Vec<usize> = (0..state.fleets.len())
        .filter(|i| !deferred.contains(i) && !grounded.contains(i))
        .collect();
    let mut passes = 0;
    while !in_chase.is_empty() && passes <= 10 {
        let this_pass = std::mem::take(&mut in_chase);
        for index in this_pass {
            let cap =
                chase
                    .iter()
                    .find(|(i, _, _, _)| *i == index)
                    .map(|&(_, quarry, left, used)| {
                        // Where the quarry now stands is where the leg now points
                        // (`10b0:3d21`) — unless it went through a gate or a
                        // wormhole, when the leg was fixed where it left from.
                        let q = &state.fleets[quarry];
                        let at = q.position;
                        let chaser = &state.fleets[index];
                        let fixed = state.no_auto_track.contains(&(chaser.owner, chaser.id, 1));
                        if let Some(leg) = state.fleets[index].waypoints.get_mut(1) {
                            if !fixed {
                                leg.position = at;
                            }
                        }
                        let quarry_done = !deferred.contains(&quarry);
                        if quarry_done {
                            left
                        } else {
                            left.min((left + used + 4) / 5)
                        }
                    });
            let owner = usize::try_from(state.fleets[index].owner).unwrap_or(usize::MAX);
            let designs = state.designs.get(owner).cloned().unwrap_or_default();
            let ife = state
                .players
                .get(owner)
                .is_some_and(|p| p.race.has_lrt(crate::race::lrt::IFE));
            if grounded.contains(&index) {
                continue;
            }
            // A fleet still transporting at its waypoint, or laying mines
            // there, stays (`MoveFleets`, `10b0:33c5`: `grTask` 1 or 6 on
            // the current order).
            if state.fleets[index].waypoints.first().is_some_and(|w| {
                w.task == stars_formats::task::TRANSPORT || w.task == stars_formats::task::LAY_MINES
            }) {
                continue;
            }
            let from = state.fleets[index].position;
            let leg_length = state.fleets[index]
                .waypoints
                .get(1)
                .map_or(0.0, |w| distance(from, w.position));
            let leg_warp = state.fleets[index].waypoints.get(1).map_or(0, |w| w.warp);
            // A waypoint is consumed when the fleet reaches it, so the list
            // getting shorter is how this pass knows the fleet arrived.
            let waypoints = state.fleets[index].waypoints.len();
            // A leg at "Use Stargate" is not flown but jumped.
            let jump = state.fleets[index]
                .waypoints
                .get(1)
                .is_some_and(|w| w.warp >= crate::stargate::WARP);
            let outcome = if jump {
                stargate_leg(state, index, rng, &mut jumped_from)
            } else {
                move_fleet(&mut state.fleets[index], &designs, ife, cap)
            };
            settle_where_it_stands(state, index);
            if let Some(entry) = chase.iter_mut().find(|(i, _, _, _)| *i == index) {
                let travelled = outcome.map_or(0, |(t, _)| t);
                entry.2 -= travelled;
                entry.3 += travelled;
                let arrived = state.fleets[index].waypoints.len() < waypoints;
                if entry.2 > 0 && !arrived && travelled > 0 {
                    // Still chasing: another pass.
                    in_chase.push(index);
                } else {
                    deferred.remove(&index);
                }
            }
            if let Some((travelled, dry)) = outcome {
                report.moved.push((state.fleets[index].id, travelled));
                if travelled > 0 {
                    let f = &state.fleets[index];
                    moved_this_turn.insert((f.owner, f.id));
                }
                let fleet_id = state.fleets[index].id;
                match dry {
                    RanDry::No => {}
                    RanDry::Stuck => state.messages.push(crate::message::Message {
                        player: owner,
                        id: crate::message::id::OUT_OF_FUEL,
                        object: crate::message::fleet_object(fleet_id),
                        params: vec![fleet_id as i16, 0],
                    }),
                    RanDry::SlowedTo(warp) => state.messages.push(crate::message::Message {
                        player: owner,
                        id: crate::message::id::OUT_OF_FUEL_SLOWED,
                        object: crate::message::fleet_object(fleet_id),
                        params: vec![fleet_id as i16, i16::from(warp)],
                    }),
                }
                // FTravelThroughMineFields: the leg is flown, and somewhere along
                // it the fleet may find somebody else's mines. A jump crosses
                // nothing.
                let mut mined = false;
                if !jump {
                    if let Some(hit) = cross_minefields(state, index, from, travelled, rng) {
                        report.mine_hits.push((state.fleets[index].id, hit));
                        // A fleet the mines caught does not heal this year
                        // (`FTravelThroughMineFields`, `10b0:…`, `fNoHeal`).
                        let f = &state.fleets[index];
                        no_heal.insert((f.owner, f.id));
                        mined = true;
                    }
                }
                // The ramscoops gather on the way (`10b0:484b`), unless a mine
                // cut the leg short or the tank ran dry.
                if !jump && !mined && dry == RanDry::No && travelled > 0 {
                    #[allow(clippy::cast_possible_truncation)]
                    let short = (leg_length - 0.99999) as i32;
                    ramscoop_gain(state, index, &designs, leg_warp, travelled.min(short));
                }
                // A Radiating Hydro-Ram Scoop's radiation, on the colonists
                // aboard a fleet that moved (`10b0:4b2d`), in the first two
                // passes only.
                if !jump && travelled > 0 && passes <= 1 {
                    radiation_deaths(state, index, &designs);
                }
                // And a fleet that has arrived may have arrived at a wormhole, in
                // which case it is not where it thinks it is.
                if waypoints > state.fleets[index].waypoints.len() {
                    if let Some((entered, left)) = traverse_wormhole(state, index) {
                        report
                            .wormhole_trips
                            .push((state.fleets[index].id, entered, left));
                    }
                    // `KillUsedWaypoints` (`1080:189a`): a fleet that has
                    // reached the last of its orders says so — unless the
                    // waypoint carries a task that will report for itself when
                    // it runs. Merge and Transfer do not, and neither does a
                    // Route at anything but a planet of yours with a route set,
                    // which this engine does not model.
                    let fleet = &state.fleets[index];
                    let task = fleet.waypoints.first().map_or(0, |w| w.task);
                    let reports_itself = matches!(
                        task,
                        stars_formats::task::TRANSPORT
                            | stars_formats::task::COLONIZE
                            | stars_formats::task::REMOTE_MINING
                            | stars_formats::task::SCRAP
                            | stars_formats::task::LAY_MINES
                            | stars_formats::task::PATROL
                    );
                    if fleet.waypoints.len() == 1 && !reports_itself {
                        let id = fleet.id;
                        state.messages.push(crate::message::Message {
                            player: owner,
                            id: crate::message::id::ORDERS_COMPLETE,
                            object: crate::message::fleet_object(id),
                            params: vec![id as i16],
                        });
                    }
                }
            }
        }
        if passes == 0 {
            // Everyone else has moved: the chasers' turn, in as many passes
            // as it takes.
            in_chase.extend(chase.iter().map(|c| c.0));
        }
        passes += 1;
    }

    // A fleet a minefield emptied is gone, and so is one whose engines
    // blew up at warp 10.
    prune_destroyed(state, &report.mine_hits);
    state
        .fleets
        .retain(|f| !blown_up.contains(&(f.owner, f.id)));

    // --- ThingDecay: an armed field goes off under everyone inside it, and
    // then every field loses a slice of itself. A field that runs out is gone.
    report.mine_hits.extend(detonate_minefields(state));
    report.mines_decayed = decay_minefields(state);

    // --- Produce: mine first, so this year's minerals are on the surface
    // before anything can spend them.
    for index in 0..state.planets.len() {
        // A planet the file only scanned has no population or installations to
        // simulate; it is present so the player's own view of the galaxy is
        // complete, not so it can be run.
        if !state.planets[index].detail.is_full() {
            continue;
        }
        let Some(race) = owner_race(state, index) else {
            continue;
        };
        let mined = mine_minerals(&mut state.planets[index], &race, None, rng);
        let id = state.planets[index].id;
        report.mined.push((id, mined));
    }

    // --- Produce: per-planet resource accounting. With no build queue
    // modelled, every resource a planet makes ends up in research, which is
    // what the original does for a planet whose queue is empty.
    for index in 0..state.planets.len() {
        if !state.planets[index].detail.is_full() {
            continue;
        }
        let Some(owner) = state.planets[index].owner else {
            continue;
        };
        let Ok(owner_index) = usize::try_from(owner) else {
            continue;
        };
        let Some(player) = state.players.get(owner_index) else {
            continue;
        };
        let race = player.race.clone();
        let research_pct = player.research_pct;
        let energy_tech = i16::from(player.research.levels[0]);

        let no_research = state.planets[index].no_research;
        let hull = starbase_hull_of(state, index);
        let Some(budget) = crate::production::planet_budget_with_starbase(
            &state.planets[index],
            &race,
            research_pct,
            0,
            no_research,
            energy_tech,
            hull,
        ) else {
            continue;
        };

        // Run the build queue against the planet's minerals and its share of
        // the resources.
        let mut available = [
            state.planets[index].surface_min[0],
            state.planets[index].surface_min[1],
            state.planets[index].surface_min[2],
            budget.production,
        ];
        let designs = state.designs.get(owner_index).cloned().unwrap_or_default();
        let mut ships_built: Vec<(u8, i32)> = Vec::new();
        let tech = state
            .players
            .get(owner_index)
            .map_or([0u8; 6], |p| p.research.levels);
        let trader_parts = state.players.get(owner_index).map_or(0, |p| p.trader_parts);
        let built = run_queue(
            &mut state.planets[index],
            &race,
            &designs,
            tech,
            trader_parts,
            state.tutorial,
            &mut available,
            &mut ships_built,
        );
        for (i, slot) in state.planets[index].surface_min.iter_mut().enumerate() {
            *slot = available[i];
        }
        let id = state.planets[index].id;
        // What the planet has to say about it: one message per kind of
        // installation finished, singular or plural, and one when the queue
        // has been worked through. The turn-3 tutorial file shows the
        // shape — `54` with `[10, planet]` and `62` with `[planet]`.
        for (item, count) in &built {
            let item = item::auto_builds(*item).unwrap_or(*item);
            let singular = match item {
                item::FACTORY => crate::message::id::BUILT_FACTORY,
                item::MINE => crate::message::id::BUILT_MINE,
                _ => continue,
            };
            // `FRemovePlayerMessage`: a message of the same kind about the
            // same planet already sent this year — a part-built factory
            // finished ahead of the auto-build order that started it — is
            // taken back and its count folded in, so the planet says it
            // once.
            let mut total = *count;
            if let Some(at) = state.messages.iter().position(|m| {
                m.player == owner_index
                    && m.object == id
                    && (m.id == singular || m.id == singular + 1)
            }) {
                let earlier = state.messages.remove(at);
                total += if earlier.id == singular {
                    1
                } else {
                    i32::from(earlier.params.first().copied().unwrap_or(1))
                };
            }
            let (message, params) = if total < 2 {
                (singular, vec![id])
            } else {
                (singular + 1, vec![n_i16(total), id])
            };
            state.messages.push(crate::message::Message {
                player: owner_index,
                id: message,
                object: id,
                params,
            });
        }
        // `Produce` (`10b8:0371`): a planet with resources whose queue is
        // empty — empty to begin with, or worked through — says so, every
        // year. It is the message the tutorial's second year opens on, and
        // the one its page 13 teaches you to switch off.
        if state.planets[index].queue.is_empty() && budget.total > 0 {
            state.messages.push(crate::message::Message {
                player: owner_index,
                id: crate::message::id::QUEUE_EMPTY,
                object: id,
                params: vec![id],
            });
        }
        // Packets go up as they are finished (`FBuildObject`'s packet arm);
        // a planetary scanner is installed and a Genesis Device goes off.
        for (item, count) in &built {
            if (item::PACKET_IRONIUM..=item::PACKET_MIXED).contains(item)
                || *item == item::AUTO_PACKET
            {
                if let Some(flung) = crate::packet::fling(state, index, *item, *count) {
                    report.packets_flung.push(flung);
                }
            } else if *item == item::PLANETARY_SCANNER
                || (item::PLANETARY_SCANNER_FIRST..=item::PLANETARY_SCANNER_LAST).contains(item)
            {
                install_planetary_scanner(state, index, *item);
            } else if *item == item::GENESIS {
                for _ in 0..*count {
                    genesis_device(state, index, rng);
                }
            }
        }
        if !built.is_empty() {
            report.built.push((id, built));
        }
        if !ships_built.is_empty() {
            let id = state.planets[index].id;
            for (slot, count) in ships_built {
                // A starbase is not a ship: it goes up over the planet.
                if slot >= crate::startup::FIRST_STARBASE_SLOT {
                    if install_starbase(state, index, slot - crate::startup::FIRST_STARBASE_SLOT) {
                        report.starbases_built.push((id, slot));
                    }
                    continue;
                }
                report.ships_built.push((id, slot, count));
                // `SHDEF.cBuilt`, which the computer players read.
                if let Some(design) = state
                    .designs
                    .get_mut(owner_index)
                    .and_then(|d| d.get_mut(usize::from(slot)))
                {
                    design.built += u32::try_from(count).unwrap_or(0);
                }
                if let Some(fleet) = add_ships_to_orbiting_fleet(state, owner, id, slot, count) {
                    // "has built a new …" / "has built N new …", about the
                    // new fleet, with the design word the message names
                    // the ship by.
                    let design_word = (owner << 5) | i16::from(slot);
                    let (message, params) = if count > 1 {
                        (
                            crate::message::id::SHIPS_BUILT,
                            vec![id, n_i16(count), design_word],
                        )
                    } else {
                        (crate::message::id::SHIP_BUILT, vec![id, design_word])
                    };
                    state.messages.push(crate::message::Message {
                        player: owner_index,
                        id: message,
                        object: crate::message::fleet_object(fleet),
                        params,
                    });
                }
            }
        }

        // Whatever the queue did not spend falls through to research, along
        // with the skim.
        report.research_spending[owner_index] += budget.research + available[COST_PARTS - 1];
    }

    // --- Produce: population update.
    for index in 0..state.planets.len() {
        let Some(race) = owner_race(state, index) else {
            continue;
        };
        let hull = starbase_hull_of(state, index);
        let change = crate::population::update_population_with_starbase(
            &mut state.planets[index],
            &race,
            hull,
        );
        if let Some(change) = change {
            let id = state.planets[index].id;
            report.population.push((id, change.delta));
        }
        // `UpdatePopulations` (`10b8:50a0`): a planet with an owner and
        // nobody left on it is given up — "died off" when this year's
        // change took the last of them, "jumped ship" when there were
        // none to begin with — and `UninhabitPlanet` clears it.
        let planet = &state.planets[index];
        if planet.owner.is_some() && planet.pop <= 0 {
            let id = planet.id;
            let owner = usize::try_from(planet.owner.unwrap_or(-1)).ok();
            let died = change.is_some_and(|c| c.delta < 0);
            let ar = race.prt() == Some(crate::race::Prt::Ar);
            if let Some(player) = owner {
                let base = if died {
                    crate::message::id::COLONISTS_DIED_OFF
                } else {
                    crate::message::id::COLONISTS_JUMPED_SHIP
                };
                state.messages.push(crate::message::Message {
                    player,
                    id: base + u16::from(ar),
                    object: id,
                    params: vec![id],
                });
            }
            let ca = race.prt() == Some(crate::race::Prt::Ca);
            crate::bombing::uninhabit(&mut state.planets[index], ca);
        }
    }

    // --- Produce: research advance.
    let slow_tech = state.slow_tech;
    for index in 0..state.players.len() {
        let player = &mut state.players[index];
        let resources = report.research_spending[index];
        player.research_last_year = resources;
        let gained = add_research(&mut player.research, &player.race, resources, slow_tech);
        // `DoResearch` (`turn2.c`): one message a level, its Goto the
        // Research dialog; the Generalized Research wording names the
        // primary field.
        let general = player.race.has_lrt(crate::race::lrt::GENERALIZED_RESEARCH);
        let tutorial = state.tutorial;
        for gain in &gained {
            state.messages.push(crate::message::Message {
                player: index,
                id: if general {
                    crate::message::id::TECH_LEVEL_GAINED_GENERAL
                } else {
                    crate::message::id::TECH_LEVEL_GAINED
                },
                object: crate::message::RESEARCH_OBJECT,
                params: vec![
                    i16::from(gain.level),
                    n_i16(i32::try_from(gain.field).unwrap_or(0)),
                    n_i16(i32::try_from(gain.continues_in).unwrap_or(0)),
                ],
            });
            // And what the level brings: `UpdateResearchStatus` walks the
            // categories from the engines up, one bit at a time, and for
            // each part the player may now build whose requirement in
            // this field is exactly the new level sends a word about it —
            // its browser word as the object for a component, the Ship
            // Design dialog for a hull. A Total Terraforming race skips
            // the three terraforming modules its trait replaces.
            let builder =
                crate::parts::Builder::player(&state.players[index]).in_tutorial(tutorial);
            let total_terraforming = builder.race.has_lrt(crate::race::lrt::TT);
            for (category_index, category) in (0..16u16).map(|i| (i, 1u16 << i)) {
                let mut item = 0usize;
                loop {
                    let status = crate::parts::availability(&builder, category, item);
                    if status == crate::parts::Availability::Missing {
                        break;
                    }
                    let skipped = category == crate::components::slot::TERRA
                        && total_terraforming
                        && matches!(item, 8 | 12 | 16);
                    if status.is_available()
                        && !skipped
                        && crate::parts::part(category, item).is_some_and(|p| {
                            p.tech[gain.field] == i8::try_from(gain.level).unwrap_or(i8::MAX)
                        })
                    {
                        use crate::components::slot;
                        use crate::message::id;
                        let (id, object) = match category {
                            slot::SB_HULL => (id::BREAKTHROUGH_STARBASE_HULL, -3),
                            slot::HULL => (id::BREAKTHROUGH_HULL, -3),
                            slot::PLANETARY if (9..14).contains(&item) => {
                                (id::BREAKTHROUGH_DEFENSE, part_word(category_index, item))
                            }
                            slot::PLANETARY if item < 9 => {
                                (id::BREAKTHROUGH_SCANNER, part_word(category_index, item))
                            }
                            _ => (id::BREAKTHROUGH_PART, part_word(category_index, item)),
                        };
                        state.messages.push(crate::message::Message {
                            player: index,
                            id,
                            object,
                            params: vec![
                                n_i16(i32::try_from(gain.field).unwrap_or(0)),
                                category as i16,
                                n_i16(i32::try_from(item).unwrap_or(0)),
                            ],
                        });
                    }
                    item += 1;
                }
            }
        }
        report.breakthroughs[index] = gained;
    }

    // --- Produce: random events, the last thing `Produce` does
    // (`10b8:0c87`), unless the game was set up without them.
    if state.random_events {
        report.events = crate::events::random_events(state, rng);
        report.skipped.retain(|s| *s != SkippedStep::RandomEvents);
    }

    // --- DoOrders(1) -> DoBattles: fleets that have come to share a place
    // with an enemy fight, before anything lands or unloads.
    report.battles = crate::combat::do_battles(state, rng);
    // --- DoBombing, straight after the battles: the bombers in orbit of an
    // enemy planet with no starbase left.
    report.bombings = crate::bombing::do_bombing(state, rng);

    // --- SatisfyOrders after movement: the tasks a fleet performs on arrival.
    // A task is consumed when it executes, which is why every waypoint in a
    // saved game that has already been reached reads 0.
    for pass in [3u8, 4] {
        let (done, drops) = crate::orders::execute_tasks_pass(state, pass, &moved_this_turn);
        report.tasks_done.extend(done);
        let settled = crate::orders::resolve_colonist_drops_with(state, &drops, Some(rng));
        report.colonised.extend(settled);
    }

    // --- DoOrders(1) -> DoThingInteractions(1): a fleet that has come to rest
    // on the Mystery Trader trades with it, and a computer player with a
    // starbase planet near one trades without sending anything. It happens
    // here, after movement and before the second pass of orders, which is why
    // a fleet cannot both trade and carry out a task in the same year: the
    // Trader keeps the fleet.
    let (trades, ai_trades) = trade_with_trader(state, rng);
    report.trades = trades;
    report.ai_trades = ai_trades;

    // --- SatisfyOrders(3): laying mines. A fleet ordered to lay does so where
    // it now is, into its own field if one reaches that far.
    for index in 0..state.fleets.len() {
        let key = (state.fleets[index].owner, state.fleets[index].id);
        let laid = lay_mines_for_fleet(state, index, moved_this_turn.contains(&key));
        let id = state.fleets[index].id;
        let owner = usize::try_from(state.fleets[index].owner).ok();
        let total: i32 = laid.iter().map(|(_, mines)| mines).sum();
        for (kind, mines) in laid {
            report.mines_laid.push((id, kind, mines));
        }
        if let (Some(player), true) = (owner, total > 0) {
            let mut params = vec![i16::try_from(id).unwrap_or(0)];
            params.extend_from_slice(&crate::message::Message::long(total));
            state.messages.push(crate::message::Message {
                player,
                id: crate::message::id::MINES_LAID,
                object: crate::message::fleet_object(id),
                params,
            });
        }
    }

    // --- SatisfyOrders(3): remote mining. A fleet that stayed put all turn
    // over an unowned planet, carrying mining robots and ordered to mine, digs
    // as `CMineFromLpfl` mines would and leaves the minerals on the surface.
    for index in 0..state.fleets.len() {
        let key = (state.fleets[index].owner, state.fleets[index].id);
        let Some(mined) = remote_mine_for_fleet(state, index, rng, moved_this_turn.contains(&key))
        else {
            continue;
        };
        report.remote_mined.push(mined);
    }

    // --- MoveThings(1): a packet thrown this year covers half a year, and
    // decays for it; and the wormholes think about moving.
    report.packets_landed.extend(move_packets(state, true, rng));
    report.wormholes_moved = move_wormholes(state, rng);

    // --- FuelFleets: a fleet in orbit of a starbase with a dock — its own
    // or a friend's — leaves the year with a full tank.
    fuel_fleets(state);

    // --- SweepForMines, which the original runs late, after the second pass
    // of orders: everything armed with beams clears what it is sitting in.
    report.mines_swept = sweep_minefields(state);

    // --- HealShips: damaged ships mend a share of their armour, faster
    // the safer they sit; a fleet that fought, hit a mine or jumped a gate
    // this year does not, and neither does a starbase that was attacked.
    no_heal.extend(jumped_from.keys().copied());
    let mut attacked_planets: std::collections::BTreeSet<i16> = std::collections::BTreeSet::new();
    for battle in &report.battles {
        for token in &battle.record.tokens {
            if token.object_class == 1 {
                if let Ok(planet) = i16::try_from(token.id) {
                    attacked_planets.insert(planet);
                }
            } else {
                no_heal.insert((i16::from(token.player), token.id));
            }
        }
    }
    heal_ships(state, &no_heal, &attacked_planets, &moved_this_turn);

    // --- UpdatePlayerScores, which the original runs near the end of the
    // year, once everything that could change a score has happened.
    report.scores = crate::score::scores(state);
    mark_the_dead(state, &report.scores);
    let (met, winners) = crate::victory::resolve(state, &report.scores);
    report.victory = met;
    report.winners = winners;
    declare_the_winners(state, &report.scores, &report.winners);

    // --- Patrol: every patrolling fleet looks for something to intercept.
    // The original does this as it writes each player's file, after
    // everything else has happened, because a patrol is decided from that
    // player's view — so it is done in `detect_things`, at the year's end.

    // --- AutoTerraform: the Claim Adjuster's free terraforming, which the
    // pipeline runs after Produce. It is a no-op for every other race.
    for index in 0..state.planets.len() {
        if !state.planets[index].detail.is_full() {
            continue;
        }
        let Some(owner) = state.planets[index].owner else {
            continue;
        };
        let Some(player) = usize::try_from(owner)
            .ok()
            .and_then(|i| state.players.get(i))
        else {
            continue;
        };
        let (race, tech) = (player.race.clone(), player.research.levels);
        if crate::terraform::auto_terraform(&mut state.planets[index], &race, tech, rng) {
            report.terraformed.push(state.planets[index].id);
        }
    }

    // --- RemoteTerraforming: Orbital Adjusters acting from orbit, step 17.
    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        let Some(orbiting) = fleet.orbiting else {
            continue;
        };
        let owner = fleet.owner;
        let Some(designs) = usize::try_from(owner)
            .ok()
            .and_then(|i| state.designs.get(i))
        else {
            continue;
        };
        let stacks: Vec<_> = fleet
            .stacks
            .iter()
            .filter_map(|st| designs.get(usize::from(st.design)).map(|d| (d, st.count)))
            .collect();
        let clicks = crate::terraform::orbital_adjusters(&stacks);
        if clicks <= 0 {
            continue;
        }
        let Some(planet_index) = state
            .planets
            .iter()
            .position(|p| p.id == i16::try_from(orbiting).unwrap_or(-1))
        else {
            continue;
        };
        let planet = &state.planets[planet_index];
        let Some(planet_owner) = planet.owner else {
            continue;
        };
        let has_starbase = planet.starbase;
        let same_owner = planet_owner == owner;
        let friendly = usize::try_from(owner)
            .ok()
            .and_then(|i| state.players.get(i))
            .is_some_and(|p| p.regards_as_friend(planet_owner));
        let Some(intent) = crate::terraform::remote_intent(same_owner, friendly, has_starbase)
        else {
            continue;
        };
        let (Some(fleet_player), Some(planet_player)) = (
            usize::try_from(owner)
                .ok()
                .and_then(|i| state.players.get(i)),
            usize::try_from(planet_owner)
                .ok()
                .and_then(|i| state.players.get(i)),
        ) else {
            continue;
        };
        let race = crate::terraform::remote_race(&fleet_player.race, &planet_player.race);
        let tech = fleet_player.research.levels;
        let done = crate::terraform::remote_terraform(
            &mut state.planets[planet_index],
            &race,
            tech,
            clicks,
            intent,
        );
        if done > 0 {
            report
                .remote_terraformed
                .push((state.planets[planet_index].id, done));
        }
    }

    // --- ValidateWaypoints: legs aimed at things that moved follow them,
    // legs aimed at fleets that are gone from the spot pick another there.
    validate_waypoints(state, rng);

    // --- UpdateGuesses: what everybody else's scanners will say of each
    // inhabited planet next year.
    update_guesses(state, rng);

    state.turn += 1;
    report.year = state.year();
    // The scoreboard is stamped with the year just finished, so it is filled
    // in after the turn has been counted.
    crate::score::update_standings(state, &report.scores, &report.victory, &report.winners);

    // --- SetVisiblePlanFleet, run for each player as their file is written:
    // what their scanners found among the space objects is remembered on
    // the objects themselves.
    report.patrols = detect_things(state, rng);
    report
}

/// The look each player's orders get as their file is written
/// (`FWriteDataFile`, `1070:5964`), for every fleet of theirs not
/// transporting and with somewhere to go. A leg aimed at a **thing** that
/// is off their map — the Trader gone, a minefield they have not detected,
/// a wormhole end their scanners do not reach this year — becomes a point
/// in space where it was, with a word (`0x110`, `0x111`, `0xf8`). A leg
/// aimed at a **fleet** gives up its `fNoAutoTrack` mark, and if the fleet
/// is gone (`0x28`), or off their map — at a planet, `0x29` and the leg is
/// aimed at the planet; in space or having gone through a gate, `0x2a` —
/// becomes a point in space where it was, or the planet standing exactly
/// there.
fn revalidate_orders(state: &mut GameState, player: usize, view: &crate::visibility::View) {
    use crate::message::{fleet_object, id, Message};
    let Ok(me) = i16::try_from(player) else {
        return;
    };
    let bit = 1u16 << (u16::try_from(player).unwrap_or(0) & 15);
    let planets: Vec<(i16, crate::movement::Point)> = state
        .planets
        .iter()
        .chain(state.known_planets.iter())
        .filter_map(|p| p.position.map(|at| (p.id, at)))
        .collect();
    let mut told: Vec<(u16, Vec<i16>)> = Vec::new();
    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        if fleet.owner != me || fleet.is_empty() || fleet.waypoints.len() < 2 {
            continue;
        }
        if fleet.waypoints[0].task == stars_formats::task::TRANSPORT {
            continue;
        }
        let fleet_id = fleet.id;
        let word = fleet_object(fleet_id);
        for leg in 1..state.fleets[index].waypoints.len() {
            let waypoint = state.fleets[index].waypoints[leg].clone();
            let mut to_point = false;
            match waypoint.target_class {
                GROBJ_THING => {
                    let Some(target) = waypoint.target else {
                        continue;
                    };
                    let gone = match target >> 13 {
                        ITH_TRADER => !state
                            .traders
                            .iter()
                            .any(|t| names_thing(Some(target), ITH_TRADER, t.id)),
                        ITH_WORMHOLE => !state.wormholes.iter().enumerate().any(|(i, w)| {
                            names_thing(Some(target), ITH_WORMHOLE, w.id)
                                && view.wormholes.contains(&i)
                        }),
                        ITH_MINEFIELD => !state.minefields.iter().any(|f| {
                            names_thing(Some(target), ITH_MINEFIELD, f.id)
                                && (f.owner == me || f.detected_by & bit != 0)
                        }),
                        _ => false,
                    };
                    if gone {
                        let which = match target >> 13 {
                            ITH_TRADER => id::TRADER_VANISHED,
                            ITH_WORMHOLE => id::WORMHOLE_VANISHED,
                            _ => id::MINEFIELD_VANISHED,
                        };
                        told.push((which, vec![word]));
                        to_point = true;
                    }
                }
                crate::fleet::grobj::FLEET => {
                    let Some(target) = waypoint.target else {
                        continue;
                    };
                    let no_auto_track = state.no_auto_track.remove(&(me, fleet_id, leg));
                    let quarry = crate::orders::split_fleet_id(target);
                    let found = state
                        .fleets
                        .iter()
                        .position(|f| (f.owner, f.id) == quarry && !f.is_empty());
                    match found {
                        None => {
                            #[allow(clippy::cast_possible_wrap)]
                            told.push((id::CHASED_FLEET_GONE, vec![word, target as i16]));
                        }
                        Some(q) if view.fleets.contains_key(&q) => continue,
                        Some(q) => {
                            let at_planet = state.fleets[q]
                                .orbiting
                                .and_then(|p| i16::try_from(p).ok())
                                .filter(|_| !no_auto_track);
                            match at_planet {
                                Some(planet) => {
                                    told.push((id::CHASED_FLEET_DUCKED, vec![word, planet]));
                                }
                                None => told.push((id::CHASED_FLEET_OUTRUN, vec![word])),
                            }
                        }
                    }
                    to_point = true;
                }
                _ => {}
            }
            if to_point {
                let w = &mut state.fleets[index].waypoints[leg];
                w.target = None;
                w.target_class = GROBJ_POSITION;
                if let Some((id, _)) = planets.iter().find(|(_, at)| *at == w.position) {
                    w.target = u16::try_from(*id).ok();
                    w.target_class = crate::fleet::grobj::PLANET;
                }
            }
        }
    }
    for (id, params) in told {
        state.messages.push(Message {
            player,
            id,
            object: params[0],
            params,
        });
    }
}

/// A message put at the front of the year's news (`FSendPrependedPlrMsg`).
fn prepend_message(state: &mut GameState, player: usize, id: u16, params: Vec<i16>) {
    state.messages.insert(
        0,
        crate::message::Message {
            player,
            id,
            object: -4,
            params,
        },
    );
}

/// `UpdatePlayerScores` (`10b8:6258`), the first thing it does with a
/// score: a player with no planet, no ship of any kind and not yet dead
/// is dead from now on, and everybody else hears it (`0xbb`).
fn mark_the_dead(state: &mut GameState, scores: &[crate::score::PlayerScore]) {
    for player in 0..state.players.len() {
        let Some(score) = scores.get(player) else {
            continue;
        };
        let nothing_left = score.planets == 0
            && score.unarmed_ships == 0
            && score.escort_ships == 0
            && score.capital_ships == 0;
        if !nothing_left || state.players[player].dead {
            continue;
        }
        state.players[player].dead = true;
        let word = i16::try_from(player).unwrap_or(0) | 0x30;
        for other in (0..state.players.len()).filter(|o| *o != player) {
            prepend_message(
                state,
                other,
                crate::message::id::PLAYER_ELIMINATED,
                vec![word],
            );
        }
    }
}

/// The end of `UpdatePlayerScores` (`10b8:6258`): with the winners known,
/// the game is over (`gd.fGameOverMan`) and everybody is told — the
/// winner alone (`0xb6`), the winners together (`0xb7`, each with the
/// others), the rest (`0xb5`, with the winners), the dead (`0xb8`); or,
/// with one player left standing, that player (`0xbc`) and the dead
/// (`0xb8`). Nothing is said while the game goes on.
fn declare_the_winners(
    state: &mut GameState,
    scores: &[crate::score::PlayerScore],
    winners: &[usize],
) {
    use crate::message::id;
    if state.players.len() < 2 || winners.is_empty() {
        return;
    }
    state.game_over = true;
    let alive = state.players.iter().filter(|p| !p.dead).count();
    if alive <= 1 {
        // The leader is the one the original congratulates, dead or not.
        let leader = scores
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| s.rank)
            .map_or(0, |(i, _)| i);
        for player in 0..state.players.len() {
            if player == leader {
                if !state.players[player].dead {
                    prepend_message(state, player, id::LAST_ONE_STANDING, Vec::new());
                }
            } else {
                prepend_message(state, player, id::GAME_OVER_DEAD, Vec::new());
            }
        }
        return;
    }
    let mask: u16 = winners.iter().fold(0, |m, w| {
        m | (1u16 << (u16::try_from(*w).unwrap_or(0) & 15))
    });
    for player in 0..state.players.len() {
        let mine = 1u16 << (u16::try_from(player).unwrap_or(0) & 15);
        let word = |m: u16| i16::from_ne_bytes(m.to_ne_bytes());
        // The original hands every one of these the mask; the record
        // keeps only what the message's parameter count allows.
        let (which, params) = if state.players[player].dead {
            (id::GAME_OVER_DEAD, Vec::new())
        } else if mask & mine == 0 {
            (id::GAME_WON_BY_OTHERS, vec![word(mask)])
        } else if mask == mine {
            (id::GAME_WON, Vec::new())
        } else {
            (id::GAME_WON_SHARED, vec![word(mask & !mine)])
        };
        prepend_message(state, player, which, params);
    }
}

/// The marks the file-writing passes leave on the space objects
/// (`SetVisPFPlanets` `1070:abde`, `SetVisPFFleets` `1070:a100`,
/// `SetVisPFThings` `1070:b9ee`), for every player in turn: a minefield a
/// scanner reaches is detected for good (`grbitPlr`) and seen this year
/// (`grbitPlrNow`); a wormhole end a scanner reaches is seen (`grbitPlr`),
/// which a jump later forgets. The Space Demolition roll for cloaked
/// fleets inside a field draws on the game's generator here, as the
/// original's does. Then, from the same view, the player's patrols pick
/// their targets and their orders are looked over (`FWriteDataFile`,
/// `1070:5964`). Returns the interceptions the patrols ordered.
fn detect_things(state: &mut GameState, rng: &mut Rng) -> Vec<(u16, u16)> {
    let mut patrols = Vec::new();
    for player in 0..state.players.len() {
        let view = crate::visibility::view_with(state, player, rng);
        let bit = 1u16 << (u16::try_from(player).unwrap_or(0) & 15);
        for &index in &view.minefields {
            if let Some(field) = state.minefields.get_mut(index) {
                field.detected_by |= bit;
                field.visible_to |= bit;
            }
        }
        for &index in &view.wormholes {
            if let Some(hole) = state.wormholes.get_mut(index) {
                hole.detected_by |= bit;
            }
        }
        patrols.extend(crate::patrol::patrol_for(state, player, &view));
        revalidate_orders(state, player, &view);
    }
    patrols
}

/// The race owning `planets[index]`, cloned so the planet can be mutated.
/// A starbase finished — `FBuildObject` (`10b8:19b2`), the `cBuilt > 0xf`
/// arm: the planet's base becomes the design in `slot` (0 to 9, the
/// player's starbase designs), a new one where there was none, the old
/// one's built count given back. A base replacing one with a smaller hull
/// number drops the ships from the planet's queue (`KillQueuedShips`,
/// `10c8:6c3a`: the ship items go, the starbase items lose their
/// progress). A planet whose old base had no mass driver looks at the new
/// one: none, and the packets come off the queue too
/// (`KillQueuedMassPackets`, `10c8:6a52`) with the fling setting cleared;
/// one, and the fling warp is set to it, a pair counting one faster. The
/// owner is told (`0xcd`, or `0xce` for a base with a dock and `0xcf`
/// for one that builds any size) with the design word and the dock's size.
///
/// Returns whether a base went up — not for a design the player has no
/// hull for.
fn install_starbase(state: &mut GameState, index: usize, slot: u8) -> bool {
    use crate::message::{id, Message};
    let planet_id = state.planets[index].id;
    let Some(owner) = state.planets[index]
        .owner
        .and_then(|o| usize::try_from(o).ok())
    else {
        return false;
    };
    let design_index = usize::from(crate::startup::FIRST_STARBASE_SLOT) + usize::from(slot);
    let Some(design) = state
        .designs
        .get(owner)
        .and_then(|d| d.get(design_index))
        .filter(|d| d.hull_id >= 0)
        .cloned()
    else {
        return false;
    };
    let Some(hull) = design.hull() else {
        return false;
    };
    let dock = i32::from(hull.cargo_max);
    let message = match dock {
        0 => id::STARBASE_BUILT,
        d if d == i32::from(u16::MAX) => id::STARBASE_BUILT_ANY_SIZE,
        _ => id::STARBASE_BUILT_WITH_DOCK,
    };
    let design_word = (i16::try_from(owner).unwrap_or(0) << 5)
        | i16::from(crate::startup::FIRST_STARBASE_SLOT + slot);
    let mut params = vec![planet_id, design_word];
    if message == id::STARBASE_BUILT_WITH_DOCK {
        params.push(i16::try_from(dock).unwrap_or(i16::MAX));
    }
    state.messages.push(Message {
        player: owner,
        id: message,
        object: planet_id,
        params,
    });

    let designs = state.designs[owner].clone();
    let planet = &mut state.planets[index];
    let old_driver = crate::production::mass_driver(planet, &designs);
    if planet.starbase {
        // A smaller base cannot finish the ships the old one was building.
        if let Some(old) = planet
            .starbase_design
            .map(usize::from)
            .map(|s| usize::from(crate::startup::FIRST_STARBASE_SLOT) + s)
            .and_then(|s| designs.get(s))
        {
            if design.hull_id < old.hull_id {
                planet.queue.retain(|q| {
                    !(q.ship && q.item < u16::from(crate::startup::FIRST_STARBASE_SLOT))
                });
                for q in planet.queue.iter_mut().filter(|q| q.ship) {
                    q.completion = 0;
                }
            }
        }
        if let Some(old) = planet.starbase_design {
            if let Some(d) = state.designs[owner]
                .get_mut(usize::from(crate::startup::FIRST_STARBASE_SLOT) + usize::from(old))
            {
                d.built = d.built.saturating_sub(1);
            }
        }
    }
    let planet = &mut state.planets[index];
    planet.starbase = true;
    planet.starbase_design = Some(slot);
    planet.starbase_damage = 0;
    if old_driver.warp < 1 {
        let now = crate::production::mass_driver(planet, &designs);
        if now.warp < 1 {
            planet.fling_warp = 0;
            planet.fling_dest = None;
            planet.queue.retain(|q| {
                q.ship
                    || !(crate::production::item::PACKET_IRONIUM
                        ..=crate::production::item::PACKET_MIXED)
                        .contains(&q.item)
            });
        } else {
            planet.fling_warp =
                u8::try_from((now.warp + i32::from(now.paired) - 4).max(0)).unwrap_or(0);
        }
    }
    if let Some(d) = state.designs[owner].get_mut(design_index) {
        d.built += 1;
    }
    true
}

/// The hull of a planet's starbase, for the Alternate Reality maximum.
fn starbase_hull_of(state: &GameState, index: usize) -> Option<i16> {
    let planet = &state.planets[index];
    let owner = usize::try_from(planet.owner?).ok()?;
    crate::production::starbase_hull(planet, state.designs.get(owner)?)
}

fn owner_race(state: &GameState, index: usize) -> Option<crate::Race> {
    let owner = state.planets[index].owner?;
    let owner_index = usize::try_from(owner).ok()?;
    state.players.get(owner_index).map(|p| p.race.clone())
}

/// Advance every planet's population by one year without a full turn.
///
/// This is the `UpdatePopulations` step on its own, kept for callers that want
/// to study growth in isolation.
pub fn update_populations(planets: &mut [Planet], players: &[Player]) {
    for planet in planets.iter_mut() {
        let Some(owner) = planet.owner else { continue };
        let Ok(index) = usize::try_from(owner) else {
            continue;
        };
        let Some(player) = players.get(index) else {
            continue;
        };
        update_population(planet, &player.race);
    }
}

/// Add newly built ships to a fleet the owner already has in orbit.
///
/// Ships appear in whichever of the owner's fleets is orbiting the planet that
/// built them, merging into an existing stack of the same design.
///
/// When the owner has **no** fleet there, a new one is created in orbit, taking
/// the planet's coordinates and the next free fleet id for that owner. That
/// needs [`GameState::apply_universe`] to have supplied the planet's position;
/// without it the ships are still reported and their cost still spent, but no
/// fleet can be made, because a fleet with no position would be worse than none
/// at all.
fn add_ships_to_orbiting_fleet(
    state: &mut GameState,
    owner: i16,
    planet: i16,
    design: u8,
    count: i32,
) -> Option<u16> {
    let orbiting = u16::try_from(planet).ok();
    let stack = crate::fleet::ShipStack {
        design,
        count,
        damaged_pct: 0,
        damage_pct: 0,
    };

    // What is built this year is a fleet of its own, one per design: the
    // tutorial's three colony ships come out as one "Santa Maria #8" and its
    // two scouts as one "Armed Probe #9", and neither joins the freighter
    // that happens to be in orbit. The new ships leave the yard with their
    // tanks full, which is what lets the tutorial send a scout off on the
    // first page — "It has been automatically fueled by your starbase".
    let position = state
        .planets
        .iter()
        .find(|p| p.id == planet)
        .and_then(|p| p.position)?;
    let fuel = usize::try_from(owner)
        .ok()
        .and_then(|o| state.designs.get(o))
        .and_then(|designs| designs.get(usize::from(design)))
        .and_then(crate::design::ShipDesign::fuel_capacity)
        .unwrap_or(0)
        * count;
    let id = next_fleet_id(state, owner);
    state.fleets.push(crate::fleet::Fleet {
        name: None,
        repeat_orders: false,
        direction: None,
        id,
        owner,
        position,
        orbiting,
        stacks: vec![stack],
        cargo: crate::fleet::Cargo {
            fuel,
            ..crate::fleet::Cargo::default()
        },
        battle_plan: 0,
        warp: None,
        waypoints: vec![crate::fleet::Waypoint {
            position,
            target: orbiting,
            target_class: 1,
            warp: 0,
            task: 0,
            transport: None,
            task_data: Vec::new(),
        }],
    });
    let index = state.fleets.len() - 1;
    auto_route_fleet(state, index, planet);
    Some(id)
}

/// `AutoRouteFleet` (`1080:1e52`): a ship built at a planet with a
/// **route** leaves the yard with a leg to the route's end — a Route task,
/// at a warp found thus: `IFindIdealWarp`'s cruising warp; "Use Stargate"
/// instead when both planets are the owner's, both starbases carry a gate,
/// the fleet carries nothing and its heaviest design would jump undamaged
/// (`MdCalcStargateDamage`); else, below warp 9 with a **dock** at the far
/// end to refuel at, the fastest warp up to 9 whose range covers the leg;
/// then, down to warp 3, the slowest warp that takes no more years over
/// the leg than that one; then lower still while the tank will not cover
/// the leg.
pub(crate) fn auto_route_fleet(state: &mut GameState, index: usize, planet: i16) {
    let owner = state.fleets[index].owner;
    let Some(destination) = state
        .planets
        .iter()
        .find(|p| p.id == planet && p.owner == Some(owner))
        .and_then(|p| p.route_dest)
        .filter(|to| *to != planet)
    else {
        return;
    };
    let Some(to) = state
        .planets
        .iter()
        .chain(state.known_planets.iter())
        .find(|p| p.id == destination)
        .and_then(|p| p.position)
    else {
        return;
    };
    let designs = usize::try_from(owner)
        .ok()
        .and_then(|o| state.designs.get(o))
        .cloned()
        .unwrap_or_default();
    let ife = usize::try_from(owner)
        .ok()
        .and_then(|o| state.players.get(o))
        .is_some_and(|p| p.race.has_lrt(crate::race::lrt::IFE));
    let fleet = &state.fleets[index];
    #[allow(clippy::cast_possible_truncation)]
    let distance = crate::movement::distance(fleet.position, to).ceil() as i32;
    let years = |warp: i32| (distance + warp * warp - 1) / (warp * warp);
    let mut warp = i32::from(crate::movement::ideal_warp(&fleet.stacks, &designs, false));
    // The gate, and the dock, at the route's end.
    let here = state.planets.iter().find(|p| p.id == planet);
    let there = state
        .planets
        .iter()
        .chain(state.known_planets.iter())
        .find(|p| p.id == destination);
    if let (Some(here), Some(there)) = (here, there) {
        if here.owner == there.owner && here.starbase && there.starbase {
            let gates = (
                crate::stargate::gate_of(here, &designs),
                crate::stargate::gate_of(there, &designs),
            );
            let empty = fleet.cargo.minerals.iter().all(|m| *m == 0) && fleet.cargo.colonists == 0;
            if let (Some(src), Some(dst)) = gates {
                let heaviest = fleet
                    .stacks
                    .iter()
                    .filter(|s| s.count != 0)
                    .filter_map(|s| designs.get(usize::from(s.design)))
                    .filter_map(crate::design::ShipDesign::mass)
                    .max()
                    .unwrap_or(0);
                #[allow(clippy::cast_possible_truncation)]
                let straight = crate::movement::distance(fleet.position, to) as i32;
                if empty
                    && crate::stargate::verdict(src, dst, straight, heaviest)
                        == crate::stargate::Verdict::Damage(0)
                {
                    warp = i32::from(crate::stargate::WARP);
                }
            }
            if warp < 9 {
                let dock = crate::production::starbase_hull(there, &designs)
                    .and_then(crate::components::hull)
                    .is_some_and(|h| h.cargo_max != 0);
                if dock {
                    warp = (1..=9)
                        .rev()
                        .find(|w| {
                            fleet.fuel_range(&designs, u8::try_from(*w).unwrap_or(0), ife)
                                >= distance
                        })
                        .unwrap_or(0);
                }
            }
        }
    }
    if (1..11).contains(&warp) {
        let at_ideal = years(warp);
        while warp >= 3 && years(warp - 1) <= at_ideal {
            warp -= 1;
        }
        while warp > 0
            && fleet.fuel_use(&designs, u8::try_from(warp).unwrap_or(0), distance, ife)
                > fleet.cargo.fuel
        {
            warp -= 1;
        }
    }
    let warp = u8::try_from(warp.clamp(0, i32::from(crate::stargate::WARP))).unwrap_or(0);
    let fleet = &mut state.fleets[index];
    fleet.waypoints.push(crate::fleet::Waypoint {
        position: to,
        target: u16::try_from(destination).ok(),
        target_class: crate::fleet::grobj::PLANET,
        warp,
        task: stars_formats::task::ROUTE,
        transport: None,
        task_data: Vec::new(),
    });
    fleet.warp = Some(warp);
}

/// The waypoint task ordering a fleet to mine from orbit (`grTaskMine`).
const TASK_REMOTE_MINE: u8 = 3;

/// Run one fleet's remote mining order, if it has one it can act on.
///
/// Source: the `grTaskMine` arm of `SatisfyOrders` (`turn3.c` in the
/// reconstructed sources), which runs at `iPass == 3` — the first pass after
/// movement:
///
/// ```c
/// if (ord.grTask == grTaskMine && iPass == 3 && lpfl->fHereAllTurn) {
///     cMine = CMineFromLpfl(lpfl);
///     if (cMine != 0) {
///         if (lppl->iPlayer == -1) EstMineralsMined(lppl, &l, cMine, 1);
///         else if (majorAdv != raMacintosh) { message; cancel the order; }
///     }
/// }
/// ```
///
/// Three gates, all of which matter: the fleet must have **stayed put the whole
/// turn**, it must be over a planet rather than deep space, and that planet must
/// be **unowned**. Mining someone's planet is refused outright and the order
/// cancelled, unless the miner is an Alternate Reality race, which is the one
/// case the routine lets pass.
///
/// The minerals land on the planet's surface, not in the fleet — collecting
/// them is a separate cargo transfer.
fn remote_mine_for_fleet(
    state: &mut GameState,
    index: usize,
    rng: &mut Rng,
    moved: bool,
) -> Option<(i16, [i32; 3])> {
    let fleet = &state.fleets[index];
    // `fHereAllTurn`: a fleet that moved this turn has not been in place long
    // enough to mine.
    if fleet.waypoints.first().map(|w| w.task) != Some(TASK_REMOTE_MINE) || moved {
        return None;
    }
    let planet_id = i16::try_from(fleet.orbiting?).ok()?;
    let owner = usize::try_from(fleet.owner).ok()?;
    let designs = state.designs.get(owner)?.clone();
    let mines = crate::mining::remote_mines(&designs, &state.fleets[index].stacks);
    if mines == 0 {
        return None;
    }
    let planet_index = state.planets.iter().position(|p| p.id == planet_id)?;
    // An owned planet is refused; only the unowned ones may be mined.
    if state.planets[planet_index].owner.is_some() {
        return None;
    }
    let race = state.players.get(owner)?.race.clone();
    let mined =
        crate::mining::mine_minerals(&mut state.planets[planet_index], &race, Some(mines), rng);
    Some((planet_id, mined))
}

/// Lay one fleet's mines, if it was ordered to.
///
/// `SatisfyOrders` at `10b0:999e`. A fleet must have been **here all turn** to
/// lay, unless its player is Space Demolition, who lay while moving at half
/// rate. The task's payload counts the years down: `5` means *indefinitely* and
/// is never spent, `0` clears the order, and anything else loses a year.
///
/// Returns what was laid, as `(kind, mines)`.
fn lay_mines_for_fleet(state: &mut GameState, index: usize, moved: bool) -> Vec<(u8, i32)> {
    let fleet = &state.fleets[index];
    let Ok(owner) = usize::try_from(fleet.owner) else {
        return Vec::new();
    };
    let demolition = state
        .players
        .get(owner)
        .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Sd));
    let laying_here =
        fleet.waypoints.first().map(|w| w.task) == Some(stars_formats::task::LAY_MINES);
    // Space Demolition lays **on the way** to a waypoint whose task is to
    // lay (`SatisfyOrders`, `10b0:999e`: a current order with no task, a
    // next one with the lay task), at the half rate of a mover.
    let laying_en_route = !laying_here
        && demolition
        && fleet.waypoints.first().map(|w| w.task) == Some(stars_formats::task::NONE)
        && fleet.waypoints.get(1).map(|w| w.task) == Some(stars_formats::task::LAY_MINES);
    if !laying_here && !laying_en_route {
        return Vec::new();
    }
    if moved && !demolition {
        return Vec::new();
    }
    let Some(designs) = state.designs.get(owner).cloned() else {
        return Vec::new();
    };

    let laid = crate::minefield::lay(&mut state.minefields, &state.fleets[index], &designs, moved);
    if laying_en_route {
        return laid;
    }

    // The countdown: 5 lays forever, 0 ends the order, anything else counts
    // down a year.
    let waypoint = &mut state.fleets[index].waypoints[0];
    let years = waypoint
        .task_data
        .get(0..2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]));
    match years {
        Some(0) => waypoint.task = stars_formats::task::NONE,
        Some(5) | None => {}
        Some(left) => {
            let next = (left - 1).to_le_bytes();
            waypoint.task_data[0] = next[0];
            waypoint.task_data[1] = next[1];
        }
    }
    laid
}

/// Fly one leg past everybody else's minefields.
///
/// Wraps [`crate::minefield::traverse`]: it needs the fleet's race for its
/// mine expertise, the relations table to know whose fields are friendly, and
/// it moves the fleet back to where it was stopped when it hits one.
fn cross_minefields(
    state: &mut GameState,
    index: usize,
    from: crate::movement::Point,
    travelled: i32,
    rng: &mut Rng,
) -> Option<crate::minefield::MineHit> {
    let owner = usize::try_from(state.fleets[index].owner).ok()?;
    let player = state.players.get(owner)?;
    let expertise = crate::minefield::mine_expertise(&player.race);
    let relations = player.relations.clone();
    let friendly = move |other: i16| -> bool {
        usize::try_from(other)
            .ok()
            .and_then(|i| relations.get(i))
            .is_some_and(|r| *r == 1)
    };
    let to = state.fleets[index].position;
    let (field_index, at) = crate::minefield::traverse(
        &state.minefields,
        &state.fleets[index],
        crate::minefield::Leg {
            from,
            to,
            travelled,
        },
        expertise,
        &friendly,
        rng,
    )?;

    // The fleet stops where it was hit.
    let stopped = crate::movement::advance(from, to, at);
    let fleet = &mut state.fleets[index];
    fleet.position = stopped;
    if stopped != to {
        fleet.orbiting = None;
    }

    let field = state.minefields[field_index].clone();
    let hit = mine_hit(state, index, &field, at, false)?;

    // The field pays for it too, and the player who found it can now see it.
    if let Some(field) = state
        .minefields
        .iter_mut()
        .find(|f| f.id == hit.field && f.owner == hit.field_owner && f.kind == hit.kind)
    {
        let cost = crate::minefield::hit_cost(field.mines);
        field.mines -= cost;
        if let Ok(bit) = u32::try_from(state.fleets[index].owner) {
            field.detected_by |= u16::try_from(1u32 << (bit & 15)).unwrap_or(0);
            field.visible_to |= u16::try_from(1u32 << (bit & 15)).unwrap_or(0);
        }
    }
    state.minefields.retain(|f| f.mines > 0);
    Some(hit)
}

/// What a minefield does to a fleet once it has it: the damage
/// ([`crate::minefield::apply_damage`]), the cargo the dead ships can no
/// longer carry dropped as salvage where the fleet stands
/// (`FleetTransferCargoBalance` into a fleet of the dead, then
/// `DropSalvage`, `10b0:5f2c`), and the two owners told.
///
/// A detonation that does nothing is not a hit (`10b0:5cf0`). A Space
/// Demolition field owner's learning of the designs it hit (`10b0:6174`)
/// is not modelled. The fleet, if emptied, is left for the movement pass
/// to prune.
fn mine_hit(
    state: &mut GameState,
    index: usize,
    field: &crate::minefield::Minefield,
    travelled: i32,
    detonation: bool,
) -> Option<crate::minefield::MineHit> {
    use crate::message::{fleet_name_word, fleet_object, id, Message, THING_OBJECT};

    let owner = usize::try_from(state.fleets[index].owner).ok()?;
    let designs = state.designs.get(owner).cloned().unwrap_or_default();
    let regen = state
        .players
        .get(owner)
        .is_some_and(|p| p.race.has_lrt(crate::race::lrt::REGENERATING_SHIELDS));
    let before = state.fleets[index].stacks.clone();
    let (design, mixed) = primary_design(&state.fleets[index]);
    let hit = crate::minefield::apply_damage(
        &mut state.fleets[index],
        &designs,
        regen,
        field,
        travelled,
        detonation,
    );
    if detonation && hit.damage == 0 {
        return None;
    }

    // The dead ships' share of the cargo goes down with them.
    let mut salvage: Option<u16> = None;
    if hit.ships_lost > 0 {
        let mut dead = Fleet {
            stacks: before
                .iter()
                .map(|was| {
                    let now = state.fleets[index]
                        .stacks
                        .iter()
                        .find(|s| s.design == was.design)
                        .map_or(0, |s| s.count);
                    crate::fleet::ShipStack {
                        design: was.design,
                        count: was.count - now,
                        damaged_pct: 0,
                        damage_pct: 0,
                    }
                })
                .collect(),
            ..state.fleets[index].clone()
        };
        dead.cargo = crate::fleet::Cargo::default();
        crate::fleet::balance_cargo(
            [&mut state.fleets[index], &mut dead],
            [&before, &[]],
            &designs,
        );
        if dead.cargo.minerals.iter().any(|m| *m > 0) {
            let at = state.fleets[index].position;
            salvage = crate::combat::drop_salvage(state, at, dead.cargo.minerals);
        }
    }

    // Both sides hear of it.
    let fleet_id = state.fleets[index].id;
    let field_owner = usize::try_from(field.owner).ok();
    // A Space Demolition player's field reads every design of what it hits
    // (`10b0:6174`), which their file this year then describes in full.
    if let Some(fo) = field_owner.filter(|fo| {
        *fo != owner
            && state
                .players
                .get(*fo)
                .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Sd))
    }) {
        for stack in before.iter().filter(|s| s.count > 0) {
            state
                .revealed_designs
                .insert((fo, owner, usize::from(stack.design)));
        }
    }
    let at = state.fleets[index].position;
    let kind = i16::from(field.kind);
    let thing_full = |ith: u16, owner: i16, id: u16| -> i16 {
        ((ith << 13) | (u16::try_from(owner).unwrap_or(0) & 0xf) << 9 | (id & 0x1ff)) as i16
    };
    let damage = i16::try_from(hit.damage).unwrap_or(i16::MAX);
    let lost = i16::try_from(hit.ships_lost).unwrap_or(i16::MAX);
    let object = fleet_object(fleet_id);
    let mut push = |player: usize, id: u16, object: i16, params: Vec<i16>| {
        state.messages.push(Message {
            player,
            id,
            object,
            params,
        });
    };
    if hit.damage == 0 {
        if Some(owner) != field_owner {
            push(
                owner,
                id::MINE_HIT_NO_DAMAGE,
                object,
                vec![object, field.owner, kind, at.x, at.y],
            );
        }
        if let Some(fo) = field_owner {
            push(
                fo,
                id::YOUR_FIELD_HIT_NO_DAMAGE,
                object,
                vec![object, kind, at.x, at.y],
            );
        }
    } else if hit.ships_lost == 0 {
        if Some(owner) != field_owner {
            let id = if detonation {
                id::MINE_DETONATED_ON
            } else {
                id::MINE_HIT
            };
            push(
                owner,
                id,
                object,
                vec![object, field.owner, kind, at.x, at.y, damage],
            );
        }
        if let Some(fo) = field_owner {
            let id = if detonation {
                id::YOUR_FIELD_DETONATED_ON
            } else {
                id::YOUR_FIELD_HIT
            };
            push(fo, id, object, vec![object, kind, at.x, at.y, damage]);
        }
    } else if hit.fleet_destroyed {
        let named = fleet_name_word(fleet_id, design, mixed);
        match salvage {
            None => {
                if Some(owner) != field_owner {
                    push(
                        owner,
                        id::MINE_HIT_DESTROYED,
                        -1,
                        vec![named, field.owner, kind, at.x, at.y],
                    );
                }
                if let Some(fo) = field_owner {
                    if fo == owner {
                        push(
                            fo,
                            id::YOUR_FIELD_DESTROYED_YOURS,
                            -1,
                            vec![named, kind, at.x, at.y],
                        );
                    } else {
                        push(
                            fo,
                            id::YOUR_FIELD_DESTROYED_FLEET,
                            THING_OBJECT,
                            vec![
                                thing_full(0, field.owner, field.id),
                                fleet_id as i16,
                                kind,
                                at.x,
                                at.y,
                            ],
                        );
                    }
                }
            }
            Some(salvage) => {
                let salvage_full = thing_full(1, -1, salvage);
                if Some(owner) != field_owner {
                    push(
                        owner,
                        id::MINE_HIT_DESTROYED_SALVAGE,
                        THING_OBJECT,
                        vec![salvage_full, named, field.owner, kind, at.x, at.y],
                    );
                }
                if let Some(fo) = field_owner {
                    push(
                        fo,
                        id::YOUR_FIELD_DESTROYED_FLEET,
                        THING_OBJECT,
                        vec![salvage_full, fleet_id as i16, kind, at.x, at.y],
                    );
                }
            }
        }
    } else {
        if Some(owner) != field_owner {
            let id = if detonation {
                id::MINE_DETONATED_ON_SHIPS_LOST
            } else {
                id::MINE_HIT_SHIPS_LOST
            };
            push(
                owner,
                id,
                object,
                vec![object, field.owner, kind, at.x, at.y, damage, lost],
            );
        }
        if let Some(fo) = field_owner {
            let id = if detonation {
                id::YOUR_FIELD_DETONATED_ON_SHIPS_LOST
            } else {
                id::YOUR_FIELD_HIT_SHIPS_LOST
            };
            push(fo, id, object, vec![object, kind, at.x, at.y, damage, lost]);
        }
    }
    Some(hit)
}

/// Set off every field that is armed to detonate.
///
/// `ThingDecay` (`10b8:70c6`) walks **every** fleet inside an armed field —
/// the owner's own included, though its mine-layer hulls are spared — and
/// takes it through `FTravelThroughMineFields` with no roll: the mines are
/// going off whether or not the fleet was moving. A fleet is caught once a
/// year however many fields go off under it (`det` bit 12), and a
/// detonation that does no damage is not a hit.
fn detonate_minefields(state: &mut GameState) -> Vec<(u16, crate::minefield::MineHit)> {
    let mut hits = Vec::new();
    let armed: Vec<crate::minefield::Minefield> = state
        .minefields
        .iter()
        .filter(|f| f.detonating)
        .cloned()
        .collect();
    let mut caught: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for field in &armed {
        for index in 0..state.fleets.len() {
            let fleet = &state.fleets[index];
            if fleet.is_empty() || caught.contains(&index) || !field.contains(fleet.position) {
                continue;
            }
            if let Some(hit) = mine_hit(state, index, field, 0, true) {
                hits.push((state.fleets[index].id, hit));
                caught.insert(index);
            }
        }
    }
    prune_destroyed(state, &hits);
    hits
}

/// Drop the fleets a minefield left nothing of.
fn prune_destroyed(state: &mut GameState, hits: &[(u16, crate::minefield::MineHit)]) {
    let gone: Vec<u16> = hits
        .iter()
        .filter(|(_, hit)| hit.fleet_destroyed)
        .map(|(id, _)| *id)
        .collect();
    if gone.is_empty() {
        return;
    }
    state
        .fleets
        .retain(|f| !(f.is_empty() && gone.contains(&f.id)));
}

/// Decay every minefield, and remove the ones that run out.
///
/// `ThingDecay` (`10b8:70c6`). See [`crate::minefield::decay_amount`] for the
/// rate; the planets that speed it up are the ones inside the field, which is
/// `CPlanetsInCircle`.
fn decay_minefields(state: &mut GameState) -> Vec<(u16, i16, i32)> {
    let mut lost = Vec::new();
    let positions: Vec<crate::movement::Point> = state
        .planets
        .iter()
        .chain(state.known_planets.iter())
        .filter_map(|p| p.position)
        .collect();

    for field in &mut state.minefields {
        let inside = positions
            .iter()
            .filter(|p| field.contains(**p))
            .count()
            .min(i32::MAX as usize);
        let demolition = usize::try_from(field.owner)
            .ok()
            .and_then(|i| state.players.get(i))
            .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Sd));
        let amount = crate::minefield::decay_amount(
            field,
            i32::try_from(inside).unwrap_or(i32::MAX),
            demolition,
        );
        lost.push((field.id, field.owner, amount.min(field.mines)));
        field.mines -= amount;
    }
    state.minefields.retain(|f| f.mines > 0);
    lost
}

/// Sweep every minefield somebody hostile is sitting in.
///
/// `SweepForMines` (`10b8:76a4`): first every fleet with beam weapons, then
/// every planet with a starbase, each clearing the fields it is inside that
/// belong to a player it is not friendly with.
fn sweep_minefields(state: &mut GameState) -> Vec<(u16, i16, i32)> {
    let mut swept = Vec::new();
    // Both sides are told: `SweepForMines` sends the sweeper one message and
    // the field's owner another.
    let mut told: Vec<(i16, i16, Sweeper, u8, crate::movement::Point, i32)> = Vec::new();

    // A sweeper is a position, an owner, and how much it can clear.
    let mut sweepers: Vec<(crate::movement::Point, i16, i32, Sweeper)> = Vec::new();
    for fleet in &state.fleets {
        let Ok(owner) = usize::try_from(fleet.owner) else {
            continue;
        };
        let Some(designs) = state.designs.get(owner) else {
            continue;
        };
        let capacity = crate::minefield::fleet_sweep(fleet, designs);
        if capacity > 0 {
            sweepers.push((
                fleet.position,
                fleet.owner,
                capacity,
                Sweeper::Fleet(fleet.id),
            ));
        }
    }
    for planet in &state.planets {
        let (Some(owner), Some(position), true) = (planet.owner, planet.position, planet.starbase)
        else {
            continue;
        };
        let capacity = usize::try_from(owner)
            .ok()
            .and_then(|i| state.designs.get(i))
            .and_then(|designs| {
                let slot = usize::from(planet.starbase_design.unwrap_or(0))
                    + usize::from(crate::startup::FIRST_STARBASE_SLOT);
                designs.get(slot)
            })
            .map_or(0, crate::minefield::sweep_capacity);
        if capacity > 0 {
            sweepers.push((position, owner, capacity, Sweeper::Planet(planet.id)));
        }
    }

    for (position, owner, capacity, from) in sweepers {
        let friend = usize::try_from(owner)
            .ok()
            .and_then(|i| state.players.get(i))
            .map(|p| p.relations.clone())
            .unwrap_or_default();
        for field in &mut state.minefields {
            if field.owner == owner {
                continue;
            }
            let friendly = usize::try_from(field.owner)
                .ok()
                .and_then(|i| friend.get(i))
                .is_some_and(|r| *r == 1);
            if friendly {
                continue;
            }
            let d2 = field.distance_squared(position);
            if d2 > i64::from(field.mines) {
                continue;
            }
            let take = crate::minefield::swept(field, capacity, d2);
            if take <= 0 {
                continue;
            }
            field.mines -= take;
            if let Ok(bit) = u32::try_from(owner) {
                field.detected_by |= u16::try_from(1u32 << (bit & 15)).unwrap_or(0);
            }
            told.push((owner, field.owner, from, field.kind, position, take));
            swept.push((field.id, field.owner, take));
        }
        state.minefields.retain(|f| f.mines > 0);
    }

    for (sweeper_owner, field_owner, from, kind, at, count) in told {
        let (id, subject) = match from {
            Sweeper::Fleet(fleet) => (
                crate::message::id::FLEET_SWEPT,
                crate::message::fleet_object(fleet),
            ),
            Sweeper::Planet(planet) => (crate::message::id::STARBASE_SWEPT, planet),
        };
        let mut params = vec![match from {
            Sweeper::Fleet(fleet) => i16::try_from(fleet).unwrap_or(0),
            Sweeper::Planet(planet) => planet,
        }];
        params.extend_from_slice(&crate::message::Message::long(count));
        params.extend_from_slice(&[field_owner, i16::from(kind), at.x, at.y]);
        if let Ok(player) = usize::try_from(sweeper_owner) {
            state.messages.push(crate::message::Message {
                player,
                id,
                object: subject,
                params: params.clone(),
            });
        }
        // The owner of the field hears it too, and is not told who did it.
        if let Ok(player) = usize::try_from(field_owner) {
            let mut theirs = vec![0];
            theirs.extend_from_slice(&crate::message::Message::long(count));
            theirs.extend_from_slice(&[i16::from(kind), at.x, at.y]);
            state.messages.push(crate::message::Message {
                player,
                id: crate::message::id::YOUR_FIELD_SWEPT,
                object: -6,
                params: theirs,
            });
        }
    }
    swept
}

/// What cleared a minefield, for the message that says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sweeper {
    /// A fleet, by its number.
    Fleet(u16),
    /// A planet's starbase, by planet id.
    Planet(i16),
}

/// Move every packet, land the ones that arrive, and decay the rest.
///
/// `MoveThings` (`10b0:18f4`) runs twice a year. The first pass, before
/// production, moves every packet a full year's distance — the square of its
/// warp — and marks it moved. The second, after production, moves only the
/// packets that have *not* moved, which are the ones thrown this year, and
/// gives them **half** a year: launched mid-year, they arrive that much later.
/// A packet that moves without arriving decays for the part of a year it flew.
///
/// Returns what landed.
fn move_packets(
    state: &mut GameState,
    after_production: bool,
    rng: &mut Rng,
) -> Vec<crate::packet::Landing> {
    let mut landed = Vec::new();
    let mut arrived: Vec<usize> = Vec::new();

    for index in 0..state.packets.len() {
        let packet = &state.packets[index];
        if packet.warp == 0 || (after_production && packet.moved) {
            continue;
        }
        if packet.mass() == 0 {
            arrived.push(index);
            continue;
        }
        let Ok(target_id) = i16::try_from(packet.target) else {
            continue;
        };
        let Some(target) = state
            .planets
            .iter()
            .chain(state.known_planets.iter())
            .find(|p| p.id == target_id)
            .and_then(|p| p.position)
        else {
            continue;
        };

        let range = if after_production {
            packet.range() / 2
        } else {
            packet.range()
        };
        let physics = usize::try_from(packet.owner)
            .ok()
            .and_then(|i| state.players.get(i))
            .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Pp));

        let distance = crate::movement::distance(state.packets[index].position, target);
        let packet = &mut state.packets[index];
        packet.moved = true;
        if crate::packet::advance(packet, target, range) {
            // It got there. What is left of it decays for the fraction of the
            // move it actually used before it lands.
            #[allow(clippy::cast_possible_truncation)]
            let part = if range > 0 {
                ((distance * 100.0 / f64::from(range)) as i32).clamp(0, 100)
            } else {
                0
            };
            let part = if after_production { part / 2 } else { part };
            if crate::packet::decay(packet, physics, part) {
                arrived.push(index);
                continue;
            }
            let packet = state.packets[index].clone();
            if let Some(result) = crate::packet::land(state, &packet, rng) {
                landed.push(result);
            }
            arrived.push(index);
        } else {
            let part = if after_production { 50 } else { 100 };
            if crate::packet::decay(packet, physics, part) {
                arrived.push(index);
            }
        }
    }

    arrived.sort_unstable();
    arrived.dedup();
    for index in arrived.into_iter().rev() {
        state.packets.remove(index);
    }
    landed
}

/// Where the Mystery Trader heads for when it picks a new destination.
///
/// `MoveThings` (`10b0:1b5c`): a point on one **edge** of the galaxy — the far
/// side or the near one, and along the x axis or the y — so that whatever it
/// does next, it crosses the map rather than loitering.
fn trader_destination(size: i32, rng: &mut Rng) -> crate::movement::Point {
    let span = size * 400;
    let edge = if rng.random(2) == 0 {
        span + 1380
    } else {
        1020
    };
    let along = i32::from(rng.random(i16::try_from(span + 361).unwrap_or(i16::MAX))) + 1020;
    let (x, y) = if rng.random(2) == 0 {
        (edge, along)
    } else {
        (along, edge)
    };
    crate::movement::Point::new(
        i16::try_from(x).unwrap_or(i16::MAX),
        i16::try_from(y).unwrap_or(i16::MAX),
    )
}

/// Fly the Mystery Traders a year, and see what becomes of them.
///
/// `MoveThings` (`10b0:1af7`), before production.
///
/// **On the way**, one year in twenty-five it changes its mind: it always
/// speeds up, and one time in three it also picks a new destination. A Trader
/// already at warp 13 or better has stopped changing its mind. Then it covers
/// the square of its warp toward wherever it is going.
///
/// **On arrival** (`10b0:1da3`) it is finished with that pass, and what happens
/// then depends on whether the galaxy has another Trader in it:
///
/// * another Trader exists — this one **leaves for good**;
/// * it is the only one — a coin flip decides between leaving and staying.
///
/// A Trader that stays **makes another pass**: it sits where it arrived, picks
/// a fresh destination, and its warp becomes `max(warp − 2, 6) + 1` — slower
/// than the pass it just finished, but never below warp 7, and warp 6 or 7
/// actually comes back *faster*. It does not move that year. Everybody is told,
/// which is the one thing about the Trader every player learns at once.
///
/// Returns what happened, per Trader, as `(id, event)`.
fn move_traders(state: &mut GameState, rng: &mut Rng) -> Vec<(u16, crate::wormhole::Event)> {
    use crate::message::{id, Message};
    use crate::wormhole::Event;

    // A universe size class, from the planet count the game info gave us; the
    // original reads `game.mdSize` directly.
    let size = i32::from(state.galaxy_planets).max(1);
    let players = state.players.len();
    let mut events = Vec::new();

    // A Trader that leaves is taken out of the galaxy there and then, which is
    // what the original does — so a second Trader arriving the same year may
    // find itself alone by the time its turn comes.
    let mut index = 0;
    while index < state.traders.len() {
        // Both of the Trader's messages go to every player at once: what it is
        // doing is the one thing the whole galaxy learns together.
        let announce = |state: &mut GameState, id: u16, trader: u16| {
            for player in 0..players {
                state.messages.push(Message {
                    player,
                    id,
                    object: -6,
                    params: vec![trader as i16],
                });
            }
        };

        let trader = &mut state.traders[index];
        let name = trader.id;

        // The course change: never for a Trader already at warp 13.
        if trader.warp <= 12 && rng.random(25) == 0 {
            if rng.random(3) == 0 {
                trader.destination = trader_destination(size, rng);
            }
            trader.warp = (trader.warp + 1) & 0x0F;
            events.push((name, Event::ChangedCourse));
            announce(state, id::TRADER_CHANGED_COURSE, name);
        }

        let range = state.traders[index].range();
        let target = state.traders[index].destination;
        let from = state.traders[index].position;
        if crate::movement::distance(from, target) > f64::from(range) {
            state.traders[index].position = crate::movement::advance(from, target, range);
            index += 1;
            continue;
        }

        // Arrived, which ends the pass. Another Trader in the galaxy means
        // this one is done; being the only one earns it a coin flip.
        let alone = state.traders.len() == 1;
        if alone && rng.random(2) != 0 {
            let trader = &mut state.traders[index];
            trader.position = target;
            // Slower than the pass it just flew, but never crawling.
            trader.warp = trader.warp.saturating_sub(2).max(6);
            trader.destination = trader_destination(size, rng);
            trader.warp = (trader.warp + 1) & 0x0F;
            events.push((name, Event::AnotherPass));
            announce(state, id::TRADER_ANOTHER_PASS, name);
            index += 1;
            continue;
        }

        events.push((name, Event::Departed));
        let trader = state.traders.remove(index);
        orders_lose_their_trader(state, &trader);
    }
    events
}

/// Turn a waypoint that was following a departed Trader into a plain position.
///
/// The original does this per player as it writes their file (`save.c`), from
/// what that player can see: a waypoint aimed at a `THING` that has gone, or
/// that this player can no longer see, becomes a bare coordinate and the player
/// is told. Only the *gone* half is modelled here — this engine has no
/// per-player visibility pass, so a Trader that is merely out of scanner range
/// keeps everybody's orders pointed at it.
fn orders_lose_their_trader(state: &mut GameState, trader: &crate::wormhole::MysteryTrader) {
    use crate::message::{fleet_object, id, Message};

    let mut told = Vec::new();
    for fleet in &mut state.fleets {
        let Ok(owner) = usize::try_from(fleet.owner) else {
            continue;
        };
        for waypoint in fleet.waypoints.iter_mut().skip(1) {
            if waypoint.target_class != GROBJ_THING
                || !names_thing(waypoint.target, ITH_TRADER, trader.id)
            {
                continue;
            }
            // Where it last was, which is where the orders now point.
            waypoint.position = trader.position;
            waypoint.target = None;
            waypoint.target_class = GROBJ_POSITION;
            told.push((owner, fleet.id));
        }
    }
    for (player, fleet) in told {
        state.messages.push(Message {
            player,
            id: id::TRADER_VANISHED,
            object: fleet_object(fleet),
            params: vec![fleet as i16],
        });
    }
}

/// `NoAutoTrackFleet` (`1080:1d32`): a fleet is about to vanish through a
/// gate or a wormhole, so every other player's leg aimed at it is marked
/// `fNoAutoTrack` and fixed where the fleet is now — the chase ends there.
fn no_auto_track_fleet(state: &mut GameState, quarry: usize) {
    let target = (state.fleets[quarry].owner, state.fleets[quarry].id);
    let at = state.fleets[quarry].position;
    for index in 0..state.fleets.len() {
        let chaser = &state.fleets[index];
        if chaser.owner == target.0 || chaser.waypoints.len() < 2 {
            continue;
        }
        let key = (chaser.owner, chaser.id);
        for leg in 1..state.fleets[index].waypoints.len() {
            let w = &mut state.fleets[index].waypoints[leg];
            if w.target_class == crate::fleet::grobj::FLEET
                && w.target
                    .is_some_and(|t| crate::orders::split_fleet_id(t) == target)
            {
                w.position = at;
                state.no_auto_track.insert((key.0, key.1, leg));
            }
        }
    }
}

/// Take a fleet that has just reached a wormhole out of the far end.
///
/// `MoveFleets` (`10b0:4ce4`), immediately after the leg is flown: a fleet
/// whose waypoint named a wormhole and that actually arrived is moved to the
/// partner end, and both ends are marked as travelled by that player — the far
/// one becomes visible to them too, which is how the other end of a wormhole is
/// discovered. The waypoint follows the fleet, so the next leg starts from
/// where it came out.
///
/// Returns the two ends, near then far.
fn traverse_wormhole(state: &mut GameState, index: usize) -> Option<(u16, u16)> {
    let fleet = &state.fleets[index];
    let owner = usize::try_from(fleet.owner).ok()?;
    let waypoint = fleet.waypoints.first()?;
    if waypoint.target_class != GROBJ_THING {
        return None;
    }
    let target = waypoint.target;
    let near = state
        .wormholes
        .iter()
        .position(|w| names_thing(target, ITH_WORMHOLE, w.id))?;
    let partner = state.wormholes[near].partner & 0x01FF;
    let far = state.wormholes.iter().position(|w| w.id == partner)?;

    let bit = 1u16 << (owner & 0x0F);
    no_auto_track_fleet(state, index);
    state.wormholes[near].traversed_by |= bit;
    state.wormholes[far].traversed_by |= bit;
    state.wormholes[far].detected_by |= bit;
    let out = state.wormholes[far].position;

    let fleet = &mut state.fleets[index];
    fleet.position = out;
    if let Some(here) = fleet.waypoints.first_mut() {
        here.position = out;
    }
    Some((state.wormholes[near].id, state.wormholes[far].id))
}

/// Which design a message should name a fleet by, and whether it is mixed.
///
/// `IshdefPrimaryFromLpfl` (`util.c`): the design with the most ships aboard,
/// later slots winning only outright. The original also counts a fuel
/// transport as one ship fewer, so that a tanker escorting warships does not
/// give the fleet its name; that refinement needs the hull table and is not
/// applied here.
fn primary_design(fleet: &Fleet) -> (u8, bool) {
    let mut best = (0u8, 0i32);
    for stack in &fleet.stacks {
        if stack.count > best.1 {
            best = (stack.design, stack.count);
        }
    }
    (
        best.0,
        fleet.stacks.iter().filter(|s| s.count > 0).count() > 1,
    )
}

/// Trade with the Mystery Trader.
///
/// `DoThingInteractions(1)` (`1110:0b3a`). Every fleet that has come to rest on
/// the Trader is considered in turn. A fleet carrying less than
/// [`crate::wormhole::TRADE_GOODS`] kilotons of minerals is turned away — and
/// told so once, on the year it arrives. A fleet carrying enough is **kept**:
/// the Trader absorbs it, and in exchange gives
///
/// * the technology it is carrying, if the player does not already have it;
/// * otherwise technology levels, [`crate::wormhole::tech_levels`] of them,
///   granted outright through [`crate::research::grant_level`];
/// * otherwise, for a player who has already researched everything, a
///   one-in-five chance of some part they are missing, and nothing at all the
///   rest of the time.
///
/// Each Trader trades once with each player, which is what its `grbitPlr` mask
/// records.
#[allow(clippy::type_complexity)]
fn trade_with_trader(
    state: &mut GameState,
    rng: &mut Rng,
) -> (
    Vec<(u16, crate::wormhole::Gift)>,
    Vec<(i16, crate::wormhole::Gift)>,
) {
    let mut fleets = Vec::new();
    let mut planets = Vec::new();
    // One Trader at a time, fleets and then planets, which is the order the
    // original visits them in and so the order they draw from the generator.
    for trader in 0..state.traders.len() {
        let at = state.traders[trader].position;
        let carried = state.traders[trader].part;
        fleets.extend(trade_with_one(state, trader, at, carried, rng));
        planets.extend(ai_trades_from_a_planet(state, trader, rng));
    }
    (fleets, planets)
}

/// What a computer player gets for having a planet near the Mystery Trader.
///
/// `DoThingInteractions` (`1110:1631`) runs a second loop, over **planets**,
/// after the one over fleets. A computer player of skill 2 or better with a
/// starbase planet within a hundred light years of the Trader trades with it
/// where it stands — no fleet, no journey, nothing for anybody else to see.
/// It is the AI's substitute for the errand a person has to run.
///
/// The terms are the same shape as a fleet's and the prices are different:
///
/// * the planet must hold **3,500 kT** of minerals for a skill-2 player, or
///   **5,000** for a skill-3 one;
/// * a part the player has not had costs the planet **everything on its
///   surface**;
/// * failing that — no part carried, or fifty draws all held — six technology
///   levels, one at a time into whichever field is furthest behind, and the
///   planet pays only the threshold. This is refused outright to a player
///   within six levels of the ceiling.
///
/// Either way the Trader marks the player off, so this and a fleet meeting are
/// the same one chance.
///
/// Returns `(planet id, gift)` for each trade made.
fn ai_trades_from_a_planet(
    state: &mut GameState,
    trader: usize,
    rng: &mut Rng,
) -> Vec<(i16, crate::wormhole::Gift)> {
    use crate::wormhole::Gift;

    /// How near the Trader a planet has to be, squared.
    const REACH2: i64 = 10_000;
    /// The six technology levels a trade is worth.
    const LEVELS: i16 = 6;

    let at = state.traders[trader].position;
    let carried = state.traders[trader].part;
    let mut done = Vec::new();

    for index in 0..state.planets.len() {
        let planet = &state.planets[index];
        let (Some(owner), Some(position)) = (planet.owner, planet.position) else {
            continue;
        };
        let Ok(owner) = usize::try_from(owner) else {
            continue;
        };
        if !planet.starbase {
            continue;
        }
        // Only a computer player, and only a capable one.
        let Some(crate::ai::Control::Computer { skill_bits, .. }) =
            state.players.get(owner).map(|p| p.control)
        else {
            continue;
        };
        if skill_bits <= 1 {
            continue;
        }
        let bit = 1u16 << (owner & 0x0F);
        if state.traders[trader].detected_by & bit != 0 {
            continue;
        }
        // The original stops the scan at the first planet more than a hundred
        // light years east of the Trader, which it can do because the `.xy`
        // stores each planet's x as an offset from the one before and so holds
        // them in ascending order. Testing every planet comes to the same
        // thing.
        let dx = i64::from(position.x) - i64::from(at.x);
        let dy = i64::from(position.y) - i64::from(at.y);
        if dx * dx + dy * dy > REACH2 {
            continue;
        }

        let held = state.planets[index].surface_min.iter().sum::<i32>();
        let price = if skill_bits == 2 { 3_500 } else { 5_000 };
        if held < price {
            continue;
        }

        // A part the player has not had, or fifty draws looking for one.
        let mut giving = carried;
        let mut tries = 50;
        while giving != 0 && giving & state.players[owner].trader_parts != 0 && tries > 0 {
            tries -= 1;
            giving = 1 << rng.random(13);
        }
        let gift = if giving != 0 && tries > 0 {
            state.players[owner].trader_parts |= giving;
            Some((Gift::Part(giving), held))
        } else {
            // Technology instead, unless there is barely any left to give.
            let player = &mut state.players[owner];
            let cap: i16 = if player.crippled { 10 } else { 26 };
            let total: i16 = player.research.levels.iter().map(|l| i16::from(*l)).sum();
            if total >= cap * 6 - LEVELS {
                None
            } else {
                for _ in 0..LEVELS {
                    let field = (0..crate::research::TECH_FIELDS)
                        .min_by_key(|f| player.research.levels[*f])
                        .unwrap_or(0);
                    player.research.levels[field] += 1;
                }
                Some((Gift::Tech(LEVELS), price))
            }
        };
        let Some((gift, mut owed)) = gift else {
            continue;
        };

        state.traders[trader].detected_by |= bit;
        // Paid out of the surface stockpile, germanium first.
        let planet = &mut state.planets[index];
        for kind in (0..3).rev() {
            if owed <= 0 {
                break;
            }
            let take = planet.surface_min[kind].min(owed);
            planet.surface_min[kind] -= take;
            owed -= take;
        }
        done.push((planet.id, gift));
    }
    done
}

/// Trade with one Mystery Trader; see [`trade_with_trader`].
fn trade_with_one(
    state: &mut GameState,
    trader: usize,
    at: crate::movement::Point,
    carried: u16,
    rng: &mut Rng,
) -> Vec<(u16, crate::wormhole::Gift)> {
    use crate::message::{fleet_name_word, fleet_object, id, Message};
    use crate::wormhole::{part, tech_levels, Gift, TRADE_GOODS};

    let mut done = Vec::new();
    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        if fleet.stacks.is_empty() || fleet.position != at {
            continue;
        }
        let Ok(owner) = usize::try_from(fleet.owner) else {
            continue;
        };
        if state.players.len() <= owner {
            continue;
        }
        let fleet_id = fleet.id;
        let object = fleet_object(fleet_id);
        let cargo: i32 = fleet.cargo.minerals.iter().sum();

        if cargo < TRADE_GOODS {
            // `fHereAllTurn`: a fleet that has been sitting here is not told
            // again. This engine records a fleet that moved by leaving its
            // warp set, which is the test remote mining and mine laying make.
            if fleet.warp.is_some() {
                state.messages.push(Message {
                    player: owner,
                    id: id::TRADER_REFUSED,
                    object,
                    params: vec![fleet_id as i16, 0],
                });
                done.push((fleet_id, Gift::Refused));
            }
            continue;
        }

        let bit = 1u16 << (owner & 0x0F);
        if state.traders[trader].detected_by & bit != 0 {
            state.messages.push(Message {
                player: owner,
                id: id::TRADER_ALREADY_MET,
                object,
                params: vec![fleet_id as i16, 0],
            });
            done.push((fleet_id, Gift::AlreadyMet));
            continue;
        }

        // From here the trade happens: the Trader marks the player off, and
        // the fleet is gone. A message about a fleet that no longer exists
        // names it rather than pointing at it.
        state.traders[trader].detected_by |= bit;
        let (design, mixed) = primary_design(&state.fleets[index]);
        let named = fleet_name_word(fleet_id, design, mixed);
        // `FRemovePlayerMessage`: a fleet the Trader has taken did not finish
        // its orders, whatever the movement pass concluded.
        state
            .messages
            .retain(|m| !(m.player == owner && m.id == id::ORDERS_COMPLETE && m.object == object));
        let fleet = &mut state.fleets[index];
        fleet.stacks.clear();
        fleet.cargo = crate::fleet::Cargo::default();
        fleet.waypoints.clear();

        let player = &state.players[owner];
        let held = player.trader_parts;
        // A shareware game stops at level 10 where a registered one goes to 26.
        let cap = if player.crippled { 10 } else { 26 };
        let levels: i16 = player.research.levels.iter().map(|l| i16::from(*l)).sum();
        let maxed = player.research.levels.iter().all(|l| i16::from(*l) >= cap);

        // The Trader's own cargo first: a part this player has not had.
        let has_new_part = carried != 0 && carried & held == 0;
        if !has_new_part && !maxed {
            // Nothing to hand over, but there is still research to buy.
            let count = tech_levels(cargo, levels);
            let message = if held & part::ALL == part::ALL {
                id::TRADER_GAVE_TECH_AGAIN
            } else {
                id::TRADER_GAVE_TECH
            };
            state.messages.push(Message {
                player: owner,
                id: message,
                object: -1,
                params: vec![named, count],
            });
            let mut given = 0;
            for _ in 0..count {
                let player = &mut state.players[owner];
                // Three years in four a field at random, otherwise — and
                // whenever that field is already at the ceiling — the one the
                // player is furthest behind in.
                let mut field = usize::from(rng.random(6).unsigned_abs()).min(5);
                if rng.random(4) >= 3 || i16::from(player.research.levels[field]) >= cap {
                    field = (0..6)
                        .min_by_key(|f| player.research.levels[*f])
                        .unwrap_or(0);
                    if i16::from(player.research.levels[field]) >= cap {
                        break;
                    }
                }
                let race = player.race.clone();
                crate::research::grant_level(&mut player.research, &race, state.slow_tech, field);
                given += 1;
            }
            done.push((fleet_id, Gift::Tech(given)));
            continue;
        }
        if !has_new_part {
            // Everything researched. One year in five the Trader finds
            // something in the hold after all.
            if rng.random(5) != 0 {
                state.messages.push(Message {
                    player: owner,
                    id: id::TRADER_GAVE_NOTHING,
                    object: -1,
                    params: vec![named, 0],
                });
                done.push((fleet_id, Gift::Nothing));
                continue;
            }
        }

        // Pick a part. The Trader's own, if the player has not had it;
        // otherwise up to twenty-five draws for one they have not.
        let mut giving = carried;
        if giving == 0 {
            giving = 1 << rng.random(13);
        }
        let mut tries = 25;
        while giving & held != 0 && tries > 0 {
            tries -= 1;
            giving = 1 << rng.random(13);
        }
        if tries <= 0 {
            giving = part::LIFEBOAT;
        }

        if giving == part::LIFEBOAT {
            // Not a part at all: a ship of the Trader's own.
            if let Some(gift) = give_trader_ship(state, owner, at, named, rng) {
                done.push((fleet_id, gift));
            }
            continue;
        }

        state.players[owner].trader_parts |= giving;
        let (message, item) = crate::wormhole::part_gift(giving);
        state.messages.push(Message {
            player: owner,
            id: message,
            object: item as i16,
            params: vec![named, 0],
        });
        done.push((fleet_id, Gift::Part(giving)));
    }

    done
}

/// The ships the Mystery Trader gives when it has no technology left.
///
/// `DoThingInteractions` (`1110:1180`), the `grbitTraderLifeboat` arm. Twenty-
/// five draws that all come back as parts the player already holds mean there
/// is nothing left to hand over, so the Trader gives **ships** instead — one of
/// its own three designs, which are the last three entries of the game's
/// built-in template table and are not otherwise buildable by anybody.
///
/// Which design, and how many:
///
/// ```text
/// offset = Random(4 − (turn > 100))       0 a quarter of the time
/// if offset > 0: offset = Random(2) + 1   otherwise the Scout or the Probe
/// ships  = Random(3) == 0 ? 2 : 1
/// if turn > 100 and not a single-player game: ships += Random(turn / 100 + 1)
/// ships = min(ships, 5)
/// if offset > 0: ships += Random(ships + 1)
/// ```
///
/// So the Lifeboat comes alone or in pairs and the other two can come in
/// numbers, and a long game gives more of them — except in a single-player
/// game, which is held to the early-game figures.
///
/// The ships need a design slot. An identical design the player already has is
/// reused (`IshFindSimilarDesign`, `1038:7c5e`: same hull, same slot counts,
/// and the same item and category in every slot that is filled); failing that
/// the first free slot is taken. With no slot free, or with 512 fleets already,
/// the Trader is reported as having tried and failed.
///
/// An AI player is given nothing at all, and not told either.
///
/// Returns `None` when nothing happened, which is only the AI case.
fn give_trader_ship(
    state: &mut GameState,
    owner: usize,
    at: crate::movement::Point,
    named: i16,
    rng: &mut Rng,
) -> Option<crate::wormhole::Gift> {
    use crate::message::{id, Message};
    use crate::startup::{ship::MT_LIFEBOAT, SHIPS};
    use crate::wormhole::Gift;

    /// A player may not have more fleets than this.
    const MAX_FLEETS: usize = 0x200;
    /// Ship designs occupy slots 0..16; starbases follow them.
    const SHIP_SLOTS: usize = crate::startup::FIRST_STARBASE_SLOT as usize;

    if matches!(
        state.players.get(owner).map(|p| &p.control),
        Some(crate::ai::Control::Computer { .. })
    ) {
        return None;
    }

    let turn = i32::from(state.turn);
    let late = i16::from(turn > 100);
    let mut offset = rng.random(4 - late);
    if offset > 0 {
        offset = rng.random(2) + 1;
    }
    let template = MT_LIFEBOAT + usize::try_from(offset).unwrap_or(0).min(2);
    let design = SHIPS[template].design();

    // A design of the player's own that is already the same ship, or the first
    // slot they have not used.
    let designs = state.designs.get(owner).map_or(&[][..], Vec::as_slice);
    let slot = designs
        .iter()
        .take(SHIP_SLOTS)
        .position(|d| same_design(d, &design))
        .or_else(|| (0..SHIP_SLOTS).find(|i| designs.get(*i).is_none_or(|d| d.hull_id < 0)));
    let fleets = state
        .fleets
        .iter()
        .filter(|f| usize::try_from(f.owner).is_ok_and(|o| o == owner))
        .count();

    let (Some(slot), true) = (slot, fleets < MAX_FLEETS) else {
        state.messages.push(Message {
            player: owner,
            id: id::TRADER_TRIED_SHIP,
            object: -1,
            params: vec![named, 0],
        });
        return Some(Gift::ShipRefused);
    };

    let mut ships = if rng.random(3) == 0 { 2 } else { 1 };
    if turn > 100 && !state.single_player {
        ships += rng.random(i16::try_from(turn / 100 + 1).unwrap_or(i16::MAX));
    }
    ships = ships.min(5);
    if offset > 0 {
        ships += rng.random(ships + 1);
    }
    let ships = i32::from(ships);

    // Install the design, unless the slot already held this very ship.
    if state.designs.len() <= owner {
        state.designs.resize_with(owner + 1, Vec::new);
    }
    let designs = &mut state.designs[owner];
    if designs.len() <= slot {
        designs.resize_with(slot + 1, || crate::design::ShipDesign {
            name: String::new(),
            picture: 0,
            stored_armor: 0,
            obsolete: false,
            designed: 0,
            built: 0,
            hull_id: -1,
            slots: Vec::new(),
        });
    }
    if !same_design(&designs[slot], &design) {
        designs[slot] = design;
    }
    let designs = designs.clone();

    let owner_id = i16::try_from(owner).unwrap_or(0);
    let id = next_fleet_id(state, owner_id);
    let mut fleet = Fleet {
        name: None,
        repeat_orders: false,
        direction: None,
        id,
        owner: owner_id,
        position: at,
        orbiting: None,
        stacks: vec![crate::fleet::ShipStack {
            design: u8::try_from(slot).unwrap_or(0),
            count: ships,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: crate::fleet::Cargo::default(),
        battle_plan: 0,
        // `fHereAllTurn`: it has not moved, so nothing this year treats it as
        // having just arrived.
        warp: None,
        waypoints: vec![crate::fleet::Waypoint {
            position: at,
            target: None,
            target_class: 4,
            warp: 0,
            task: stars_formats::task::NONE,
            transport: None,
            task_data: Vec::new(),
        }],
    };
    // Full tanks.
    fleet.cargo.fuel = fleet.fuel_capacity(&designs);
    state.fleets.push(fleet);

    state.messages.push(Message {
        player: owner,
        id: id::TRADER_GAVE_SHIP,
        object: crate::message::fleet_object(id),
        params: vec![named, i16::try_from(ships).unwrap_or(i16::MAX)],
    });
    Some(Gift::Ship {
        design: template,
        ships,
    })
}

/// Whether two designs are the same ship, whatever they are called.
///
/// `IshFindSimilarDesign` (`1038:7c5e`) compares the hull, the number of slots
/// and then each slot: the **count** always, and the item and its category
/// whenever the slot is filled. An empty slot matches any other empty slot, and
/// the design's name is not part of it.
fn same_design(design: &crate::design::ShipDesign, other: &crate::design::ShipDesign) -> bool {
    design.hull_id >= 0
        && design.hull_id == other.hull_id
        && design.slots.len() == other.slots.len()
        && design.slots.iter().zip(&other.slots).all(|(a, b)| {
            a.count == b.count && (a.count == 0 || (a.item == b.item && a.category == b.category))
        })
}

/// Let the wormholes wander.
///
/// `MoveThings` (`10b0:194c`), after production. Each end rolls against
/// [`crate::wormhole::jump_chance`]; a **jump** puts it anywhere in the galaxy
/// and forgets who had seen it, while otherwise it merely drifts within twelve
/// light years of where it was. Either way the original tries up to a hundred
/// positions and takes the first that scores nothing against
/// [`crate::wormhole::position_score`], or the best it found.
///
/// Returns the ends that jumped.
fn move_wormholes(state: &mut GameState, rng: &mut Rng) -> Vec<u16> {
    if state.wormholes.is_empty() {
        return Vec::new();
    }
    let size = i32::from(state.galaxy_size).max(0);
    let span = size * 400;
    let planets: Vec<crate::movement::Point> = state
        .planets
        .iter()
        .chain(state.known_planets.iter())
        .filter_map(|p| p.position)
        .collect();
    let fleets: Vec<crate::movement::Point> = state.fleets.iter().map(|f| f.position).collect();

    let mut jumped = Vec::new();
    for index in 0..state.wormholes.len() {
        let others: Vec<(u16, crate::movement::Point)> = state
            .wormholes
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, w)| (w.id, w.position))
            .collect();

        let hole = &state.wormholes[index];
        let base = hole.position;
        let chance = crate::wormhole::jump_chance(hole.stability, hole.years_still);
        let jumping = i32::from(rng.random(100)) < chance;
        let partner = hole.partner & 0x01FF;

        let mut best: Option<(u8, crate::movement::Point)> = None;
        for _ in 0..100 {
            let at = if jumping {
                crate::movement::Point::new(
                    i16::try_from(
                        i32::from(rng.random(i16::try_from(span + 400).unwrap_or(i16::MAX))) + 1000,
                    )
                    .unwrap_or(i16::MAX),
                    i16::try_from(
                        i32::from(rng.random(i16::try_from(span + 400).unwrap_or(i16::MAX))) + 1000,
                    )
                    .unwrap_or(i16::MAX),
                )
            } else {
                crate::movement::Point::new(
                    base.x + rng.random(25) - 12,
                    base.y + rng.random(25) - 12,
                )
            };
            if at == base {
                continue;
            }
            let score =
                crate::wormhole::position_score(at, size, partner, &others, &planets, &fleets);
            if score == 0 {
                best = Some((0, at));
                break;
            }
            if best.is_none_or(|(worst, _)| score < worst) {
                best = Some((score, at));
            }
        }

        let hole = &mut state.wormholes[index];
        if let Some((_, at)) = best {
            hole.position = at;
        }
        if jumping {
            hole.years_still = 0;
            // Nobody knows where it went.
            hole.detected_by = 0;
            jumped.push(hole.id);
        } else {
            hole.years_still = hole.years_still.saturating_add(1);
        }
    }
    jumped
}

/// `FuelFleets` (`10b0:2efa`): fill the tank of every fleet sitting at a
/// starbase that can fill it.
///
/// The planet has to be owned, have a starbase, and be the fleet's own or
/// belong to a player who regards the fleet's owner as a **friend**; and
/// the base's hull has to have a dock (`wtCargoMax != 0`), which an Orbital
/// Fort has not. Fleets elsewhere gain what their fuel transports (two
/// hundred a year per hull 25 or 26) and Anti-matter Generators make, up to
/// their capacity.
fn fuel_fleets(state: &mut GameState) {
    use crate::components::slot;
    use crate::relations::{regard, Relation};

    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        let Ok(owner) = usize::try_from(fleet.owner) else {
            continue;
        };
        let designs = state.designs.get(owner).cloned().unwrap_or_default();
        let capacity = fleet.fuel_capacity(&designs);

        let dock = fleet
            .orbiting
            .and_then(|id| i16::try_from(id).ok())
            .and_then(|id| state.planets.iter().find(|p| p.id == id))
            .filter(|planet| planet.starbase)
            .and_then(|planet| planet.owner.map(|host| (planet, host)))
            .filter(|(_, host)| {
                *host == fleet.owner
                    || usize::try_from(*host)
                        .is_ok_and(|h| regard(state, h, owner) == Relation::Friend)
            })
            .is_some_and(|(planet, host)| {
                let base = planet
                    .starbase_design
                    .map(usize::from)
                    .map(|s| usize::from(crate::startup::FIRST_STARBASE_SLOT) + s)
                    .and_then(|s| {
                        usize::try_from(host)
                            .ok()
                            .and_then(|h| state.designs.get(h))
                            .and_then(|d| d.get(s))
                    });
                base.and_then(crate::design::ShipDesign::hull)
                    .is_some_and(|hull| hull.cargo_max != 0)
            });
        if dock {
            state.fleets[index].cargo.fuel = capacity;
            continue;
        }

        // What the fleet makes for itself.
        let mut made = 0;
        for stack in &fleet.stacks {
            let Some(design) = designs.get(usize::from(stack.design)) else {
                continue;
            };
            if design.hull_id == 25 || design.hull_id == 26 {
                made += 200 * stack.count;
            }
            for fitted in &design.slots {
                if fitted.category == slot::SPECIAL_E && fitted.item == 16 {
                    made += 50 * i32::from(fitted.count) * stack.count;
                }
            }
        }
        if made > 0 {
            let fleet = &mut state.fleets[index];
            fleet.cargo.fuel = (fleet.cargo.fuel + made).min(capacity);
        }
    }
}

/// A component's word for a message object: bits 14 and 15 set, the
/// category's index in bits 8..=11 and the item in the low byte
/// (`UpdateResearchStatus`, `10b8:80fe`), which Goto opens the
/// Technology Browser on.
fn part_word(category_index: u16, item: usize) -> i16 {
    (0xc000 | (category_index << 8) | (u16::try_from(item).unwrap_or(0) & 0xff)) as i16
}

/// A count as a message parameter carries it.
fn n_i16(count: i32) -> i16 {
    i16::try_from(count).unwrap_or(i16::MAX)
}

/// The lowest fleet number a player is not already using.
///
/// Fleet numbers are per player and are reused once a fleet is gone, which is
/// why this looks for the first gap rather than counting. `LpflNew`
/// (`1038:300c`) walks the player's fleets in order from a count of
/// `0xffff` and stops at the first whose number is not the last plus one, so
/// the numbers start at **zero** — the tutorial's Armed Probe #1 is fleet 0,
/// and the Teamster its Stove Top builds in 2413, after that probe is lost,
/// is Teamster #1 in its place.
#[must_use]
pub fn next_fleet_id(state: &GameState, owner: i16) -> u16 {
    let mut used: Vec<u16> = state
        .fleets
        .iter()
        .filter(|f| f.owner == owner)
        .map(|f| f.id)
        .collect();
    used.sort_unstable();
    let mut id = 0;
    for taken in used {
        if taken == id {
            id += 1;
        } else if taken > id {
            break;
        }
    }
    id
}

/// Run a planet's production queue for one year.
///
/// Items are taken in order, each spending from what is left. An item that
/// completes everything it wanted is dropped; one that is only part-built keeps
/// its progress for next year.
///
/// Three rules that are easy to miss, all from `Produce` (`10b8:0000`) and
/// `CBuildProdItem` (`10b8:0c92`):
///
/// * **The queue stops** at the first ordinary item that could not be finished
///   (`if (mdStatus > 4)`). Everything behind it waits — which is the manual's
///   "your people will not work to complete the original item until the new
///   item you placed in the queue is complete" (p. 7-1). An **auto-build** item
///   that could not finish does *not* stop it.
/// * An auto-build item's "up to N" is a **target, not a countdown**. It is
///   clamped to the year's cap before building and its stored count is left
///   alone, so the entry means the same thing next year.
/// * **Auto alchemy** in front of another item is skipped and marks the next
///   item as alchemy-assisted; only as the last item in the queue does it run,
///   and then it runs flat out.
///
/// Returns what was completed, as `(item id, count)` pairs, and separately the
/// ships finished, as `(design slot, count)`.
///
/// `designs` is the owning player's design list, indexed by slot; a ship entry
/// naming a slot the list does not hold is skipped rather than guessed at.
#[allow(clippy::too_many_arguments)]
fn run_queue(
    planet: &mut Planet,
    race: &crate::Race,
    designs: &[crate::design::ShipDesign],
    tech: [u8; 6],
    trader_parts: u16,
    tutorial: bool,
    available: &mut [i32; COST_PARTS],
    ships_built: &mut Vec<(u8, i32)>,
) -> Vec<(u16, i32)> {
    /// What auto alchemy's count becomes when it is the last item in the
    /// queue, and so is allowed to run flat out.
    const ALCHEMY_FLAT_OUT: i32 = 1020;

    let mut completed: Vec<(u16, i32)> = Vec::new();
    let mut queue = std::mem::take(&mut planet.queue);
    let last = queue.len().saturating_sub(1);
    // The part-built unit an auto-build entry leaves behind, as a concrete
    // entry to go in front of it: `(index of the auto entry, entry)`.
    let mut leftovers: Vec<(usize, crate::production::QueueItem)> = Vec::new();
    // What one unit of alchemy costs, and whether the entry just passed over
    // was auto alchemy — which is what lets the next item ask for some.
    let alchemy_cost = planetary_item_cost(item::ALCHEMY, race, tutorial).map(|c| c.resources);
    let mut alchemy: Option<i32> = None;

    // The index is the point: it says whether the entry is the last in the
    // queue, which is what decides how auto alchemy behaves.
    #[allow(clippy::needless_range_loop)]
    for index in 0..queue.len() {
        // Auto alchemy only runs as the last item in the queue; anywhere else
        // it stands aside and lends a hand to whatever follows.
        let alchemy_last =
            !queue[index].ship && queue[index].item == item::AUTO_ALCHEMY && index == last;
        if !queue[index].ship && queue[index].item == item::AUTO_ALCHEMY && !alchemy_last {
            alchemy = alchemy_cost;
            continue;
        }

        let entry = &mut queue[index];
        if entry.ship {
            // A ship costs its design; anything finished joins a fleet at the
            // planet. A design we do not hold is left alone rather than guessed.
            let Some(slot) = u8::try_from(entry.item).ok() else {
                continue;
            };
            if !designs
                .get(usize::from(slot))
                .is_some_and(|d| d.hull_id >= 0)
            {
                continue;
            }
            let who = crate::parts::Builder {
                race,
                levels: tech,
                researching: 0,
                trader_parts,
                starbase: slot >= crate::startup::FIRST_STARBASE_SLOT,
                tutorial,
            };
            // A starbase over a planet that has one is priced as an
            // upgrade (`GetProductionCosts`).
            let Some(cost) =
                crate::production::design_cost_at(planet, designs, usize::from(slot), &who)
            else {
                continue;
            };
            let outcome = build_item(
                cost,
                entry.count,
                entry.completion,
                available,
                false,
                alchemy.take(),
            );
            if outcome.alchemised > 0 {
                completed.push((item::ALCHEMY, outcome.alchemised));
            }
            if outcome.built > 0 {
                ships_built.push((slot, outcome.built));
            }
            entry.count = outcome.remaining;
            entry.completion = outcome.completion_pct;
            if outcome.status.stops_the_queue() {
                break;
            }
            continue;
        }
        // `GetProductionCosts`: the installations, and the packets, the
        // planetary scanners and the Genesis Device, which are costed as
        // parts and miniaturised like any other component.
        let who = crate::parts::Builder {
            race,
            levels: tech,
            researching: 0,
            trader_parts,
            starbase: false,
            tutorial,
        };
        let Some(cost) = crate::production::item_cost(entry.item, &who) else {
            continue;
        };
        let auto = entry.is_auto();

        // An auto-build item builds up to its cap for the year, not up to the
        // figure the player typed — and that figure survives the year.
        // As the last item in the queue, auto alchemy runs flat out — the
        // original overwrites its count with 1020 rather than reading it.
        let wanted = if alchemy_last {
            ALCHEMY_FLAT_OUT
        } else if auto {
            entry
                .count
                .min(auto_build_cap(planet, race, tech, designs, entry.item))
        } else {
            entry.count
        };

        let outcome = build_item(
            cost,
            wanted,
            entry.completion,
            available,
            auto,
            alchemy.take(),
        );
        if outcome.alchemised > 0 {
            completed.push((item::ALCHEMY, outcome.alchemised));
        }
        if outcome.built > 0 {
            match item::auto_builds(entry.item).unwrap_or(entry.item) {
                item::MINE => planet.mines += i16::try_from(outcome.built).unwrap_or(0),
                item::FACTORY => planet.factories += i16::try_from(outcome.built).unwrap_or(0),
                item::DEFENSE => planet.defenses += i16::try_from(outcome.built).unwrap_or(0),
                item::TERRAFORM => {
                    for _ in 0..outcome.built {
                        if !crate::terraform::terraform_one_step(planet, race, tech) {
                            break;
                        }
                    }
                }
                _ => {}
            }
            completed.push((entry.item, outcome.built));
        }
        let entry = &mut queue[index];
        if auto {
            // An auto-build entry keeps no progress of its own. What was
            // part-paid this year becomes a **concrete** entry for one of the
            // item, in front of the auto entry, which is how a real file
            // shows it — `Factory ×1 at 87%` ahead of `Factories (Auto Build)
            // 100` — and why the tutorial finds three entries in a queue it
            // put two things into.
            if outcome.completion_pct > 0 {
                let concrete = item::auto_builds(entry.item).unwrap_or(entry.item);
                leftovers.push((
                    index,
                    crate::production::QueueItem {
                        count: 1,
                        item: concrete,
                        ship: false,
                        completion: outcome.completion_pct,
                    },
                ));
            }
            entry.completion = 0;
        } else {
            entry.count = outcome.remaining;
            entry.completion = outcome.completion_pct;
        }
        if outcome.status.stops_the_queue() {
            break;
        }
    }

    // Drop anything finished; an auto-build entry stays even at zero, because
    // it becomes buildable again as the planet grows. The part-built units
    // go in ahead of the auto entries that started them, last first so the
    // indices stay good.
    for (index, leftover) in leftovers.into_iter().rev() {
        queue.insert(index, leftover);
    }
    queue.retain(|e| e.count > 0 || e.is_auto());
    planet.queue = queue;
    completed
}

/// A planetary scanner finished on a planet (`FBuildObject`, `10b8:1a5c`,
/// its `0x12..=0x1b` arm): the generic item (`0x1b`) is resolved to the
/// owner's best planetary scanner (`LookupBestPlanetaryScanner`), the
/// specific ones name theirs, and the planet's `iScanner` becomes that
/// scanner's index — Viewer 50 is 0, as the corpus's home worlds store.
/// The owner is told (`idmHasBuiltNewPlanetaryScanner`, parameters
/// `[planet, -0x8000, index]`).
fn install_planetary_scanner(state: &mut GameState, index: usize, item: u16) {
    let Some(owner) = state.planets[index]
        .owner
        .and_then(|o| usize::try_from(o).ok())
    else {
        return;
    };
    let scanner = if item == item::PLANETARY_SCANNER {
        let levels = state
            .players
            .get(owner)
            .map_or([0u8; 6], |p| p.research.levels);
        crate::components::best_planetary_scanner(&levels)
            .map_or(0, |p| u8::try_from(p.id - 1).unwrap_or(0))
    } else {
        u8::try_from(item - item::PLANETARY_SCANNER_FIRST).unwrap_or(0)
    };
    state.planets[index].scanner = Some(scanner);
    let id = state.planets[index].id;
    state.messages.push(crate::message::Message {
        player: owner,
        id: crate::message::id::BUILT_PLANETARY_SCANNER,
        object: id,
        params: vec![id, -0x8000, i16::from(scanner)],
    });
}

/// A Genesis Device finished (`FBuildObject`'s `0xd` arm): every player is
/// told (`idmStrongFundamentalForcesHaveRebirthed`); unless the owner is
/// Alternate Reality the mines, factories and defences are levelled and
/// the scanner lost (`iScanner` set to 31); and for each of the three
/// axes the surface minerals are emptied, the climate — original and
/// current alike — becomes `Random(50) + Random(50) + 1` and the
/// concentration `Random(40) + Random(40) + 25`. The people stay.
fn genesis_device(state: &mut GameState, index: usize, rng: &mut Rng) {
    let id = state.planets[index].id;
    for player in 0..state.players.len() {
        state.messages.push(crate::message::Message {
            player,
            id: crate::message::id::GENESIS_DEVICE,
            object: id,
            params: vec![id],
        });
    }
    let ar = state.planets[index]
        .owner
        .and_then(|o| usize::try_from(o).ok())
        .and_then(|o| state.players.get(o))
        .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Ar));
    let planet = &mut state.planets[index];
    if !ar {
        planet.mines = 0;
        planet.factories = 0;
        planet.defenses = 0;
        planet.scanner = None;
    }
    let mut env = [0i8; 3];
    for (axis, climate) in env.iter_mut().enumerate() {
        planet.surface_min[axis] = 0;
        let a = rng.random(50);
        let b = rng.random(50);
        *climate = i8::try_from(a + b + 1).unwrap_or(i8::MAX);
        let a = rng.random(40);
        let b = rng.random(40);
        planet.min_conc[axis] = u8::try_from(a + b + 25).unwrap_or(u8::MAX);
    }
    planet.env = env;
    planet.env_orig = Some(env);
}

/// What running out of fuel did to a fleet's leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RanDry {
    /// It did not.
    No,
    /// The tank is empty and the engines cannot turn for nothing at any
    /// warp: the fleet is stranded (`idmHasRunFuel`).
    Stuck,
    /// The tank is empty, so the leg has been slowed to the fastest warp
    /// the engines run free at (`idmHasRunFuelFleetsSpeedHasDecreased`).
    SlowedTo(u8),
}

/// A fleet that has moved and stands in deep space exactly on a planet is
/// in orbit of it: `MoveFleets` (`10b0:4ddb`) asks `FFindNearestObject`
/// for a planet at the fleet's own point — mask `0x81`, planets within
/// no distance at all — and writes its id in when one is there, which is
/// how a chaser that catches a fleet over a planet ends up orbiting the
/// planet. Then, as `KillUsedWaypoints` has it (`1080:1bfb`), a first
/// waypoint still aimed at a **fleet** — with no Transport or Merge on
/// it — is rewritten to say where the fleet now is: the planet, or a
/// point in space.
fn settle_where_it_stands(state: &mut GameState, index: usize) {
    let at = state.fleets[index].position;
    if state.fleets[index].orbiting.is_none() {
        let here = state
            .planets
            .iter()
            .find(|p| p.position == Some(at))
            .and_then(|p| u16::try_from(p.id).ok());
        state.fleets[index].orbiting = here;
    }
    let orbiting = state.fleets[index].orbiting;
    if let Some(first) = state.fleets[index].waypoints.first_mut() {
        if first.target_class == crate::fleet::grobj::FLEET
            && first.task != stars_formats::task::TRANSPORT
            && first.task != stars_formats::task::MERGE
        {
            match orbiting {
                Some(planet) => {
                    first.target_class = crate::fleet::grobj::PLANET;
                    first.target = Some(planet);
                }
                None => {
                    first.target_class = crate::fleet::grobj::POSITION;
                    first.target = None;
                }
            }
        }
    }
}

/// A fleet has reached its next waypoint: the waypoint is done with, and
/// the fleet is orbiting whatever it named. Its current-leg warp becomes
/// the next leg's, or nothing when there is none.
fn arrive(fleet: &mut Fleet) {
    fleet.warp = fleet.waypoints.get(2).map(|w| w.warp);
    // Only a waypoint aimed at a **planet** puts the fleet in orbit: one
    // aimed at a fleet, a wormhole or a point in space names no planet, so
    // arriving there leaves the fleet in deep space.
    fleet.orbiting = fleet
        .waypoints
        .get(1)
        .filter(|w| w.target_class == crate::fleet::grobj::PLANET)
        .and_then(|w| w.target);
    // `KillUsedWaypoints` copies the waypoint reached over the one left
    // and drops it — `DeleteWpFar(lpfl, 1, fRepOrders)` (`1050:9e28`) —
    // and with **Repeat Orders** on, the drop puts it back at the end of
    // the route instead, task and all, so the route circles. Not when
    // there is only the one leg, not when the last waypoint already stands
    // where this one does, and not for a Merge with a fleet (`1080:1bfb`).
    if fleet.waypoints.len() > 1 {
        let reached = fleet.waypoints.remove(1);
        let merge_with_fleet = reached.task == stars_formats::task::MERGE
            && reached.target_class == crate::fleet::grobj::FLEET;
        let recycle = fleet.repeat_orders
            && !merge_with_fleet
            && fleet.waypoints.len() > 1
            && fleet.waypoints.last().map(|w| w.position) != Some(reached.position);
        fleet.waypoints[0] = reached.clone();
        if recycle {
            fleet.waypoints.push(reached);
        }
    }
}

/// Move one fleet along its current leg.
///
/// A fleet covers `warp^2` light years a year toward its next waypoint,
/// stopping exactly on it if that would overshoot. On arrival the waypoint is
/// consumed, so the following one becomes the next leg.
///
/// Fuel: `MoveFleets` first asks whether the tank covers the **whole of the
/// rest of the leg** (`EstFuelUse` over the remaining distance). If it does,
/// the year's travel is flown and paid for, and the fuel range is not
/// consulted at all — a fleet with exactly enough arrives on its last drop.
/// If it does not, the fleet goes as far as the tank's range allows this
/// year and the tank is **zeroed**, not debited (`10b0:42c3`). Then, when
/// that leaves it dry and short of the waypoint, the original looks for the
/// fastest warp at which the rest of the leg costs nothing — counting up
/// from 1 until one costs fuel, and taking the one before — and writes that
/// onto the leg, so a stranded freighter creeps on at warp 1 rather than
/// sitting still; if even warp 1 costs fuel it stays where it is. Either way
/// the player is told.
///
/// Returns the distance travelled and what the fuel did, or `None` if the
/// fleet had nowhere to go.
/// Put a fleet through the stargates at both ends of its leg — the
/// stargate branch of `MoveFleets` (`10b0:354f`–`3d0d`).
///
/// The fleet must stand at a planet whose starbase has a gate (unless
/// every ship carries a Jump Gate), the leg must end at a planet with one,
/// and both must be the player's own or a friend's. A race that is not
/// Interstellar Traveler cannot take cargo through: minerals and
/// colonists are put down on the planet first — colonists only on a
/// planet of its own, else the jump is refused. The jump itself is
/// [`crate::stargate::jump`]. Returns the leg's length on arrival, as a
/// flown leg's travel, or `None` when the fleet stays where it is.
fn stargate_leg(
    state: &mut GameState,
    index: usize,
    rng: &mut Rng,
    jumped_from: &mut std::collections::BTreeMap<(i16, u16), crate::movement::Point>,
) -> Option<(i32, RanDry)> {
    use crate::message::{fleet_object, id, Message};
    use crate::relations::{regard, Relation};

    let fleet = &state.fleets[index];
    let Ok(owner) = usize::try_from(fleet.owner) else {
        return None;
    };
    let fleet_id = fleet.id;
    let from = fleet.position;
    let here = fleet.waypoints.first()?;
    let leg = fleet.waypoints.get(1)?.clone();
    let designs = state.designs.get(owner).cloned().unwrap_or_default();
    let player_designs = |state: &GameState, who: Option<i16>| -> Vec<crate::design::ShipDesign> {
        who.and_then(|w| usize::try_from(w).ok())
            .and_then(|w| state.designs.get(w))
            .cloned()
            .unwrap_or_default()
    };
    let find_planet = |state: &GameState, pred: &dyn Fn(&Planet) -> bool| -> Option<Planet> {
        state
            .planets
            .iter()
            .chain(state.known_planets.iter())
            .find(|p| pred(p))
            .cloned()
    };
    let friendly = |state: &GameState, host: i16| -> bool {
        host == fleet.owner
            || usize::try_from(host).is_ok_and(|h| regard(state, h, owner) == Relation::Friend)
    };
    let tell = |state: &mut GameState, id: u16, params: Vec<i16>| {
        state.messages.push(Message {
            player: owner,
            id,
            object: fleet_object(fleet_id),
            params,
        });
    };
    let fleet_param = fleet_id as i16;

    // The gate the fleet leaves from: the planet it stands at.
    let source = if here.target_class == crate::fleet::grobj::PLANET {
        here.target
            .and_then(|t| i16::try_from(t).ok())
            .and_then(|pid| find_planet(state, &|p| p.id == pid))
    } else {
        None
    };
    let source_gate = source
        .as_ref()
        .and_then(|p| crate::stargate::gate_of(p, &player_designs(state, p.owner)));
    let source_only_dst = source_gate.is_none();
    if source_gate.is_none() && !crate::stargate::can_jumpgate(fleet, &designs) {
        let place = source
            .as_ref()
            .map_or(vec![from.x, from.y], |p| vec![-1, p.id]);
        tell(
            state,
            id::STARGATE_NONE_HERE,
            [vec![fleet_param], place].concat(),
        );
        return None;
    }
    if let (Some(src), Some(_)) = (&source, source_gate) {
        if let Some(host) = src.owner {
            if !friendly(state, host) {
                tell(
                    state,
                    id::STARGATE_BLOCKED_HERE,
                    vec![fleet_param, src.id, src.id],
                );
                return None;
            }
        }
    }

    // The gate it arrives at: the planet the leg names, or the planet
    // standing where the leg points.
    let destination = if leg.target_class == crate::fleet::grobj::PLANET {
        leg.target
            .and_then(|t| i16::try_from(t).ok())
            .and_then(|pid| find_planet(state, &|p| p.id == pid))
    } else {
        find_planet(state, &|p| p.position == Some(leg.position))
    };
    let src_param = source.as_ref().map_or(-1, |p| p.id);
    let Some(dst) = destination else {
        tell(
            state,
            id::STARGATE_NONE_THERE,
            vec![fleet_param, src_param, leg.position.x, leg.position.y],
        );
        return None;
    };
    let Some(dst_gate) = crate::stargate::gate_of(&dst, &player_designs(state, dst.owner)) else {
        tell(
            state,
            id::STARGATE_NONE_THERE,
            vec![fleet_param, src_param, -1, dst.id],
        );
        return None;
    };
    if let Some(host) = dst.owner {
        if !friendly(state, host) {
            tell(
                state,
                id::STARGATE_BLOCKED_THERE,
                vec![fleet_param, src_param, dst.id, dst.id],
            );
            return None;
        }
    }
    let src_gate = source_gate.unwrap_or(dst_gate);

    // Cargo does not go through, except an Interstellar Traveler's.
    let interstellar = state
        .players
        .get(owner)
        .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::It));
    if !source_only_dst && !interstellar {
        let src = source.as_ref()?;
        let cargo = state.fleets[index].cargo;
        if cargo.colonists > 0 && src.owner != Some(fleet.owner) {
            tell(
                state,
                id::STARGATE_COLONISTS_ABOARD,
                vec![fleet_param, src.id],
            );
            return None;
        }
        let minerals: i32 = cargo.minerals.iter().sum();
        let colonists = cargo.colonists;
        if minerals != 0 || colonists != 0 {
            if let Some(planet) = state.planets.iter_mut().find(|p| p.id == src.id) {
                for (kind, held) in cargo.minerals.iter().enumerate() {
                    planet.surface_min[kind] += held;
                }
                planet.pop += colonists;
            }
            let f = &mut state.fleets[index];
            f.cargo.minerals = [0; 3];
            f.cargo.colonists = 0;
            let [c_lo, c_hi] = Message::long(colonists);
            let [m_lo, m_hi] = Message::long(minerals);
            let (which, params) = if colonists != 0 && minerals != 0 {
                (
                    id::STARGATE_UNLOADED_BOTH,
                    vec![fleet_param, c_lo, c_hi, m_lo, m_hi, src.id],
                )
            } else if colonists != 0 {
                (
                    id::STARGATE_UNLOADED_COLONISTS,
                    vec![fleet_param, c_lo, c_hi, c_lo, src.id],
                )
            } else {
                (
                    id::STARGATE_UNLOADED_MINERALS,
                    vec![fleet_param, m_lo, m_hi, m_lo, src.id],
                )
            };
            tell(state, which, params.clone());
            // The planet's owner hears of it too, when it is somebody else.
            if let Some(host) = src.owner.and_then(|h| usize::try_from(h).ok()) {
                if host != owner {
                    state.messages.push(Message {
                        player: host,
                        id: which,
                        object: src.id,
                        params,
                    });
                }
            }
        }
    }

    // The jump, judged on the straight-line distance as `DGetDistance`
    // reports it, truncated to a whole light year.
    #[allow(clippy::cast_possible_truncation)]
    let span = distance(from, leg.position) as i32;
    let before = state.fleets[index].stacks.clone();
    match crate::stargate::jump(
        &mut state.fleets[index],
        &designs,
        interstellar,
        src_gate,
        dst_gate,
        span,
        rng,
    ) {
        Err(crate::stargate::Refused::TooFar) => {
            tell(
                state,
                id::STARGATE_TOO_FAR,
                vec![fleet_param, src_param, dst.id],
            );
            None
        }
        Err(crate::stargate::Refused::TooMassive(slot)) => {
            tell(
                state,
                id::STARGATE_TOO_MASSIVE,
                vec![fleet_param, src_param, dst.id, i16::from(slot)],
            );
            None
        }
        Err(crate::stargate::Refused::Annihilated) => {
            tell(
                state,
                id::STARGATE_ANNIHILATED,
                vec![fleet_param, src_param, dst.id],
            );
            // Nothing is left to arrive: the fleet is gone with its cargo.
            let f = &mut state.fleets[index];
            f.stacks.clear();
            f.cargo = crate::fleet::Cargo::default();
            None
        }
        Ok(jump) => {
            if jump.lost != 0 {
                if jump.lost > i32::from(i16::MAX) {
                    let [lo, hi] = Message::long(jump.lost);
                    tell(
                        state,
                        id::STARGATE_LOST_UNBELIEVABLE,
                        vec![fleet_param, src_param, dst.id, lo, hi],
                    );
                } else {
                    #[allow(clippy::cast_possible_truncation)]
                    let lost = jump.lost as i16;
                    tell(
                        state,
                        crate::stargate::loss_message(jump.sent, jump.lost),
                        vec![fleet_param, src_param, dst.id, lost],
                    );
                }
                // The dead ships' share of the cargo goes with them.
                let mut dead = Fleet {
                    stacks: jump.dead.clone(),
                    cargo: crate::fleet::Cargo::default(),
                    ..state.fleets[index].clone()
                };
                crate::fleet::balance_cargo(
                    [&mut state.fleets[index], &mut dead],
                    [&before, &[]],
                    &designs,
                );
            }
            // Through: the fleet stands at the far gate, its leg done, and
            // whoever was chasing it stops at the gate it left by.
            no_auto_track_fleet(state, index);
            let f = &mut state.fleets[index];
            jumped_from.insert((f.owner, f.id), from);
            f.position = leg.position;
            arrive(f);
            Some((span, RanDry::No))
        }
    }
}

/// Repair rates (`HealShips`, `10b8:444c`), in 500ths of the armour a
/// year, by where the fleet sits: moving; stopped in space; at somebody
/// else's planet or nobody's; at its own planet without a starbase (or
/// with one that was attacked this year); at its own starbase without a
/// dock; at its own dock. The player's guide's *Damage Repair* topic gives
/// the same as 1, 2, 3, 5, 8 and 20 percent.
const HEAL_MOVING: i32 = 5;
const HEAL_STOPPED: i32 = 10;
const HEAL_FOREIGN_ORBIT: i32 = 15;
const HEAL_OWN_PLANET: i32 = 25;
const HEAL_OWN_STARBASE: i32 = 40;
const HEAL_OWN_DOCK: i32 = 100;
/// What a Fuel Transport (hull 25) or Super Fuel Xport (hull 26) in the
/// fleet adds, in the same units: 5 and 10 percent.
const HEAL_FUEL_TRANSPORT: i32 = 25;
const HEAL_SUPER_FUEL_XPORT: i32 = 50;
/// A starbase's own repair a year, and an Inner Strength starbase's:
/// 10 and 15 percent.
const HEAL_STARBASE: u16 = 50;
const HEAL_STARBASE_INNER_STRENGTH: u16 = 75;

/// `HealShips` (`10b8:444c`): every damaged design in every fleet that may
/// heal this year mends its armour by the rate its place earns
/// ([`HEAL_MOVING`] and the rest), plus a Fuel Transport's or Super Fuel
/// Xport's bonus — the better of the two, not both — the whole doubled for
/// an Inner Strength race. A design whose damage is no more than the rate
/// is whole again. Then every starbase not attacked this year mends
/// [`HEAL_STARBASE`] of its own, more for Inner Strength.
///
/// `no_heal` names the fleets that fought, hit a mine or jumped a gate
/// (`fNoHeal`), `attacked` the planets whose starbase fought
/// (`PLANET.fNoHeal`), and `moved` the fleets that were not here all turn.
fn heal_ships(
    state: &mut GameState,
    no_heal: &std::collections::BTreeSet<(i16, u16)>,
    attacked: &std::collections::BTreeSet<i16>,
    moved: &std::collections::BTreeSet<(i16, u16)>,
) {
    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        let key = (fleet.owner, fleet.id);
        if no_heal.contains(&key) || fleet.is_empty() {
            continue;
        }
        if !fleet
            .stacks
            .iter()
            .any(|s| s.count > 0 && (s.damaged_pct != 0 || s.damage_pct != 0))
        {
            continue;
        }
        let Ok(owner) = usize::try_from(fleet.owner) else {
            continue;
        };
        let designs = state.designs.get(owner).cloned().unwrap_or_default();
        let mut bonus = 0;
        for stack in fleet.stacks.iter().filter(|s| s.count > 0) {
            match designs.get(usize::from(stack.design)).map(|d| d.hull_id) {
                Some(26) => bonus = HEAL_SUPER_FUEL_XPORT,
                Some(25) if bonus < 5 => bonus = HEAL_FUEL_TRANSPORT,
                _ => {}
            }
        }
        let planet = fleet
            .orbiting
            .and_then(|id| i16::try_from(id).ok())
            .and_then(|id| state.planets.iter().find(|p| p.id == id));
        let mut rate = if moved.contains(&key) {
            HEAL_MOVING
        } else {
            match planet {
                None => HEAL_STOPPED,
                Some(p) if p.owner == Some(fleet.owner) => {
                    if !p.starbase || attacked.contains(&p.id) {
                        HEAL_OWN_PLANET
                    } else {
                        let dock = crate::production::starbase_hull(p, &designs)
                            .and_then(crate::components::hull)
                            .is_some_and(|h| h.cargo_max != 0);
                        if dock {
                            HEAL_OWN_DOCK
                        } else {
                            HEAL_OWN_STARBASE
                        }
                    }
                }
                Some(_) => HEAL_FOREIGN_ORBIT,
            }
        };
        if state
            .players
            .get(owner)
            .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Is))
        {
            rate <<= 1;
        }
        let mend = rate + bonus;
        for stack in &mut state.fleets[index].stacks {
            if stack.count <= 0 || (stack.damaged_pct == 0 && stack.damage_pct == 0) {
                continue;
            }
            if mend < stack.damage_pct {
                stack.damage_pct -= mend;
            } else {
                stack.damage_pct = 0;
                stack.damaged_pct = 0;
            }
        }
    }

    for planet in &mut state.planets {
        if !planet.starbase || attacked.contains(&planet.id) || planet.starbase_damage == 0 {
            continue;
        }
        let inner_strength = planet
            .owner
            .and_then(|o| usize::try_from(o).ok())
            .and_then(|o| state.players.get(o))
            .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Is));
        let rate = if inner_strength {
            HEAL_STARBASE_INNER_STRENGTH
        } else {
            HEAL_STARBASE
        };
        planet.starbase_damage = planet.starbase_damage.saturating_sub(rate);
    }
}

/// Whether a fleet is the kind a battle plan's target class names
/// (`FMatchTarget`, `util.c`), by its designs' hull categories: armed ships
/// are scouts, warships and utility hulls (categories 2 to 4), unarmed
/// ships a fleet with none of those, bombers and freighters categories 1
/// and 5, fuel transports 7, freighters 1; any other class matches
/// everything.
fn matches_target_class(fleet: &Fleet, designs: &[crate::design::ShipDesign], class: u8) -> bool {
    use crate::battle::TargetClass;
    let categories: Vec<u8> = fleet
        .stacks
        .iter()
        .filter(|s| s.count != 0)
        .filter_map(|s| designs.get(usize::from(s.design)))
        .filter_map(|d| d.hull().map(|h| h.category))
        .collect();
    let armed = categories.iter().any(|c| (2..5).contains(c));
    match TargetClass::from_raw(class) {
        TargetClass::ArmedShips => armed,
        TargetClass::UnarmedShips => !armed,
        TargetClass::BombersFreighters => categories.iter().any(|c| *c == 1 || *c == 5),
        TargetClass::FuelTransports => categories.contains(&7),
        TargetClass::Freighters => categories.contains(&1),
        _ => true,
    }
}

/// `ValidateWaypoints` (`1038:68c6`), at the year's end: every fleet's
/// position is held inside the galaxy, and each leg beyond the first is
/// looked at again.
///
/// A leg aimed at a **thing** follows it to where it now is — a wormhole
/// or the Trader having moved — unless the thing is gone, or is a wormhole
/// the player has never seen and it has moved, when the leg becomes a
/// point in space where it last was, with `0xf8` said.
///
/// A leg aimed at a **fleet** (and not marked to stop tracking) whose
/// fleet is gone, or no longer where the leg points, is aimed instead at
/// the heaviest fleet of the same owner still at the spot that the
/// chaser's battle plan would target — one no other chaser has been
/// aimed at this year first; failing any, one of the rest drawn by lot,
/// the heaviest of them preferred. With nothing there the leg is left as
/// it was. A fleet the leg still finds is marked as being chased.
fn validate_waypoints(state: &mut GameState, rng: &mut Rng) {
    use crate::message::{fleet_object, id, Message};

    // `dGal`: 400 light years for a tiny universe, 400 more a size.
    let span = (state.galaxy_size.max(0) + 1) * 400;
    let (low, high) = (1000i16, 1000i16.saturating_add(span));
    let mut targeted: std::collections::BTreeSet<(i16, u16)> = std::collections::BTreeSet::new();

    for index in 0..state.fleets.len() {
        if state.fleets[index].is_empty() {
            continue;
        }
        {
            let fleet = &mut state.fleets[index];
            let at = fleet.position;
            let held = crate::movement::Point::new(at.x.clamp(low, high), at.y.clamp(low, high));
            if held != at {
                fleet.position = held;
                if let Some(first) = fleet.waypoints.first_mut() {
                    first.position = held;
                }
            }
        }
        let owner = state.fleets[index].owner;
        let Ok(owner_index) = usize::try_from(owner) else {
            continue;
        };
        let plan_target = state
            .players
            .get(owner_index)
            .and_then(|p| {
                p.battle_plans
                    .get(usize::from(state.fleets[index].battle_plan))
            })
            .map_or(1, |p| p.primary_target);
        let fleet_id = state.fleets[index].id;

        for leg in 1..state.fleets[index].waypoints.len() {
            let waypoint = state.fleets[index].waypoints[leg].clone();
            match waypoint.target_class {
                GROBJ_THING => {
                    let Some(word) = waypoint.target else {
                        continue;
                    };
                    let hole = state
                        .wormholes
                        .iter()
                        .find(|w| names_thing(Some(word), ITH_WORMHOLE, w.id))
                        .map(|w| (w.position, w.detected_by & (1 << (owner & 15)) != 0));
                    let trader = state
                        .traders
                        .iter()
                        .find(|t| names_thing(Some(word), ITH_TRADER, t.id))
                        .map(|t| (t.position, true));
                    let found = hole.or(trader);
                    let lost = match found {
                        None => true,
                        Some((at, seen)) => !seen && at != waypoint.position,
                    };
                    if lost {
                        if found.is_some() {
                            state.messages.push(Message {
                                player: owner_index,
                                id: id::WORMHOLE_VANISHED,
                                object: fleet_object(fleet_id),
                                params: vec![fleet_id as i16],
                            });
                        }
                        let w = &mut state.fleets[index].waypoints[leg];
                        w.target_class = GROBJ_POSITION;
                        w.target = None;
                    } else if let Some((at, _)) = found {
                        state.fleets[index].waypoints[leg].position = at;
                    }
                }
                crate::fleet::grobj::FLEET => {
                    if state.no_auto_track.contains(&(owner, fleet_id, leg)) {
                        continue;
                    }
                    let Some(word) = waypoint.target else {
                        continue;
                    };
                    let (their_owner, number) = crate::orders::split_fleet_id(word);
                    let quarry = state
                        .fleets
                        .iter()
                        .position(|f| f.owner == their_owner && f.id == number && !f.is_empty());
                    let still_there =
                        quarry.is_some_and(|q| state.fleets[q].position == waypoint.position);
                    if still_there {
                        if let Some(q) = quarry {
                            targeted.insert((state.fleets[q].owner, state.fleets[q].id));
                        }
                        continue;
                    }
                    let their_designs = usize::try_from(their_owner)
                        .ok()
                        .and_then(|o| state.designs.get(o))
                        .cloned()
                        .unwrap_or_default();
                    let candidates: Vec<usize> = state
                        .fleets
                        .iter()
                        .enumerate()
                        .filter(|(i, f)| {
                            *i != index
                                && f.owner == their_owner
                                && !f.is_empty()
                                && f.position == waypoint.position
                                && matches_target_class(f, &their_designs, plan_target)
                        })
                        .map(|(i, _)| i)
                        .collect();
                    // The heaviest not yet chased; ties by a coin.
                    let mut best: Option<(usize, i32)> = None;
                    for &c in &candidates {
                        let f = &state.fleets[c];
                        if targeted.contains(&(f.owner, f.id)) {
                            continue;
                        }
                        let weight = f.mass(&their_designs);
                        let take = match best {
                            None => true,
                            Some((_, w)) => weight > w || (weight == w && rng.random(2) == 0),
                        };
                        if take {
                            best = Some((c, weight));
                        }
                    }
                    let mut chosen = best.map(|(c, _)| c);
                    if chosen.is_none() {
                        // Everyone here is chased already: one by lot, the
                        // heaviest winning outright.
                        let mut by_lot: Option<usize> = None;
                        let mut heaviest: Option<(usize, i32)> = None;
                        let mut seen = 0i16;
                        for &c in &candidates {
                            let f = &state.fleets[c];
                            let weight = f.mass(&their_designs);
                            let take = match heaviest {
                                None => true,
                                Some((_, w)) => weight > w || (weight == w && rng.random(2) == 0),
                            };
                            if take {
                                heaviest = Some((c, weight));
                            }
                            seen += 1;
                            if rng.random(seen) == 0
                                && (seen == 1
                                    || !targeted.contains(&(f.owner, f.id))
                                    || rng.random(2) != 0)
                            {
                                by_lot = Some(c);
                            }
                        }
                        chosen = heaviest.map(|(c, _)| c).or(by_lot);
                    }
                    if let Some(c) = chosen {
                        let (o, n, at) = {
                            let f = &state.fleets[c];
                            (f.owner, f.id, f.position)
                        };
                        let w = &mut state.fleets[index].waypoints[leg];
                        w.target = Some((u16::try_from(o).unwrap_or(0) << 9) | (n & 0x1ff));
                        w.position = at;
                        targeted.insert((o, n));
                    }
                }
                _ => {}
            }
        }
    }
}

/// `UpdateGuesses` (`10b8:532c`): the population and defence estimates a
/// planet shows to other players' scanners, written on every planet at
/// the end of the year.
///
/// The population guess is a quarter of the population in hundreds,
/// jittered — `(pop + Random(pop / 4) − pop / 8) / 4`, held to `1..=4090`
/// — and `0` for an Alternate Reality planet, whose people are not on the
/// ground; a planet with nobody on it has neither guess. The defence
/// guess is `(104 − pct) / 6` held to `1..=15`, `pct` the percentage of a
/// bombing run the defences let through (`CalcPctSurvive`), and `0`
/// without defences.
fn update_guesses(state: &mut GameState, rng: &mut Rng) {
    for index in 0..state.planets.len() {
        let planet = &state.planets[index];
        if planet.pop <= 0 {
            state.planets[index].pop_guess = Some(0);
            state.planets[index].defense_guess = Some(0);
            continue;
        }
        let owner = planet.owner.and_then(|o| usize::try_from(o).ok());
        let player = owner.and_then(|o| state.players.get(o));
        let alternate = player.is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Ar));
        let pop = i64::from(planet.pop);
        let guess = if alternate {
            0
        } else {
            let jitter = i64::from(rng.random(i16::try_from(pop >> 2).unwrap_or(i16::MAX)));
            ((pop + jitter - (pop >> 3)) >> 2).clamp(1, 4090)
        };
        let defence = if planet.defenses == 0 {
            0
        } else {
            let pct = player.map_or(1.0, |p| {
                crate::bombing::pct_survive(planet, &p.race, p.research.levels).0
            });
            #[allow(clippy::cast_possible_truncation)]
            let pct = (pct * 100.0) as i64;
            ((104 - pct) / 6).clamp(1, 15)
        };
        let planet = &mut state.planets[index];
        planet.pop_guess = u32::try_from(guess * 400).ok();
        planet.defense_guess = u8::try_from(defence).ok();
    }
}

/// The five engines rated for warp 10 (`MoveFleets`, `10b0:3fd1`): the
/// Interspace-10, Enigma Pulsar, Trans-Star 10, Trans-Galactic Mizer
/// Scoop and Galaxy Scoop, by index in [`crate::components::ENGINES`].
const SAFE_AT_WARP_TEN: [u8; 5] = [7, 8, 9, 0xe, 0xf];

/// The Radiating Hydro-Ram Scoop's index in [`crate::components::ENGINES`]
/// (`EstFuelUse` sets `gd.fRadiatingEngine` for engine id 10).
const RADIATING_ENGINE: u8 = 10;

/// What the first pass of `MoveFleets` does to a fleet before it moves
/// (`10b0:3496`–`423d`), for every fleet with a leg to fly:
///
/// * a **Cheap Engines** race's fleet ordered above warp 6 (and not
///   through a gate) fails to start one year in ten (`Random(10) == 0`)
///   and stays put, told by `idmUnableEngageEngines…` (`0xf2`);
/// * an **Alternate Reality** fleet carrying more than ten hundred
///   colonists loses `(colonists × 3 + 33) / 100` hundred of them to the
///   acceleration (`0xc1`);
/// * at **warp 10**, every ship whose engine is not one of the five rated
///   for it blows up one time in ten. One lost is `0xdf`, more are `0xe0`,
///   all of them `0xe1` and the fleet is gone; the dead ships' share of
///   the cargo goes with them.
///
/// Returns the fleets that will not move this year, and those (by
/// `(owner, id)`) that no longer exist.
#[allow(clippy::type_complexity)]
fn first_pass_mishaps(
    state: &mut GameState,
    rng: &mut Rng,
) -> (
    std::collections::BTreeSet<usize>,
    std::collections::BTreeSet<(i16, u16)>,
) {
    use crate::message::{fleet_object, id, Message};

    let mut grounded = std::collections::BTreeSet::new();
    let mut blown_up = std::collections::BTreeSet::new();
    for index in 0..state.fleets.len() {
        let fleet = &state.fleets[index];
        let Some(leg) = fleet.waypoints.get(1) else {
            continue;
        };
        if leg.warp == 0 || fleet.is_empty() {
            continue;
        }
        let warp = leg.warp;
        let Ok(owner) = usize::try_from(fleet.owner) else {
            continue;
        };
        let fleet_id = fleet.id;
        let Some(player) = state.players.get(owner) else {
            continue;
        };
        let cheap = player.race.has_lrt(crate::race::lrt::CHEAP_ENGINES);
        let alternate = player.race.prt() == Some(crate::race::Prt::Ar);
        let designs = state.designs.get(owner).cloned().unwrap_or_default();
        let tell = |state: &mut GameState, id: u16, params: Vec<i16>| {
            state.messages.push(Message {
                player: owner,
                id,
                object: fleet_object(fleet_id),
                params,
            });
        };
        let fleet_param = fleet_id as i16;

        if cheap && warp > 6 && warp != crate::stargate::WARP && rng.random(10) == 0 {
            tell(state, id::BALKY_ENGINES, vec![fleet_param]);
            grounded.insert(index);
            continue;
        }
        if warp >= crate::stargate::WARP {
            continue;
        }

        if alternate && state.fleets[index].cargo.colonists > 10 {
            let colonists = i64::from(state.fleets[index].cargo.colonists);
            let die = (colonists * 3 + 33) / 100;
            if die > 0 {
                #[allow(clippy::cast_possible_truncation)]
                let die = die as i32;
                state.fleets[index].cargo.colonists -= die;
                let [lo, hi] = Message::long(die);
                tell(
                    state,
                    id::WARP_ACCELERATION_KILLED,
                    vec![lo, hi, fleet_param],
                );
            }
        }

        if warp == 10 {
            let before = state.fleets[index].stacks.clone();
            let mut lost_total = 0i32;
            for stack in &mut state.fleets[index].stacks {
                if stack.count <= 0 {
                    continue;
                }
                let engine = designs
                    .get(usize::from(stack.design))
                    .and_then(|d| {
                        d.slots
                            .iter()
                            .find(|s| s.is(crate::components::slot::ENGINE))
                    })
                    .map(|s| s.item);
                if engine.is_some_and(|e| SAFE_AT_WARP_TEN.contains(&e)) {
                    continue;
                }
                let mut lost = 0;
                for _ in 0..stack.count {
                    if rng.random(10) == 0 {
                        lost += 1;
                    }
                }
                stack.count -= lost;
                lost_total += lost;
            }
            if lost_total == 0 {
                continue;
            }
            if state.fleets[index].is_empty() {
                tell(state, id::WARP_TEN_LOST_FLEET, vec![fleet_param]);
                let f = &mut state.fleets[index];
                f.stacks.clear();
                f.cargo = crate::fleet::Cargo::default();
                blown_up.insert((f.owner, f.id));
                grounded.insert(index);
                continue;
            }
            let mut dead = Fleet {
                stacks: before
                    .iter()
                    .zip(&state.fleets[index].stacks)
                    .map(|(was, now)| crate::fleet::ShipStack {
                        design: was.design,
                        count: was.count - now.count,
                        damaged_pct: 0,
                        damage_pct: 0,
                    })
                    .collect(),
                cargo: crate::fleet::Cargo::default(),
                ..state.fleets[index].clone()
            };
            crate::fleet::balance_cargo(
                [&mut state.fleets[index], &mut dead],
                [&before, &[]],
                &designs,
            );
            if lost_total == 1 {
                tell(state, id::WARP_TEN_LOST_ONE, vec![fleet_param, 0]);
            } else {
                #[allow(clippy::cast_possible_truncation)]
                tell(
                    state,
                    id::WARP_TEN_LOST_SHIPS,
                    vec![lost_total as i16, fleet_param],
                );
            }
        }
    }
    (grounded, blown_up)
}

/// What a fleet's ramscoops gather over `travelled` light years at `warp`
/// (`LCalcFuelGainFromRamScoops`, `1038:56b8`), into the tank as far as it
/// has room (`ChgCargo`), and the player told the whole figure, capped at
/// 32,500 (`10b0:484b`–`494e`).
///
/// Per ship whose engine burns nothing at this warp: the engine count,
/// plus twice it when the next warp is free too, three times for the one
/// after and four for the one after that — the more headroom the scoop
/// has, the more it gathers. Nothing at warp 10 or above.
fn ramscoop_gain(
    state: &mut GameState,
    index: usize,
    designs: &[crate::design::ShipDesign],
    warp: u8,
    travelled: i32,
) {
    use crate::message::{fleet_object, id, Message};

    if warp >= 10 || travelled <= 0 {
        return;
    }
    let fleet = &state.fleets[index];
    let room = fleet.fuel_capacity(designs) - fleet.cargo.fuel;
    if room <= 0 {
        return;
    }
    let mut per_ly: i64 = 0;
    for stack in fleet.stacks.iter().filter(|s| s.count > 0) {
        let Some(design) = designs.get(usize::from(stack.design)) else {
            continue;
        };
        let Some(slot) = design
            .slots
            .iter()
            .find(|s| s.is(crate::components::slot::ENGINE))
        else {
            continue;
        };
        let Some(engine) = crate::components::ENGINES.get(usize::from(slot.item)) else {
            continue;
        };
        let free = |w: u8| {
            engine
                .fuel_used
                .get(usize::from(w))
                .is_some_and(|f| *f == 0)
        };
        let engines = i64::from(slot.count);
        let mut share = 0i64;
        if free(warp) {
            share += engines;
            if free(warp + 1) {
                share += engines * 2;
                if warp < 9 && free(warp + 2) {
                    share += engines * 3;
                    if warp < 8 && free(warp + 3) {
                        share += engines * 4;
                    }
                }
            }
        }
        per_ly += share * i64::from(stack.count);
    }
    let gain = per_ly * i64::from(travelled);
    if gain <= 0 {
        return;
    }
    #[allow(clippy::cast_possible_truncation)]
    let taken = gain.min(i64::from(room)) as i32;
    let fleet = &mut state.fleets[index];
    fleet.cargo.fuel += taken;
    let (owner, fleet_id) = (fleet.owner, fleet.id);
    #[allow(clippy::cast_possible_truncation)]
    let said = gain.min(32_500) as i16;
    if let Ok(player) = usize::try_from(owner) {
        state.messages.push(Message {
            player,
            id: id::RAMSCOOP_FUEL,
            object: fleet_object(fleet_id),
            params: vec![fleet_id as i16, said],
        });
    }
}

/// The radiation of a Radiating Hydro-Ram Scoop, on the colonists aboard
/// a fleet that moved (`MoveFleets`, `10b0:4b2d`–`4ce0`).
///
/// A race whose radiation range reaches 170 in total (`min + max`), or
/// that is immune to it, is untouched. Otherwise `(86 − mid) / 2` percent
/// of the colonists die, `mid` the middle of the range, at least one
/// hundred and never more than are aboard (`0x74`).
fn radiation_deaths(state: &mut GameState, index: usize, designs: &[crate::design::ShipDesign]) {
    use crate::message::{fleet_object, id, Message};

    let fleet = &state.fleets[index];
    if fleet.cargo.colonists <= 0 {
        return;
    }
    let radiating = fleet.stacks.iter().filter(|s| s.count > 0).any(|s| {
        designs.get(usize::from(s.design)).is_some_and(|d| {
            d.slots
                .iter()
                .any(|h| h.is(crate::components::slot::ENGINE) && h.item == RADIATING_ENGINE)
        })
    });
    if !radiating {
        return;
    }
    let Ok(owner) = usize::try_from(fleet.owner) else {
        return;
    };
    let Some(player) = state.players.get(owner) else {
        return;
    };
    let (low, high) = (
        i32::from(player.race.env_min[2]),
        i32::from(player.race.env_max[2]),
    );
    if high < 0 || low + high >= 170 {
        return;
    }
    let mid = (low + high) / 2;
    let colonists = fleet.cargo.colonists;
    let dead = (((86 - mid) >> 1) * colonists / 100).max(1).min(colonists);
    let fleet_id = fleet.id;
    #[allow(clippy::cast_possible_truncation)]
    state.messages.push(Message {
        player: owner,
        id: id::ENGINE_RADIATION_KILLED,
        object: fleet_object(fleet_id),
        params: vec![dead as i16, fleet_id as i16],
    });
    state.fleets[index].cargo.colonists -= dead;
}

fn move_fleet(
    fleet: &mut Fleet,
    designs: &[crate::design::ShipDesign],
    ife: bool,
    cap: Option<i32>,
) -> Option<(i32, RanDry)> {
    let (target, warp) = fleet.next_leg()?;
    let from = fleet.position;
    let d = distance(from, target);
    if d <= 0.0 {
        // Already standing on the waypoint — a chase whose quarry has
        // gone, say: the leg is done with, and the waypoint is consumed as
        // if it had just been reached.
        arrive(fleet);
        return Some((0, RanDry::No));
    }
    // A chase is flown in pieces: this piece is at most `cap`.
    let year = |warp: u8| {
        let full = travel_this_year(i16::from(warp), d, None);
        cap.map_or(full, |c| full.min(c.max(0)))
    };

    let mut dry = RanDry::No;
    let travel = if designs.is_empty() {
        year(warp)
    } else {
        let wanted = year(warp);
        #[allow(clippy::cast_possible_truncation)]
        let whole_leg = fleet.fuel_use(designs, warp, (d + 0.9999) as i32, ife);
        if fleet.cargo.fuel >= whole_leg {
            let burned = fleet.fuel_use(designs, warp, wanted, ife);
            fleet.cargo.fuel = (fleet.cargo.fuel - burned).max(0);
            wanted
        } else {
            let range = fleet.fuel_range(designs, warp, ife);
            let travel = wanted.min(range);
            fleet.cargo.fuel = 0;
            // Dry, and short of the waypoint: the leg is slowed to what the
            // fleet can still fly.
            if f64::from(travel) < d - 0.99999 {
                #[allow(clippy::cast_possible_truncation)]
                let left = ((d - f64::from(travel)).ceil() as i32).max(1);
                let mut probe: u8 = 1;
                while probe < 10 && fleet.fuel_use(designs, probe, left, ife) == 0 {
                    probe += 1;
                }
                dry = if probe < 2 {
                    RanDry::Stuck
                } else {
                    if let Some(leg) = fleet.waypoints.get_mut(1) {
                        leg.warp = probe - 1;
                    }
                    RanDry::SlowedTo(probe - 1)
                };
            }
            travel
        }
    };
    let to = advance(from, target, travel);
    fleet.position = to;

    if to == target {
        arrive(fleet);
    } else {
        fleet.orbiting = None;
        // The leg continues from where the fleet now is.
        if let Some(here) = fleet.waypoints.first_mut() {
            here.position = to;
        }
    }
    Some((travel, dry))
}
