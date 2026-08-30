//! Differential tests for the **`.xN` player-orders file** (the order log)
//! against the real `fixtures/games/exodus/*/EXODUS.X6` sequence — 40
//! consecutive turns of player 6's submitted orders.
//!
//! These are the first orders-bearing fixtures in the project, so they are what
//! validates [`stars_formats::orders`]. Each test skips (rather than fails) when
//! the fixtures are absent so a checkout without the sample games stays green.

use std::path::{Path, PathBuf};

use stars_formats::{object_owner, order_log, FileType, LogRecordType, StarsFile};

/// Exodus is played from the perspective of player 6 (`.x6`), 0-based index 5.
const EXODUS_PLAYER: u8 = 5;

fn exodus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/games/exodus")
}

/// The game years for which an `EXODUS.X6` orders file exists, sorted ascending.
fn exodus_order_years() -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(exodus_root()) else {
        return Vec::new();
    };
    let mut years: Vec<u32> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_string_lossy().parse::<u32>().ok())
        .filter(|y| exodus_root().join(y.to_string()).join("EXODUS.X6").exists())
        .collect();
    years.sort_unstable();
    years
}

fn read_order_file(year: u32) -> Vec<u8> {
    std::fs::read(exodus_root().join(year.to_string()).join("EXODUS.X6")).unwrap()
}

/// Every `EXODUS.X6` decodes as an orders file (`dtLog`, player 6) and
/// re-encodes byte-for-byte through the container.
#[test]
fn exodus_x6_sequence_round_trips() {
    let years = exodus_order_years();
    if years.is_empty() {
        eprintln!("skipping: no exodus .x6 orders present");
        return;
    }

    let mut game_id: Option<u32> = None;
    for year in &years {
        let bytes = read_order_file(*year);
        let label = format!("{year}/EXODUS.X6");

        let file = StarsFile::decode(&bytes).unwrap_or_else(|e| panic!("decode {label}: {e}"));
        assert_eq!(
            file.header.file_type,
            FileType::Orders,
            "{label}: file type"
        );
        assert_eq!(file.header.player, EXODUS_PLAYER, "{label}: player index");

        match game_id {
            None => game_id = Some(file.header.game_id),
            Some(id) => assert_eq!(file.header.game_id, id, "{label}: shared game id"),
        }

        let reencoded = file
            .encode()
            .unwrap_or_else(|e| panic!("encode {label}: {e}"));
        assert_eq!(reencoded, bytes, "{label}: re-encode not byte-identical");
    }

    assert!(
        years.len() >= 30,
        "expected a long orders sequence, got {}",
        years.len()
    );
}

/// Each orders file carries exactly one log header whose `cbLog` equals the
/// total framed byte length of the operation records that follow it, and whose
/// serial number is constant across the whole game.
#[test]
fn exodus_x6_log_header_is_consistent() {
    let years = exodus_order_years();
    if years.is_empty() {
        eprintln!("skipping: no exodus .x6 orders present");
        return;
    }

    let mut serial: Option<i32> = None;
    let mut config: Option<[u8; 11]> = None;
    for year in &years {
        let bytes = read_order_file(*year);
        let label = format!("{year}/EXODUS.X6");
        let file = StarsFile::decode(&bytes).unwrap();
        let log = order_log(&file);

        let header = log
            .header
            .unwrap_or_else(|| panic!("{label}: no log header (RTLOGHDR)"));

        // cbLog = sum of the framed sizes (2-byte block header + payload) of
        // every operation record between the log header and the footer.
        let ops_bytes: usize = log.records.iter().map(|r| 2 + r.data.len()).sum();
        assert_eq!(
            header.log_byte_count as usize, ops_bytes,
            "{label}: RTLOGHDR cbLog vs operation-record bytes"
        );

        // The serial number and 11 config bytes are stable across the game.
        match serial {
            None => serial = Some(header.serial_number),
            Some(s) => assert_eq!(header.serial_number, s, "{label}: shared serial number"),
        }
        match config {
            None => config = Some(header.config),
            Some(c) => assert_eq!(header.config, c, "{label}: shared config bytes"),
        }
    }
}

/// Every operation record classifies to a *known* log record type (never a
/// stray `Other(_)`), and no `.xN` operation is a plain state-file block.
#[test]
fn exodus_x6_operations_are_known() {
    let years = exodus_order_years();
    if years.is_empty() {
        eprintln!("skipping: no exodus .x6 orders present");
        return;
    }

    for year in &years {
        let bytes = read_order_file(*year);
        let label = format!("{year}/EXODUS.X6");
        let file = StarsFile::decode(&bytes).unwrap();
        let log = order_log(&file);

        for rec in &log.records {
            assert!(
                !matches!(rec.record_type, LogRecordType::Other(_)),
                "{label}: unexpected order record type {:?}",
                rec.record_type
            );
        }
    }
}

/// The typed operation decoders produce coherent values on the real data:
/// waypoint/fleet-order-delete operations reference player-6 fleets, and
/// research operations stay within the six tech fields.
#[test]
fn exodus_x6_typed_operations_are_coherent() {
    let years = exodus_order_years();
    if years.is_empty() {
        eprintln!("skipping: no exodus .x6 orders present");
        return;
    }

    let mut saw_waypoint = false;
    let mut saw_research = false;
    for year in &years {
        let bytes = read_order_file(*year);
        let label = format!("{year}/EXODUS.X6");
        let file = StarsFile::decode(&bytes).unwrap();
        let log = order_log(&file);

        for rec in &log.records {
            if let Some(wp) = rec.as_waypoint() {
                saw_waypoint = true;
                assert_eq!(
                    object_owner(wp.fleet_id),
                    EXODUS_PLAYER,
                    "{label}: waypoint fleet not owned by player 6"
                );
                assert!(wp.warp <= 15, "{label}: warp nibble out of range");
            }
            if let Some(del) = rec.as_fleet_order_delete() {
                assert_eq!(
                    object_owner(del.fleet_id),
                    EXODUS_PLAYER,
                    "{label}: deleted-order fleet not owned by player 6"
                );
            }
            if let Some(res) = rec.as_research() {
                saw_research = true;
                assert!(res.pct_resources <= 100, "{label}: research % out of range");
                assert!(res.current_field <= 5, "{label}: tech field out of range");
            }
        }
    }

    // The 40-turn capture definitely contains waypoint and research orders.
    assert!(
        saw_waypoint,
        "expected at least one waypoint order in the run"
    );
    assert!(
        saw_research,
        "expected at least one research order in the run"
    );
}
