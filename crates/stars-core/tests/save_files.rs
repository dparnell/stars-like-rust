//! Writing `.hst` and `.mN` files, checked two ways.
//!
//! **Round trip.** A generated game is written, read back and compared field by
//! field: planets, players, fleets and designs must all survive.
//!
//! **Against the original engine.** The turn-0 fixture is loaded and written
//! out again, and the result is compared block by block with what Stars! itself
//! wrote. Everything this project models has to come back byte for byte; the
//! only differences allowed are the file header (whose version word and cipher
//! salt are ours to choose) and the two sections a `GameState` does not carry —
//! space objects and messages.
//!
//! Tests skip rather than fail when the fixtures are absent.

use std::collections::BTreeMap;

use stars_core::newgame::{generate, NewGame, NewPlayer, Size};
use stars_core::{opponents, save, GameState, Planet, Race, Rng};
use stars_formats::{StarsFile, Universe};

const TURN0: &str = "../../fixtures/incoming/turn0";

/// A three-player game to write and read back.
fn a_game() -> stars_core::newgame::Created {
    let config = NewGame {
        name: "Written".into(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("Turindrones").as_player(),
            opponents::opponent(3, 2).expect("Rototills").as_player(),
        ],
        ..NewGame::default()
    };
    let mut rng = Rng::randomize(config.id);
    generate(&config, &mut rng).expect("generates")
}

/// Every planet a state knows about, in id order.
fn planets(state: &GameState) -> Vec<&Planet> {
    let mut all: Vec<&Planet> = state
        .planets
        .iter()
        .chain(state.known_planets.iter())
        .collect();
    all.sort_by_key(|p| p.id);
    all
}

#[test]
fn a_generated_game_survives_a_host_file() {
    let made = a_game();
    let before = &made.state;
    let bytes = save::host_file(before).expect("writes");
    let file = StarsFile::decode(&bytes).expect("decodes");
    let (mut after, report) = GameState::from_file(&file);
    after.apply_universe(&made.universe);

    assert_eq!(after.turn, before.turn);
    assert_eq!(after.players.len(), before.players.len());
    assert_eq!(after.fleets.len(), before.fleets.len());
    assert_eq!(report.players_loaded, before.players.len());

    // Planets: everything the file carries about them.
    let (want, got) = (planets(before), planets(&after));
    assert_eq!(got.len(), want.len(), "planet count");
    for (a, b) in want.iter().zip(got.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.owner, b.owner, "planet {}", a.id);
        assert_eq!(a.env, b.env, "planet {} environment", a.id);
        assert_eq!(a.min_conc, b.min_conc, "planet {} concentrations", a.id);
        assert_eq!(a.min_level, b.min_level, "planet {} decay", a.id);
        assert_eq!(a.artifact, b.artifact, "planet {} artifact", a.id);
        assert_eq!(a.homeworld, b.homeworld, "planet {} homeworld", a.id);
        assert_eq!(a.starbase, b.starbase, "planet {} starbase", a.id);
        assert_eq!(a.name, b.name, "planet {} name", a.id);
        assert_eq!(a.position, b.position, "planet {} position", a.id);
        if a.owner.is_some() {
            assert_eq!(a.pop, b.pop, "planet {} population", a.id);
            assert_eq!(a.mines, b.mines, "planet {} mines", a.id);
            assert_eq!(a.factories, b.factories, "planet {} factories", a.id);
            assert_eq!(a.defenses, b.defenses, "planet {} defences", a.id);
            assert_eq!(a.surface_min, b.surface_min, "planet {} minerals", a.id);
            assert_eq!(
                a.starbase_design, b.starbase_design,
                "planet {} starbase design",
                a.id
            );
        }
    }

    // Players: race, technology, names and who is playing.
    for (i, (a, b)) in before.players.iter().zip(after.players.iter()).enumerate() {
        assert_eq!(a.race, b.race, "player {i} race");
        assert_eq!(a.research.levels, b.research.levels, "player {i} tech");
        assert_eq!(a.research_pct, b.research_pct, "player {i} research share");
        assert_eq!(a.name, b.name, "player {i} name");
        assert_eq!(a.plural_name, b.plural_name, "player {i} plural name");
        assert_eq!(a.control, b.control, "player {i} control");
        assert_eq!(a.relations, b.relations, "player {i} relations");
    }

    // Fleets, in order.
    for (a, b) in before.fleets.iter().zip(after.fleets.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.owner, b.owner);
        assert_eq!(a.position, b.position, "fleet {} position", a.id);
        assert_eq!(a.orbiting, b.orbiting, "fleet {} orbit", a.id);
        assert_eq!(a.stacks, b.stacks, "fleet {} ships", a.id);
        assert_eq!(a.cargo, b.cargo, "fleet {} cargo", a.id);
        assert_eq!(a.waypoints.len(), b.waypoints.len(), "fleet {}", a.id);
    }

    // Designs, per player and per slot, including their names.
    for (player, designs) in before.designs.iter().enumerate() {
        let other = after.designs.get(player).cloned().unwrap_or_default();
        for (slot, design) in designs.iter().enumerate() {
            if design.hull_id < 0 {
                continue;
            }
            let got = other
                .get(slot)
                .unwrap_or_else(|| panic!("player {player} lost design {slot}"));
            assert_eq!(got.hull_id, design.hull_id, "player {player} design {slot}");
            assert_eq!(got.slots, design.slots, "player {player} design {slot}");
            assert_eq!(got.name, design.name, "player {player} design {slot}");
        }
    }
}

#[test]
fn a_generated_game_survives_a_player_file() {
    let made = a_game();
    for player in 0..made.state.players.len() {
        let bytes = save::player_file(&made.state, player).expect("writes");
        let file = StarsFile::decode(&bytes).expect("decodes");
        let (after, _) = GameState::from_file(&file);

        let owner = i16::try_from(player).expect("player index");
        let mine: Vec<&Planet> = planets(&made.state)
            .into_iter()
            .filter(|p| p.owner == Some(owner))
            .collect();
        assert_eq!(after.planets.len(), mine.len(), "player {player} planets");
        assert_eq!(
            after.fleets.len(),
            made.state
                .fleets
                .iter()
                .filter(|f| f.owner == owner)
                .count(),
            "player {player} fleets"
        );
        // A turn file carries one player block, and the loader indexes players
        // by number, so the slots before it are placeholders.
        assert_eq!(
            after.players.len(),
            player + 1,
            "player {player}: players are indexed by number"
        );
        assert_eq!(after.players[player].race, made.state.players[player].race);
        assert_eq!(after.players[player].name, made.state.players[player].name);
        assert_eq!(
            after.designs.get(player).map_or(0, Vec::len),
            made.state.designs[player].len(),
            "player {player} designs"
        );
    }
}

/// A written game keeps playing: it generates turns and its population grows.
#[test]
fn a_written_game_still_generates_turns() {
    let made = a_game();
    let bytes = save::host_file(&made.state).expect("writes");
    let file = StarsFile::decode(&bytes).expect("decodes");
    let (mut state, _) = GameState::from_file(&file);
    state.apply_universe(&made.universe);

    let before: i32 = state.planets.iter().map(|p| p.pop).sum();
    let mut rng = Rng::randomize(state.seed);
    for _ in 0..5 {
        stars_core::generate_turn(&mut state, &mut rng);
    }
    assert_eq!(state.year(), 2405);
    assert!(
        state.planets.iter().map(|p| p.pop).sum::<i32>() > before,
        "population should grow"
    );
}

/// Writing the same game twice gives the same bytes.
#[test]
fn writing_is_deterministic() {
    let made = a_game();
    assert_eq!(
        save::host_file(&made.state).expect("writes"),
        save::host_file(&made.state).expect("writes")
    );
    assert_eq!(
        save::player_file(&made.state, 0).expect("writes"),
        save::player_file(&made.state, 0).expect("writes")
    );
}

/// Rebuilding the turn-0 fixture reproduces every record the original wrote.
///
/// The comparison is per block **type**, because the rebuilt file legitimately
/// lacks two kinds of block: the object section (space objects, which a
/// `GameState` does not carry) and messages. Everything else has to match
/// exactly, in order.
#[test]
fn rebuilding_a_real_host_file_reproduces_its_records() {
    let Some(original) = read_file(&format!("{TURN0}/Game.hst")) else {
        eprintln!("no turn-0 fixture; skipping");
        return;
    };
    let (mut state, _) = GameState::from_file(&original);
    if let Some(universe) = read_universe(&format!("{TURN0}/Game.xy")) {
        state.apply_universe(&universe);
    }
    let rebuilt = StarsFile::decode(&save::host_file(&state).expect("writes")).expect("decodes");
    compare_by_type(&original, &rebuilt, "Game.hst", &[43]);
}

/// The same, for one player's turn file.
#[test]
fn rebuilding_a_real_player_file_reproduces_its_records() {
    for (player, name) in [(0usize, "Game.m1"), (1, "Game.m2"), (2, "Game.m3")] {
        let Some(original) = read_file(&format!("{TURN0}/{name}")) else {
            eprintln!("no turn-0 fixture; skipping");
            return;
        };
        let Some(host) = read_file(&format!("{TURN0}/Game.hst")) else {
            return;
        };
        let (mut state, _) = GameState::from_file(&host);
        if let Some(universe) = read_universe(&format!("{TURN0}/Game.xy")) {
            state.apply_universe(&universe);
        }
        let rebuilt = StarsFile::decode(&save::player_file(&state, player).expect("writes"))
            .expect("decodes");
        // Messages (type 12) are the one section a turn file has that a
        // `GameState` does not carry.
        compare_by_type(&original, &rebuilt, name, &[12]);
    }
}

/// Compare two decoded files block by block within each type, ignoring the
/// header (ours to seed) and the types named in `ignore`.
fn compare_by_type(original: &StarsFile, rebuilt: &StarsFile, label: &str, ignore: &[u8]) {
    let group = |file: &StarsFile| -> BTreeMap<u8, Vec<Vec<u8>>> {
        let mut map: BTreeMap<u8, Vec<Vec<u8>>> = BTreeMap::new();
        for block in &file.blocks {
            if block.type_id == 8 || ignore.contains(&block.type_id) {
                continue;
            }
            map.entry(block.type_id)
                .or_default()
                .push(block.data.clone());
        }
        map
    };
    let want = group(original);
    let got = group(rebuilt);

    assert_eq!(
        want.keys().collect::<Vec<_>>(),
        got.keys().collect::<Vec<_>>(),
        "{label}: the rebuilt file has different block types"
    );
    let mut blocks = 0usize;
    for (type_id, expected) in &want {
        let actual = &got[type_id];
        assert_eq!(
            actual.len(),
            expected.len(),
            "{label}: type {type_id} block count"
        );
        for (i, (a, b)) in expected.iter().zip(actual.iter()).enumerate() {
            assert_eq!(a, b, "{label}: type {type_id} block {i} differs");
            blocks += 1;
        }
    }
    eprintln!("{label}: {blocks} blocks rebuilt byte for byte");
}

fn read_file(path: &str) -> Option<StarsFile> {
    StarsFile::decode(&std::fs::read(path).ok()?).ok()
}

fn read_universe(path: &str) -> Option<Universe> {
    Universe::decode(&std::fs::read(path).ok()?).ok()
}

/// A fleet's name survives a save, and sits where the game puts it.
///
/// No file in the fixtures has a name block — nobody renamed a fleet in any
/// captured game — so this is checked against the binary rather than against
/// real data: `WriteFleet` (`1070:8776`) writes the fleet, then its orders, and
/// then the name if there is one. The test asserts exactly that ordering as
/// well as the round trip.
#[test]
fn a_named_fleet_keeps_its_name() {
    let mut made = a_game();
    let named = made
        .state
        .fleets
        .iter()
        .position(|f| f.owner == 0)
        .expect("a fleet");
    let id = made.state.fleets[named].id;
    made.state.fleets[named].name = Some("Bold Endeavour".into());
    // A name too long to pack falls back to a literal string; write one of
    // those too, so both arms of the codec are exercised by a real file.
    let awkward = made
        .state
        .fleets
        .iter()
        .position(|f| f.owner == 1)
        .expect("another player's fleet");
    let long_name: String = std::iter::repeat_n('#', 21).collect();
    made.state.fleets[awkward].name = Some(long_name.clone());

    let bytes = save::host_file(&made.state).expect("writes");
    let file = StarsFile::decode(&bytes).expect("decodes");

    // The name block follows the fleet's waypoints, not the fleet block.
    let mut seen_fleet = false;
    let mut seen_waypoint = false;
    let mut checked = false;
    for block in &file.blocks {
        match block.type_id {
            16 => {
                seen_fleet = true;
                seen_waypoint = false;
            }
            19 | 20 => seen_waypoint = true,
            stars_formats::FLEET_NAME_BLOCK => {
                assert!(seen_fleet && seen_waypoint, "a name follows its waypoints");
                checked = true;
            }
            _ => {}
        }
    }
    assert!(checked, "the names were written");

    let (after, _) = GameState::from_file(&file);
    assert_eq!(
        after
            .fleets
            .iter()
            .find(|f| f.owner == 0 && f.id == id)
            .and_then(|f| f.name.clone()),
        Some("Bold Endeavour".to_string())
    );
    assert_eq!(
        after
            .fleets
            .iter()
            .filter(|f| f.owner == 1)
            .find_map(|f| f.name.clone()),
        Some(long_name),
        "the literal fallback survives too"
    );
    // An unnamed fleet still carries no block at all.
    assert!(
        after.fleets.iter().filter(|f| f.name.is_none()).count() > 1,
        "unnamed fleets stay unnamed"
    );
}

/// Minefields survive the trip from a real game into the model and back out.
///
/// The exodus games are full of them: 24,193 minefield objects across the
/// fixtures, laid by fleets carrying the task 6,892 waypoints are set to.
#[test]
fn minefields_load_and_save() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/games/exodus/2450/exodus.m6");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipping: no exodus fixture");
        return;
    };
    let file = StarsFile::decode(&bytes).expect("decodes");
    let before = stars_formats::thing_section(&file);
    let mines: Vec<_> = before
        .things
        .iter()
        .filter(|t| matches!(t.kind, stars_formats::ThingKind::Minefield(_)))
        .collect();
    if mines.is_empty() {
        eprintln!("skipping: that turn holds no minefields");
        return;
    }

    let (state, _) = GameState::from_file(&file);
    assert_eq!(state.minefields.len(), mines.len(), "every field loaded");
    assert_eq!(
        state.other_things.len(),
        before.things.len() - mines.len(),
        "and everything else is carried rather than dropped"
    );
    for (loaded, original) in state.minefields.iter().zip(&mines) {
        let stars_formats::ThingKind::Minefield(mine) = original.kind else {
            unreachable!()
        };
        assert_eq!(loaded.mines, mine.mines);
        assert_eq!(loaded.kind, mine.kind);
        assert_eq!(i16::from(original.player), loaded.owner);
        assert_eq!(
            loaded.position,
            stars_core::movement::Point::new(original.x, original.y)
        );
        // The radius the game draws is the square root of the count.
        let radius = loaded.radius();
        assert!(radius > 0.0 && radius * radius <= f64::from(mine.mines) + 1.0);
    }

    // Written back out, the section comes again with the same objects in it.
    let written = save::host_file(&state).expect("writes");
    let reread = StarsFile::decode(&written).expect("decodes");
    let after = stars_formats::thing_section(&reread);
    assert_eq!(
        after.count as usize,
        before.things.len(),
        "the count record matches the objects written"
    );
    let (again, _) = GameState::from_file(&reread);
    assert_eq!(again.minefields, state.minefields, "field for field");
}

/// The host's password rides in the host file, and only there.
///
/// `WriteDataFile` writes it as a type-36 record straight after the player
/// blocks, for a host file with a password set and nothing else, so a game
/// without one writes exactly the bytes it always did.
#[test]
fn a_host_password_is_written_after_the_player_blocks_and_read_back() {
    let made = a_game();
    let mut state = made.state.clone();
    assert_eq!(state.host_password, 0, "a new game has no host password");

    let without = save::host_file(&state).expect("writes");
    let file = StarsFile::decode(&without).expect("decodes");
    assert!(
        !file.blocks.iter().any(|b| b.type_id == 36),
        "no password, so no block"
    );
    let (read, _) = GameState::from_file(&file);
    assert_eq!(read.host_password, 0);

    // With one, the block appears in the right place: after the last player
    // block and before the first planet.
    state.host_password = stars_formats::password_salt("orbital");
    let with = save::host_file(&state).expect("writes");
    let file = StarsFile::decode(&with).expect("decodes");
    let kinds: Vec<u8> = file.blocks.iter().map(|b| b.type_id).collect();
    let salt_at = kinds.iter().position(|t| *t == 36).expect("the salt block");
    let last_player = kinds
        .iter()
        .rposition(|t| *t == 6)
        .expect("the player blocks");
    let first_planet = kinds.iter().position(|t| *t == 13).expect("a planet block");
    assert!(last_player < salt_at && salt_at < first_planet);
    assert_eq!(
        file.blocks[salt_at].data,
        stars_formats::password_salt("orbital").to_le_bytes()
    );

    let (read, _) = GameState::from_file(&file);
    assert_eq!(read.host_password, state.host_password);

    // A player's file never carries it: the record is the host's alone.
    let player = save::player_file(&state, 0).expect("writes");
    let file = StarsFile::decode(&player).expect("decodes");
    assert!(!file.blocks.iter().any(|b| b.type_id == 36));
    let (read, _) = GameState::from_file(&file);
    assert_eq!(read.host_password, 0);
}
