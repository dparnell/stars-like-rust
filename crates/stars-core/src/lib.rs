//! # stars-core
//!
//! The deterministic, platform-agnostic heart of the Stars! reimplementation.
//!
//! This crate owns the game model ([`GameState`]) and the simulation systems
//! that advance it, together with the turn/order processing pipeline and the
//! AI.
//!
//! ## Non-negotiable constraints
//!
//! * **Deterministic** — identical inputs and seed must produce identical
//!   outputs on every platform (native and wasm). All randomness flows through
//!   [`Rng`], a reproduction of the original engine's PRNG. The handful of
//!   formulas that use floating point (habitability, scanner combination,
//!   movement geometry) use only `sqrt`, `powf(0.25)` and exactly-representable
//!   constants, all of which IEEE-754 requires to be correctly rounded.
//! * **Headless** — no filesystem, no rendering, no platform APIs. File bytes
//!   enter and leave through `stars-formats`; presentation lives in the UI
//!   crates.
//!
//! ## Provenance
//!
//! Every formula here is reverse-engineered from `stars.2.7j.exe` and written
//! up under `docs/formulas/`, with the Ghidra address of the original routine
//! and the `MANUAL.PDF` page that documents the same rule. The specs are the
//! contract; this code is one implementation of them, and the golden vectors
//! under `docs/vectors/` are checked against both.
//!
//! ## What is implemented
//!
//! | Subsystem | Module | Status |
//! |-----------|--------|--------|
//! | PRNG | [`rng`] | complete (`Random`/`Randomize`) |
//! | Habitability & maximum population | [`hab`] | complete except Alternate Reality |
//! | Population growth & death | [`population`] | complete except Alternate Reality |
//! | Mining & concentration decay | [`mining`] | complete |
//! | Resources, mine/factory caps | [`resources`] | complete except Alternate Reality |
//! | Scanner ranges | [`scanning`] | ranges complete; per-design scanners need ship designs |
//! | Fleet movement & fuel | [`movement`] | geometry and fuel complete; engine tables need parts data |
//! | Production, research, combat, AI | — | delivery Step 4 |
//!
//! Alternate Reality races live on their starbases, so their population,
//! mining and scanning all depend on ship designs; those paths return `None`
//! until the design layer lands in Step 5.

#![forbid(unsafe_code)]

pub mod hab;
pub mod mining;
pub mod movement;
pub mod planet;
pub mod population;
pub mod race;
pub mod resources;
pub mod rng;
pub mod scanning;

pub use hab::{calc_planet_max_pop, max_pop_for_hab, pct_planet_desirability};
pub use mining::{mine_minerals, minerals_mined, mines_operating};
pub use movement::{advance, distance, travel_per_year, travel_this_year, FuelStack, Point};
pub use planet::Planet;
pub use population::{chg_pop_from_planet, pct_true_max_growth, update_population, PopChange};
pub use race::{Prt, Race, RaceStat};
pub use resources::{
    factories_operating, max_factories, max_mines, max_operable_factories, max_operable_mines,
    resources_at_planet,
};
pub use rng::Rng;
pub use scanning::{combine_ranges, planet_scanner_range, ScannerRange};

/// The complete, serializable state of a game at a single turn boundary.
///
/// Fleets, designs and the tech tree join this in Step 4; today it carries the
/// planetary state that the implemented systems operate on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    /// Turn counter, 0-based, matching the file header's field; the in-game
    /// year is [`GameState::year`].
    pub turn: i16,
    /// The game's random seed, as stored in the file header.
    pub seed: u32,
    /// Every planet in the universe, indexed by planet id.
    pub planets: Vec<Planet>,
    /// Every player's race.
    pub races: Vec<Race>,
}

impl GameState {
    /// An empty game at turn 0 (year 2400).
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self {
            turn: 0,
            seed,
            planets: Vec::new(),
            races: Vec::new(),
        }
    }

    /// The in-game year: Stars! counts turns from 2400.
    #[must_use]
    pub fn year(&self) -> i32 {
        2400 + i32::from(self.turn)
    }

    /// The race of the player owning `planet`, if it is owned.
    #[must_use]
    pub fn owner_race(&self, planet: &Planet) -> Option<&Race> {
        let owner = planet.owner?;
        self.races.get(usize::try_from(owner).ok()?)
    }

    /// Advance every planet's population by one year.
    ///
    /// This is the `UpdatePopulations` step of the original's turn pipeline
    /// (`FGenerateTurn`, `10b0:0000`). The rest of the pipeline — production,
    /// movement, combat — lands in Step 4.
    pub fn update_populations(&mut self) {
        for i in 0..self.planets.len() {
            let Some(owner) = self.planets[i].owner else {
                continue;
            };
            let Some(race) = self.races.get(usize::try_from(owner).unwrap_or(usize::MAX)) else {
                continue;
            };
            let race = race.clone();
            population::update_population(&mut self.planets[i], &race);
        }
    }
}
