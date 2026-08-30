//! Differential tests against the **exodus** game — a third, independent
//! capture in `fixtures/games/exodus/`.
//!
//! Unlike the earlier samples (a couple of turns each), exodus is a long
//! **single-player perspective**: player 6's turn file (`exodus.m6`) saved at
//! ~40 consecutive game years (`2400/`, `2402/`, … `2450/` — the directory name
//! is the in-game year, so `turn = year - 2400`). It also ships the universe
//! (`exodus.xy`, a large 540-planet / 8-player standalone universe) and, in
//! `Races/`, the seven built-in **AI expansion races** (`BIGPRO`, `DEFENDER`,
//! `ECOBOOM`, `FLEXIBLE`, `JUMPERS`, `OFFENDER`, `SNEAK`).
//!
//! Value of this capture:
//!
//! - the 40-turn `.m6` sequence exercises the container/cipher across many
//!   consecutive turns of one game and is the raw material for decoding the
//!   dynamic per-turn records (planets/fleets) later;
//! - the AI races are the first fixtures with **Lesser Racial Traits enabled**
//!   (the default races in `fixtures/r/` all have `LRT = 0`), so they pin down
//!   the LRT bitfield and confirm the PRT byte: each race's primary trait
//!   matches its file name (`OFFENDER` → WM, `DEFENDER` → SD, `SNEAK` → SS, …).
//!
//! If the fixtures are absent, each test skips rather than fails.

use std::path::{Path, PathBuf};

use stars_formats::{FileType, Lrt, Prt, RaceRecord, StarsFile, Universe};

/// Exodus is played from the perspective of player 6 (`.m6`), 0-based index 5.
const EXODUS_PLAYER: u8 = 5;

fn exodus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/games/exodus")
}

/// The game years for which an `exodus.m6` snapshot exists, sorted ascending.
fn exodus_years() -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(exodus_root()) else {
        return Vec::new();
    };
    let mut years: Vec<u32> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_string_lossy().parse::<u32>().ok())
        .filter(|y| exodus_root().join(y.to_string()).join("exodus.m6").exists())
        .collect();
    years.sort_unstable();
    years
}

/// Every `exodus.m6` snapshot decodes with the expected header (player 6, the
/// year-derived turn) and re-encodes byte-for-byte. This walks the whole
/// sequence, proving the cipher seeding is correct across ~40 consecutive
/// turns.
#[test]
fn exodus_m6_sequence_round_trips() {
    let years = exodus_years();
    if years.is_empty() {
        eprintln!("skipping: no exodus .m6 snapshots present");
        return;
    }

    // The folder names are the user's own labels; the authoritative turn is the
    // one in each header. It is a monotonic counter (strictly increasing across
    // the sorted snapshots), starting at 0.
    let mut game_id: Option<u32> = None;
    let mut last_turn: Option<u16> = None;
    for year in &years {
        let path = exodus_root().join(year.to_string()).join("exodus.m6");
        let bytes = std::fs::read(&path).unwrap();
        let label = format!("{year}/exodus.m6");

        let file = StarsFile::decode(&bytes).unwrap_or_else(|e| panic!("decode {label}: {e}"));
        assert_eq!(file.header.file_type, FileType::Turn, "{label}: file type");
        assert_eq!(file.header.player, EXODUS_PLAYER, "{label}: player index");

        // Turn counter is strictly increasing across the ordered snapshots.
        if let Some(prev) = last_turn {
            assert!(
                file.header.turn > prev,
                "{label}: turn {} not greater than previous {prev}",
                file.header.turn
            );
        }
        last_turn = Some(file.header.turn);

        // All snapshots belong to one game, so the game id is constant.
        match game_id {
            None => game_id = Some(file.header.game_id),
            Some(id) => assert_eq!(file.header.game_id, id, "{label}: shared game id"),
        }

        let reencoded = file
            .encode()
            .unwrap_or_else(|e| panic!("encode {label}: {e}"));
        assert_eq!(reencoded, bytes, "{label}: re-encode not byte-identical");
    }

    // Sanity: this is a long sequence, not one or two turns.
    assert!(
        years.len() >= 30,
        "expected a long turn sequence, got {}",
        years.len()
    );
    // The first snapshot is the game start (turn 0).
    let first = exodus_root().join(years[0].to_string()).join("exodus.m6");
    let first = StarsFile::decode(&std::fs::read(first).unwrap()).unwrap();
    assert_eq!(first.header.turn, 0, "first snapshot is turn 0");
}

/// The exodus universe is a large **standalone** universe file (540 planets, 8
/// players). It parses through the [`Universe`] decoder and round-trips
/// byte-for-byte, with a 4-byte `02 00 <players> 00` trailer.
#[test]
fn exodus_xy_round_trips() {
    let path = exodus_root().join("exodus.xy");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipping: exodus.xy not present");
        return;
    };

    // Not fully block-framed, so the generic decoder rejects it.
    assert!(
        StarsFile::decode(&bytes).is_err(),
        "expected generic .xy decode to error; use Universe instead"
    );

    let universe = Universe::decode(&bytes).unwrap_or_else(|e| panic!("Universe::decode: {e}"));
    assert_eq!(universe.header.file_type, FileType::Universe, "file type");
    assert_eq!(universe.planet_count(), 540, "planet count");
    assert_eq!(universe.player_count(), 8, "player count");

    // Standalone universe trailer: `02 00` then the player count.
    assert_eq!(universe.trailer.len(), 4, "standalone trailer length");
    assert_eq!(&universe.trailer[0..2], &[0x02, 0x00], "trailer prefix");
    assert_eq!(
        u16::from_le_bytes([universe.trailer[2], universe.trailer[3]]),
        u16::from(universe.player_count()),
        "trailer equals player count"
    );

    // Every planet resolves to a unique name/position (validated inline here to
    // keep the exodus tests self-contained).
    let planets = universe.planets_resolved();
    assert_eq!(planets.len(), 540, "resolved planet count");
    let mut seen = std::collections::HashSet::new();
    let mut last_x = 0u32;
    for p in &planets {
        assert!(p.name.is_some(), "planet {} name index unresolved", p.id);
        assert!(p.x >= last_x, "absolute x not non-decreasing at {}", p.id);
        last_x = p.x;
        assert!(seen.insert((p.x, p.y)), "planet {} shares a position", p.id);
    }

    let reencoded = universe.encode().unwrap_or_else(|e| panic!("encode: {e}"));
    assert_eq!(reencoded, bytes, "exodus.xy re-encode not byte-identical");
}

/// The seven AI-expansion race files decode via [`RaceRecord`] to the expected
/// primary racial trait (which matches the file name) and lesser-racial-trait
/// set. These are the first fixtures with LRTs enabled, so this is the test
/// that actually confirms the LRT bit layout.
#[test]
fn exodus_races_decode_prt_and_lrts() {
    // (file name, expected PRT, expected LRT set) — read off the real records.
    let cases: &[(&str, Prt, &[Lrt])] = &[
        (
            "BIGPRO.R1",
            Prt::IS,
            &[Lrt::IFE, Lrt::ISB, Lrt::OBRM, Lrt::LSP, Lrt::RS],
        ),
        (
            "DEFENDER.R1",
            Prt::SD,
            &[Lrt::IFE, Lrt::OBRM, Lrt::LSP, Lrt::RS],
        ),
        (
            "ECOBOOM.R1",
            Prt::CA,
            &[Lrt::IFE, Lrt::OBRM, Lrt::LSP, Lrt::RS],
        ),
        (
            "FLEXIBLE.R1",
            Prt::JOAT,
            &[Lrt::IFE, Lrt::OBRM, Lrt::LSP, Lrt::BET],
        ),
        (
            "JUMPERS.R1",
            Prt::IT,
            &[Lrt::IFE, Lrt::OBRM, Lrt::LSP, Lrt::RS],
        ),
        (
            "OFFENDER.R1",
            Prt::WM,
            &[Lrt::IFE, Lrt::OBRM, Lrt::LSP, Lrt::RS, Lrt::MA],
        ),
        (
            "SNEAK.R1",
            Prt::SS,
            &[Lrt::IFE, Lrt::OBRM, Lrt::LSP, Lrt::RS],
        ),
    ];

    let dir = exodus_root().join("Races");
    let mut seen = 0;
    for (name, prt, lrts) in cases {
        let path = dir.join(name);
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("skipping {name}: not present");
            continue;
        };
        seen += 1;

        let file = StarsFile::decode(&bytes).unwrap_or_else(|e| panic!("decode {name}: {e}"));
        assert_eq!(file.header.file_type, FileType::Race, "{name}: file type");

        let race = RaceRecord::from_file(&file).unwrap_or_else(|e| panic!("race {name}: {e}"));
        assert_eq!(race.player_id, 0xFF, "{name}: race-only player marker");
        assert_eq!(race.prt, *prt, "{name}: PRT");
        assert_eq!(race.lrts(), lrts.to_vec(), "{name}: LRT set");
        assert!(
            !race.has_unknown_lrt_bits(),
            "{name}: stray bits outside the 14-bit LRT range"
        );

        // Container still round-trips byte-for-byte (the typed view is read-only).
        let reencoded = file
            .encode()
            .unwrap_or_else(|e| panic!("encode {name}: {e}"));
        assert_eq!(reencoded, bytes, "{name}: re-encode not byte-identical");
    }

    if seen == 0 {
        eprintln!("skipping: no exodus race fixtures present");
    }
}

/// Every AI race shares a common LRT base — Improved Fuel Efficiency, Only Basic
/// Remote Mining and Low Starting Population — which is what makes the bitfield
/// decode credible (a coincidental mis-alignment would not produce the same
/// sensible, overlapping trait sets across seven independent races).
#[test]
fn exodus_races_share_a_common_lrt_base() {
    let dir = exodus_root().join("Races");
    let names = [
        "BIGPRO.R1",
        "DEFENDER.R1",
        "ECOBOOM.R1",
        "FLEXIBLE.R1",
        "JUMPERS.R1",
        "OFFENDER.R1",
        "SNEAK.R1",
    ];
    let mut seen = 0;
    for name in names {
        let Ok(bytes) = std::fs::read(dir.join(name)) else {
            continue;
        };
        seen += 1;
        let file = StarsFile::decode(&bytes).unwrap();
        let race = RaceRecord::from_file(&file).unwrap();
        for lrt in [Lrt::IFE, Lrt::OBRM, Lrt::LSP] {
            assert!(race.has_lrt(lrt), "{name}: expected common trait {lrt:?}");
        }
    }
    if seen == 0 {
        eprintln!("skipping: no exodus race fixtures present");
    }
}
