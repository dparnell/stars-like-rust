//! Building a [`GameState`] from a decoded save file.
//!
//! This is the bridge between the format layer and the simulation. It takes a
//! `stars_formats::StarsFile` — already decoded and decrypted by the caller —
//! and assembles the planets, players and races the turn engine works on.
//!
//! It does no I/O of its own: the caller reads the bytes and decodes them, so
//! the core stays free of the filesystem.
//!
//! ## What a file can tell you
//!
//! A **host** file (`.hst`) holds every player and every planet, so it loads
//! completely. A **player** file (`.mN`) holds one player's view: their own
//! planets in full, everyone else's as fragments, and only their own race. The
//! loader takes what is there and leaves the rest out rather than inventing
//! it — [`LoadReport`] says what was skipped.

use crate::planet::Planet;
use crate::production::QueueItem;
use crate::race::{Prt, Race, RaceStat};
use crate::research::{NextField, Research, TECH_FIELDS};
use crate::{GameState, Player};

use stars_formats::{
    planet_records_in, player_records_in, production_queue_records, DesignRecord, PlanetRecord,
    RaceRecord, StarsFile,
};

/// What a load did and did not manage to include.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadReport {
    /// Planets loaded with enough detail to simulate.
    pub planets_loaded: usize,
    /// Planets present in the file but too sparse to simulate — another
    /// player's, seen only at a distance.
    pub planets_partial: usize,
    /// Players whose race the file carries.
    pub players_loaded: usize,
    /// Ship and starbase designs loaded.
    pub designs_loaded: usize,
}

/// Convert a decoded race record into the simulation's race.
#[must_use]
pub fn race_from_record(record: &RaceRecord) -> Race {
    let mut attrs = [0i16; 16];
    attrs[RaceStat::ResGen as usize] = i16::from(record.economy.resource_per_colonist);
    attrs[RaceStat::FactProd as usize] = i16::from(record.economy.produce_per_factory);
    attrs[RaceStat::FactBuild as usize] = i16::from(record.economy.factory_build_cost);
    attrs[RaceStat::FactOperate as usize] = i16::from(record.economy.factories_operated);
    attrs[RaceStat::MineProd as usize] = i16::from(record.economy.produce_per_mine);
    attrs[RaceStat::MineBuild as usize] = i16::from(record.economy.mine_build_cost);
    attrs[RaceStat::MineOperate as usize] = i16::from(record.economy.mines_operated);
    attrs[RaceStat::MajorAdv as usize] = prt_from_abbrev(record.prt.abbrev()) as i16;
    for (field, cost) in record.research_cost.iter().enumerate() {
        attrs[RaceStat::TechBonus1 as usize + field] = i16::from(*cost);
    }

    // A `0xFF` bound marks the race immune on that axis, which the simulation
    // detects through a negative upper bound.
    let axis = |h: stars_formats::HabRange| -> (i8, i8, i8) {
        match (h.center, h.low, h.high) {
            (Some(c), Some(l), Some(x)) => (c as i8, l as i8, x as i8),
            _ => (0, 0, -1),
        }
    };
    let (gc, gl, gh) = axis(record.gravity);
    let (tc, tl, th) = axis(record.temperature);
    let (rc, rl, rh) = axis(record.radiation);

    Race {
        attrs,
        lrt_bits: u32::from(record.lrt_bits),
        env_center: [gc, tc, rc],
        env_min: [gl, tl, rl],
        env_max: [gh, th, rh],
        pct_ideal_growth: record.growth_rate as i8,
    }
}

fn prt_from_abbrev(abbrev: &str) -> Prt {
    match abbrev {
        "HE" => Prt::He,
        "SS" => Prt::Ss,
        "WM" => Prt::Wm,
        "CA" => Prt::Ca,
        "IS" => Prt::Is,
        "SD" => Prt::Sd,
        "PP" => Prt::Pp,
        "IT" => Prt::It,
        "AR" => Prt::Ar,
        _ => Prt::Joat,
    }
}

/// Convert a decoded planet record, if it carries enough to simulate.
///
/// A planet needs an owner, an environment, mineral concentrations, a
/// population and its installations. Anything less is a planet seen from a
/// distance, and is skipped rather than filled in with guesses.
#[must_use]
pub fn planet_from_record(record: &PlanetRecord) -> Option<Planet> {
    let owner = record.owner?;
    let env = record.environment?;
    let conc = record.concentration?;
    let pop = record.population?;
    let installations = record.installations?;

    let surface = record.surface_minerals.unwrap_or(stars_formats::Minerals {
        ironium: 0,
        boranium: 0,
        germanium: 0,
    });

    Some(Planet {
        id: i16::try_from(record.id).ok()?,
        owner: Some(i16::from(owner)),
        env: [
            env.gravity as i8,
            env.temperature as i8,
            env.radiation as i8,
        ],
        min_conc: [conc.ironium, conc.boranium, conc.germanium],
        // The sub-concentration accumulator is not stored per planet in the
        // formats decoded so far; it starts full.
        min_level: [0, 0, 0],
        surface_min: [
            i32::try_from(surface.ironium).unwrap_or(0),
            i32::try_from(surface.boranium).unwrap_or(0),
            i32::try_from(surface.germanium).unwrap_or(0),
        ],
        // Stored in hundreds of colonists; the record multiplies that out.
        pop: i32::try_from(pop / 100).ok()?,
        delta_pop: installations.delta_pop,
        mines: i16::try_from(installations.mines).ok()?,
        factories: i16::try_from(installations.factories).ok()?,
        homeworld: record.homeworld,
        starbase: record.has_starbase,
        queue: Vec::new(),
        no_research: installations.no_research,
    })
}

/// Convert a decoded design record.
#[must_use]
pub fn design_from_record(record: &DesignRecord) -> crate::design::ShipDesign {
    crate::design::ShipDesign {
        hull_id: i16::from(record.hull_id),
        slots: record
            .slots
            .iter()
            .map(|s| crate::design::DesignSlot {
                category: s.category,
                item: s.item_id,
                count: s.count,
            })
            .collect(),
    }
}

impl GameState {
    /// Build a game state from a decoded save file.
    ///
    /// Reads the file's **latest** segment, since a player file can hold an
    /// unopened turn followed by the current one.
    ///
    /// Returns the state and a [`LoadReport`] describing what it could and
    /// could not include.
    #[must_use]
    pub fn from_file(file: &StarsFile) -> (Self, LoadReport) {
        let segment = file.latest_segment();
        let blocks = file.segment_blocks(segment);
        let mut report = LoadReport::default();

        let mut state = Self::new(segment.header.game_id);
        state.turn = i16::try_from(segment.header.turn).unwrap_or(0);

        // Players, indexed by player number so a sparse file still lines up.
        if let Ok(records) = player_records_in(blocks) {
            for record in &records {
                let index = usize::from(record.player_number);
                if state.players.len() <= index {
                    state
                        .players
                        .resize_with(index + 1, || Player::new(Race::humanoid()));
                }
                if let Some(race) = record.race.as_ref() {
                    let mut player = Player::new(race_from_record(race));
                    player.research_pct = race.research_percentage;
                    if let Some(research) = record.research {
                        player.research = Research {
                            levels: research.levels,
                            points: research
                                .points
                                .map(|p| i32::try_from(p).unwrap_or(i32::MAX)),
                            current_field: usize::from(research.current_field).min(TECH_FIELDS - 1),
                            next_field: NextField::from_raw(research.next_field),
                        };
                        player.research_last_year =
                            i32::try_from(research.last_year_resources).unwrap_or(0);
                    }
                    state.players[index] = player;
                    report.players_loaded += 1;
                }
            }
        }

        // Planets. A production queue block follows the planet it belongs to,
        // so the most recent planet claims it.
        for record in planet_records_in(blocks) {
            match planet_from_record(&record) {
                Some(planet) => {
                    state.planets.push(planet);
                    report.planets_loaded += 1;
                }
                None => report.planets_partial += 1,
            }
        }

        // Queues are stored in planet order for the planets that have one.
        let queues = production_queue_records(file);
        for (queue, planet) in queues.iter().zip(state.planets.iter_mut()) {
            planet.queue = queue
                .items
                .iter()
                .map(|i| QueueItem {
                    count: i32::from(i.count),
                    item: i.item,
                    completion: i32::from(i.completion),
                })
                .collect();
        }

        report.designs_loaded = stars_formats::design_records(file)
            .map(|d| d.len())
            .unwrap_or(0);

        (state, report)
    }
}
