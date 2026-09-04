//! Differential tests against **real** Stars! sample files.
//!
//! These read the fixtures the user supplied in `fixtures/incoming/` and assert
//! the core correctness contracts. The samples come from one game captured at
//! two points:
//!
//! - `turn0/` — a fresh 3-player game (`Game.{xy,m1,m2,m3,hst}`);
//! - `turn1/` — the same game after the first turn was generated, adding the
//!   player history (`Game.h1/.h2/.h3`) and one player's orders (`Game.x1`),
//!   plus the advanced `.hst`/`.mN`/`.xy`.
//!
//! A second, **independent** game lives in `fixtures/games/tutorial/` (the
//! shipped "Tutorial Game": 2 players, a tiny 24-planet universe, partly played
//! to turn 3). It has a different `game_id`, so it proves the container/cipher
//! machinery is game-independent and that the seeding is correct on yet another
//! non-zero turn (turn 3).
//!
//! The contracts checked here:
//!
//! - fully block-framed files (`.hst`, `.mN`, `.hN`, `.xN`) **decode → encode
//!   byte-for-byte**, for both turn 0 and turn 1 (proving the cipher seeding is
//!   correct on non-zero turns);
//! - the file header decodes to the expected shared `game_id`, per-file type,
//!   per-player numbering, and turn;
//! - the `.xy` universe header + game-info (type-7) block decrypt to the
//!   expected game name and player count, and its planet array decodes to
//!   unique absolute positions and resolved planet names — see
//!   `docs/formats/xy.md`.
//!
//! If the fixtures are absent (e.g. a checkout without the sample game), each
//! test skips rather than fails so CI stays green.

use std::path::{Path, PathBuf};

use stars_formats::{planet_headers, BlockType, FileType, StarsFile, Universe};

/// Validate a decoded universe's planet array: every planet resolves to a name
/// in the master table, positions are unique, absolute x is non-decreasing (the
/// running-sum-of-`x_offset` invariant), and y fits the 12-bit field.
fn assert_planets_valid(label: &str, universe: &Universe) {
    let planets = universe.planets_resolved();
    assert_eq!(
        planets.len(),
        universe.planet_count(),
        "{label}: planet count"
    );
    let mut seen = std::collections::HashSet::new();
    let mut last_x = 0u32;
    for p in &planets {
        assert!(
            p.name.is_some(),
            "{label}: planet {} name index {} not in table",
            p.id,
            p.name_index
        );
        assert!(
            p.x >= last_x,
            "{label}: absolute x not non-decreasing at planet {}",
            p.id
        );
        last_x = p.x;
        assert!(p.y < 4096, "{label}: planet {} y out of 12-bit range", p.id);
        assert!(
            seen.insert((p.x, p.y)),
            "{label}: planet {} shares a position",
            p.id
        );
    }
}

/// Expected per-game id shared by every file of the `incoming/` sample game.
const GAME_ID: u32 = 0x2a03_1dd8;

/// Expected per-game id shared by every file of the shipped Tutorial Game.
const TUTORIAL_GAME_ID: u32 = 0x008c_ef49;

/// Read a fixture by path relative to `fixtures/incoming/`, e.g.
/// `"turn0/Game.hst"`. Returns `None` (so the test skips) if it is absent.
fn fixture(rel: &str) -> Option<Vec<u8>> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/incoming")
        .join(rel);
    std::fs::read(&path).ok()
}

/// Read a fixture by path relative to `fixtures/games/tutorial/`, e.g.
/// `"tutorial.hst"`. Returns `None` (so the test skips) if it is absent.
fn tutorial_fixture(rel: &str) -> Option<Vec<u8>> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/games/tutorial")
        .join(rel);
    std::fs::read(&path).ok()
}

/// Core round-trip assertion: the given framed-file bytes must decode with the
/// expected header fields and re-encode byte-for-byte.
fn assert_framed(
    label: &str,
    bytes: &[u8],
    expected_game_id: u32,
    expected_type: FileType,
    expected_player: u8,
    expected_turn: u16,
) {
    let file = StarsFile::decode(bytes).unwrap_or_else(|e| panic!("decode {label}: {e}"));
    assert_eq!(file.header.game_id, expected_game_id, "{label}: game id");
    assert_eq!(file.header.file_type, expected_type, "{label}: file type");
    assert_eq!(file.header.player, expected_player, "{label}: player index");
    assert_eq!(file.header.turn, expected_turn, "{label}: turn");

    let reencoded = file
        .encode()
        .unwrap_or_else(|e| panic!("encode {label}: {e}"));
    assert_eq!(
        reencoded, bytes,
        "{label}: re-encode is not byte-for-byte identical"
    );
}

/// Decode → encode must reproduce the original bytes exactly for framed files
/// of the `incoming/` sample game, with the header decoding to the expected
/// type, player, and turn.
fn assert_round_trips(rel: &str, expected_type: FileType, expected_player: u8, expected_turn: u16) {
    let Some(bytes) = fixture(rel) else {
        eprintln!("skipping {rel}: fixture not present");
        return;
    };
    assert_framed(
        rel,
        &bytes,
        GAME_ID,
        expected_type,
        expected_player,
        expected_turn,
    );
}

// ---- turn 0 (fresh game) --------------------------------------------------

#[test]
fn turn0_hst_round_trips() {
    // Host file: shared, so it carries the "no specific player" marker (31).
    assert_round_trips("turn0/Game.hst", FileType::Host, 31, 0);
}

#[test]
fn turn0_m1_round_trips() {
    assert_round_trips("turn0/Game.m1", FileType::Turn, 0, 0);
}

#[test]
fn turn0_m2_round_trips() {
    assert_round_trips("turn0/Game.m2", FileType::Turn, 1, 0);
}

#[test]
fn turn0_m3_round_trips() {
    assert_round_trips("turn0/Game.m3", FileType::Turn, 2, 0);
}

// ---- turn 1 (after one generated turn) ------------------------------------

#[test]
fn turn1_hst_round_trips() {
    // Non-zero turn exercises the turn-dependent cipher seeding.
    assert_round_trips("turn1/Game.hst", FileType::Host, 31, 1);
}

#[test]
fn turn1_m1_round_trips() {
    assert_round_trips("turn1/Game.m1", FileType::Turn, 0, 1);
}

#[test]
fn turn1_m2_round_trips() {
    assert_round_trips("turn1/Game.m2", FileType::Turn, 1, 1);
}

#[test]
fn turn1_m3_round_trips() {
    assert_round_trips("turn1/Game.m3", FileType::Turn, 2, 1);
}

/// Player history file (`.hN`) — a new file type first seen in the turn-1 set.
#[test]
fn turn1_h1_round_trips() {
    assert_round_trips("turn1/Game.h1", FileType::History, 0, 1);
}

/// Player orders file (`.xN`) — a new file type first seen in the turn-1 set.
#[test]
fn turn1_x1_round_trips() {
    assert_round_trips("turn1/Game.x1", FileType::Orders, 0, 1);
}

// ---- cross-file invariants -------------------------------------------------

/// Every framed file across both turns must agree on the shared game id.
#[test]
fn all_files_share_game_id() {
    let names = [
        "turn0/Game.hst",
        "turn0/Game.m1",
        "turn0/Game.m2",
        "turn0/Game.m3",
        "turn1/Game.hst",
        "turn1/Game.m1",
        "turn1/Game.m2",
        "turn1/Game.m3",
        "turn1/Game.h1",
        "turn1/Game.h2",
        "turn1/Game.h3",
        "turn1/Game.x1",
    ];
    let mut seen = 0;
    for name in names {
        if let Some(bytes) = fixture(name) {
            let file = StarsFile::decode(&bytes).unwrap_or_else(|e| panic!("decode {name}: {e}"));
            assert_eq!(file.header.game_id, GAME_ID, "{name}: game id");
            seen += 1;
        }
    }
    if seen == 0 {
        eprintln!("skipping: no framed fixtures present");
    }
}

/// The host file's block inventory and planet records match the sample game:
/// three players, 128 planets numbered `0..=127`, and three inhabited
/// (extended) planets — the homeworlds.
#[test]
fn hst_block_inventory_and_planets() {
    let Some(bytes) = fixture("turn0/Game.hst") else {
        eprintln!("skipping: turn0/Game.hst not present");
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();

    let counts = file.block_counts();
    // One player record per player, and one planet record per planet.
    assert_eq!(counts.get(&BlockType::Player), Some(&3), "player blocks");
    assert_eq!(counts.get(&BlockType::Planet), Some(&128), "planet blocks");
    // The shared host file self-identifies as a host file.
    assert_eq!(file.header.file_type, FileType::Host);

    let planets = planet_headers(&file);
    assert_eq!(planets.len(), 128, "planet count");

    // Planet ids form the contiguous sequence 0..=127.
    let mut ids: Vec<u16> = planets.iter().map(|p| p.id).collect();
    ids.sort_unstable();
    assert_eq!(ids, (0..128).collect::<Vec<_>>(), "planet ids contiguous");

    // Exactly the three homeworlds carry the extended (inhabited) record.
    let extended: Vec<u16> = planets
        .iter()
        .filter(|p| p.is_extended())
        .map(|p| p.id)
        .collect();
    assert_eq!(extended.len(), 3, "three inhabited planets (homeworlds)");
}

/// The `.xy` universe file is not fully block-framed (a raw planet array
/// follows the game-info block), so [`StarsFile::decode`] still errors on it;
/// the dedicated [`Universe`] parser handles it and round-trips byte-for-byte.
/// Checked for both captured turns (the `.xy` is unchanged across a turn
/// generation).
fn assert_xy_universe(rel: &str) {
    let Some(bytes) = fixture(rel) else {
        eprintln!("skipping {rel}: fixture not present");
        return;
    };

    // The generic full-file decoder does not handle the trailing planet array.
    assert!(
        StarsFile::decode(&bytes).is_err(),
        "{rel}: expected generic .xy decode to error; use Universe instead"
    );

    let universe =
        Universe::decode(&bytes).unwrap_or_else(|e| panic!("Universe::decode {rel}: {e}"));

    // Header + game-info decode as before.
    assert_eq!(universe.header.game_id, GAME_ID, "{rel}: game id");
    assert_eq!(universe.header.file_type, FileType::Universe, "{rel}: type");
    let gi = &universe.game_info;
    assert_eq!(gi[8] & 0x1F, 3, "{rel}: sample game has 3 players");
    let name_end = gi[32..64].iter().position(|&b| b == 0).unwrap_or(32);
    let name = std::str::from_utf8(&gi[32..32 + name_end]).unwrap();
    assert_eq!(name, "A Barefoot JayWalk", "{rel}: game name");

    // Planet array: one record per planet, matching the 128 planets in the
    // `.hst`. Absolute positions are unique and every name index resolves.
    assert_eq!(universe.planet_count(), 128, "{rel}: planet count");
    assert_planets_valid(rel, &universe);

    // An in-game `.xy` carries a 2-byte `00 00` trailer.
    assert_eq!(universe.trailer, [0, 0], "{rel}: in-game trailer");

    // The killer contract: re-encode is byte-for-byte identical.
    let reencoded = universe
        .encode()
        .unwrap_or_else(|e| panic!("encode {rel}: {e}"));
    assert_eq!(reencoded, bytes, "{rel}: .xy re-encode not byte-identical");
}

#[test]
fn turn0_xy_universe_round_trips() {
    assert_xy_universe("turn0/Game.xy");
}

#[test]
fn turn1_xy_universe_round_trips() {
    assert_xy_universe("turn1/Game.xy");
}

/// Read every `*.xy` fixture under `fixtures/xy/` (standalone universe files of
/// various sizes the user supplied), returning `(name, bytes)` pairs.
fn xy_dir_fixtures() -> Vec<(String, Vec<u8>)> {
    let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/xy");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("xy") {
            if let Ok(bytes) = std::fs::read(&path) {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                out.push((name, bytes));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Every standalone `.xy` universe file (of assorted sizes) decodes and
/// re-encodes byte-for-byte, with the planet count driven by the game-info
/// block and coordinates that are in range and non-overlapping.
///
/// These are different universes/sizes from the sample game, so they exercise
/// the count-from-game-info path and the optional trailer (which the sample
/// in-game `.xy` lacks). Skips cleanly if the directory is empty.
#[test]
fn xy_dir_universes_round_trip() {
    let fixtures = xy_dir_fixtures();
    if fixtures.is_empty() {
        eprintln!("skipping: no fixtures/xy/*.xy present");
        return;
    }

    for (name, bytes) in fixtures {
        let universe =
            Universe::decode(&bytes).unwrap_or_else(|e| panic!("Universe::decode {name}: {e}"));

        // The generic full-file decoder still rejects a `.xy` (not fully framed).
        assert!(
            StarsFile::decode(&bytes).is_err(),
            "{name}: expected generic .xy decode to error; use Universe instead"
        );

        // Self-identifies as a universe definition.
        assert_eq!(
            universe.header.file_type,
            FileType::Universe,
            "{name}: file type"
        );

        // Planet count is authoritative from the game-info block.
        assert_eq!(
            universe.planet_count(),
            u16::from_le_bytes([universe.game_info[10], universe.game_info[11]]) as usize,
            "{name}: planet count matches game-info"
        );
        assert!(universe.planet_count() > 0, "{name}: has planets");

        // Absolute positions are unique and every name index resolves.
        assert_planets_valid(&name, &universe);

        // Standalone universe files carry a 4-byte trailer: a constant `02 00`
        // followed by the player count (game-info offset 8).
        assert_eq!(
            universe.trailer.len(),
            4,
            "{name}: expected a 4-byte trailer"
        );
        assert_eq!(
            &universe.trailer[0..2],
            &[0x02, 0x00],
            "{name}: trailer prefix"
        );
        let trailer_val = u16::from_le_bytes([universe.trailer[2], universe.trailer[3]]);
        assert_eq!(
            trailer_val,
            u16::from(universe.player_count()),
            "{name}: trailer equals player count"
        );

        // The killer contract: re-encode is byte-for-byte identical.
        let reencoded = universe
            .encode()
            .unwrap_or_else(|e| panic!("encode {name}: {e}"));
        assert_eq!(reencoded, bytes, "{name}: .xy re-encode not byte-identical");
    }
}

// ---- Tutorial Game (a second, independent game) ---------------------------
//
// `fixtures/games/tutorial/` holds the game shipped with Stars! for its
// tutorial. It has a different `game_id` from the `incoming/` sample and its
// player files were saved at turn 3, so it independently confirms the container
// and cipher seeding are game- and turn-agnostic.

/// Every framed tutorial file decodes with the expected header and re-encodes
/// byte-for-byte. The `.hst`/`.m2` are at turn 0, while player 1's
/// `.m1`/`.h1`/`.x1` were saved at turn 3.
#[test]
fn tutorial_framed_files_round_trip() {
    let cases = [
        ("tutorial.hst", FileType::Host, 31u8, 0u16),
        ("tutorial.m1", FileType::Turn, 0, 3),
        ("tutorial.m2", FileType::Turn, 1, 0),
        ("tutorial.h1", FileType::History, 0, 3),
        ("tutorial.x1", FileType::Orders, 0, 3),
    ];
    let mut seen = 0;
    for (rel, ty, player, turn) in cases {
        let Some(bytes) = tutorial_fixture(rel) else {
            eprintln!("skipping {rel}: fixture not present");
            continue;
        };
        assert_framed(rel, &bytes, TUTORIAL_GAME_ID, ty, player, turn);
        seen += 1;
    }
    if seen == 0 {
        eprintln!("skipping: no tutorial fixtures present");
    }
}

/// The tutorial host file inventories a 2-player game: two player records, a
/// 24-planet universe numbered `0..=23`, and exactly two inhabited (extended)
/// planets — the two homeworlds.
#[test]
fn tutorial_hst_block_inventory_and_planets() {
    let Some(bytes) = tutorial_fixture("tutorial.hst") else {
        eprintln!("skipping: tutorial.hst not present");
        return;
    };
    let file = StarsFile::decode(&bytes).unwrap();

    let counts = file.block_counts();
    assert_eq!(counts.get(&BlockType::Player), Some(&2), "player blocks");
    assert_eq!(counts.get(&BlockType::Planet), Some(&24), "planet blocks");
    assert_eq!(file.header.file_type, FileType::Host);

    let planets = planet_headers(&file);
    assert_eq!(planets.len(), 24, "planet count");

    let mut ids: Vec<u16> = planets.iter().map(|p| p.id).collect();
    ids.sort_unstable();
    assert_eq!(ids, (0..24).collect::<Vec<_>>(), "planet ids contiguous");

    let extended: Vec<u16> = planets
        .iter()
        .filter(|p| p.is_extended())
        .map(|p| p.id)
        .collect();
    assert_eq!(extended.len(), 2, "two inhabited planets (homeworlds)");
}

/// The tutorial `.xy` is the smallest universe verified so far: an **in-game**
/// `.xy` (2-byte `00 00` trailer) of 24 planets named "Tutorial Game",
/// round-tripping byte-for-byte with the planet count driven by the game-info
/// block, and every planet resolving to a unique name and position.
#[test]
fn tutorial_xy_universe_round_trips() {
    let Some(bytes) = tutorial_fixture("tutorial.xy") else {
        eprintln!("skipping: tutorial.xy not present");
        return;
    };

    // Not fully framed, so the generic decoder still errors on it.
    assert!(
        StarsFile::decode(&bytes).is_err(),
        "expected generic .xy decode to error; use Universe instead"
    );

    let universe = Universe::decode(&bytes).unwrap_or_else(|e| panic!("Universe::decode: {e}"));

    assert_eq!(universe.header.game_id, TUTORIAL_GAME_ID, "game id");
    assert_eq!(universe.header.file_type, FileType::Universe, "type");
    assert_eq!(universe.planet_count(), 24, "planet count");
    assert_eq!(universe.player_count(), 2, "player count");

    // Game name from the game-info block.
    let gi = &universe.game_info;
    let name_end = gi[32..64].iter().position(|&b| b == 0).unwrap_or(32);
    let name = std::str::from_utf8(&gi[32..32 + name_end]).unwrap();
    assert_eq!(name, "Tutorial Game", "game name");

    // Unique absolute positions and resolved names.
    assert_planets_valid("tutorial.xy", &universe);

    // Spot-check a couple of resolved planet names against the master table.
    let planets = universe.planets_resolved();
    assert_eq!(planets[0].name, Some("Lever"), "tutorial planet 0 name");
    assert_eq!(planets[23].name, Some("Bloop"), "tutorial planet 23 name");

    // In-game `.xy` carries a 2-byte `00 00` trailer.
    assert_eq!(universe.trailer, [0, 0], "in-game .xy trailer");

    let reencoded = universe.encode().unwrap_or_else(|e| panic!("encode: {e}"));
    assert_eq!(reencoded, bytes, "tutorial.xy re-encode not byte-identical");
}

/// Every real file header re-encodes to the bytes it was read from.
///
/// `FileHeader::to_payload` is what lets this crate write a file it did not
/// read, so it has to be an exact inverse of the parser — including the
/// version, salt and flag packing, which no round-trip through
/// `StarsFile::encode` would exercise (that one keeps the payload verbatim).
#[test]
fn every_header_re_encodes_exactly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    if !root.is_dir() {
        eprintln!("skipping: no fixtures");
        return;
    }
    let mut stack = vec![root];
    let mut checked = 0;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            // The header block is the first framed block of every file.
            if bytes.len() < 18 {
                continue;
            }
            let size = usize::from(u16::from_le_bytes([bytes[0], bytes[1]]) & 0x03FF);
            if size < 16 || bytes.len() < 2 + size {
                continue;
            }
            let payload = &bytes[2..2 + 16];
            let Ok(header) = stars_formats::FileHeader::parse(payload) else {
                continue;
            };
            assert_eq!(
                header.to_payload(),
                payload,
                "{} header does not re-encode",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no fixture headers were checked");
    eprintln!("{checked} headers re-encoded exactly");
}

/// Every `.xy` game-info block decodes into `GameInfo` and back unchanged.
#[test]
fn every_game_info_re_encodes_exactly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    if !root.is_dir() {
        eprintln!("skipping: no fixtures");
        return;
    }
    let mut stack = vec![root];
    let mut checked = 0;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "xy") {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(universe) = Universe::decode(&bytes) else {
                continue;
            };
            let info = universe.game().expect("game info");
            assert_eq!(
                info.encode(),
                universe.game_info,
                "{} game info does not re-encode",
                path.display()
            );
            assert_eq!(
                info.players,
                i16::from(universe.player_count()),
                "{} player count disagrees with the block",
                path.display()
            );
            assert_eq!(
                info.planets as usize,
                universe.planet_count(),
                "{} planet count disagrees with the block",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no .xy fixtures were checked");
    eprintln!("{checked} game-info blocks re-encoded exactly");
}
