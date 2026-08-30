//! Differential tests against **real** Stars! race files (`.rN`).
//!
//! The user supplied a set of exported race files in `fixtures/r/` — the six
//! built-in default races (Antethereal, Humanoid, Insectoid, Nucleoid,
//! Rabbitoid, Silicanoid) plus a "random" race. Each is a fully block-framed
//! Stars! file: a plaintext header block (type 8), a single encrypted
//! race/player block (type 6) and a plaintext footer (type 0).
//!
//! Contracts checked here:
//!
//! - each race file **decodes → encodes byte-for-byte**, proving the shared
//!   container (framing + header + cipher) handles `.rN` files unchanged;
//! - the header decodes to `FileType::Race` and the "no specific player"
//!   marker (31);
//! - the file's block shape is exactly `[header(8), race(6), footer(0)]`.

use std::path::{Path, PathBuf};

use stars_formats::{FileType, StarsFile};

/// All race fixtures the user provided, by base filename.
const RACE_FILES: &[&str] = &[
    "antetherial.r1",
    "humanoid.r1",
    "insectoid.r1",
    "nucleoid.r1",
    "rabitoid.r1",
    "random.r1",
    "silicanoid.r1",
];

/// Read a race fixture by filename from `fixtures/r/`, or `None` if absent.
fn fixture(name: &str) -> Option<Vec<u8>> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/r")
        .join(name);
    std::fs::read(&path).ok()
}

#[test]
fn race_files_round_trip() {
    let mut seen = 0;
    for &name in RACE_FILES {
        let Some(bytes) = fixture(name) else {
            eprintln!("skipping {name}: fixture not present");
            continue;
        };
        seen += 1;

        let file = StarsFile::decode(&bytes).unwrap_or_else(|e| panic!("decode {name}: {e}"));

        // A race file is owned by "no specific player" (31) and self-identifies
        // as a race definition.
        assert_eq!(file.header.file_type, FileType::Race, "{name}: file type");
        assert_eq!(file.header.player, 31, "{name}: player marker");

        // Block shape: header (8), a single race/player block (6), footer (0).
        let shape: Vec<u8> = file.blocks.iter().map(|b| b.type_id).collect();
        assert_eq!(shape, vec![8, 6, 0], "{name}: unexpected block shape");

        let reencoded = file
            .encode()
            .unwrap_or_else(|e| panic!("encode {name}: {e}"));
        assert_eq!(
            reencoded, bytes,
            "{name}: re-encode is not byte-for-byte identical"
        );
    }

    if seen == 0 {
        eprintln!("skipping: no race fixtures present in fixtures/r/");
    }
}

/// The race/player block (type 6) begins with the `0xFF` "race-only" player id,
/// distinguishing it from the full player blocks found in `.mN`/`.hst` files.
#[test]
fn race_block_starts_with_race_marker() {
    for &name in RACE_FILES {
        let Some(bytes) = fixture(name) else {
            continue;
        };
        let file = StarsFile::decode(&bytes).unwrap();
        let race = file
            .blocks
            .iter()
            .find(|b| b.type_id == 6)
            .unwrap_or_else(|| panic!("{name}: no race block"));
        assert_eq!(race.data[0], 0xFF, "{name}: race-block player marker");
    }
}
