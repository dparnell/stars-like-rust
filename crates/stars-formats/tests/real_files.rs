//! Differential tests against **real** Stars! sample files.
//!
//! These read the fixtures the user supplied in `fixtures/incoming/` (a fresh
//! 3-player game) and assert the core correctness contracts:
//!
//! - fully block-framed files (`.hst`, `.mN`) **decode → encode byte-for-byte**;
//! - the file header decodes to the expected shared `game_id`, per-file type,
//!   and per-player numbering;
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

fn fixture(name: &str) -> Option<Vec<u8>> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/incoming")
        .join(name);
    std::fs::read(&path).ok()
}

/// Decode → encode must reproduce the original bytes exactly for framed files.
fn assert_round_trips(name: &str, expected_type: FileType, expected_player: u8) {
    let Some(bytes) = fixture(name) else {
        eprintln!("skipping {name}: fixture not present");
        return;
    };

    let file = StarsFile::decode(&bytes).unwrap_or_else(|e| panic!("decode {name}: {e}"));
    assert_eq!(file.header.game_id, GAME_ID, "{name}: game id");
    assert_eq!(file.header.file_type, expected_type, "{name}: file type");
    assert_eq!(file.header.player, expected_player, "{name}: player index");

    let reencoded = file
        .encode()
        .unwrap_or_else(|e| panic!("encode {name}: {e}"));
    assert_eq!(
        reencoded, bytes,
        "{name}: re-encode is not byte-for-byte identical"
    );
}

#[test]
fn hst_round_trips() {
    // Host file: shared, so it carries the "no specific player" marker (31).
    assert_round_trips("Game.hst", FileType::Host, 31);
}

#[test]
fn m1_round_trips() {
    assert_round_trips("Game.m1", FileType::Turn, 0);
}

#[test]
fn m2_round_trips() {
    assert_round_trips("Game.m2", FileType::Turn, 1);
}

#[test]
fn m3_round_trips() {
    assert_round_trips("Game.m3", FileType::Turn, 2);
}

/// The three player files must all agree on the shared game id.
#[test]
fn player_files_share_game_id() {
    let mut seen = 0;
    for name in ["Game.m1", "Game.m2", "Game.m3"] {
        if let Some(bytes) = fixture(name) {
            let file = StarsFile::decode(&bytes).unwrap();
            assert_eq!(file.header.game_id, GAME_ID);
            seen += 1;
        }
    }
    if seen == 0 {
        eprintln!("skipping: no player fixtures present");
    }
}

/// The `.xy` universe file is not fully block-framed (a raw planet array
/// follows the game-info block), so full decode is expected to error today.
/// We can still decrypt its header + game-info block manually.
#[test]
fn xy_header_and_game_info_decode() {
    let Some(bytes) = fixture("Game.xy") else {
        eprintln!("skipping Game.xy: fixture not present");
        return;
    };

    // Full framing currently fails on the trailing planet array.
    assert!(
        StarsFile::decode(&bytes).is_err(),
        "expected .xy full-frame decode to error until planet array is decoded"
    );

    // First block = plaintext header.
    let first = &split_blocks(&bytes[..18]).unwrap()[0];
    let header = FileHeader::parse(&first.data).unwrap();
    assert_eq!(header.game_id, GAME_ID);
    assert_eq!(header.file_type, FileType::Universe);

    // Second block (type 7, 64 bytes) = encrypted game-info; decrypt it.
    let mut rng = header.init_rng();
    let game_info = rng.apply(&bytes[0x14..0x14 + 64]);
    let players = game_info[8] & 0x1F;
    assert_eq!(players, 3, "sample game has 3 players");
    let name_end = game_info[32..64].iter().position(|&b| b == 0).unwrap_or(32);
    let name = std::str::from_utf8(&game_info[32..32 + name_end]).unwrap();
    assert_eq!(name, "A Barefoot JayWalk");
}
