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
//! can never mistake a partial turn for a complete one.

use crate::fleet::Fleet;
use crate::mining::mine_minerals;
use crate::movement::{advance, distance, travel_this_year};
use crate::planet::Planet;
use crate::population::update_population;
use crate::production::{
    auto_build_cap, build_item, item, planet_budget, planetary_item_cost, COST_PARTS,
};
use crate::research::{add_research, Breakthrough};
use crate::rng::Rng;
use crate::{GameState, Player};

/// A step of the original turn pipeline that this crate does not yet perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkippedStep {
    /// Player orders: cargo transfers, waypoint changes, production edits.
    Orders,
    /// Fleet movement, fuel and mine-field traversal.
    FleetMovement,
    /// Mineral packets, wormholes and Mystery Trader movement and decay.
    Things,
    /// The production build queue (needs the components table).
    BuildQueue,
    /// Battle resolution.
    Combat,
    /// Random events (comet strikes and the like).
    RandomEvents,
    /// Score calculation.
    Scores,
    /// The ship the Mystery Trader gives when it has no technology left to
    /// hand over. It needs the game's own Mystery Trader hull designs, which
    /// this engine does not carry.
    TraderShip,
}

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
    /// Fleets that moved, as `(fleet id, light years travelled)`.
    pub moved: Vec<(u16, i32)>,
    /// Population change per planet id, in units of 100 colonists.
    pub population: Vec<(i16, i32)>,
    /// Resources each player put into research.
    pub research_spending: Vec<i32>,
    /// Technology levels gained, per player.
    pub breakthroughs: Vec<Vec<Breakthrough>>,
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
    /// Packets that landed, as `(target planet, minerals delivered, damage)`.
    pub packets_landed: Vec<(i16, [i32; 3], i32)>,
    /// Wormholes that jumped this year, by id.
    pub wormholes_moved: Vec<u16>,
    /// Fleets that went through a wormhole, as `(fleet id, entered, left)`.
    pub wormhole_trips: Vec<(u16, u16, u16)>,
    /// Fleets that reached the Mystery Trader, and what came of it.
    pub trades: Vec<(u16, crate::wormhole::Gift)>,
    /// Interceptions a patrol ordered, as `(patrolling fleet, target fleet)`.
    pub patrols: Vec<(u16, u16)>,
    /// The scoreboard, one entry per player, after the year's events.
    pub scores: Vec<crate::score::PlayerScore>,
    /// Which victory conditions each player meets.
    pub victory: Vec<crate::victory::Met>,
    /// Players who have won, if the game is old enough for anyone to.
    pub winners: Vec<usize>,
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
        skipped: vec![
            SkippedStep::Orders,
            SkippedStep::Things,
            SkippedStep::Combat,
            SkippedStep::RandomEvents,
            SkippedStep::Scores,
        ],
        research_spending: vec![0; state.players.len()],
        breakthroughs: vec![Vec::new(); state.players.len()],
        ..TurnReport::default()
    };

    // --- DoOrders(0): the recorded cargo transfers, before anything moves or
    // produces. A transfer applied here feeds this year's growth.
    if !orders.cargo.is_empty() {
        report.skipped.retain(|s| *s != SkippedStep::Orders);
        let (applied, drops) = crate::orders::apply_cargo_transfers(state, &orders.cargo);
        report.transfers = applied;
        // DropColonists: settle every landing together, so rival claims on one
        // planet are weighed against each other rather than one at a time.
        report.colonised = crate::orders::resolve_colonist_drops(state, &drops);
    }

    // --- MoveThings(0): the Mystery Trader crosses a year, and the packets
    // already in flight do too, before anything else happens.
    move_trader(state, rng);
    report.packets_landed = move_packets(state, false);

    // --- MoveFleets, which happens before Produce.
    for index in 0..state.fleets.len() {
        let owner = usize::try_from(state.fleets[index].owner).unwrap_or(usize::MAX);
        let designs = state.designs.get(owner).cloned().unwrap_or_default();
        let ife = state
            .players
            .get(owner)
            .is_some_and(|p| p.race.has_lrt(crate::race::lrt::IFE));
        let from = state.fleets[index].position;
        // A waypoint is consumed when the fleet reaches it, so the list
        // getting shorter is how this pass knows the fleet arrived.
        let waypoints = state.fleets[index].waypoints.len();
        if let Some(travelled) = move_fleet(&mut state.fleets[index], &designs, ife) {
            report.moved.push((state.fleets[index].id, travelled));
            // FTravelThroughMineFields: the leg is flown, and somewhere along
            // it the fleet may find somebody else's mines.
            if let Some(hit) = cross_minefields(state, index, from, travelled, rng) {
                report.mine_hits.push((state.fleets[index].id, hit));
            }
            // And a fleet that has arrived may have arrived at a wormhole, in
            // which case it is not where it thinks it is.
            if waypoints > state.fleets[index].waypoints.len() {
                if let Some((entered, left)) = traverse_wormhole(state, index) {
                    report
                        .wormhole_trips
                        .push((state.fleets[index].id, entered, left));
                }
            }
        }
    }

    // Last year's news is last year's; the original queues a player's
    // messages afresh each time it generates a turn.
    state.messages.clear();

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
        let Some(budget) = planet_budget(
            &state.planets[index],
            &race,
            research_pct,
            0,
            no_research,
            energy_tech,
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
        let built = run_queue(
            &mut state.planets[index],
            &race,
            &designs,
            tech,
            &mut available,
            &mut ships_built,
        );
        for (i, slot) in state.planets[index].surface_min.iter_mut().enumerate() {
            *slot = available[i];
        }
        if !built.is_empty() {
            let id = state.planets[index].id;
            report.built.push((id, built));
        }
        if !ships_built.is_empty() {
            let id = state.planets[index].id;
            for (slot, count) in ships_built {
                report.ships_built.push((id, slot, count));
                add_ships_to_orbiting_fleet(state, owner, id, slot, count);
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
        if let Some(change) = update_population(&mut state.planets[index], &race) {
            let id = state.planets[index].id;
            report.population.push((id, change.delta));
        }
    }

    // --- Produce: research advance.
    for (index, player) in state.players.iter_mut().enumerate() {
        let resources = report.research_spending[index];
        player.research_last_year = resources;
        let gained = add_research(
            &mut player.research,
            &player.race,
            resources,
            state.slow_tech,
        );
        report.breakthroughs[index] = gained;
    }

    // --- SatisfyOrders after movement: the tasks a fleet performs on arrival.
    // A task is consumed when it executes, which is why every waypoint in a
    // saved game that has already been reached reads 0.
    {
        let (done, drops) = crate::orders::execute_arrival_tasks(state);
        report.tasks_done = done;
        let settled = crate::orders::resolve_colonist_drops(state, &drops);
        report.colonised.extend(settled);
    }

    // --- DoOrders(1) -> DoThingInteractions(1): a fleet that has come to rest
    // on the Mystery Trader trades with it. It happens here, after movement and
    // before the second pass of orders, which is why a fleet cannot both trade
    // and carry out a task in the same year: the Trader keeps the fleet.
    report.trades = trade_with_trader(state, rng);
    if report
        .trades
        .iter()
        .any(|(_, g)| *g == crate::wormhole::Gift::Ship)
    {
        report.skipped.push(SkippedStep::TraderShip);
    }

    // --- SatisfyOrders(3): laying mines. A fleet ordered to lay does so where
    // it now is, into its own field if one reaches that far.
    for index in 0..state.fleets.len() {
        let laid = lay_mines_for_fleet(state, index);
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
        let Some(mined) = remote_mine_for_fleet(state, index, rng) else {
            continue;
        };
        report.remote_mined.push(mined);
    }

    // --- MoveThings(1): a packet thrown this year covers half a year, and
    // decays for it; and the wormholes think about moving.
    report.packets_landed.extend(move_packets(state, true));
    report.wormholes_moved = move_wormholes(state, rng);

    // --- SweepForMines, which the original runs late, after the second pass
    // of orders: everything armed with beams clears what it is sitting in.
    report.mines_swept = sweep_minefields(state);

    // --- UpdatePlayerScores, which the original runs near the end of the
    // year, once everything that could change a score has happened.
    report.scores = crate::score::scores(state);
    let (met, winners) = crate::victory::resolve(state, &report.scores);
    report.victory = met;
    report.winners = winners;

    // --- Patrol: every patrolling fleet looks for something to intercept.
    // The original does this as it writes each player's file, after everything
    // else has happened, because a patrol is decided from that player's view.
    report.patrols = crate::patrol::patrol(state);

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

    state.turn += 1;
    report.year = state.year();
    report
}

/// The race owning `planets[index]`, cloned so the planet can be mutated.
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
) {
    let orbiting = u16::try_from(planet).ok();
    let stack = crate::fleet::ShipStack {
        design,
        count,
        damaged_pct: 0,
        damage_pct: 0,
    };

    if let Some(fleet) = state
        .fleets
        .iter_mut()
        .find(|f| f.owner == owner && f.orbiting == orbiting)
    {
        if let Some(existing) = fleet.stacks.iter_mut().find(|s| s.design == design) {
            existing.count += count;
        } else {
            fleet.stacks.push(stack);
        }
        return;
    }

    // No fleet in orbit: start one, if we know where the planet is.
    let Some(position) = state
        .planets
        .iter()
        .find(|p| p.id == planet)
        .and_then(|p| p.position)
    else {
        return;
    };
    let id = next_fleet_id(state, owner);
    state.fleets.push(crate::fleet::Fleet {
        name: None,
        repeat_orders: false,
        id,
        owner,
        position,
        orbiting,
        stacks: vec![stack],
        cargo: crate::fleet::Cargo::default(),
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
) -> Option<(i16, [i32; 3])> {
    let fleet = &state.fleets[index];
    // `fHereAllTurn`: a fleet that moved this turn has not been in place long
    // enough to mine.
    if fleet.waypoints.first().map(|w| w.task) != Some(TASK_REMOTE_MINE) || fleet.warp.is_some() {
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
fn lay_mines_for_fleet(state: &mut GameState, index: usize) -> Vec<(u8, i32)> {
    let fleet = &state.fleets[index];
    if fleet.waypoints.first().map(|w| w.task) != Some(stars_formats::task::LAY_MINES) {
        return Vec::new();
    }
    let Ok(owner) = usize::try_from(fleet.owner) else {
        return Vec::new();
    };
    // `fHereAllTurn`: this engine records a fleet that moved by leaving its
    // warp set, which is the same test remote mining makes.
    let moved = fleet.warp.is_some();
    let demolition = state
        .players
        .get(owner)
        .is_some_and(|p| p.race.prt() == Some(crate::race::Prt::Sd));
    if moved && !demolition {
        return Vec::new();
    }
    let Some(designs) = state.designs.get(owner).cloned() else {
        return Vec::new();
    };

    let laid = crate::minefield::lay(&mut state.minefields, &state.fleets[index], &designs, moved);

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
    let hit = crate::minefield::traverse(
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
    let stopped = crate::movement::advance(from, to, hit.travelled);
    let fleet = &mut state.fleets[index];
    fleet.position = stopped;
    if stopped != to {
        fleet.orbiting = None;
    }

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

/// Set off every field that is armed to detonate.
///
/// `ThingDecay` (`10b8:70c6`) walks the fleets inside an armed field and takes
/// them through it as though they had been caught, with no roll: the mines are
/// going off whether or not the fleet was moving.
fn detonate_minefields(state: &mut GameState) -> Vec<(u16, crate::minefield::MineHit)> {
    let mut hits = Vec::new();
    let armed: Vec<crate::minefield::Minefield> = state
        .minefields
        .iter()
        .filter(|f| f.detonating)
        .cloned()
        .collect();
    for field in &armed {
        for fleet in &state.fleets {
            if fleet.owner == field.owner || !field.contains(fleet.position) {
                continue;
            }
            hits.push((fleet.id, crate::minefield::damage(fleet, field, 0)));
        }
    }
    hits
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
/// Returns what landed, as `(planet, minerals delivered, damage done)`.
fn move_packets(state: &mut GameState, after_production: bool) -> Vec<(i16, [i32; 3], i32)> {
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
            if let Some(result) = land_packet(state, index) {
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

/// Land a packet on its target planet.
///
/// `10b0:1f03`. The receiving planet catches what its own mass driver can — the
/// ratio of the squared warps, halved for an **Inner Tech** receiver — keeps
/// that plus a ninth of the rest, and takes damage for whatever came in too
/// fast. This engine does not model mass drivers on a planet yet, so nothing is
/// caught; a packet's whole mass counts as uncaught, which is the worst case
/// for the receiver and the case the fixtures' unowned targets are in anyway.
fn land_packet(state: &mut GameState, index: usize) -> Option<(i16, [i32; 3], i32)> {
    let packet = state.packets[index].clone();
    let target = i16::try_from(packet.target).ok()?;
    let planet = state.planets.iter_mut().find(|p| p.id == target)?;

    let driver_warp = 0;
    let inner_tech = false;
    let caught = crate::packet::caught_per_mille(packet.speed(), driver_warp, inner_tech);
    let kept = crate::packet::kept_per_mille(caught);

    let mut delivered = [0i32; 3];
    for (kind, amount) in packet.minerals.iter().enumerate() {
        let share = i32::from(*amount) * kept / 1000;
        delivered[kind] = share;
        planet.surface_min[kind] += share;
    }
    let damage = crate::packet::damage(packet.speed(), driver_warp, inner_tech, packet.mass());
    Some((target, delivered, damage))
}

/// Fly the Mystery Trader a year.
///
/// `MoveThings` (`10b0:1af7`), before production. One year in twenty-five it
/// changes its mind: it always **speeds up**, and one time in three it also
/// picks a new destination on the edge of the galaxy. Then it covers the square
/// of its warp toward wherever it is going, and stops when it arrives.
fn move_trader(state: &mut GameState, rng: &mut Rng) {
    let Some(trader) = state.trader.as_mut() else {
        return;
    };
    // A universe size class, from the planet count the game info gave us; the
    // original reads `game.mdSize` directly.
    let size = i32::from(state.galaxy_planets).max(1);
    if trader.warp <= 12 && rng.random(25) == 0 {
        if rng.random(3) == 0 {
            // The edge it heads for, and how far along that edge.
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
            trader.destination = crate::movement::Point::new(
                i16::try_from(x).unwrap_or(i16::MAX),
                i16::try_from(y).unwrap_or(i16::MAX),
            );
        }
        trader.warp = (trader.warp + 1) & 0x0F;
    }

    let range = trader.range();
    let target = trader.destination;
    let distance = crate::movement::distance(trader.position, target);
    if distance <= f64::from(range) {
        trader.position = target;
    } else {
        trader.position = crate::movement::advance(trader.position, target, range);
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
    /// `grobj` for a waypoint aimed at a `THING`.
    const GROBJ_THING: u8 = 8;

    let fleet = &state.fleets[index];
    let owner = usize::try_from(fleet.owner).ok()?;
    let waypoint = fleet.waypoints.first()?;
    if waypoint.target_class != GROBJ_THING {
        return None;
    }
    // A waypoint holds a thing's full id; a wormhole's own id is the low nine
    // bits of it.
    let id = waypoint.target? & 0x01FF;

    let near = state.wormholes.iter().position(|w| w.id == id)?;
    let partner = state.wormholes[near].partner & 0x01FF;
    let far = state.wormholes.iter().position(|w| w.id == partner)?;

    let bit = 1u16 << (owner & 0x0F);
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
fn trade_with_trader(state: &mut GameState, rng: &mut Rng) -> Vec<(u16, crate::wormhole::Gift)> {
    use crate::message::{fleet_name_word, fleet_object, id, Message};
    use crate::wormhole::{part, tech_levels, Gift, TRADE_GOODS};

    let Some(trader) = state.trader.as_ref() else {
        return Vec::new();
    };
    let at = trader.position;
    let carried = trader.part;
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
        let met = state
            .trader
            .as_ref()
            .is_some_and(|t| t.detected_by & bit != 0);
        if met {
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
        if let Some(trader) = state.trader.as_mut() {
            trader.detected_by |= bit;
        }
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
            // A ship, which this engine cannot build: it needs the game's own
            // Mystery Trader hulls.
            done.push((fleet_id, Gift::Ship));
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
    let size = i32::from(state.galaxy_planets).max(1);
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

/// The lowest fleet number a player is not already using.
///
/// Fleet numbers are per player and are reused once a fleet is gone, which is
/// why this looks for the first gap rather than counting.
#[must_use]
pub fn next_fleet_id(state: &GameState, owner: i16) -> u16 {
    let mut used: Vec<u16> = state
        .fleets
        .iter()
        .filter(|f| f.owner == owner)
        .map(|f| f.id)
        .collect();
    used.sort_unstable();
    let mut id = 1;
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
/// completes everything it wanted is dropped; one that is only part-built
/// keeps its progress for next year.
///
/// Returns what was completed, as `(item id, count)` pairs, and separately the
/// ships finished, as `(design slot, count)`.
///
/// `designs` is the owning player's design list, indexed by slot; a ship entry
/// naming a slot the list does not hold is skipped rather than guessed at.
fn run_queue(
    planet: &mut Planet,
    race: &crate::Race,
    designs: &[crate::design::ShipDesign],
    tech: [u8; 6],
    available: &mut [i32; COST_PARTS],
    ships_built: &mut Vec<(u8, i32)>,
) -> Vec<(u16, i32)> {
    let mut completed: Vec<(u16, i32)> = Vec::new();
    let mut queue = std::mem::take(&mut planet.queue);

    for entry in &mut queue {
        if entry.ship {
            // A ship costs its design; anything finished joins a fleet at the
            // planet. A design we do not hold is left alone rather than guessed.
            let Some(slot) = u8::try_from(entry.item).ok() else {
                continue;
            };
            let Some(cost) = designs
                .get(usize::from(slot))
                .and_then(crate::design::ShipDesign::cost)
            else {
                continue;
            };
            let outcome = build_item(
                crate::production::ItemCost {
                    minerals: cost.minerals,
                    resources: cost.resources,
                },
                entry.count,
                entry.completion,
                available,
                false,
            );
            if outcome.built > 0 {
                ships_built.push((slot, outcome.built));
            }
            entry.count = outcome.remaining;
            entry.completion = outcome.completion_pct;
            continue;
        }
        let Some(cost) = planetary_item_cost(entry.item, race, false) else {
            continue; // an item this does not cost yet, such as a packet
        };
        let auto = entry.is_auto();

        // Auto-build installations are capped by what the planet will be able
        // to operate; a manual order was already clamped when it was queued.
        let mut wanted = entry.count;
        if auto {
            wanted = wanted.min(auto_build_cap(planet, race, entry.item));
        }

        let outcome = build_item(cost, wanted, entry.completion, available, auto);
        if outcome.built > 0 {
            match item::auto_builds(entry.item).unwrap_or(entry.item) {
                item::MINE => planet.mines += i16::try_from(outcome.built).unwrap_or(0),
                item::FACTORY => planet.factories += i16::try_from(outcome.built).unwrap_or(0),
                item::MIN_TERRAFORM | item::MAX_TERRAFORM => {
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
        entry.count = outcome.remaining;
        entry.completion = outcome.completion_pct;
    }

    // Drop anything finished; an auto-build entry stays even at zero, because
    // it becomes buildable again as the planet grows.
    queue.retain(|e| e.count > 0 || e.is_auto());
    planet.queue = queue;
    completed
}

/// Move one fleet along its current leg.
///
/// A fleet covers `warp^2` light years a year toward its next waypoint,
/// stopping exactly on it if that would overshoot. On arrival the waypoint is
/// consumed, so the following one becomes the next leg.
///
/// Fuel is deducted, and a fleet that cannot afford the whole leg travels only
/// as far as its fuel allows and arrives empty — which is what the original
/// does before dropping the fleet's warp.
///
/// Returns the distance travelled, or `None` if the fleet had nowhere to go.
fn move_fleet(fleet: &mut Fleet, designs: &[crate::design::ShipDesign], ife: bool) -> Option<i32> {
    let (target, warp) = fleet.next_leg()?;
    let from = fleet.position;
    let d = distance(from, target);
    if d <= 0.0 {
        return None;
    }

    let range = if designs.is_empty() {
        None
    } else {
        Some(fleet.fuel_range(designs, warp, ife))
    };
    let travel = travel_this_year(i16::from(warp), d, range);
    if travel > 0 && !designs.is_empty() {
        let burned = fleet.fuel_use(designs, warp, travel, ife);
        fleet.cargo.fuel = (fleet.cargo.fuel - burned).max(0);
    }
    let to = advance(from, target, travel);
    fleet.position = to;

    if to == target {
        // Arrived: this waypoint is done with, and the fleet is orbiting
        // whatever it named.
        // A waypoint aimed at a `THING` — a wormhole, say — names no planet,
        // so arriving at one leaves the fleet in deep space.
        fleet.orbiting = fleet
            .waypoints
            .get(1)
            .filter(|w| w.target_class != 8)
            .and_then(|w| w.target);
        if !fleet.waypoints.is_empty() {
            fleet.waypoints.remove(0);
        }
    } else {
        fleet.orbiting = None;
        // The leg continues from where the fleet now is.
        if let Some(here) = fleet.waypoints.first_mut() {
            here.position = to;
        }
    }
    Some(travel)
}
