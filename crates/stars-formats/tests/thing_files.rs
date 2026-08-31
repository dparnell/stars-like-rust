//! Differential tests for the space-object ("thing") records (`rtThing`, type
//! id 43): minefields, mineral packets, wormholes and mystery traders.
//!
//! The section is written as a 2-byte count record followed by `count` full
//! 18-byte `THING` records (see `docs/formats/thing.md`). These tests assert
//! that the decoded [`ThingSection`] is self-consistent (count == records) and
//! that every object classifies into a known [`ThingType`], on:
//!
//! - the fresh **host** file `fixtures/incoming/turn0/Game.hst` (4 objects at
//!   game start) and its turn-1 counterpart, and
//! - the late-game **exodus** `.m6` snapshots, which accumulate 1–3 objects.
//!
//! If a fixture is absent the test skips rather than fails.

use std::path::{Path, PathBuf};

use stars_formats::{thing_section, StarsFile, ThingKind, ThingType};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Assert a file's thing section is self-consistent and fully classified.
fn assert_thing_section(bytes: &[u8], label: &str) -> usize {
    let file = StarsFile::decode(bytes).unwrap_or_else(|e| panic!("decode {label}: {e}"));
    let section = thing_section(&file);

    // The declared count must equal the number of 18-byte records decoded.
    assert_eq!(
        section.count as usize,
        section.things.len(),
        "{label}: count {} != decoded {}",
        section.count,
        section.things.len()
    );

    for (i, t) in section.things.iter().enumerate() {
        // Every object in the fixtures is one of the four known subtypes.
        assert!(
            !matches!(t.thing_type, ThingType::Unknown(_)),
            "{label}: thing {i} has unknown subtype (ith={})",
            t.ith
        );
        // The subtype tag and the decoded payload variant agree.
        let ok = matches!(
            (t.thing_type, &t.kind),
            (ThingType::Minefield, ThingKind::Minefield(_))
                | (ThingType::MineralPacket, ThingKind::MineralPacket(_))
                | (ThingType::Wormhole, ThingKind::Wormhole(_))
                | (ThingType::MysteryTrader, ThingKind::MysteryTrader(_))
        );
        assert!(ok, "{label}: thing {i} tag/payload mismatch");
        // The owning-player nibble is a valid player index.
        assert!(t.player < 16, "{label}: thing {i} bad player {}", t.player);
    }
    section.things.len()
}

/// The fresh host file starts with four space objects, all classifiable, and
/// its turn-1 counterpart is likewise consistent.
#[test]
fn host_files_thing_sections() {
    let mut seen = false;
    for (rel, expect) in [
        ("incoming/turn0/Game.hst", Some(4usize)),
        ("incoming/turn1/Game.hst", Some(4usize)),
    ] {
        let path = fixtures().join(rel);
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        seen = true;
        let n = assert_thing_section(&bytes, rel);
        if let Some(e) = expect {
            assert_eq!(n, e, "{rel}: expected {e} objects, got {n}");
        }
    }
    if !seen {
        eprintln!("skipping: no host fixtures present");
    }
}

/// Every exodus `.m6` snapshot that carries objects decodes a self-consistent,
/// fully-classified thing section, and across the run we observe objects.
#[test]
fn exodus_m6_thing_sections() {
    let root = fixtures().join("games/exodus");
    let Ok(entries) = std::fs::read_dir(&root) else {
        eprintln!("skipping: no exodus fixtures present");
        return;
    };
    let mut years: Vec<u32> = entries
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().parse::<u32>().ok())
        .filter(|y| root.join(y.to_string()).join("exodus.m6").exists())
        .collect();
    years.sort_unstable();
    if years.is_empty() {
        eprintln!("skipping: no exodus .m6 snapshots present");
        return;
    }

    let mut total_objects = 0usize;
    for year in &years {
        let path = root.join(year.to_string()).join("exodus.m6");
        let bytes = std::fs::read(&path).unwrap();
        total_objects += assert_thing_section(&bytes, &format!("{year}/exodus.m6"));
    }

    // The late-game snapshots do contain objects (minefields/packets/etc.).
    assert!(
        total_objects > 0,
        "expected at least one space object across the exodus run"
    );
}
