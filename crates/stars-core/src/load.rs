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

use crate::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use crate::movement::Point;
use crate::planet::Planet;
use crate::production::QueueItem;
use crate::race::{lrt, Prt, Race, RaceStat};
use crate::research::{NextField, Research, TECH_FIELDS};
use crate::{GameState, Player};

use stars_formats::{
    planet_records_in, player_records_in, production_queues_by_planet, DesignRecord, FleetRecord,
    PlanetRecord, RaceRecord, StarsFile,
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
    /// Fleets loaded.
    pub fleets_loaded: usize,
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
    attrs[RaceStat::UseLeftover as usize] = i16::from(record.spend_leftover_points);
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
        // The fourteen selectable traits are a 16-bit field; two more that the
        // race wizard also offers are stored as checkbox bits at offset 81, at
        // the bit positions `ibitRaceTech3` (29) and `ibitRaceCheapFact` (31)
        // occupy in the engine's own `grbitAttr`.
        lrt_bits: u32::from(record.lrt_bits)
            | (u32::from(record.expensive_tech_starts_at_level_3) << lrt::TECH3)
            | (u32::from(record.factories_cost_one_less_germanium) << lrt::CHEAP_FACT),
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
        // Filled in by GameState::apply_universe; the .mN/.hst carry no coords.
        position: None,
        name: None,
        id: i16::try_from(record.id).ok()?,
        detail: crate::planet::Detail::Full,
        owner: Some(i16::from(owner)),
        env: [
            env.gravity as i8,
            env.temperature as i8,
            env.radiation as i8,
        ],
        env_orig: record.original_environment.map(|e| {
            [
                i8::try_from(e.gravity).unwrap_or(0),
                i8::try_from(e.temperature).unwrap_or(0),
                i8::try_from(e.radiation).unwrap_or(0),
            ]
        }),
        min_conc: [conc.ironium, conc.boranium, conc.germanium],
        // The sub-concentration decay accumulators, in 1/256ths. A planet
        // block stores one byte per mineral whose accumulator is non-zero and
        // omits the rest; `0` is what the mining formula reads as "full".
        min_level: record.min_level,
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
        defenses: i16::try_from(installations.defenses).unwrap_or(0),
        homeworld: record.homeworld,
        starbase: record.has_starbase,
        artifact: record.artifact,
        starbase_design: record.starbase.map(|s| s.design),
        queue: Vec::new(),
        no_research: installations.no_research,
        // Stored one-based so that zero can mean "no route"; `AutoRouteFleet`
        // (`1080:1e52`) subtracts the one before it looks the planet up.
        route_dest: record
            .route_dest
            .filter(|id| *id != 0)
            .and_then(|id| i16::try_from(id).ok())
            .map(|id| id - 1),
    })
}

/// Build a planet from a record that does **not** describe it fully.
///
/// A player's file records the planets it owns in full and everything else at
/// whatever detail it has scanned: some carry environment and mineral
/// concentrations, some only a header. Those planets cannot be simulated — the
/// population and installations simply are not there — but a player's own view
/// of the galaxy is exactly what they are, and the AI's colonisation search
/// works from it.
///
/// The unknown fields are left at their [`Planet::unowned`] defaults; callers
/// must check [`Planet::detail`] before trusting anything beyond the id, the
/// owner, and whatever [`Detail::Scanned`] guarantees.
#[must_use]
pub fn partial_planet_from_record(record: &PlanetRecord) -> Option<Planet> {
    use crate::planet::Detail;

    let id = i16::try_from(record.id).ok()?;
    let mut planet = Planet::unowned(id);
    planet.owner = record.owner.map(i16::from);
    planet.starbase = record.has_starbase;
    planet.starbase_design = record.starbase.map(|s| s.design);
    planet.homeworld = record.homeworld;
    planet.artifact = record.artifact;

    planet.detail = match (record.environment, record.concentration) {
        (Some(env), Some(conc)) => {
            planet.env = [
                env.gravity as i8,
                env.temperature as i8,
                env.radiation as i8,
            ];
            planet.min_conc = [conc.ironium, conc.boranium, conc.germanium];
            planet.env_orig = record.original_environment.map(|e| {
                [
                    i8::try_from(e.gravity).unwrap_or(0),
                    i8::try_from(e.temperature).unwrap_or(0),
                    i8::try_from(e.radiation).unwrap_or(0),
                ]
            });
            Detail::Scanned
        }
        _ => Detail::Minimal,
    };
    Some(planet)
}

/// Convert a decoded fleet record.
///
/// A fleet seen only at a distance carries no ship list; those are skipped,
/// because a fleet with no stacks is not something the simulation can act on.
#[must_use]
pub fn fleet_from_record(record: &FleetRecord) -> Option<Fleet> {
    if record.dead || record.ships.is_empty() {
        return None;
    }
    let cargo = record.cargo.map_or(Cargo::default(), |c| Cargo {
        minerals: [
            i32::try_from(c.ironium).unwrap_or(0),
            i32::try_from(c.boranium).unwrap_or(0),
            i32::try_from(c.germanium).unwrap_or(0),
        ],
        colonists: i32::try_from(c.population).unwrap_or(0),
        fuel: i32::try_from(c.fuel).unwrap_or(0),
    });

    // Damage is recorded per design slot, so it is matched onto the stacks.
    let damage_for = |slot: u8| -> (i32, i32) {
        record
            .damage
            .iter()
            .find(|d| d.design_slot == slot)
            .map_or((0, 0), |d| (i32::from(d.ships_pct), i32::from(d.armor_pct)))
    };

    Some(Fleet {
        name: None,
        repeat_orders: record.repeat_orders,
        waypoints: Vec::new(),
        id: record.id,
        owner: i16::from(record.owner),
        position: Point::new(
            i16::try_from(record.x).unwrap_or(0),
            i16::try_from(record.y).unwrap_or(0),
        ),
        orbiting: record.orbiting,
        stacks: record
            .ships
            .iter()
            .map(|s| {
                let (damaged_pct, damage_pct) = damage_for(s.design_slot);
                ShipStack {
                    design: s.design_slot,
                    count: i32::from(s.count),
                    damaged_pct,
                    damage_pct,
                }
            })
            .collect(),
        cargo,
        battle_plan: record.battle_plan.unwrap_or(0),
        warp: record.warp,
    })
}

/// Convert a decoded design record.
#[must_use]
pub fn design_from_record(record: &DesignRecord) -> crate::design::ShipDesign {
    crate::design::ShipDesign {
        name: record.name.clone(),
        picture: record.pic,
        stored_armor: record.armor.unwrap_or(0),
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
                    player.name.clone_from(&record.singular_name);
                    player.plural_name.clone_from(&record.plural_name);
                    player.logo = record.logo;
                    if let Some(queue) = record.default_queue.clone() {
                        player.default_queue = queue;
                    }
                    player.control = crate::ai::Control::from_flags(record.flags_byte);
                    player.password = record.password.unwrap_or(0);
                    player.relations.clone_from(&record.player_relations);
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

        // Minefields, out of the object section. The other kinds of THING —
        // packets, wormholes, the Mystery Trader — are not modelled, so they
        // are left where they are rather than half-loaded.
        for thing in stars_formats::thing_section(file).things {
            let stars_formats::ThingKind::Minefield(mine) = thing.kind else {
                // Not modelled, but not thrown away either.
                state.other_things.push(thing);
                continue;
            };
            state.minefields.push(crate::minefield::Minefield {
                id: thing.id,
                owner: i16::from(thing.player),
                position: Point::new(thing.x, thing.y),
                mines: mine.mines,
                kind: mine.kind,
                detonating: mine.detonate,
                detected_by: mine.players_seen,
                visible_to: mine.players_seen_now,
                turn: thing.turn,
            });
        }

        // Battle plans. A type-30 block names its owner in its low nibble, and
        // the blocks come in the order the player holds them, so the first one
        // a player owns replaces the defaults and the rest append.
        {
            let mut seen: Vec<bool> = vec![false; state.players.len()];
            for block in blocks
                .iter()
                .filter(|b| b.block_type() == stars_formats::block::BlockType::BattlePlan)
            {
                let Ok(plan) = stars_formats::BattlePlanRecord::from_payload(&block.data) else {
                    continue;
                };
                let owner = usize::from(plan.race_id);
                let Some(player) = state.players.get_mut(owner) else {
                    continue;
                };
                if !seen.get(owner).copied().unwrap_or(false) {
                    player.battle_plans.clear();
                    if let Some(flag) = seen.get_mut(owner) {
                        *flag = true;
                    }
                }
                player.battle_plans.push(plan);
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
                None => {
                    // Not a planet this file describes fully. Keep it, apart
                    // from the simulated ones: it is part of what this player
                    // knows about the galaxy.
                    if let Some(planet) = partial_planet_from_record(&record) {
                        state.known_planets.push(planet);
                    }
                    report.planets_partial += 1;
                }
            }
        }

        // A queue block carries no planet id: it belongs to the planet block it
        // immediately follows, and only planets that have a queue get one. They
        // therefore cannot be zipped against the planet list by index.
        for (id, queue) in production_queues_by_planet(blocks) {
            let Ok(id) = i16::try_from(id) else { continue };
            let Some(planet) = state.planets.iter_mut().find(|p| p.id == id) else {
                continue;
            };
            planet.queue = queue
                .items
                .iter()
                .map(|i| QueueItem {
                    count: i32::from(i.count),
                    item: i.item,
                    ship: i.class == stars_formats::QueueClass::Fleet,
                    completion: i32::from(i.completion),
                })
                .collect();
        }

        // Designs, indexed by owner and then by design slot.
        //
        // A design block says which *slot* it fills and whether it is a
        // starbase, but not whose it is: the file lists each player's designs
        // in turn, and the player blocks say how many each has. So the blocks
        // are handed out by consuming those counts in file order. A `.mN` has
        // one player block and its own designs; a `.hst` has one per player and
        // all of them, which is why attributing them to the file's header
        // player — 31 in a host file — put every design on player 15.
        //
        // Anything left over after the counts are used up (a foreign design a
        // player has learned, or a file whose counts do not add up) falls back
        // to the file's own player, which is what a `.mN` wants.
        let mut ship_quota: Vec<(usize, usize)> = Vec::new();
        let mut base_quota: Vec<(usize, usize)> = Vec::new();
        if let Ok(records) = stars_formats::player_records_in(blocks) {
            for record in &records {
                let owner = usize::from(record.player_number).min(15);
                ship_quota.push((owner, usize::from(record.ship_design_count)));
                base_quota.push((owner, usize::from(record.starbase_design_count)));
            }
        }
        ship_quota.reverse();
        base_quota.reverse();

        let fallback = usize::from(segment.header.player).min(15);
        let mut designs_loaded = 0usize;
        for block in blocks {
            if block.block_type() != stars_formats::block::BlockType::Design {
                continue;
            }
            let Ok(record) = stars_formats::DesignRecord::from_payload(&block.data) else {
                continue;
            };
            designs_loaded += 1;
            if !record.full_design {
                continue;
            }
            let quota = if record.starbase {
                &mut base_quota
            } else {
                &mut ship_quota
            };
            while quota.last().is_some_and(|(_, left)| *left == 0) {
                quota.pop();
            }
            let owner = match quota.last_mut() {
                Some((owner, left)) => {
                    *left -= 1;
                    *owner
                }
                None => fallback,
            };

            let slot = if record.starbase {
                usize::from(crate::startup::FIRST_STARBASE_SLOT) + usize::from(record.design_number)
            } else {
                usize::from(record.design_number)
            };
            if state.designs.len() <= owner {
                state.designs.resize_with(owner + 1, Vec::new);
            }
            if state.designs[owner].len() <= slot {
                state.designs[owner].resize_with(slot + 1, || crate::design::ShipDesign {
                    name: String::new(),
                    picture: 0,
                    stored_armor: 0,
                    hull_id: -1,
                    slots: Vec::new(),
                });
            }
            state.designs[owner][slot] = design_from_record(&record);
        }
        report.designs_loaded = designs_loaded;

        // Fleets, with the waypoint blocks that follow each one. Association
        // is by position in the block stream: a fleet's waypoints are written
        // immediately after it.
        let mut pending: Option<Fleet> = None;
        for block in blocks {
            match block.type_id {
                // Fleet blocks: full (16), and the two partial forms.
                16..=18 => {
                    if let Some(fleet) = pending.take() {
                        state.fleets.push(fleet);
                        report.fleets_loaded += 1;
                    }
                    pending = stars_formats::FleetRecord::decode(&block.data, block.type_id)
                        .as_ref()
                        .and_then(fleet_from_record);
                }
                // The name a player gave the fleet above, written after its
                // waypoints. See `stars_formats::FLEET_NAME_BLOCK`.
                stars_formats::FLEET_NAME_BLOCK => {
                    if let Some(fleet) = pending.as_mut() {
                        let name = stars_formats::decode_user_string(&block.data);
                        fleet.name = (!name.is_empty()).then_some(name);
                    }
                }
                // Waypoint blocks, which belong to the fleet above them.
                19 | 20 => {
                    if let (Some(fleet), Some(w)) = (
                        pending.as_mut(),
                        stars_formats::WaypointRecord::decode(&block.data),
                    ) {
                        fleet.waypoints.push(Waypoint {
                            position: Point::new(
                                i16::try_from(w.x).unwrap_or(0),
                                i16::try_from(w.y).unwrap_or(0),
                            ),
                            target: w.object_id,
                            target_class: w.object_class,
                            warp: w.warp,
                            task: w.task,
                            transport: w.transport(),
                            task_data: w.task_data.clone(),
                        });
                    }
                }
                _ => {}
            }
        }
        if let Some(fleet) = pending.take() {
            state.fleets.push(fleet);
            report.fleets_loaded += 1;
        }

        (state, report)
    }
}
