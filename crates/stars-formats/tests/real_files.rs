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
//! The contracts checked here:
//!
//! - fully block-framed files (`.hst`, `.mN`, `.hN`, `.xN`) **decode → encode
//!   byte-for-byte**, for both turn 0 and turn 1 (proving the cipher seeding is
//!   correct on non-zero turns);
//! - the file header decodes to the expected shared `game_id`, per-file type,
//!   per-player numbering, and turn;
//! - the `.xy` universe header + game-info (type-7) block decrypt to the
//!   expected game name and player count (its trailing planet array is not yet
//!   decoded — see `docs/formats/xy.md`).
//!
//! If the fixtures are absent (e.g. a checkout without the sample game), each
//! test skips rather than fails so CI stays green.

use std::path::{Path, PathBuf};

use stars_formats::{planet_headers, BlockType, FileType, StarsFile, Universe};

/// Expected per-game id shared by every file of the sample game.
const GAME_ID: u32 = 0x2a03_1dd8;

/// Read a fixture by path relative to `fixtures/incoming/`, e.g.
/// `"turn0/Game.hst"`. Returns `None` (so the test skips) if it is absent.
fn fixture(rel: &str) -> Option<Vec<u8>> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/incoming")
        .join(rel);
    std::fs::read(&path).ok()
}

/// Decode → encode must reproduce the original bytes exactly for framed files,
/// with the header decoding to the expected type, player, and turn.
fn assert_round_trips(rel: &str, expected_type: FileType, expected_player: u8, expected_turn: u16) {
    let Some(bytes) = fixture(rel) else {
        eprintln!("skipping {rel}: fixture not present");
        return;
    };

    let file = StarsFile::decode(&bytes).unwrap_or_else(|e| panic!("decode {rel}: {e}"));
    assert_eq!(file.header.game_id, GAME_ID, "{rel}: game id");
    assert_eq!(file.header.file_type, expected_type, "{rel}: file type");
    assert_eq!(file.header.player, expected_player, "{rel}: player index");
    assert_eq!(file.header.turn, expected_turn, "{rel}: turn");

    let reencoded = file
        .encode()
        .unwrap_or_else(|e| panic!("encode {rel}: {e}"));
    assert_eq!(
        reencoded, bytes,
        "{rel}: re-encode is not byte-for-byte identical"
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
    // `.hst`. Coordinates are bounded to the 10-bit field and no two planets
    // share a position (both broken at any offset other than the real one).
    assert_eq!(universe.planet_count(), 128, "{rel}: planet count");
    let mut seen = std::collections::HashSet::new();
    for (i, p) in universe.planets.iter().enumerate() {
        assert!(
            p.x < 1024 && p.y < 1024,
            "{rel}: planet {i} coord out of range"
        );
        assert!(
            seen.insert((p.x, p.y)),
            "{rel}: planet {i} shares a position"
        );
    }

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

        // Coordinates are 10-bit-bounded and no two planets share a position.
        let mut seen = std::collections::HashSet::new();
        for (i, p) in universe.planets.iter().enumerate() {
            assert!(
                p.x < 1024 && p.y < 1024,
                "{name}: planet {i} coord out of range"
            );
            assert!(
                seen.insert((p.x, p.y)),
                "{name}: planet {i} shares a position"
            );
        }

        // These standalone universe files carry a 2-byte trailer equal to the
        // player count (game-info offset 8).
        assert_eq!(
            universe.trailer.len(),
            2,
            "{name}: expected a 2-byte trailer"
        );
        let trailer_val = u16::from_le_bytes([universe.trailer[0], universe.trailer[1]]);
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
