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
//! | New game creation | [`newgame`] | universe, homeworlds and starting fleets |
//! | Race advantage points | [`advantage`] | complete |
//!
//! Alternate Reality races live on their starbases, so their population,
//! mining and scanning all depend on ship designs; those paths return `None`
//! until the design layer lands in Step 5.

#![forbid(unsafe_code)]

pub mod advantage;
pub mod ai;
pub mod battle;
pub mod bombing;
pub mod browser;
pub mod components;
pub mod design;
pub mod fleet;
pub mod ground;
pub mod hab;
pub mod load;
pub mod message;
pub mod minefield;
pub mod mining;
pub mod movement;
pub mod newgame;
pub mod opponents;
pub mod orders;
pub mod packet;
pub mod parts;
pub mod patrol;
pub mod planet;
pub mod population;
pub mod presets;
pub mod production;
pub mod race;
pub mod relations;
pub mod replay;
pub mod research;
pub mod resources;
pub mod rng;
pub mod save;
pub mod scanning;
pub mod score;
pub mod scoresheet;
pub mod startup;
pub mod terraform;
pub mod turn;
pub mod victory;
pub mod visibility;
pub mod wormhole;

// `battle::distance` is board geometry and `movement::distance` is interstellar,
// so neither is re-exported bare; use the module path.
pub use advantage::{advantage_points, innate_race_habitability};
pub use battle::{movement_this_round, start_square, target_score, torpedo_accuracy, Tactic};
pub use design::{Cost, DesignSlot, ShipDesign};
pub use fleet::{Cargo, Fleet, ShipStack};
pub use hab::{calc_planet_max_pop, max_pop_for_hab, pct_planet_desirability};
pub use load::{design_from_record, planet_from_record, race_from_record, LoadReport};
pub use mining::{mine_minerals, minerals_mined, mines_operating};
pub use movement::{advance, distance, travel_per_year, travel_this_year, FuelStack, Point};
pub use newgame::{
    generate as new_game, Created, Density, NewGame, NewPlayer, Size, StartDistance,
};
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
pub use turn::{generate_turn, generate_turn_with_orders, SkippedStep, TurnOrders, TurnReport};

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
    /// How this player regards each other player, indexed by player number:
    /// `0` neutral, `1` friend, `2` enemy (`PLAYER.rgmdRelation`, at offset
    /// `0x70`). Empty when the file did not carry the table.
    ///
    /// Read by remote terraforming, which helps a friend's planet and harms
    /// anyone else's — see [`terraform::remote_intent`].
    pub relations: Vec<u8>,
    /// Whether a person or one of the built-in opponents plays this slot.
    ///
    /// Defaults to [`ai::Control::Human`], which is the safe reading for a
    /// player whose record was not in the file: the engine waits for orders
    /// rather than inventing them.
    pub control: ai::Control,
    /// The race's singular name, which is also how the game names the player
    /// (`"Humanoid"`).
    pub name: String,
    /// The race's plural name (`"Humanoids"`). May be empty: a good many
    /// player blocks in the fixtures store no plural.
    pub plural_name: String,
    /// The race emblem, `0..=31` — which of the game's logos the player is
    /// drawn with. Cosmetic, and stored in the player block, so it travels
    /// with the player.
    pub logo: u8,
    /// The production queue a planet this player takes starts with
    /// (`PLAYER.zpq1`), and whether it starts exempt from the research skim.
    ///
    /// Applied by [`crate::orders::apply_default_queue`] whenever a planet
    /// changes hands, which is where the original applies it.
    pub default_queue: stars_formats::DefaultQueue,
    /// The salt of the player's turn password, or `0` for none.
    ///
    /// Stars! stores a checksum of the password rather than the password —
    /// [`stars_formats::password`] — and this is that value, at offset 12 of
    /// the player block. It travels with the player and is what a
    /// `rtChgPassword` order changes. Nothing in the engine reads it: whether
    /// to ask for a password before opening a turn is a question for a
    /// frontend, not for the simulation.
    pub password: u32,
    /// Which messages this player has silenced (`rtMsgFilt`, type 33).
    ///
    /// A reading preference rather than a rule: nothing in the simulation looks
    /// at it, and a filtered message is still sent and still written to the
    /// file. It is carried so that a player's choice survives a turn — see
    /// [`crate::message::set_filtered`].
    pub message_filter: stars_formats::MessageFilter,
    /// Which Mystery Trader technologies this player has already been given
    /// (`PLAYER.grbitTrader`, offset `0x52`), as a mask of
    /// [`wormhole::part`] bits.
    ///
    /// The Trader checks it before handing anything over, and never gives the
    /// same part twice.
    pub trader_parts: u16,
    /// Whether this is a shareware ("crippled") game for this player
    /// (`PLAYER.fCrippled`, bit 1 of the word at offset `0x54`).
    ///
    /// It caps technology at level 10 instead of 26, which the Mystery Trader
    /// respects: it will not push a shareware player past the ceiling their
    /// own research could not reach either.
    pub crippled: bool,
    /// The player's battle plans, in file order, as the type-30 blocks carry
    /// them (`PLAYER.rgbtlplan`).
    ///
    /// A fleet's `battle_plan` indexes this list. A new game gives everyone
    /// the five in [`DEFAULT_BATTLE_PLANS`]; the player can rename, retune,
    /// add and delete them, which is the `rtBtlPlan` (30) order operation.
    pub battle_plans: Vec<stars_formats::BattlePlanRecord>,
    /// The planets this player has scanned, by id — what a planet record's
    /// `det & 0xff > 2` says in the player's own file. Kept for the computer
    /// players, whose turns are run from the host's state and who must not
    /// see what they have not scanned. Not written to any file.
    pub explored: std::collections::BTreeSet<i16>,
}

/// The five battle plans a new game gives every player.
///
/// `(plan id, tactic, primary target, secondary target, attack who, name)`,
/// transcribed from `rgbtlplanT` and checked byte for byte against the turn-0
/// fixture. The plan id of the last two is **3 in both**, which is what the
/// file holds; the field is four bits wide in the decoder but only the low two
/// of them appear to be the plan number.
pub const DEFAULT_BATTLE_PLANS: [(u8, u8, u8, u8, u8, &str); 5] = [
    (0, 4, 3, 1, 2, "Default"),
    (1, 4, 2, 3, 2, "Kill Starbase"),
    (2, 3, 3, 4, 2, "Max-Defense"),
    (3, 1, 5, 0, 2, "Sniper"),
    (3, 0, 0, 0, 2, "Chicken"),
];

/// The battle plans a new game gives player `player`.
#[must_use]
pub fn default_battle_plans(player: usize) -> Vec<stars_formats::BattlePlanRecord> {
    let race_id = u8::try_from(player).unwrap_or(0) & 0x0F;
    DEFAULT_BATTLE_PLANS
        .iter()
        .map(|(plan_id, tactic, primary, secondary, attack, name)| {
            stars_formats::BattlePlanRecord {
                race_id,
                plan_id: *plan_id,
                tactic: *tactic,
                primary_target: *primary,
                secondary_target: *secondary,
                attack_who: *attack,
                name: (*name).to_string(),
                trailing: Vec::new(),
            }
        })
        .collect()
}

impl Player {
    /// Whether this player considers `other` a friend (relations value `1`).
    ///
    /// A player with no relations table recorded regards nobody as a friend,
    /// which is the neutral reading rather than a guess.
    #[must_use]
    pub fn regards_as_friend(&self, other: i16) -> bool {
        usize::try_from(other)
            .ok()
            .and_then(|i| self.relations.get(i))
            .is_some_and(|r| *r == 1)
    }

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
            relations: Vec::new(),
            control: ai::Control::Human,
            name: "Humanoid".to_string(),
            plural_name: "Humanoids".to_string(),
            logo: 0,
            default_queue: stars_formats::DefaultQueue::default(),
            password: 0,
            message_filter: stars_formats::MessageFilter::new(),
            trader_parts: 0,
            crippled: false,
            battle_plans: default_battle_plans(0),
            explored: std::collections::BTreeSet::new(),
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
    /// The game's "one human player" option (`GAME.fSinglePlr`).
    ///
    /// Read by the Mystery Trader, which withholds its late-game bonus of
    /// extra ships from a single-player game — see [`crate::wormhole`].
    pub single_player: bool,
    /// Whether this game **is the tutorial**: `fTutorial`, bit 3 of the
    /// game's flag word in the `.xy`. Distinct from [`Self::tutorial`], the
    /// client's "the tutor is running" flag. The engine reads it in one
    /// place: a Jack of All Trades' built-in scanner is a fixed 40 light
    /// years normal and 20 penetrating in the tutorial, where in any other
    /// game it scales with Electronics (`GetShdefScannerRange`,
    /// `1038:50d0`).
    pub tutorial_game: bool,
    /// Whether the **tutorial** is running: bit 11 of the client's `gd`
    /// word, which `StartTutor` sets (`10f8:0748`) and `EndTutor` clears
    /// (`10f8:0c21`).
    ///
    /// It is not a game setting and lives in no file — the tutorial is
    /// generated on the player's own machine, so the host code sees the
    /// client's flag. Two things read it: `GetProductionCosts`
    /// (`10d0:4885`) prices a factory at two of every mineral instead of
    /// four germanium, and the AI does not shuffle its planets
    /// (`docs/formulas/ai.md`).
    pub tutorial: bool,
    /// The salt of the **host's** password; `0` when there is none.
    ///
    /// It guards host mode rather than a turn, and it lives in the host file
    /// alone: `WriteDataFile` writes it as a type-36 record straight after the
    /// player blocks, and only for a host file with a password set
    /// (`save.c`, `if (iPlayer == iNoPlayer && lSaltCur != 0)`). The loader
    /// reads it from the same place (`file.c`). Like every password in Stars!,
    /// what is stored is a salt of the typed text and never the text — see
    /// [`stars_formats::password`].
    pub host_password: u32,
    /// How many planets the whole galaxy has (`GAME.cPlanMax`), which the
    /// "owns a percentage of all planets" victory condition is measured
    /// against. Zero when the file did not say.
    pub galaxy_planets: i16,
    /// The game's victory conditions, exactly as `GAME.rgvc` holds them. Read
    /// through [`stars_formats::GameInfo`]; see [`crate::victory`].
    pub victory: [u8; stars_formats::victory::COUNT],
    /// What the host has to tell each player about the year just generated.
    ///
    /// Cleared at the start of a turn and written into each player's file. See
    /// [`crate::message`].
    pub messages: Vec<crate::message::Message>,
    /// Every minefield in play. They are `THING`s in the file, and the only
    /// kind of `THING` this engine models — see [`crate::minefield`].
    pub minefields: Vec<crate::minefield::Minefield>,
    /// Every mineral packet in flight. See [`crate::packet`].
    pub packets: Vec<crate::packet::Packet>,
    /// Every wormhole end in play. See [`crate::wormhole`].
    pub wormholes: Vec<crate::wormhole::Wormhole>,
    /// Every Mystery Trader in the galaxy.
    ///
    /// A list rather than a single Trader because a galaxy may hold several:
    /// 374 of the fixture files carry two and some carry three. Each trades
    /// separately, and each remembers on its own who has already been.
    pub traders: Vec<crate::wormhole::MysteryTrader>,
    /// The scoreboard, one row per player, as the file carries it.
    ///
    /// The host writes these; a client only reads them. What one player is
    /// told about another is entirely the host's choice — see
    /// [`crate::score::Standing`] — so this is **not** recomputed on load.
    pub standings: Vec<crate::score::Standing>,
    /// Every player's score year by year, from the `.hN` history file, in turn
    /// order. Indexed by player; empty for a player the file says nothing
    /// about.
    pub timeline: Vec<Vec<crate::score::Year>>,
    /// Any space object none of the above covers, exactly as the file held
    /// it.
    ///
    /// Nothing here simulates them, and that is the point: they are carried
    /// verbatim so that writing a game back does not quietly delete them.
    pub other_things: Vec<stars_formats::Thing>,
}

impl GameState {
    /// Fill in planet coordinates and names from the universe file.
    ///
    /// A game's planet positions live only in the `.xy`, so a `GameState` built
    /// from a `.hst` or `.mN` alone has none. Call this with the matching
    /// universe to complete it; without it, everything that needs a position
    /// degrades rather than guessing — a newly built ship, for instance, is
    /// reported but cannot be given a fleet.
    ///
    /// Planets are matched by id, which is the record's index in both files.
    /// Returns how many planets were placed.
    pub fn apply_universe(&mut self, universe: &stars_formats::Universe) -> usize {
        // The universe file is also where the game's own settings live: the
        // research option, the size of the galaxy and the victory conditions.
        // A `.hst` or `.mN` carries none of them.
        if let Ok(info) = universe.game() {
            self.slow_tech = info.flags & stars_formats::game_flag::SLOW_TECH != 0;
            self.tutorial_game = info.flags & stars_formats::game_flag::TUTORIAL != 0;
            self.single_player = info.flags & stars_formats::game_flag::SINGLE_PLAYER != 0;
            self.galaxy_planets = info.planets;
            self.victory = info.victory_bytes();
        }
        let resolved = universe.planets_resolved();
        let mut placed = 0;
        for planet in self.planets.iter_mut().chain(self.known_planets.iter_mut()) {
            let Ok(index) = usize::try_from(planet.id) else {
                continue;
            };
            let Some(source) = resolved.get(index) else {
                continue;
            };
            planet.position = Some(movement::Point::new(
                i16::try_from(source.x).unwrap_or(i16::MAX),
                i16::try_from(source.y).unwrap_or(i16::MAX),
            ));
            planet.name = source.name;
            placed += 1;
        }
        placed
    }

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
            single_player: false,
            tutorial: false,
            tutorial_game: false,
            host_password: 0,
            galaxy_planets: 0,
            victory: [0; stars_formats::victory::COUNT],
            messages: Vec::new(),
            minefields: Vec::new(),
            packets: Vec::new(),
            wormholes: Vec::new(),
            traders: Vec::new(),
            standings: Vec::new(),
            timeline: Vec::new(),
            other_things: Vec::new(),
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
