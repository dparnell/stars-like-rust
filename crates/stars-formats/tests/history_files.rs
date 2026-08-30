//! Differential tests for the **history-file header** (`rtHistHdr`, type 32)
//! against the real `.hN` fixtures.
//!
//! The header's `cPlanet` field records how many planet blocks the history file
//! carries (the planets that player has knowledge of), so it is cross-checked
//! against the actual count of planet blocks in the same file:
//!
//! - `fixtures/incoming/turn1/Game.h{1,2,3}` — the 3-player 128-planet game
//!   (each player sees only 1–2 planets on turn 1);
//! - `fixtures/games/tutorial/tutorial.h1` — the Tutorial Game.
//!
//! If the fixtures are absent (a checkout without the sample games), each test
//! skips rather than fails so CI stays green.

use std::path::{Path, PathBuf};

use stars_formats::{history_header, BlockType, StarsFile};

fn read_rel(rel: &str) -> Option<Vec<u8>> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(rel);
    std::fs::read(&path).ok()
}

/// Number of planet blocks (full/partial/minimal) in a decoded file.
fn planet_block_count(file: &StarsFile) -> usize {
    file.blocks
        .iter()
        .filter(|b| {
            matches!(
                b.block_type(),
                BlockType::Planet | BlockType::PartialPlanet | BlockType::MinimalPlanet
            )
        })
        .count()
}

/// Decode a `.hN` history file and assert exactly one `rtHistHdr` block is
/// present whose `cPlanet` equals the number of planet blocks that follow.
fn assert_history_consistent(label: &str, bytes: &[u8]) {
    let file = StarsFile::decode(bytes).unwrap_or_else(|e| panic!("decode {label}: {e}"));

    let hist_blocks = file
        .blocks
        .iter()
        .filter(|b| b.block_type() == BlockType::HistoryHeader)
        .count();
    assert_eq!(
        hist_blocks, 1,
        "{label}: expected exactly one rtHistHdr block"
    );

    let hdr = history_header(&file).unwrap_or_else(|| panic!("{label}: no history header"));
    assert_eq!(
        hdr.planet_count as usize,
        planet_block_count(&file),
        "{label}: rtHistHdr cPlanet vs planet-block count"
    );
}

#[test]
fn game_history_headers_are_consistent() {
    // If the fixtures are absent this simply checks nothing (skips cleanly).
    for player in 1..=3 {
        let rel = format!("incoming/turn1/Game.h{player}");
        if let Some(bytes) = read_rel(&rel) {
            assert_history_consistent(&rel, &bytes);
        }
    }
}

#[test]
fn tutorial_history_header_is_consistent() {
    if let Some(bytes) = read_rel("games/tutorial/tutorial.h1") {
        assert_history_consistent("tutorial.h1", &bytes);
    }
}
