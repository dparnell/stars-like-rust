//! Writing a game out as the files the original engine reads: a `.hst` and one
//! `.mN` per player.
//!
//! This is the mirror of [`crate::load`]. Where that turns a decoded file into
//! a [`GameState`], this turns a [`GameState`] back into records and assembles
//! them into files, so a game this project **generated** can be saved and
//! opened again rather than living only in memory.
//!
//! ## What it is not
//!
//! It is **not** the path a loaded game saves through. A real save file is
//! mostly data this project models partially or not at all — messages, battle
//! recordings, scores, per-player scan history — and re-deriving a file from
//! the simulation would quietly drop all of it. A game opened from disk is
//! still written back by replacing only the blocks the player edited (see
//! `stars_ui::App::to_bytes`). This module is for files that have no original
//! to preserve.
//!
//! ## What a written file contains
//!
//! The block order mirrors `fixtures/incoming/turn0/`, a real game as the New
//! Game wizard left it:
//!
//! ```text
//! .hst   header, one player block each, one planet block each, every
//!        player's ship designs, every player's fleets (each followed by its
//!        waypoints), every player's starbase designs, the object section,
//!        five battle plans each, footer
//! .mN    header, that player's block, their planets, their designs, their
//!        fleets and waypoints, their starbase designs, their battle plans,
//!        footer
//! ```
//!
//! A fleet the player has named carries its name in a block after its
//! waypoints; one they have not carries nothing, which is why no file in the
//! fixtures has a single such block.
//!
//! What a fresh game has none of — messages, battles, scores, the other
//! players' partially-scanned planets — is simply absent, which is what the
//! turn-0 fixture's own `.mN` files look like.
//!
//! ## What is not written
//!
//! [`GameState`] does not model everything a player block holds. The fields it
//! has no source for are written as zero and named here so the gap is visible:
//! the race emblem (`logo`), the per-player message filter, the victory-point
//! bookkeeping, and the thirty bytes from offset 82 to 111 that nothing has
//! identified. Space objects (minefields, packets, wormholes) are written as an
//! empty section, because [`GameState`] does not carry them.

use stars_formats::block::Block;
use stars_formats::{
    BattlePlanRecord, DesignRecord, FileHeader, FileType, FleetRecord, FormatError, PlanetRecord,
    PlayerRecord, ProductionQueueRecord, ResearchState, Result, StarsFile, WaypointRecord,
};

use crate::ai::Control;
use crate::design::ShipDesign;
use crate::fleet::Fleet;
use crate::planet::Planet;
use crate::race::{Prt, Race};
use crate::{GameState, Player};

/// Size of the fixed player/race region of a type-6 block.
const PLAYER_FIXED_LEN: usize = 0x70;

/// Offset of `PLAYER.idPlanetHome` within that region.
const HOME_PLANET_OFFSET: usize = 8;

/// Offset of `PLAYER.lSalt`.
const SALT_OFFSET: usize = 12;

/// The salt `GenerateWorld` gives every computer player.
const AI_SALT: u32 = 0x094D_ABEE;

/// The turn a starting design records as its design year (`SHDEF.turn`).
const DESIGN_TURN: u16 = 1;

/// The `det` byte of a design block, and the low bit of its second byte, as
/// every design block in the fixtures carries them.
const DESIGN_FLAGS0: u8 = 3;
/// The low bits of a design block's second byte.
const DESIGN_FLAGS1: u8 = 1;

/// A waypoint that sits on a planet: object class 1 with `fValidTask` set.
const WAYPOINT_ON_PLANET: u8 = 0x11;

/// The five battle plans a new game gives every player.
///
/// `(plan id, tactic, primary target, secondary target, attack who, name)`,
/// transcribed from `rgbtlplanT` and checked byte for byte against the turn-0
/// fixture. The plan id of the last two is **3 in both**, which is what the
/// file holds; the field is four bits wide in this crate's decoder but only the
/// low two of them appear to be the plan number.
const DEFAULT_BATTLE_PLANS: [(u8, u8, u8, u8, u8, &str); 5] = [
    (0, 4, 3, 1, 2, "Default"),
    (1, 4, 2, 3, 2, "Kill Starbase"),
    (2, 3, 3, 4, 2, "Max-Defense"),
    (3, 1, 5, 0, 2, "Sniper"),
    (3, 0, 0, 0, 2, "Chicken"),
];

/// Write the host file for a game.
///
/// # Errors
/// [`FormatError::Malformed`] if a record does not fit its block — a name
/// longer than a length byte can count, or a block over 1023 bytes.
pub fn host_file(state: &GameState) -> Result<Vec<u8>> {
    let header = FileHeader::new(
        state.seed,
        FileType::Host,
        HOST_PLAYER,
        state.turn.unsigned_abs(),
        salt_for(state.seed, HOST_PLAYER, state.turn),
    );
    let mut body = Vec::new();

    // Only the first player's block carries the universe's planet count; every
    // other one reads zero in every host file in the fixtures.
    let planets = u16::try_from(all_planets(state).count()).unwrap_or(u16::MAX);
    for (index, player) in state.players.iter().enumerate() {
        let mut record = player_record(state, index, player)?;
        record.planets = if index == 0 { planets } else { 0 };
        body.push(block(6, record.encode()?)?);
    }
    for planet in all_planets(state) {
        body.push(block(13, planet_record(planet).encode())?);
        if !planet.queue.is_empty() {
            body.push(block(28, queue_record(planet).encode())?);
        }
    }
    for (index, designs) in state.designs.iter().enumerate() {
        push_designs(&mut body, state, index, designs, false)?;
    }
    for index in 0..state.players.len() {
        push_fleets(&mut body, state, index)?;
    }
    for (index, designs) in state.designs.iter().enumerate() {
        push_designs(&mut body, state, index, designs, true)?;
    }
    // The object section: a count record and that many objects. A generated
    // game has none, so the count is zero.
    body.push(block(43, 0u16.to_le_bytes().to_vec())?);
    for index in 0..state.players.len() {
        push_battle_plans(&mut body, index)?;
    }

    StarsFile::build(&header, &body, footer(state))
}

/// Write one player's turn file.
///
/// # Errors
/// [`FormatError::Malformed`] if `player` is not in the game, or if a record
/// does not fit its block.
pub fn player_file(state: &GameState, player: usize) -> Result<Vec<u8>> {
    let record = state
        .players
        .get(player)
        .ok_or_else(|| FormatError::Malformed(format!("no player {player} in this game")))?;
    let number = u8::try_from(player)
        .map_err(|_| FormatError::Malformed(format!("player {player} is out of range")))?;

    let header = FileHeader::new(
        state.seed,
        FileType::Turn,
        number,
        state.turn.unsigned_abs(),
        salt_for(state.seed, number, state.turn),
    );
    let owner = i16::from(number);
    let mine: Vec<&Planet> = all_planets(state)
        .filter(|p| p.owner == Some(owner))
        .collect();

    let mut header_record = player_record(state, player, record)?;
    header_record.planets = u16::try_from(mine.len()).unwrap_or(u16::MAX);
    let mut body = vec![block(6, header_record.encode()?)?];

    for planet in mine {
        body.push(block(13, planet_record(planet).encode())?);
        if !planet.queue.is_empty() {
            body.push(block(28, queue_record(planet).encode())?);
        }
    }
    if let Some(designs) = state.designs.get(player) {
        push_designs(&mut body, state, player, designs, false)?;
    }
    push_fleets(&mut body, state, player)?;
    if let Some(designs) = state.designs.get(player) {
        push_designs(&mut body, state, player, designs, true)?;
    }
    push_battle_plans(&mut body, player)?;

    StarsFile::build(&header, &body, footer(state))
}

/// Every planet the state knows about, in id order.
///
/// A game read from a file splits its planets in two: the ones it holds in
/// full and the ones it only knows about from a distance. A host file
/// describes every planet in the universe, and for an unowned one there is
/// nothing more to know than its environment and concentrations, so both lists
/// are written.
fn all_planets(state: &GameState) -> impl Iterator<Item = &Planet> {
    let mut all: Vec<&Planet> = state
        .planets
        .iter()
        .chain(state.known_planets.iter())
        .collect();
    all.sort_by_key(|p| p.id);
    all.into_iter()
}

/// The player number a shared file carries.
const HOST_PLAYER: u8 = 31;

/// The two-byte footer every `.hst` and `.mN` in the fixtures ends with.
fn footer(_state: &GameState) -> Vec<u8> {
    vec![0, 0]
}

/// Frame one record as a block.
fn block(type_id: u8, data: Vec<u8>) -> Result<Block> {
    Block::new(type_id, data)
}

/// A deterministic 11-bit cipher salt.
///
/// Any value works — it is stored in the header the reader seeds from — but it
/// must be stable so that writing the same game twice gives the same bytes.
fn salt_for(game_id: u32, player: u8, turn: i16) -> u16 {
    let mixed = game_id.rotate_left(u32::from(player) % 32)
        ^ (u32::from(turn.unsigned_abs()) << 7)
        ^ (u32::from(player) << 3);
    ((mixed ^ (mixed >> 11)) & 0x07FF) as u16
}

/// Build a player's type-6 record.
fn player_record(state: &GameState, index: usize, player: &Player) -> Result<PlayerRecord> {
    let number = u8::try_from(index)
        .map_err(|_| FormatError::Malformed(format!("player {index} is out of range")))?;
    let owner = i16::from(number);

    let mut fixed = vec![0u8; PLAYER_FIXED_LEN];
    let home = state
        .planets
        .iter()
        .find(|p| p.homeworld && p.owner == Some(owner))
        .or_else(|| state.planets.iter().find(|p| p.owner == Some(owner)));
    if let Some(home) = home {
        let id = u16::try_from(home.id).unwrap_or(0);
        fixed[HOME_PLANET_OFFSET..HOME_PLANET_OFFSET + 2].copy_from_slice(&id.to_le_bytes());
    }
    if matches!(player.control, Control::Computer { .. }) {
        fixed[SALT_OFFSET..SALT_OFFSET + 4].copy_from_slice(&AI_SALT.to_le_bytes());
    }

    let designs = state.designs.get(index);
    let ship_designs = designs.map_or(0, |d| {
        d.iter()
            .take(usize::from(crate::startup::FIRST_STARBASE_SLOT))
            .filter(|d| d.hull_id >= 0)
            .count()
    });
    let starbase_designs = designs.map_or(0, |d| {
        d.iter()
            .skip(usize::from(crate::startup::FIRST_STARBASE_SLOT))
            .filter(|d| d.hull_id >= 0)
            .count()
    });

    Ok(PlayerRecord {
        player_number: number,
        ship_design_count: u8::try_from(ship_designs).unwrap_or(u8::MAX),
        starbase_design_count: u8::try_from(starbase_designs).unwrap_or(0),
        // Overwritten by the caller: a host file puts the universe's planet
        // count on the first player alone, a turn file the player's own count.
        planets: 0,
        fleets: u16::try_from(state.fleets.iter().filter(|f| f.owner == owner).count())
            .unwrap_or(u16::MAX),
        logo: player.logo,
        full_data: true,
        flags_byte: player.control.to_flags(),
        player_relations: if player.relations.is_empty() {
            vec![0; state.players.len()]
        } else {
            player.relations.clone()
        },
        race: Some(race_record(&player.race, number)),
        singular_name: player.name.clone(),
        plural_name: player.plural_name.clone(),
        research: Some(research_state(player)),
        fixed,
        // A player block whose plural name is empty carries one extra zero
        // byte, and one with a plural name does not. Exact across all 74,903
        // player blocks in the fixtures.
        trailing: if player.plural_name.is_empty() {
            vec![0]
        } else {
            Vec::new()
        },
    })
}

/// Build the race half of a player record.
fn race_record(race: &Race, player_id: u8) -> stars_formats::RaceRecord {
    use stars_formats::{Economy, HabRange, RaceRecord};

    let axis = |i: usize| -> HabRange {
        if race.is_immune(i) {
            HabRange {
                center: None,
                low: None,
                high: None,
            }
        } else {
            HabRange {
                center: Some(race.env_center[i].unsigned_abs()),
                low: Some(race.env_min[i].unsigned_abs()),
                high: Some(race.env_max[i].unsigned_abs()),
            }
        }
    };
    let stat = |s: crate::race::RaceStat| u8::try_from(race.stat(s)).unwrap_or(0);
    let mut research_cost = [1u8; 6];
    for (field, cost) in research_cost.iter_mut().enumerate() {
        *cost = u8::try_from(race.attrs[crate::race::RaceStat::TechBonus1 as usize + field])
            .unwrap_or(1);
    }

    RaceRecord {
        player_id,
        gravity: axis(0),
        temperature: axis(1),
        radiation: axis(2),
        growth_rate: race.pct_ideal_growth.unsigned_abs(),
        full_data: true,
        research_percentage: 15,
        economy: Economy {
            resource_per_colonist: stat(crate::race::RaceStat::ResGen),
            produce_per_factory: stat(crate::race::RaceStat::FactProd),
            factory_build_cost: stat(crate::race::RaceStat::FactBuild),
            factories_operated: stat(crate::race::RaceStat::FactOperate),
            produce_per_mine: stat(crate::race::RaceStat::MineProd),
            mine_build_cost: stat(crate::race::RaceStat::MineBuild),
            mines_operated: stat(crate::race::RaceStat::MineOperate),
        },
        spend_leftover_points: stat(crate::race::RaceStat::UseLeftover),
        research_cost,
        prt: file_prt(race.prt()),
        // Only the fourteen selectable traits can be stored; the internal bits
        // above them are not part of a race record. See `crate::opponents`.
        lrt_bits: (race.lrt_bits & 0x3FFF) as u16,
        expensive_tech_starts_at_level_3: race.has_lrt(crate::race::lrt::TECH3),
        factories_cost_one_less_germanium: race.has_lrt(crate::race::lrt::CHEAP_FACT),
        singular_name: String::new(),
        plural_name: String::new(),
    }
}

/// Map the simulation's primary trait onto the file's.
fn file_prt(prt: Option<Prt>) -> stars_formats::Prt {
    use stars_formats::Prt as F;
    match prt {
        Some(Prt::He) => F::HE,
        Some(Prt::Ss) => F::SS,
        Some(Prt::Wm) => F::WM,
        Some(Prt::Ca) => F::CA,
        Some(Prt::Is) => F::IS,
        Some(Prt::Sd) => F::SD,
        Some(Prt::Pp) => F::PP,
        Some(Prt::It) => F::IT,
        Some(Prt::Ar) => F::AR,
        Some(Prt::Joat) => F::JOAT,
        None => F::Unknown(0),
    }
}

/// Build the research half of a player record.
fn research_state(player: &Player) -> ResearchState {
    let mut points = [0u32; 6];
    for (i, p) in player.research.points.iter().enumerate() {
        points[i] = u32::try_from(*p).unwrap_or(0);
    }
    ResearchState {
        levels: player.research.levels,
        points,
        budget_pct: player.research_pct,
        current_field: u8::try_from(player.research.current_field).unwrap_or(0),
        next_field: match player.research.next_field {
            crate::research::NextField::Field(f) => u8::try_from(f).unwrap_or(0),
            crate::research::NextField::Same => 6,
            crate::research::NextField::Lowest => 7,
        },
        last_year_resources: u32::try_from(player.research_last_year).unwrap_or(0),
    }
}

/// Build a planet's type-13 record.
fn planet_record(planet: &Planet) -> PlanetRecord {
    use stars_formats::{Concentration, Environment, Installations, Minerals, Starbase};

    let owned = planet.owner.is_some();
    let env = Environment {
        gravity: planet.env[0].unsigned_abs(),
        temperature: planet.env[1].unsigned_abs(),
        radiation: planet.env[2].unsigned_abs(),
    };
    // The original environment is stored only when it differs — the `fIncEVO`
    // flag is what says so, and an untouched planet leaves it clear.
    let original = planet
        .env_orig
        .filter(|orig| *orig != planet.env)
        .map(|orig| Environment {
            gravity: orig[0].unsigned_abs(),
            temperature: orig[1].unsigned_abs(),
            radiation: orig[2].unsigned_abs(),
        });

    PlanetRecord {
        block_type: 13,
        id: u16::try_from(planet.id).unwrap_or(0),
        owner: planet.owner.and_then(|o| u8::try_from(o).ok()),
        detail: 7,
        homeworld: planet.homeworld,
        include: true,
        has_starbase: planet.starbase,
        terraformed: original.is_some(),
        has_installations: owned,
        artifact: planet.artifact,
        has_surface_minerals: owned,
        routing: false,
        first_year: false,
        concentration: Some(Concentration {
            ironium: planet.min_conc[0],
            boranium: planet.min_conc[1],
            germanium: planet.min_conc[2],
        }),
        min_level: planet.min_level,
        environment: Some(env),
        original_environment: original,
        // `uPopGuess` is the owner's own estimate, which a fresh planet records
        // as a quarter of its population; the decoder scales it by 1000.
        pop_guess: owned.then(|| u32::try_from(planet.pop / 4).unwrap_or(0) * 1000),
        defense_guess: owned.then_some(0),
        surface_minerals: owned.then(|| Minerals {
            ironium: u32::try_from(planet.surface_min[0]).unwrap_or(0),
            boranium: u32::try_from(planet.surface_min[1]).unwrap_or(0),
            germanium: u32::try_from(planet.surface_min[2]).unwrap_or(0),
        }),
        population: owned.then(|| u32::try_from(planet.pop).unwrap_or(0) * 100),
        installations: owned.then(|| Installations {
            delta_pop: planet.delta_pop,
            mines: u16::try_from(planet.mines).unwrap_or(0),
            factories: u16::try_from(planet.factories).unwrap_or(0),
            defenses: u16::try_from(planet.defenses).unwrap_or(0),
            scanner: 0,
            artifact: planet.artifact,
            no_research: planet.no_research,
            unused5: 0,
            unused2: 0,
        }),
        starbase: (planet.starbase && owned).then(|| Starbase {
            design: planet.starbase_design.unwrap_or(0) & 0x0F,
            damage_pct: 0,
            fling_dest: 0,
            warp: 0,
            no_heal: false,
        }),
        route_dest: None,
        trailing: Vec::new(),
    }
}

/// Build a planet's production queue record.
fn queue_record(planet: &Planet) -> ProductionQueueRecord {
    use stars_formats::production::{QueueClass, QueueItem};
    ProductionQueueRecord {
        planet_id: None,
        items: planet
            .queue
            .iter()
            .map(|entry| QueueItem {
                count: u16::try_from(entry.count).unwrap_or(0),
                item: entry.item,
                class: if entry.ship {
                    QueueClass::Fleet
                } else {
                    QueueClass::Planet
                },
                completion: u16::try_from(entry.completion).unwrap_or(0),
            })
            .collect(),
    }
}

/// Append one player's ship or starbase design blocks.
fn push_designs(
    body: &mut Vec<Block>,
    state: &GameState,
    player: usize,
    designs: &[ShipDesign],
    starbases: bool,
) -> Result<()> {
    let first = usize::from(crate::startup::FIRST_STARBASE_SLOT);
    let range: Box<dyn Iterator<Item = usize>> = if starbases {
        Box::new(first..designs.len())
    } else {
        Box::new(0..designs.len().min(first))
    };
    for slot in range {
        let design = &designs[slot];
        if design.hull_id < 0 {
            continue;
        }
        let number = u8::try_from(if starbases { slot - first } else { slot }).unwrap_or(0);
        let built = ships_of(state, player, slot);
        body.push(block(
            26,
            design_record(design, number, starbases, built).encode()?,
        )?);
    }
    Ok(())
}

/// How many ships of one design slot the player has in play.
///
/// A starbase design counts the planets flying it; a ship design counts the
/// stacks across every fleet.
fn ships_of(state: &GameState, player: usize, slot: usize) -> u32 {
    let owner = i16::try_from(player).unwrap_or(-1);
    if slot >= usize::from(crate::startup::FIRST_STARBASE_SLOT) {
        // `PLANET.isb` indexes the starbase list, so subtract the offset the
        // combined design array puts them at.
        let design =
            u8::try_from(slot - usize::from(crate::startup::FIRST_STARBASE_SLOT)).unwrap_or(0);
        return u32::try_from(
            state
                .planets
                .iter()
                .filter(|p| p.owner == Some(owner) && p.starbase_design == Some(design))
                .count(),
        )
        .unwrap_or(0);
    }
    let slot = u8::try_from(slot).unwrap_or(u8::MAX);
    u32::try_from(
        state
            .fleets
            .iter()
            .filter(|f| f.owner == owner)
            .flat_map(|f| f.stacks.iter())
            .filter(|s| s.design == slot)
            .map(|s| i64::from(s.count))
            .sum::<i64>(),
    )
    .unwrap_or(0)
}

/// Build a design's type-26 record.
fn design_record(design: &ShipDesign, number: u8, starbase: bool, built: u32) -> DesignRecord {
    DesignRecord {
        full_design: true,
        transferred: false,
        starbase,
        design_number: number,
        hull_id: u8::try_from(design.hull_id).unwrap_or(0),
        pic: design.picture,
        armor: Some(design.stored_armor),
        mass: None,
        turn_designed: Some(DESIGN_TURN),
        total_built: Some(built),
        total_remaining: Some(built),
        slots: design
            .slots
            .iter()
            .map(|s| stars_formats::Slot {
                category: s.category,
                item_id: s.item,
                count: s.count,
            })
            .collect(),
        name: design.name.clone(),
        flags0: DESIGN_FLAGS0,
        flags1: DESIGN_FLAGS1,
        trailing: Vec::new(),
    }
}

/// Append one player's fleets, each followed by its waypoints.
fn push_fleets(body: &mut Vec<Block>, state: &GameState, player: usize) -> Result<()> {
    let owner = i16::try_from(player).unwrap_or(-1);
    for fleet in state.fleets.iter().filter(|f| f.owner == owner) {
        let waypoints = waypoint_records(fleet);
        body.push(block(16, fleet_record(fleet, waypoints.len()).encode(16))?);
        for waypoint in &waypoints {
            body.push(block(waypoint.block_type(), waypoint.encode())?);
        }
        // A fleet the player has named carries the name after its waypoints,
        // and one they have not carries nothing at all.
        if let Some(name) = fleet.name.as_ref().filter(|n| !n.is_empty()) {
            body.push(block(
                stars_formats::FLEET_NAME_BLOCK,
                stars_formats::encode_user_string(name),
            )?);
        }
    }
    Ok(())
}

/// Build a fleet's type-16 record.
fn fleet_record(fleet: &Fleet, waypoints: usize) -> FleetRecord {
    let mut slots = 0u16;
    let mut ships = Vec::new();
    let mut byte_counts = true;
    for stack in &fleet.stacks {
        if stack.count <= 0 {
            continue;
        }
        slots |= 1 << stack.design;
        let count = u16::try_from(stack.count).unwrap_or(u16::MAX);
        if count > 0xFF {
            byte_counts = false;
        }
        ships.push(stars_formats::ShipStack {
            design_slot: stack.design,
            count,
        });
    }

    FleetRecord {
        id: fleet.id,
        owner: u8::try_from(fleet.owner).unwrap_or(0),
        detail: 7,
        include: true,
        repeat_orders: fleet.repeat_orders,
        dead: false,
        byte_counts,
        flags_high: 0,
        orbiting: fleet.orbiting,
        x: fleet.position.x.unsigned_abs(),
        y: fleet.position.y.unsigned_abs(),
        ship_slots: slots,
        ships,
        cargo: Some(stars_formats::Cargo {
            ironium: u32::try_from(fleet.cargo.minerals[0]).unwrap_or(0),
            boranium: u32::try_from(fleet.cargo.minerals[1]).unwrap_or(0),
            germanium: u32::try_from(fleet.cargo.minerals[2]).unwrap_or(0),
            population: u32::try_from(fleet.cargo.colonists).unwrap_or(0) * 100,
            fuel: u32::try_from(fleet.cargo.fuel).unwrap_or(0),
        }),
        battle_plan: Some(fleet.battle_plan),
        waypoint_count: Some(u8::try_from(waypoints).unwrap_or(u8::MAX)),
        damage: Vec::new(),
        delta_x: None,
        delta_y: None,
        warp: None,
        mass: None,
        warp_high: 0,
        partial_unused: 0,
        trailing: Vec::new(),
    }
}

/// Build a fleet's waypoint records.
fn waypoint_records(fleet: &Fleet) -> Vec<WaypointRecord> {
    fleet
        .waypoints
        .iter()
        .map(|w| {
            let task_data = w
                .transport
                .as_ref()
                .map(stars_formats::TransportTask::encode)
                .unwrap_or_default();
            WaypointRecord {
                x: w.position.x.unsigned_abs(),
                y: w.position.y.unsigned_abs(),
                object_id: w.target,
                object_type: WAYPOINT_ON_PLANET,
                object_class: WAYPOINT_ON_PLANET & 0x0F,
                valid_task: true,
                no_auto_track: false,
                warp: w.warp,
                task: w.task,
                task_data,
            }
        })
        .collect()
}

/// Append one player's five default battle plans.
fn push_battle_plans(body: &mut Vec<Block>, player: usize) -> Result<()> {
    let race_id = u8::try_from(player).unwrap_or(0) & 0x0F;
    for (plan_id, tactic, primary, secondary, attack, name) in DEFAULT_BATTLE_PLANS {
        let record = BattlePlanRecord {
            race_id,
            plan_id,
            tactic,
            primary_target: primary,
            secondary_target: secondary,
            attack_who: attack,
            name: name.to_string(),
            trailing: Vec::new(),
        };
        body.push(block(30, record.encode()?)?);
    }
    Ok(())
}
