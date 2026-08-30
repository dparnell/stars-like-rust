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

use stars_formats::block::split_blocks;
use stars_formats::{FileHeader, FileType, StarsFile};

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

/// The `.xy` universe file is not fully block-framed (a raw planet array
/// follows the game-info block), so full decode is expected to error today.
/// We can still decrypt its header + game-info block manually. Checked for both
/// captured turns (the `.xy` is unchanged across a turn generation).
fn assert_xy_header_and_game_info(rel: &str) {
    let Some(bytes) = fixture(rel) else {
        eprintln!("skipping {rel}: fixture not present");
        return;
    };

    // Full framing currently fails on the trailing planet array.
    assert!(
        StarsFile::decode(&bytes).is_err(),
        "{rel}: expected .xy full-frame decode to error until planet array is decoded"
    );

    // First block = plaintext header.
    let first = &split_blocks(&bytes[..18]).unwrap()[0];
    let header = FileHeader::parse(&first.data).unwrap();
    assert_eq!(header.game_id, GAME_ID, "{rel}: game id");
    assert_eq!(header.file_type, FileType::Universe, "{rel}: file type");

    // Second block (type 7, 64 bytes) = encrypted game-info; decrypt it.
    let mut rng = header.init_rng();
    let game_info = rng.apply(&bytes[0x14..0x14 + 64]);
    let players = game_info[8] & 0x1F;
    assert_eq!(players, 3, "{rel}: sample game has 3 players");
    let name_end = game_info[32..64].iter().position(|&b| b == 0).unwrap_or(32);
    let name = std::str::from_utf8(&game_info[32..32 + name_end]).unwrap();
    assert_eq!(name, "A Barefoot JayWalk", "{rel}: game name");
}

#[test]
fn turn0_xy_header_and_game_info_decode() {
    assert_xy_header_and_game_info("turn0/Game.xy");
}

#[test]
fn turn1_xy_header_and_game_info_decode() {
    assert_xy_header_and_game_info("turn1/Game.xy");
}
