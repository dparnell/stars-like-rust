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

pub mod ai;
pub mod battle;
pub mod components;
pub mod design;
pub mod fleet;
pub mod hab;
pub mod load;
pub mod mining;
pub mod movement;
pub mod planet;
pub mod population;
pub mod production;
pub mod race;
pub mod research;
pub mod resources;
pub mod rng;
pub mod scanning;
pub mod terraform;
pub mod turn;

// `battle::distance` is board geometry and `movement::distance` is interstellar,
// so neither is re-exported bare; use the module path.
pub use battle::{movement_this_round, start_square, target_score, torpedo_accuracy, Tactic};
pub use design::{Cost, DesignSlot, ShipDesign};
pub use fleet::{Cargo, Fleet, ShipStack};
pub use hab::{calc_planet_max_pop, max_pop_for_hab, pct_planet_desirability};
pub use load::{design_from_record, planet_from_record, race_from_record, LoadReport};
pub use mining::{mine_minerals, minerals_mined, mines_operating};
pub use movement::{advance, distance, travel_per_year, travel_this_year, FuelStack, Point};
pub use planet::Planet;
pub use population::{chg_pop_from_planet, pct_true_max_growth, update_population, PopChange};
pub use production::{
    auto_build_cap, build_item, planet_budget, planetary_item_cost, BuildOutcome, ItemCost,
    PlanetBudget, QueueItem,
};
pub use race::{Prt, Race, RaceStat};
pub use research::{add_research, tech_level_cost, NextField, Research, TechField};
pub use resources::{
    factories_operating, max_factories, max_mines, max_operable_factories, max_operable_mines,
    resources_at_planet,
};
pub use rng::Rng;
pub use scanning::{combine_ranges, planet_scanner_range, ScannerRange};
pub use turn::{generate_turn, SkippedStep, TurnReport};

/// One player: their race, their research, and the settings that drive both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    /// The player's race.
    pub race: Race,
    /// Technology levels and research progress.
    pub research: research::Research,
    /// Share of resources spent on research, in percent (`PLAYER.pctResearch`).
    pub research_pct: u8,
    /// Resources research received from the most recent turn
    /// (`PLAYER.lResLastYear`).
    pub research_last_year: i32,
    /// Whether the player has been eliminated.
    pub dead: bool,
    /// Whether a person or one of the built-in opponents plays this slot.
    ///
    /// Defaults to [`ai::Control::Human`], which is the safe reading for a
    /// player whose record was not in the file: the engine waits for orders
    /// rather than inventing them.
    pub control: ai::Control,
}

impl Player {
    /// A player with the given race, at zero technology.
    #[must_use]
    pub fn new(race: Race) -> Self {
        Self {
            race,
            research: research::Research::default(),
            // The game's default research allocation.
            research_pct: 15,
            research_last_year: 0,
            dead: false,
            control: ai::Control::Human,
        }
    }
}

/// The complete, serializable state of a game at a single turn boundary.
///
/// Fleets and ship designs join this once the components table is decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    /// Turn counter, 0-based, matching the file header's field; the in-game
    /// year is [`GameState::year`].
    pub turn: i16,
    /// The game's random seed, as stored in the file header.
    pub seed: u32,
    /// Every planet in the universe, indexed by planet id.
    pub planets: Vec<Planet>,
    /// Planets this file records but does not describe fully — everything the
    /// player has scanned but does not own.
    ///
    /// Kept apart from [`Self::planets`] deliberately. These carry no
    /// population and no installations, so anything that simulates a planet
    /// must not see them; but a player's own view of the galaxy is exactly
    /// this list plus the owned planets, and decisions like the AI's
    /// colonisation search are made from that view. See
    /// [`crate::planet::Detail`].
    pub known_planets: Vec<Planet>,
    /// Every player.
    pub players: Vec<Player>,
    /// Every fleet in play.
    pub fleets: Vec<Fleet>,
    /// Each player's ship designs, indexed by design slot.
    pub designs: Vec<Vec<crate::design::ShipDesign>>,
    /// The game's "slower tech advances" option, which doubles research costs.
    pub slow_tech: bool,
}

impl GameState {
    /// An empty game at turn 0 (year 2400).
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self {
            turn: 0,
            seed,
            planets: Vec::new(),
            known_planets: Vec::new(),
            players: Vec::new(),
            fleets: Vec::new(),
            designs: Vec::new(),
            slow_tech: false,
        }
    }

    /// The in-game year: Stars! counts turns from 2400.
    #[must_use]
    pub fn year(&self) -> i32 {
        2400 + i32::from(self.turn)
    }

    /// The player owning `planet`, if it is owned.
    #[must_use]
    pub fn owner(&self, planet: &Planet) -> Option<&Player> {
        let owner = planet.owner?;
        self.players.get(usize::try_from(owner).ok()?)
    }

    /// The race of the player owning `planet`, if it is owned.
    #[must_use]
    pub fn owner_race(&self, planet: &Planet) -> Option<&Race> {
        Some(&self.owner(planet)?.race)
    }

    /// Advance every planet's population by one year.
    ///
    /// This is the `UpdatePopulations` step on its own; [`turn::generate_turn`]
    /// runs it in the right place in the pipeline.
    pub fn update_populations(&mut self) {
        let players = std::mem::take(&mut self.players);
        turn::update_populations(&mut self.planets, &players);
        self.players = players;
    }
}
