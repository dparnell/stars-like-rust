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

use crate::mining::mine_minerals;
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
    /// Auto and remote terraforming.
    Terraforming,
    /// Random events (comet strikes and the like).
    RandomEvents,
    /// Score calculation.
    Scores,
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
    /// Population change per planet id, in units of 100 colonists.
    pub population: Vec<(i16, i32)>,
    /// Resources each player put into research.
    pub research_spending: Vec<i32>,
    /// Technology levels gained, per player.
    pub breakthroughs: Vec<Vec<Breakthrough>>,
    /// Pipeline steps not performed, and therefore not reflected above.
    pub skipped: Vec<SkippedStep>,
}

/// Advance the game by one year.
///
/// The steps performed, in the original's order, are: mining, the per-planet
/// resource and research split, population update, and the research advance.
/// Everything else is reported in [`TurnReport::skipped`].
///
/// `rng` supplies the mining rounding draws; pass a generator seeded from the
/// game to reproduce a specific turn.
pub fn generate_turn(state: &mut GameState, rng: &mut Rng) -> TurnReport {
    let mut report = TurnReport {
        skipped: vec![
            SkippedStep::Orders,
            SkippedStep::FleetMovement,
            SkippedStep::Things,
            SkippedStep::Combat,
            SkippedStep::Terraforming,
            SkippedStep::RandomEvents,
            SkippedStep::Scores,
        ],
        research_spending: vec![0; state.players.len()],
        breakthroughs: vec![Vec::new(); state.players.len()],
        ..TurnReport::default()
    };

    // --- Produce: mine first, so this year's minerals are on the surface
    // before anything can spend them.
    for index in 0..state.planets.len() {
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

        let no_research = state.planets[index].no_research;
        let Some(budget) =
            planet_budget(&state.planets[index], &race, research_pct, 0, no_research)
        else {
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
        let built = run_queue(&mut state.planets[index], &race, &mut available);
        for (i, slot) in state.planets[index].surface_min.iter_mut().enumerate() {
            *slot = available[i];
        }
        if !built.is_empty() {
            let id = state.planets[index].id;
            report.built.push((id, built));
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

/// Run a planet's production queue for one year.
///
/// Items are taken in order, each spending from what is left. An item that
/// completes everything it wanted is dropped; one that is only part-built
/// keeps its progress for next year.
///
/// Returns what was completed, as `(item id, count)` pairs. Only the planetary
/// installations are built here — ship designs need the design layer wired to
/// a fleet, which the turn pipeline does not have yet.
fn run_queue(
    planet: &mut Planet,
    race: &crate::Race,
    available: &mut [i32; COST_PARTS],
) -> Vec<(u16, i32)> {
    let mut completed: Vec<(u16, i32)> = Vec::new();
    let mut queue = std::mem::take(&mut planet.queue);

    for entry in &mut queue {
        let Some(cost) = planetary_item_cost(entry.item, race, false) else {
            continue; // a ship design; not built here yet
        };
        let auto = entry.item >= item::AUTO_BUILD_BASE;

        // Auto-build installations are capped by what the planet will be able
        // to operate; a manual order was already clamped when it was queued.
        let mut wanted = entry.count;
        if auto {
            wanted = wanted.min(auto_build_cap(planet, race, entry.item));
        }

        let outcome = build_item(cost, wanted, entry.completion, available, auto);
        if outcome.built > 0 {
            let bare = if auto {
                entry.item - item::AUTO_BUILD_BASE
            } else {
                entry.item
            };
            match bare {
                item::MINE => planet.mines += i16::try_from(outcome.built).unwrap_or(0),
                item::FACTORY => planet.factories += i16::try_from(outcome.built).unwrap_or(0),
                _ => {}
            }
            completed.push((entry.item, outcome.built));
        }
        entry.count = outcome.remaining;
        entry.completion = outcome.completion_pct;
    }

    // Drop anything finished; an auto-build entry stays even at zero, because
    // it becomes buildable again as the planet grows.
    queue.retain(|e| e.count > 0 || e.item >= item::AUTO_BUILD_BASE);
    planet.queue = queue;
    completed
}
