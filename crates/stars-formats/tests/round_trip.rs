//! Every record encoder, checked against every record in the fixtures.
//!
//! The crate's contract is `write(read(bytes)) == bytes`. Until now that held
//! only at the whole-file level, where a block's payload is carried verbatim.
//! These tests assert it one record at a time, which is what a `.hst` or `.mN`
//! written from scratch depends on: a decoder that quietly drops a field is
//! harmless while the bytes are kept, and silently destructive once they are
//! not.
//!
//! Two real decoder bugs were found this way and are fixed: planet blocks were
//! dropping the concentration-decay accumulators (`rgpctMinLevel`), and the
//! plural race/player name was read to the end of the block instead of being
//! bounded by its own length byte, so any record with padding after it decoded
//! trailing spaces into the name.
//!
//! The tests skip rather than fail when the fixtures are absent.

use std::path::{Path, PathBuf};

use stars_formats::{
    BattlePlanRecord, DesignRecord, FleetRecord, PlanetRecord, PlayerRecord, ProductionQueueRecord,
    StarsFile, Thing, WaypointRecord, THING_SIZE,
};

/// Every file under `fixtures/`, in a stable order.
fn fixtures() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"),
        &mut out,
    );
    out.sort();
    out
}

/// Every decodable file in the fixtures, as decoded files.
fn files() -> Vec<StarsFile> {
    fixtures()
        .iter()
        .filter_map(|path| std::fs::read(path).ok())
        .filter_map(|bytes| StarsFile::decode(&bytes).ok())
        .collect()
}

/// Run `check` over every block of one type, and report how many were checked.
fn sweep(type_ids: &[u8], label: &str, mut check: impl FnMut(&[u8]) -> Option<Vec<u8>>) {
    let files = files();
    if files.is_empty() {
        eprintln!("skipping {label}: no fixtures");
        return;
    }
    let mut checked = 0usize;
    for file in &files {
        for block in &file.blocks {
            if !type_ids.contains(&block.type_id) {
                continue;
            }
            let Some(encoded) = check(&block.data) else {
                continue;
            };
            assert_eq!(
                encoded, block.data,
                "{label}: a type-{} block does not re-encode",
                block.type_id
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "{label}: no blocks were checked");
    eprintln!("{label}: {checked} blocks re-encoded exactly");
}

#[test]
fn every_planet_block_re_encodes() {
    let files = files();
    if files.is_empty() {
        eprintln!("skipping: no fixtures");
        return;
    }
    let mut checked = 0usize;
    for file in &files {
        for block in &file.blocks {
            let type_id = match block.type_id {
                13..=15 => block.type_id,
                _ => continue,
            };
            let Some(record) = PlanetRecord::decode(&block.data, type_id) else {
                continue;
            };
            assert_eq!(record.block_type, type_id);
            assert_eq!(
                record.encode(),
                block.data,
                "a type-{type_id} planet block does not re-encode"
            );
            checked += 1;
        }
    }
    assert!(checked > 0);
    eprintln!("planet: {checked} blocks re-encoded exactly");
}

#[test]
fn every_fleet_block_re_encodes() {
    let files = files();
    if files.is_empty() {
        eprintln!("skipping: no fixtures");
        return;
    }
    let mut checked = 0usize;
    for file in &files {
        for block in &file.blocks {
            let type_id = match block.type_id {
                16..=18 => block.type_id,
                _ => continue,
            };
            let Some(record) = FleetRecord::decode(&block.data, type_id) else {
                continue;
            };
            assert_eq!(
                record.encode(type_id),
                block.data,
                "a type-{type_id} fleet block does not re-encode"
            );
            checked += 1;
        }
    }
    assert!(checked > 0);
    eprintln!("fleet: {checked} blocks re-encoded exactly");
}

#[test]
fn every_waypoint_block_re_encodes() {
    let files = files();
    if files.is_empty() {
        eprintln!("skipping: no fixtures");
        return;
    }
    let mut checked = 0usize;
    for file in &files {
        for block in &file.blocks {
            if block.type_id != 19 && block.type_id != 20 {
                continue;
            }
            let Some(record) = WaypointRecord::decode(&block.data) else {
                continue;
            };
            assert_eq!(record.encode(), block.data, "a waypoint does not re-encode");
            assert_eq!(
                record.block_type(),
                block.type_id,
                "a waypoint would be written as the wrong block type"
            );
            checked += 1;
        }
    }
    assert!(checked > 0);
    eprintln!("waypoint: {checked} blocks re-encoded exactly");
}

#[test]
fn every_design_block_re_encodes() {
    sweep(&[26], "design", |data| {
        DesignRecord::from_payload(data)
            .ok()
            .and_then(|r| r.encode().ok())
    });
}

#[test]
fn every_player_block_re_encodes() {
    sweep(&[6], "player", |data| {
        PlayerRecord::from_payload(data)
            .ok()
            .and_then(|r| r.encode().ok())
    });
}

#[test]
fn every_battle_plan_block_re_encodes() {
    sweep(&[30], "battle plan", |data| {
        BattlePlanRecord::from_payload(data)
            .ok()
            .and_then(|r| r.encode().ok())
    });
}

#[test]
fn every_production_queue_block_re_encodes() {
    sweep(&[28], "production queue", |data| {
        Some(ProductionQueueRecord::decode(data).encode())
    });
}

#[test]
fn every_thing_record_re_encodes() {
    sweep(&[43], "thing", |data| {
        if data.len() != THING_SIZE {
            // The two-byte count record that heads the section.
            return None;
        }
        Thing::decode(data).map(|t| t.encode().to_vec())
    });
}

/// The packed-string encoder, over every name in the fixtures.
///
/// Design names, battle-plan names and race names all go through it, and the
/// tests above already assert the bytes; this asserts the round trip at the
/// string level so a failure says which name broke.
#[test]
fn every_name_round_trips_through_the_string_codec() {
    let files = files();
    if files.is_empty() {
        eprintln!("skipping: no fixtures");
        return;
    }
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for file in &files {
        for block in &file.blocks {
            match block.type_id {
                26 => {
                    if let Ok(r) = DesignRecord::from_payload(&block.data) {
                        names.insert(r.name);
                    }
                }
                30 => {
                    if let Ok(r) = BattlePlanRecord::from_payload(&block.data) {
                        names.insert(r.name);
                    }
                }
                6 => {
                    if let Ok(r) = PlayerRecord::from_payload(&block.data) {
                        names.insert(r.singular_name);
                        names.insert(r.plural_name);
                    }
                }
                _ => {}
            }
        }
    }
    assert!(names.len() > 50, "expected many distinct names");
    for name in &names {
        let field = stars_formats::strings::encode_field(name).expect("short enough");
        assert_eq!(
            &stars_formats::decode_stars_string(&field),
            name,
            "name does not survive the codec"
        );
    }
    eprintln!("strings: {} distinct names round-tripped", names.len());
}
