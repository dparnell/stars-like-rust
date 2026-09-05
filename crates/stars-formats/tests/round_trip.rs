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
    order_log, BattlePlanRecord, CargoTransfer, DesignRecord, FleetMerge, FleetOrderDelete,
    FleetOrderTask, FleetPlan, FleetRecord, FleetRepeatOrders, FleetSplit, LogHeader,
    LogRecordType, PlanetRecord, PlanetRoutingOrder, PlayerRecord, ProductionQueueRecord,
    Relations, ResearchOrder, ShipDesignChange, StarsFile, Thing, ThingParam, WaypointOrder,
    WaypointRecord, THING_SIZE,
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

/// Every order-log record re-encodes, and every `.xN` file rebuilds whole.
///
/// The order log is the one format whose records are *operations* rather than
/// state, and the one this project has to be able to write for a game to be
/// playable against a real host. The per-record check is the same contract as
/// everywhere else; the whole-file check additionally exercises `cbLog`, which
/// the writer recomputes rather than copying.
#[test]
fn every_order_record_re_encodes() {
    let mut checked: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    let mut files = 0usize;

    for path in fixtures() {
        if path
            .extension()
            .is_none_or(|e| !e.to_string_lossy().to_lowercase().starts_with('x'))
            || path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("xy"))
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        files += 1;

        for block in &file.blocks {
            let data = &block.data;
            let mut note = |label: &'static str| *checked.entry(label).or_default() += 1;
            match LogRecordType::from_id(block.type_id) {
                LogRecordType::Header => {
                    let record = LogHeader::decode(data).expect("log header");
                    assert_eq!(record.encode(), data.as_slice(), "{}", path.display());
                    note("log header");
                }
                LogRecordType::FleetOrderInsert | LogRecordType::FleetOrderUpdate => {
                    let record = WaypointOrder::decode(data).expect("waypoint order");
                    assert_eq!(&record.encode(), data, "{}", path.display());
                    note("waypoint");
                }
                LogRecordType::FleetOrderDelete => {
                    let record = FleetOrderDelete::decode(data).expect("order delete");
                    assert_eq!(
                        record.encode().as_slice(),
                        data.as_slice(),
                        "{}",
                        path.display()
                    );
                    note("order delete");
                }
                LogRecordType::Research => {
                    let record = ResearchOrder::decode(data).expect("research");
                    assert_eq!(
                        record.encode().as_slice(),
                        data.as_slice(),
                        "{}",
                        path.display()
                    );
                    note("research");
                }
                LogRecordType::PlanetRouting => {
                    let record = PlanetRoutingOrder::decode(data).expect("planet routing");
                    assert_eq!(
                        record.encode().as_slice(),
                        data.as_slice(),
                        "{}",
                        path.display()
                    );
                    note("planet routing");
                }
                kind @ (LogRecordType::CargoXfer8
                | LogRecordType::CargoXfer16
                | LogRecordType::CargoXfer32
                | LogRecordType::FleetCargoXfer) => {
                    let record = CargoTransfer::decode(data, kind).expect("cargo transfer");
                    assert_eq!(
                        record.encode(kind).expect("a transfer op"),
                        *data,
                        "{}",
                        path.display()
                    );
                    note("cargo transfer");
                }
                LogRecordType::PlanetProdQueue => {
                    let record = ProductionQueueRecord::decode_change(data).expect("queue change");
                    assert_eq!(record.encode_change(), *data, "{}", path.display());
                    note("production queue");
                }
                LogRecordType::ShipDesign => {
                    let record = ShipDesignChange::decode(data).expect("design change");
                    assert_eq!(
                        record.encode().expect("encodes"),
                        *data,
                        "{}",
                        path.display()
                    );
                    note("ship design");
                }
                LogRecordType::FleetFlagBit => {
                    let record = FleetRepeatOrders::decode(data).expect("repeat orders");
                    assert_eq!(
                        record.encode().as_slice(),
                        data.as_slice(),
                        "{}",
                        path.display()
                    );
                    note("repeat orders");
                }
                LogRecordType::FleetOrderAttrNib => {
                    let record = FleetOrderTask::decode(data).expect("order task");
                    assert_eq!(
                        record.encode().as_slice(),
                        data.as_slice(),
                        "{}",
                        path.display()
                    );
                    note("waypoint task");
                }
                LogRecordType::FleetPlan => {
                    let record = FleetPlan::decode(data).expect("battle plan");
                    assert_eq!(
                        record.encode().as_slice(),
                        data.as_slice(),
                        "{}",
                        path.display()
                    );
                    note("battle plan");
                }
                LogRecordType::Relations => {
                    let record = Relations::decode(data);
                    assert_eq!(record.encode(), *data, "{}", path.display());
                    assert!(
                        !record.toward.is_empty() && record.toward.len() <= 16,
                        "{}: one byte per player",
                        path.display()
                    );
                    note("relations");
                }
                LogRecordType::FleetSplit => {
                    let record = FleetSplit::decode(data).expect("fleet split");
                    assert_eq!(
                        record.encode().as_slice(),
                        data.as_slice(),
                        "{}",
                        path.display()
                    );
                    note("fleet split");
                }
                LogRecordType::FleetMerge => {
                    let record = FleetMerge::decode(data).expect("fleet merge");
                    assert_eq!(record.encode(), *data, "{}", path.display());
                    assert!(
                        record.survivor().is_some() && !record.absorbed().is_empty(),
                        "{}: a merge names at least two fleets",
                        path.display()
                    );
                    note("fleet merge");
                }
                LogRecordType::ThingByteParam => {
                    let record = ThingParam::decode(data).expect("thing param");
                    assert_eq!(
                        record.encode().as_slice(),
                        data.as_slice(),
                        "{}",
                        path.display()
                    );
                    note("thing param");
                }
                _ => {}
            }
        }

        // And the whole file, rebuilt from the parsed log.
        let log = order_log(&file);
        let rebuilt = log.to_file(&file.header).expect("writes");
        assert_eq!(
            rebuilt,
            bytes,
            "{} does not rebuild byte for byte",
            path.display()
        );
        assert_eq!(
            log.log_byte_count(),
            usize::from(log.header.expect("a log header").log_byte_count),
            "{}: cbLog disagrees with the records",
            path.display()
        );
    }

    if files == 0 {
        eprintln!("skipping: no .xN fixtures");
        return;
    }
    eprintln!("order log: {files} files rebuilt byte for byte; records {checked:?}");
}
