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

    // --- MoveFleets, which happens before Produce.
    for index in 0..state.fleets.len() {
        let owner = usize::try_from(state.fleets[index].owner).unwrap_or(usize::MAX);
        let designs = state.designs.get(owner).cloned().unwrap_or_default();
        let ife = state
            .players
            .get(owner)
            .is_some_and(|p| p.race.has_lrt(crate::race::lrt::IFE));
        if let Some(travelled) = move_fleet(&mut state.fleets[index], &designs, ife) {
            report.moved.push((state.fleets[index].id, travelled));
        }
    }

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
/// When the owner has **no** fleet there, the ships are reported but not
/// placed: a new fleet needs a position, and a planet's coordinates live in the
/// `.xy` file rather than in `GameState`. The cost has still been spent, which
/// is what the production side needs; only the fleet is missing.
fn add_ships_to_orbiting_fleet(
    state: &mut GameState,
    owner: i16,
    planet: i16,
    design: u8,
    count: i32,
) {
    let orbiting = u16::try_from(planet).ok();
    let Some(fleet) = state
        .fleets
        .iter_mut()
        .find(|f| f.owner == owner && f.orbiting == orbiting)
    else {
        return;
    };
    if let Some(stack) = fleet.stacks.iter_mut().find(|s| s.design == design) {
        stack.count += count;
    } else {
        fleet.stacks.push(crate::fleet::ShipStack {
            design,
            count,
            damaged_pct: 0,
            damage_pct: 0,
        });
    }
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
        fleet.orbiting = fleet.waypoints.get(1).and_then(|w| w.target);
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
