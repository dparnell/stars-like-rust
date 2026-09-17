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
//! .mN    header, that player's block and a short one for each other player
//!        they know of, their planets and the partial records of the ones
//!        they have scanned, their designs and what they know of the others',
//!        their fleets and waypoints and the partial records of the fleets
//!        on their map, the starbase designs likewise, their battle plans,
//!        the battles, messages, standings and the space objects they can
//!        see, footer
//! ```
//!
//! What a player sees of everybody else is [`crate::visibility::view`] —
//! the original's `SetVisiblePlanFleet` passes, run by `FWriteDataFile`
//! (`1070:5964`) as each file is written — plus what this year's battles
//! showed them (`WriteBattles`, `1070:709c`): every design that fought
//! theirs is written in full, and so is one a Space Demolition player's
//! field hit or a Packet Physics player's packet was caught by
//! ([`GameState::revealed_designs`]). A War Monger sees every design on
//! their map in full (`SetVisPFFinish`, `1070:c41c`); a Claim Adjuster
//! sees every player record in full (`1070:5964`). See
//! `docs/formats/writing.md`.
//!
//! A fleet the player has named carries its name in a block after its
//! waypoints; one they have not carries nothing, which is why no file in the
//! fixtures has a single such block.
//!
//! What a fresh game has none of — battles, scores, the other players'
//! planets within scanner range — is simply absent, which is what the turn-0
//! fixture's own `.mN` files look like. Messages it does have: the five
//! `GenerateWorld` sends every player, which the fixture's `Game.m1` carries
//! and its `Game.m2`, a computer player's, does not.
//!
//! ## What is not written
//!
//! [`GameState`] does not model everything a player block holds. The fields it
//! has no source for are written as zero and named here so the gap is visible:
//! the race emblem (`logo`), the per-player message filter, the victory-point
//! bookkeeping, and the thirty bytes from offset 82 to 111 that nothing has
//! identified.

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
use crate::visibility::{Detail, View};
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
    // The host's password, straight after the player blocks and only when
    // there is one — `save.c`'s `if (iPlayer == iNoPlayer && lSaltCur != 0)`.
    // A host file with no password carries no such block, which is why every
    // fixture in this repository has none.
    if state.host_password != 0 {
        body.push(block(36, state.host_password.to_le_bytes().to_vec())?);
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
    // The object section: a count record and that many objects. Minefields are
    // the only kind of object this engine models.
    push_things(state, &mut body, None)?;
    for index in 0..state.players.len() {
        push_battle_plans(state, &mut body, index)?;
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
    let mut view = crate::visibility::view(state, player);
    let others = others_in_view(state, player, &mut view);
    let prt = record.race.prt();

    let mut header_record = player_record(state, player, record)?;
    header_record.planets = u16::try_from(view.planets.len()).unwrap_or(u16::MAX);
    let mut body = vec![block(6, header_record.encode()?)?];
    // Everybody else the player knows of, in a short record — the name and
    // the counts — unless the player is a Claim Adjuster, who reads every
    // record in full (`1070:5964`), or the other player is dead
    // (`WriteRtPlr`, `1070:551c`).
    for (index, other) in state.players.iter().enumerate() {
        if index == player || !(others.players.contains(&index) || other.dead) {
            continue;
        }
        let mut short = player_record(state, index, other)?;
        short.planets = 0;
        short.fleets = u16::try_from(
            view.fleets
                .iter()
                .filter(|(i, _)| {
                    state
                        .fleets
                        .get(**i)
                        .is_some_and(|f| usize::try_from(f.owner) == Ok(index))
                })
                .count(),
        )
        .unwrap_or(u16::MAX);
        short.ship_design_count = others.design_count(index, false);
        short.starbase_design_count = others.design_count(index, true);
        if !(prt == Some(Prt::Ca) || other.dead) {
            short.full_data = false;
            short.race = None;
            short.research = None;
            short.default_queue = None;
            short.password = None;
            short.trader_parts = None;
            short.player_relations.clear();
            short.fixed.truncate(8);
            // `PLAYER.det`, the low bits of the word at 6: `detSome`.
            short.fixed[6] = 0x03;
        }
        body.push(block(6, short.encode()?)?);
    }

    for planet in all_planets(state) {
        let Some(detail) = view.planet(planet.id) else {
            continue;
        };
        if planet.owner == Some(owner) {
            body.push(block(13, planet_record(planet).encode())?);
            if !planet.queue.is_empty() {
                body.push(block(28, queue_record(planet).encode())?);
            }
        } else {
            body.push(block(14, partial_planet_record(planet, detail).encode())?);
        }
    }
    // Each player's designs in player order, the file's own in full and
    // the others' as far as they are known; the fleets and the starbase
    // designs the same way.
    for starbases in [false, true] {
        if starbases {
            push_fleets(&mut body, state, player)?;
            push_fleets_seen(&mut body, state, player, &view)?;
        }
        for (index, designs) in state.designs.iter().enumerate() {
            if index == player {
                push_designs(&mut body, state, player, designs, starbases)?;
            } else {
                push_designs_seen(&mut body, state, player, index, designs, &others, starbases)?;
            }
        }
    }
    push_battles(state, &mut body, player)?;
    push_messages(state, &mut body, player)?;
    push_player_messages(state, &mut body, player)?;
    push_standings(state, &mut body, player)?;
    push_things(state, &mut body, Some((player, &view)))?;
    push_battle_plans(state, &mut body, player)?;

    StarsFile::build(&header, &body, footer(state))
}

/// Write one player's history file — the `.hN` beside their `.mN`, which
/// the original's **client** keeps for itself: the planets the player has
/// a record of, their message filter, and the score sheet's timeline.
///
/// The layout is the fixtures' (`Game.h1`, `tutorial.h1`): the history
/// header (`rtHistHdr`, type 32) with the planet count and the year, one
/// partial planet record (type 14, detail 3) per planet known — its
/// environment and concentrations, the owner's population and defence
/// estimates for an owned one, and the year it was last seen as a trailing
/// word — the message filter (type 33) and one history row (type 45, the
/// turn in place of the rank) per year of the timeline. The two short
/// player and design blocks a real file also carries are not written; the
/// original writes the file afresh from its own turn file every year, so
/// nothing here is lost on it.
///
/// `known` is the set of planets the player has a record of, by id — the
/// client's own knowledge, which the host's state does not hold for a human
/// player; a computer player's is its [`Player::explored`]. The player's
/// own planets are written whether or not they are in it.
///
/// # Errors
/// [`FormatError::Malformed`] if `player` is not in the game, or if a record
/// does not fit its block.
pub fn history_file(
    state: &GameState,
    player: usize,
    known: &std::collections::BTreeSet<i16>,
) -> Result<Vec<u8>> {
    let record = state
        .players
        .get(player)
        .ok_or_else(|| FormatError::Malformed(format!("no player {player} in this game")))?;
    let number = u8::try_from(player)
        .map_err(|_| FormatError::Malformed(format!("player {player} is out of range")))?;
    let header = FileHeader::new(
        state.seed,
        FileType::History,
        number,
        state.turn.unsigned_abs(),
        salt_for(state.seed, number, state.turn),
    );
    let owner = i16::from(number);

    // Every planet the player has a record of, in id order.
    let known: Vec<&Planet> = all_planets(state)
        .filter(|p| p.owner == Some(owner) || known.contains(&p.id))
        .collect();
    let mut body = Vec::new();
    let mut hist = Vec::with_capacity(4);
    hist.extend_from_slice(&u16::try_from(known.len()).unwrap_or(u16::MAX).to_le_bytes());
    hist.extend_from_slice(&(state.turn.unsigned_abs() + 1).to_le_bytes());
    body.push(block(32, hist)?);
    for planet in known {
        let mut partial = partial_planet_record(planet, Detail::Some);
        // A planet of somebody else's carries the first-year flag, which
        // the fixtures show every planet but the player's own keeping year
        // after year.
        partial.first_year = planet.owner != Some(owner);
        partial.trailing = state.turn.unsigned_abs().to_le_bytes().to_vec();
        body.push(block(14, partial.encode())?);
    }
    body.push(block(
        stars_formats::MESSAGE_FILTER_BLOCK,
        record.message_filter.encode().to_vec(),
    )?);
    // The timeline: one row a year for every player, the turn where a
    // standing keeps its rank.
    let years: std::collections::BTreeSet<u16> =
        state.timeline.iter().flatten().map(|y| y.turn).collect();
    for turn in years {
        for (who, rows) in state.timeline.iter().enumerate() {
            let Some(year) = rows.iter().find(|y| y.turn == turn) else {
                continue;
            };
            let standing = crate::score::Standing {
                player: who,
                known: true,
                winner: false,
                rank: turn,
                victory: 0,
                score: year.score,
            };
            body.push(block(45, score_record(&standing, true).encode())?);
        }
    }
    StarsFile::build(&header, &body, Vec::new())
}

/// The current standings, as the turn file carries them: a type-45 row
/// for the player's own score, and for every other's when the game's
/// scores are public — a fixture game with them private holds the one
/// row.
fn push_standings(state: &GameState, body: &mut Vec<Block>, player: usize) -> Result<()> {
    for standing in &state.standings {
        if standing.player == player || (state.public_scores && standing.known) {
            body.push(block(45, score_record(standing, false).encode())?);
        }
    }
    Ok(())
}

/// A standing as a score block, a history row when `history`.
fn score_record(standing: &crate::score::Standing, history: bool) -> stars_formats::ScoreRecord {
    let vc = standing.victory >> 6;
    stars_formats::ScoreRecord {
        player_id: u8::try_from(standing.player).unwrap_or(0),
        known: standing.known,
        victory: stars_formats::VictoryConditions {
            owns_planets: vc & 1 != 0,
            attains_tech: vc & 2 != 0,
            exceeds_score: vc & 4 != 0,
            exceeds_second_place: vc & 8 != 0,
            production_capacity: vc & 16 != 0,
            capital_ships: vc & 32 != 0,
            highest_score: vc & 64 != 0,
        },
        winner: standing.winner,
        history,
        rank: standing.rank,
        score: u32::try_from(standing.score.score).unwrap_or(0),
        resources: u32::try_from(standing.score.resources).unwrap_or(0),
        planets: u16::try_from(standing.score.planets).unwrap_or(u16::MAX),
        starbases: u16::try_from(standing.score.starbases).unwrap_or(u16::MAX),
        unarmed_ships: crate::score::pack(standing.score.unarmed_ships),
        escort_ships: crate::score::pack(standing.score.escort_ships),
        capital_ships: crate::score::pack(standing.score.capital_ships),
        tech_levels: u16::try_from(standing.score.tech_levels).unwrap_or(u16::MAX),
    }
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

/// The player id a race-only block carries, in place of a 0-based player
/// number: a race in a `.rN` file belongs to nobody yet.
const RACE_ONLY_PLAYER: u8 = 0xFF;

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
        default_queue: Some(player.default_queue.clone()),
        password: Some(player.password),
        trader_parts: Some(player.trader_parts),
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
        ai_player: race.has_lrt(crate::race::lrt::AI_PLAYER),
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
        // as a quarter of its population in hundreds; the decoder scales it
        // to colonists.
        pop_guess: owned.then(|| {
            planet
                .pop_guess
                .unwrap_or_else(|| u32::try_from(planet.pop / 4).unwrap_or(0) * 400)
        }),
        defense_guess: owned.then(|| planet.defense_guess.unwrap_or(0)),
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
            // The game stores 31 for a planet with no scanner, not zero —
            // zero is the Viewer 50.
            scanner: planet.scanner.unwrap_or(31),
            artifact: planet.artifact,
            no_research: planet.no_research,
            unused5: 0,
            unused2: 0,
        }),
        starbase: (planet.starbase && owned).then(|| Starbase {
            design: planet.starbase_design.unwrap_or(0) & 0x0F,
            damage_pct: planet.starbase_damage,
            // The mass driver's target is stored one-based, zero meaning it is
            // not set — the same convention as the route.
            fling_dest: planet.fling_dest.map_or(0, |id| id.unsigned_abs() + 1),
            warp: planet.fling_warp,
            no_heal: false,
        }),
        // Stored one-based, zero meaning "no route".
        route_dest: Some(planet.route_dest.map_or(0, |id| id.unsigned_abs() + 1)),
        trailing: Vec::new(),
    }
}

/// A planet as somebody who does not own it sees it — the `rtPlanetB`
/// (type 14) arm of `WritePlanet` (`1070:7a6a`): the id, owner and whether
/// it has a starbase at [`Detail::Minimal`]; the environment,
/// concentrations and the owner's population and defence guesses from
/// [`Detail::Some`]; the surface minerals at [`Detail::More`], when there
/// are any. A planet seen only [`Detail::Obscure`]ly is written at
/// `detSome` with `fInclude` clear. A starbase is named by its design
/// alone.
fn partial_planet_record(planet: &Planet, detail: Detail) -> PlanetRecord {
    let mut partial = planet_record(planet);
    partial.block_type = 14;
    partial.detail = match detail {
        Detail::Minimal => 1,
        Detail::Obscure | Detail::Some => 3,
        Detail::More | Detail::Full => 4,
    };
    partial.include = detail != Detail::Obscure;
    partial.has_installations = false;
    partial.routing = false;
    partial.population = None;
    partial.installations = None;
    partial.route_dest = None;
    partial.artifact = false;
    partial.has_surface_minerals =
        partial.detail >= 4 && planet.owner.is_some() && planet.surface_min.iter().any(|m| *m > 0);
    if !partial.has_surface_minerals {
        partial.surface_minerals = None;
    }
    partial.starbase = partial.starbase.map(|mut sb| {
        sb.damage_pct = 0;
        sb.fling_dest = 0;
        sb.warp = 0;
        sb
    });
    partial
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
///
/// Also the body of a `rtLogShDef` order (`LogChangeShDef`), which embeds the
/// same record after its header word — so a client that lets the player change
/// a design writes one of these into the order log.
#[must_use]
pub fn design_record(design: &ShipDesign, number: u8, starbase: bool, built: u32) -> DesignRecord {
    DesignRecord {
        full_design: true,
        transferred: false,
        starbase,
        design_number: number,
        hull_id: u8::try_from(design.hull_id).unwrap_or(0),
        pic: design.picture,
        armor: Some(design.stored_armor),
        mass: None,
        turn_designed: Some(if design.designed > 0 {
            u16::try_from(design.designed).unwrap_or(DESIGN_TURN)
        } else {
            DESIGN_TURN
        }),
        total_built: Some(built.max(design.built)),
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
        flags1: DESIGN_FLAGS1 | (u8::from(design.obsolete) << 1),
        trailing: Vec::new(),
    }
}

/// What a player's file says of the other players and their designs.
#[derive(Debug, Default)]
struct Others {
    /// The other players the file carries a record of: anyone with a
    /// planet, fleet, design or space object on the player's map, or in a
    /// battle with them.
    players: std::collections::BTreeSet<usize>,
    /// The other players' designs the file carries, by `(owner, slot)`, and
    /// whether in full: hull, picture, mass and name otherwise.
    designs: std::collections::BTreeMap<(usize, usize), bool>,
}

impl Others {
    /// Note a design of another player's, at least as fully as `full`.
    fn design(&mut self, owner: usize, slot: usize, full: bool) {
        self.players.insert(owner);
        let entry = self.designs.entry((owner, slot)).or_insert(full);
        *entry |= full;
    }

    /// How many of `owner`'s ship or starbase designs the file carries.
    fn design_count(&self, owner: usize, starbases: bool) -> u8 {
        let first = usize::from(crate::startup::FIRST_STARBASE_SLOT);
        u8::try_from(
            self.designs
                .keys()
                .filter(|(o, slot)| *o == owner && (*slot >= first) == starbases)
                .count(),
        )
        .unwrap_or(u8::MAX)
    }
}

/// What the file-writing passes settle about everybody else, given what
/// the player's scanners found: `MarkFleet` (`1070:885e`) puts every
/// design of a fleet on the map in the file and `MarkPlanet` (`1070:8adc`)
/// the starbase design of a planet seen in any detail but obscurely;
/// `SetVisPFFinish` (`1070:c41c`) writes those in part — in full for a War
/// Monger, or once revealed by a minefield or a packet; and `WriteBattles`
/// (`1070:709c`) writes every design that fought the player in full and
/// puts the fleets that survived and the planet they fought over on the
/// map. The owners of the space objects on the map are known too.
fn others_in_view(state: &GameState, player: usize, view: &mut View) -> Others {
    let mut out = Others::default();
    let bit = 1u16 << (u16::try_from(player).unwrap_or(0) & 15);
    let war_monger = state
        .players
        .get(player)
        .is_some_and(|p| p.race.prt() == Some(Prt::Wm));
    let first_base = usize::from(crate::startup::FIRST_STARBASE_SLOT);

    for record in state.battles.iter().filter(|b| b.player_mask & bit != 0) {
        for other in 0..state.players.len() {
            if other != player && record.player_mask & (1 << (other & 15)) != 0 {
                out.players.insert(other);
            }
        }
        for token in &record.tokens {
            let owner = usize::from(token.player);
            if owner == player {
                continue;
            }
            let slot = usize::from(token.design);
            if token.object_class == crate::fleet::grobj::PLANET {
                // A starbase token's design is `16 + isb` (`1070:8226`).
                out.design(owner, first_base + slot.saturating_sub(16), true);
            } else {
                out.design(owner, slot, true);
                if let Some(index) = state
                    .fleets
                    .iter()
                    .position(|f| usize::try_from(f.owner) == Ok(owner) && f.id == token.id)
                {
                    if !state.fleets[index].stacks.is_empty() {
                        view.mark_fleet(index, Detail::Some);
                    }
                }
            }
        }
        if let Ok(planet) = i16::try_from(record.planet) {
            if planet >= 0 && state.planets.iter().any(|p| p.id == planet) {
                view.mark_planet(planet, Detail::Minimal);
            }
        }
    }

    for &index in view.fleets.keys() {
        let Some(fleet) = state.fleets.get(index) else {
            continue;
        };
        let Ok(owner) = usize::try_from(fleet.owner) else {
            continue;
        };
        if owner == player {
            continue;
        }
        out.players.insert(owner);
        for stack in fleet.stacks.iter().filter(|s| s.count > 0) {
            out.design(owner, usize::from(stack.design), war_monger);
        }
    }
    for (&id, &detail) in &view.planets {
        let Some(planet) = all_planets(state).find(|p| p.id == id) else {
            continue;
        };
        let Some(owner) = planet.owner.and_then(|o| usize::try_from(o).ok()) else {
            continue;
        };
        if owner == player {
            continue;
        }
        out.players.insert(owner);
        if detail != Detail::Obscure && planet.starbase {
            if let Some(base) = planet.starbase_design {
                out.design(owner, first_base + usize::from(base), war_monger);
            }
        }
    }
    for &index in &view.packets {
        if let Some(owner) = state
            .packets
            .get(index)
            .and_then(|p| usize::try_from(p.owner).ok())
        {
            if owner != player {
                out.players.insert(owner);
            }
        }
    }
    for &index in &view.minefields {
        if let Some(owner) = state
            .minefields
            .get(index)
            .and_then(|f| usize::try_from(f.owner).ok())
        {
            if owner != player {
                out.players.insert(owner);
            }
        }
    }
    for &(seer, owner, slot) in &state.revealed_designs {
        if seer == player && owner != player {
            out.design(owner, slot, true);
        }
    }

    // MarkPlayersThatSentMsgs (`1030:9604`): whoever wrote to the player,
    // or to everybody, is known to them.
    for letter in &state.player_messages {
        let Ok(from) = usize::try_from(letter.from) else {
            continue;
        };
        let to_me = letter.to == stars_formats::PlayerMessage::EVERYBODY
            || usize::try_from(letter.to - 1) == Ok(player);
        if from != player && to_me && from < state.players.len() {
            out.players.insert(from);
        }
    }
    // MarkPlanetsPlayerLost (`1030:93c6`): a planet the player was told
    // they lost this year — stormed, died off, jumped ship, bombed out —
    // stays on their map in some detail.
    for message in state.messages.iter().filter(|m| m.player == player) {
        use crate::message::id;
        let planet = match message.id {
            id::LANDING_STORMED | id::BOMBED_YOU => message.params.get(1),
            id::COLONISTS_DIED_OFF | id::COLONISTS_JUMPED_SHIP => message.params.first(),
            _ => None,
        };
        if let Some(&planet) = planet {
            if all_planets(state).any(|p| p.id == planet) {
                view.mark_planet(planet, Detail::Some);
            }
        }
    }
    out
}

/// Append what the player's file says of another player's ship or
/// starbase designs: in full where they have fought or been revealed, and
/// otherwise the short form — hull, picture, mass and name
/// (`WriteRtShDef`, `1070:574e`).
fn push_designs_seen(
    body: &mut Vec<Block>,
    state: &GameState,
    player: usize,
    owner: usize,
    designs: &[ShipDesign],
    others: &Others,
    starbases: bool,
) -> Result<()> {
    if owner == player {
        return Ok(());
    }
    let first = usize::from(crate::startup::FIRST_STARBASE_SLOT);
    for (&(who, slot), &full) in &others.designs {
        if who != owner || (slot >= first) != starbases {
            continue;
        }
        let Some(design) = designs.get(slot).filter(|d| d.hull_id >= 0) else {
            continue;
        };
        let number = u8::try_from(if starbases { slot - first } else { slot }).unwrap_or(0);
        let mut record = design_record(design, number, starbases, ships_of(state, owner, slot));
        if !full {
            record.full_design = false;
            record.armor = None;
            record.turn_designed = None;
            record.total_built = None;
            record.total_remaining = None;
            record.slots.clear();
            record.mass = Some(u16::try_from(design.mass().unwrap_or(0)).unwrap_or(u16::MAX));
        }
        body.push(block(26, record.encode()?)?);
    }
    Ok(())
}

/// Append the other players' fleets on the player's map, in the partial
/// form (type 17): the ships, the heading and warp, the mass, and at
/// [`Detail::More`] the minerals aboard (`WriteFleet`, `1070:81c6`).
fn push_fleets_seen(
    body: &mut Vec<Block>,
    state: &GameState,
    player: usize,
    view: &View,
) -> Result<()> {
    for (&index, &detail) in &view.fleets {
        let Some(fleet) = state.fleets.get(index) else {
            continue;
        };
        let Ok(owner) = usize::try_from(fleet.owner) else {
            continue;
        };
        if owner == player || fleet.stacks.iter().all(|s| s.count <= 0) {
            continue;
        }
        let designs = state.designs.get(owner).map(Vec::as_slice).unwrap_or(&[]);
        let mut record = fleet_record(fleet, 0);
        record.detail = detail.code().min(4);
        record.battle_plan = None;
        record.waypoint_count = None;
        record.cargo = (detail >= Detail::More).then(|| stars_formats::Cargo {
            ironium: u32::try_from(fleet.cargo.minerals[0]).unwrap_or(0),
            boranium: u32::try_from(fleet.cargo.minerals[1]).unwrap_or(0),
            germanium: u32::try_from(fleet.cargo.minerals[2]).unwrap_or(0),
            population: 0,
            fuel: 0,
        });
        let (dx, dy, warp) = heading(fleet);
        record.delta_x = Some(dx);
        record.delta_y = Some(dy);
        record.warp = Some(warp);
        record.warp_high = u8::from(warp > 0);
        record.mass = Some(u32::try_from(fleet.mass(designs)).unwrap_or(0));
        body.push(block(17, record.encode(17))?);
    }
    Ok(())
}

/// The heading a fleet on somebody else's map is drawn with — `dirFltX`,
/// `dirFltY` and `iwarpFlt`, set as the fleet moves (`10b0:4686`): the
/// vector to its next waypoint halved until both parts fit a signed byte,
/// each stored biased by 127, and the warp of the leg. A fleet that is not
/// moving carries zeroes, which the reader takes as no heading.
fn heading(fleet: &Fleet) -> (u8, u8, u8) {
    if let Some((dx, dy)) = fleet.direction {
        let warp = fleet.warp.unwrap_or(0);
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        return (((dx + 127) & 0xff) as u8, ((dy + 127) & 0xff) as u8, warp);
    }
    let (Some(warp), Some(next)) = (fleet.warp.filter(|w| *w > 0), fleet.waypoints.get(1)) else {
        return (0, 0, 0);
    };
    let mut dx = i32::from(next.position.x) - i32::from(fleet.position.x);
    let mut dy = i32::from(next.position.y) - i32::from(fleet.position.y);
    if dx == 0 && dy == 0 {
        return (0, 0, 0);
    }
    while dx.abs() > 127 || dy.abs() > 127 {
        dx /= 2;
        dy /= 2;
    }
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    (
        ((dx + 127) & 0xff) as u8,
        ((dy + 127) & 0xff) as u8,
        warp & 0x0f,
    )
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
            // A Transport task's instructions are re-encoded from the model;
            // every other task's payload — the Lay Minefield countdown above
            // all — is written back exactly as it was read.
            let task_data = w
                .transport
                .as_ref()
                .map(stars_formats::TransportTask::encode)
                .unwrap_or_else(|| w.task_data.clone());
            WaypointRecord {
                x: w.position.x.unsigned_abs(),
                y: w.position.y.unsigned_abs(),
                object_id: w.target,
                // Byte 7 is the class plus `fValidTask`; a generated game's
                // waypoints all sit on planets, which is the familiar 0x11.
                object_type: (w.target_class & 0x0F) | 0x10,
                object_class: w.target_class & 0x0F,
                valid_task: true,
                no_auto_track: false,
                warp: w.warp,
                task: w.task,
                task_data,
            }
        })
        .collect()
}

/// Append the year's battle recordings this player may watch: every one
/// whose `player_mask` names them, as a type-31 block, continued in
/// type-39 blocks past 1,023 bytes — which is how `WriteBattles`
/// (`1070:709c`) splits a long recording.
fn push_battles(state: &GameState, body: &mut Vec<Block>, player: usize) -> Result<()> {
    const PIECE: usize = 1023;
    let bit = 1u16 << (player & 15);
    for record in state.battles.iter().filter(|b| b.player_mask & bit != 0) {
        let bytes = record.encode();
        let mut first = true;
        for chunk in bytes.chunks(PIECE) {
            let type_id = if first {
                stars_formats::battle::BATTLE_BLOCK
            } else {
                stars_formats::battle::CONTINUE_BLOCK
            };
            body.push(block(type_id, chunk.to_vec())?);
            first = false;
        }
    }
    Ok(())
}

/// Append this player's messages, as one block.
///
/// The original appends every message for a player into one buffer and writes
/// that, which is why a message block in a real file is a **run** of records
/// rather than one. A player with nothing to be told gets no block.
fn push_messages(state: &GameState, body: &mut Vec<Block>, player: usize) -> Result<()> {
    let records: Vec<stars_formats::MessageRecord> = state
        .messages
        .iter()
        .filter(|m| m.player == player)
        .map(crate::message::Message::record)
        .collect();
    if records.is_empty() {
        return Ok(());
    }
    body.push(block(
        stars_formats::MESSAGE_BLOCK,
        stars_formats::MessageRecord::encode_all(&records),
    )?);
    Ok(())
}

/// Append what other players wrote to this one, one block a message
/// (`WritePlayerMessages`, `1030:97c0`, after the year's messages): every
/// message to everybody from somebody else, and every one addressed to
/// this player.
fn push_player_messages(state: &GameState, body: &mut Vec<Block>, player: usize) -> Result<()> {
    for message in state.player_messages.iter().filter(|m| m.is_for(player)) {
        body.push(block(
            stars_formats::PLAYER_MESSAGE_BLOCK,
            message.encode(),
        )?);
    }
    Ok(())
}

/// Append the object section: a count, then that many 18-byte `THING`s.
///
/// The minefields the engine models, then the objects it does not but is
/// carrying — packets, wormholes, the Mystery Trader — so that writing a game
/// back does not delete them. See `docs/formats/thing.md` for the section's
/// shape.
/// Which of the space objects a file carries, and with what flags —
/// `FWriteDataFile`'s count (`1070:5964`): a host file every one; a
/// player's file their own minefields and every one they have ever
/// detected (`grbitPlr`), the packets they see (all of them for a Packet
/// Physics race, `1070:9654`), every Mystery Trader (`1070:9654`), and
/// the wormhole ends their scanners reach this year.
fn push_things(
    state: &GameState,
    body: &mut Vec<Block>,
    player: Option<(usize, &View)>,
) -> Result<()> {
    let bit = player.map_or(0, |(p, _)| 1u16 << (u16::try_from(p).unwrap_or(0) & 15));
    let me = player.and_then(|(p, _)| i16::try_from(p).ok());
    let packet_physics = player.is_some_and(|(p, _)| {
        state
            .players
            .get(p)
            .is_some_and(|r| r.race.prt() == Some(Prt::Pp))
    });
    let minefields: Vec<&crate::minefield::Minefield> = state
        .minefields
        .iter()
        .filter(|f| me.is_none() || Some(f.owner) == me || f.detected_by & bit != 0)
        .collect();
    let packets: Vec<(usize, &crate::packet::Packet)> = state
        .packets
        .iter()
        .enumerate()
        .filter(|(i, _)| match player {
            None => true,
            Some((_, view)) => packet_physics || view.packets.contains(i),
        })
        .collect();
    let wormholes: Vec<(usize, &crate::wormhole::Wormhole)> = state
        .wormholes
        .iter()
        .enumerate()
        .filter(|(i, _)| player.is_none_or(|(_, view)| view.wormholes.contains(i)))
        .collect();
    let others = if player.is_none() {
        state.other_things.as_slice()
    } else {
        &[]
    };
    let total =
        minefields.len() + packets.len() + wormholes.len() + state.traders.len() + others.len();
    // No section at all when there is nothing to put in it (`1070:5964`).
    if total == 0 {
        return Ok(());
    }
    let count = u16::try_from(total).unwrap_or(u16::MAX);
    body.push(block(43, count.to_le_bytes().to_vec())?);
    for field in minefields {
        let mine = stars_formats::Minefield {
            mines: field.mines,
            players_seen: field.detected_by,
            kind: field.kind,
            detonate: field.detonating,
            players_seen_now: field.visible_to,
        };
        let thing = stars_formats::Thing {
            id: field.id & 0x01FF,
            player: u8::try_from(field.owner.max(0)).unwrap_or(0) & 0x0F,
            ith: 0,
            thing_type: stars_formats::ThingType::Minefield,
            x: field.position.x,
            y: field.position.y,
            kind: stars_formats::ThingKind::Minefield(mine),
            union: mine_union(&mine),
            turn: field.turn,
        };
        body.push(block(43, thing.encode().to_vec())?);
    }
    for (_, packet) in packets {
        let carried = stars_formats::MineralPacket {
            target_planet: packet.target,
            warp: packet.warp,
            moved: packet.moved,
            include: packet.include || player.is_some(),
            minerals: packet.minerals,
            // `wtMax` is the remaining mass over ten, rounded up, which
            // `FPacketDecay` recomputes every time it takes a bite.
            mass_max: u16::try_from((packet.mass() + 9) / 10).unwrap_or(0),
            decay_rate: packet.decay_rate,
        };
        let thing = stars_formats::Thing {
            id: packet.id & 0x01FF,
            player: u8::try_from(packet.owner.max(0)).unwrap_or(0) & 0x0F,
            ith: 1,
            thing_type: stars_formats::ThingType::MineralPacket,
            x: packet.position.x,
            y: packet.position.y,
            kind: stars_formats::ThingKind::MineralPacket(carried),
            union: packet_union(&carried),
            turn: packet.turn,
        };
        body.push(block(43, thing.encode().to_vec())?);
    }
    for (_, hole) in wormholes {
        let carried = stars_formats::Wormhole {
            stability: hole.stability,
            last_move: hole.years_still,
            dest_known: hole.dest_known,
            include: hole.include || player.is_some(),
            players_seen: hole.detected_by | bit,
            players_traversed: hole.traversed_by,
            partner_id: hole.partner,
        };
        let mut union = [0u8; 10];
        let w0 = u16::from(carried.stability & 0x03)
            | ((carried.last_move & 0x03FF) << 2)
            | (u16::from(carried.dest_known) << 12)
            | (u16::from(carried.include) << 13);
        union[0..2].copy_from_slice(&w0.to_le_bytes());
        union[2..4].copy_from_slice(&carried.players_seen.to_le_bytes());
        union[4..6].copy_from_slice(&carried.players_traversed.to_le_bytes());
        union[6..8].copy_from_slice(&carried.partner_id.to_le_bytes());
        body.push(block(
            43,
            stars_formats::Thing {
                id: hole.id & 0x01FF,
                player: 0,
                ith: 2,
                thing_type: stars_formats::ThingType::Wormhole,
                x: hole.position.x,
                y: hole.position.y,
                kind: stars_formats::ThingKind::Wormhole(carried),
                union,
                turn: hole.turn,
            }
            .encode()
            .to_vec(),
        )?);
    }
    for trader in &state.traders {
        let carried = stars_formats::MysteryTrader {
            dest_x: trader.destination.x,
            dest_y: trader.destination.y,
            warp: trader.warp,
            include: trader.include || player.is_some(),
            players_seen: trader.detected_by,
            part: trader.part,
        };
        let mut union = [0u8; 10];
        union[0..2].copy_from_slice(&carried.dest_x.to_le_bytes());
        union[2..4].copy_from_slice(&carried.dest_y.to_le_bytes());
        let w4 = u16::from(carried.warp & 0x0F) | (u16::from(carried.include) << 4);
        union[4..6].copy_from_slice(&w4.to_le_bytes());
        union[6..8].copy_from_slice(&carried.players_seen.to_le_bytes());
        union[8..10].copy_from_slice(&carried.part.to_le_bytes());
        body.push(block(
            43,
            stars_formats::Thing {
                id: trader.id & 0x01FF,
                player: 0,
                ith: 3,
                thing_type: stars_formats::ThingType::MysteryTrader,
                x: trader.position.x,
                y: trader.position.y,
                kind: stars_formats::ThingKind::MysteryTrader(carried),
                union,
                turn: trader.turn,
            }
            .encode()
            .to_vec(),
        )?);
    }
    for thing in others {
        body.push(block(43, thing.encode().to_vec())?);
    }
    Ok(())
}

/// The ten union bytes of a mineral packet.
fn packet_union(packet: &stars_formats::MineralPacket) -> [u8; 10] {
    let mut out = [0u8; 10];
    let w0 = (packet.target_planet & 0x03FF)
        | (u16::from(packet.warp & 0x0F) << 10)
        | (u16::from(packet.moved) << 14)
        | (u16::from(packet.include) << 15);
    out[0..2].copy_from_slice(&w0.to_le_bytes());
    for (index, amount) in packet.minerals.iter().enumerate() {
        let at = 2 + index * 2;
        out[at..at + 2].copy_from_slice(&amount.to_le_bytes());
    }
    let w4 = (packet.mass_max & 0x3FFF) | (u16::from(packet.decay_rate & 0x03) << 14);
    out[8..10].copy_from_slice(&w4.to_le_bytes());
    out
}

/// The ten union bytes of a minefield, which is what `Thing::encode` writes.
fn mine_union(mine: &stars_formats::Minefield) -> [u8; 10] {
    let mut out = [0u8; 10];
    out[0..4].copy_from_slice(&mine.mines.to_le_bytes());
    out[4..6].copy_from_slice(&mine.players_seen.to_le_bytes());
    out[6] = mine.kind;
    out[7] = u8::from(mine.detonate);
    out[8..10].copy_from_slice(&mine.players_seen_now.to_le_bytes());
    out
}

/// Append one player's battle plans, in the order they are held.
///
/// The owner nibble is stamped from the player index rather than trusted from
/// the record: a plan belongs to whoever the file says it does. A player whose
/// plans somehow went missing gets the defaults, so a file never ships a fleet
/// pointing at a plan that is not there.
fn push_battle_plans(state: &GameState, body: &mut Vec<Block>, player: usize) -> Result<()> {
    let race_id = u8::try_from(player).unwrap_or(0) & 0x0F;
    let fallback = crate::default_battle_plans(player);
    let plans = state
        .players
        .get(player)
        .map(|p| p.battle_plans.as_slice())
        .filter(|p| !p.is_empty())
        .unwrap_or(&fallback);
    for plan in plans {
        let mut record: BattlePlanRecord = plan.clone();
        record.race_id = race_id;
        body.push(block(30, record.encode()?)?);
    }
    Ok(())
}

/// Turn a race back into the record a `.rN` file is written from.
///
/// The inverse of [`crate::load::race_from_record`], and the direction the
/// race wizard needs: it edits a [`Race`] and has to hand a record to
/// [`stars_formats::write_race_fields`]. It is the same conversion a player
/// block uses, with the names filled in and the race-only player marker in
/// place of a player id.
///
/// Two fields do not survive the round trip because [`Race`] does not carry
/// them — the research percentage and the `full_data` flag — so they are set
/// to what a freshly written race file holds: 15% and true.
#[must_use]
pub fn record_from_race(
    race: &crate::Race,
    singular: &str,
    plural: &str,
) -> stars_formats::RaceRecord {
    let mut record = race_record(race, RACE_ONLY_PLAYER);
    record.singular_name = singular.to_string();
    record.plural_name = plural.to_string();
    record
}

/// Write a race out as a `.rN` file.
///
/// A race file is the smallest thing this crate writes: a plaintext header, one
/// encrypted type-6 block holding the race record, and an empty footer — the
/// three-block shape every fixture has (`docs/formats/race-r.md`). The header
/// is stamped as [`FileType::Race`], turn 1, player [`HOST_PLAYER`] (31, the
/// "no specific player" marker), exactly as the seven shipped races are.
///
/// The block is a player block with the race in it, so it goes out through
/// [`PlayerRecord::encode`]: the counts are all zero and the relations table is
/// empty, which is what a race that has not joined a game yet has to say.
/// `logo` is the emblem index the wizard picked (0..=31).
///
/// The game id seeds the cipher and is otherwise unused in a race file — no
/// game owns it yet — so it is derived from the names to keep writing the same
/// race twice byte-for-byte stable.
///
/// # Errors
/// [`FormatError::Malformed`] if a name is too long for its length byte or the
/// record does not fit its block.
pub fn race_file(race: &crate::Race, singular: &str, plural: &str, logo: u8) -> Result<Vec<u8>> {
    const RACE_TURN: u16 = 1;
    let game_id = name_seed(singular, plural);
    let header = FileHeader::new(
        game_id,
        FileType::Race,
        HOST_PLAYER,
        RACE_TURN,
        salt_for(game_id, HOST_PLAYER, RACE_TURN as i16),
    );
    let record = record_from_race(race, singular, plural);
    race_file_with_header(&header, &record, logo)
}

/// Write a race record out as a `.rN` file under a header of the caller's own.
///
/// The body of [`race_file`], split out so a test can rebuild a shipped race
/// file under the header that file carries and compare the result byte for
/// byte — everything after the header is this function's to produce.
///
/// # Errors
/// As [`race_file`].
pub fn race_file_with_header(
    header: &FileHeader,
    record: &stars_formats::RaceRecord,
    logo: u8,
) -> Result<Vec<u8>> {
    let player = PlayerRecord {
        player_number: record.player_id,
        ship_design_count: 0,
        starbase_design_count: 0,
        planets: 0,
        fleets: 0,
        logo: logo & 0x1F,
        full_data: true,
        // Offset 7 is `0` in every shipped race file: none of the player flags
        // (human, AI skill) mean anything until the race joins a game.
        flags_byte: 0,
        player_relations: Vec::new(),
        race: Some(record.clone()),
        singular_name: record.singular_name.clone(),
        plural_name: record.plural_name.clone(),
        research: None,
        default_queue: None,
        password: None,
        trader_parts: None,
        fixed: Vec::new(),
        trailing: Vec::new(),
    };
    StarsFile::build(header, &[block(6, player.encode()?)?], Vec::new())
}

/// A stable game id for a race file, folded from its two names.
fn name_seed(singular: &str, plural: &str) -> u32 {
    let mut seed: u32 = 0x9E37_79B9;
    for byte in singular.bytes().chain([0]).chain(plural.bytes()) {
        seed = seed.rotate_left(5) ^ u32::from(byte).wrapping_mul(0x0100_0193);
    }
    seed
}
