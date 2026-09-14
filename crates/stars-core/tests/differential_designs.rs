//! Ship designs checked against real save files.
//!
//! A full design record stores the hull and the fitted slots **and** the mass
//! and armour the original engine computed from them. That makes the design
//! model directly falsifiable: rebuild the numbers from the hull and component
//! tables and compare.
//!
//! The battle recordings give a second, independent check. Each token names
//! the design it was built from and carries the mass and shield total the
//! engine used in that battle.
//!
//! Fixtures are optional: each test skips when the sample game is absent.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use stars_core::design::{DesignSlot, ShipDesign};
use stars_formats::{battle_records_in, design_records, DesignRecord, StarsFile};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

fn to_design(r: &DesignRecord) -> ShipDesign {
    ShipDesign {
        name: String::new(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: i16::from(r.hull_id),
        slots: r
            .slots
            .iter()
            .map(|s| DesignSlot {
                category: s.category,
                item: s.item_id,
                count: s.count,
            })
            .collect(),
    }
}

/// Every fixture file worth reading designs out of.
fn fixture_files() -> Vec<PathBuf> {
    let root = workspace_root();
    let mut out = Vec::new();
    for rel in [
        "fixtures/incoming/turn0/Game.hst",
        "fixtures/incoming/turn1/Game.hst",
        "fixtures/incoming/turn0/Game.m1",
        "fixtures/incoming/turn1/Game.m1",
        "fixtures/incoming/turn1/Game.m2",
        "fixtures/incoming/turn1/Game.m3",
        "fixtures/games/tutorial/tutorial.m1",
    ] {
        let p = root.join(rel);
        if p.is_file() {
            out.push(p);
        }
    }
    let games = root.join("fixtures/games/exodus");
    if games.is_dir() {
        let mut years: Vec<i32> = std::fs::read_dir(&games)
            .expect("readable fixture dir")
            .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
            .collect();
        years.sort_unstable();
        for year in years {
            let p = games.join(year.to_string()).join("exodus.m6");
            if p.is_file() {
                out.push(p);
            }
        }
    }
    out
}

/// Full design records do not store a mass — the game recomputes it — so the
/// only place an engine-computed mass appears is a battle token, which the
/// test below checks. This one confirms the shape of that gap rather than
/// leaving it unstated.
#[test]
fn full_designs_do_not_store_a_mass() {
    let files = fixture_files();
    if files.is_empty() {
        eprintln!("skipping: no fixtures");
        return;
    }

    let mut checked = 0;
    let mut skipped_no_hull = 0;
    let mut failures = Vec::new();

    for path in &files {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let Ok(records) = design_records(&file) else {
            continue;
        };
        for record in records {
            // Partial designs carry a mass but no slots; there is nothing to
            // recompute from.
            if !record.full_design {
                continue;
            }
            // A partial design (a foreign one you have only seen) stores a
            // mass; a full one does not.
            if let Some(mass) = record.mass {
                failures.push(format!(
                    "{}: full design {:?} unexpectedly stores a mass of {mass}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    record.name
                ));
            }
            // Every full design must at least resolve to a known hull.
            let design = to_design(&record);
            if design.mass().is_none() {
                skipped_no_hull += 1;
                failures.push(format!(
                    "{}: design {:?} names unknown hull {}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    record.name,
                    record.hull_id
                ));
            }
            checked += 1;
        }
    }

    eprintln!("{checked} full designs checked, {skipped_no_hull} with an unknown hull");
    for f in failures.iter().take(10) {
        eprintln!("  {f}");
    }
    assert!(checked >= 10, "expected a meaningful sample, got {checked}");
    assert!(
        failures.is_empty(),
        "{} designs are not shaped as expected: {failures:?}",
        failures.len()
    );
}

/// Armour, computed the way `UpdateShdefCost` does.
///
/// The rule has three parts beyond the hull's own armour: fitted armour is
/// **halved for a race with Regenerating Shields**, the Croby Sharmor and
/// Langston Shell shields add 65 apiece, and a Multi Cargo Pod adds 50. The
/// halving is why this test needs each design's owner: it is a race property,
/// not a property of the design.
///
/// Attributing a design to its owner is the awkward part. In a `.hst` every
/// player block is written before any design, so position does not identify an
/// owner; this test therefore reads only **player files**, where the full
/// designs belong to the player the header names.
#[test]
fn computed_armour_matches_what_the_engine_stored() {
    let files = fixture_files();
    if files.is_empty() {
        eprintln!("skipping: no fixtures");
        return;
    }

    /// Bit 13 of the lesser-racial-trait word.
    const REGENERATING_SHIELDS: u16 = 1 << 13;

    let mut checked = 0;
    let mut uncached = 0;
    let mut regenerating = 0;
    let mut foreign = 0;
    let mut failures = Vec::new();

    for path in &files {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };

        // A host file carries every player's designs with no way to tell them
        // apart by position, so only player files are usable here.
        let segment = file.latest_segment();
        let owner = segment.header.player;
        if owner >= 16 {
            continue;
        }
        // Before the first turn is generated the cached values are not yet
        // meaningful: ships carry zero and the starbase a placeholder.
        if segment.header.turn == 0 {
            uncached += 1;
            continue;
        }
        let Ok(players) =
            stars_formats::player_records_in(file.segment_blocks(file.latest_segment()))
        else {
            continue;
        };
        let Some(race) = players
            .iter()
            .find(|p| p.player_number == owner)
            .and_then(|p| p.race.as_ref())
        else {
            continue;
        };
        let owner_regenerating = race.lrt_bits & REGENERATING_SHIELDS != 0;

        for block in file.segment_blocks(file.latest_segment()) {
            if block.type_id != 26 {
                continue;
            }
            let Ok(record) = DesignRecord::from_payload(&block.data) else {
                continue;
            };
            if !record.full_design {
                continue;
            }
            let Some(expected) = record.armor else {
                continue;
            };
            // Zero means the host has not cached it yet (a game's first files).
            if expected == 0 {
                uncached += 1;
                continue;
            }

            let design = to_design(&record);
            let Some(armor) = design.armor(owner_regenerating) else {
                continue;
            };
            checked += 1;
            if owner_regenerating {
                regenerating += 1;
            }
            if i64::from(armor) != i64::from(expected) {
                // A player file also carries foreign designs, learned in full
                // by fighting them. Those belong to another race, so the
                // halving follows that race's traits rather than this file
                // owner's.
                let other = design.armor(!owner_regenerating).unwrap_or(-1);
                if i64::from(other) == i64::from(expected) {
                    foreign += 1;
                    continue;
                }
                failures.push(format!(
                    "{}: design {:?} on hull {} computes {armor} armour, engine stored \
                     {expected} (regenerating shields: {owner_regenerating})",
                    path.display(),
                    record.name,
                    record.hull_id
                ));
            }
        }
    }

    eprintln!(
        "{checked} designs checked for armour: {regenerating} under a Regenerating Shields \
         owner, {foreign} matching the other race's rule (foreign designs learned in \
         battle), {uncached} files skipped as not yet cached; {} disagree",
        failures.len()
    );
    for f in failures.iter().take(10) {
        eprintln!("  {f}");
    }
    assert!(checked >= 10, "expected a meaningful sample, got {checked}");
    assert!(
        regenerating > 0,
        "no Regenerating Shields design in the sample, so the halving is untested"
    );
    assert!(
        foreign > 0,
        "expected some foreign designs, whose owner's traits differ from the file's"
    );
    assert!(
        failures.is_empty(),
        "{} designs disagree on armour",
        failures.len()
    );
}

#[test]
fn battle_token_mass_matches_the_design_it_names() {
    let root = workspace_root();
    let games = root.join("fixtures/games/exodus");
    if !games.is_dir() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    let mut years: Vec<i32> = std::fs::read_dir(&games)
        .expect("readable fixture dir")
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let mut checked = 0;
    let mut exact = 0;
    let mut laden = 0;
    let mut failures = Vec::new();

    for year in years {
        let path = games.join(year.to_string()).join("exodus.m6");
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let blocks = file.segment_blocks(file.latest_segment());
        let Ok(records) = design_records(&file) else {
            continue;
        };

        // The file's own player is the only one whose designs it carries in
        // full, so index by design slot.
        let by_slot: BTreeMap<u8, ShipDesign> = records
            .iter()
            .filter(|r| r.full_design && !r.starbase)
            .map(|r| (r.design_number, to_design(r)))
            .collect();

        for battle in battle_records_in(blocks) {
            for token in battle.tokens.iter().filter(|t| t.player == 5) {
                // Starbases index a separate design table.
                if token.is_starbase() {
                    continue;
                }
                let Some(design) = by_slot.get(&token.design) else {
                    continue;
                };
                let Some(mass) = design.mass() else { continue };
                checked += 1;
                let recorded = i64::from(token.mass);
                let computed = i64::from(mass);
                if recorded == computed {
                    exact += 1;
                } else if recorded > computed {
                    // Cargo adds mass, and a freighter's load varies battle to
                    // battle, so the recorded mass is the design's plus
                    // whatever it was carrying.
                    laden += 1;
                    let capacity = design.cargo_capacity().unwrap_or(0);
                    if recorded - computed > i64::from(capacity) {
                        failures.push(format!(
                            "{year} battle {:#06x}: design slot {} computes {computed}, \
                             recorded {recorded}, excess {} exceeds cargo capacity {capacity}",
                            battle.id,
                            token.design,
                            recorded - computed
                        ));
                    }
                } else {
                    failures.push(format!(
                        "{year} battle {:#06x}: design slot {} computes {computed} but the \
                         battle recorded only {recorded}",
                        battle.id, token.design
                    ));
                }
            }
        }
    }

    eprintln!(
        "{checked} battle tokens checked against their design's mass: \
         {exact} exact, {laden} carrying cargo"
    );
    for f in failures.iter().take(10) {
        eprintln!("  {f}");
    }
    assert!(checked >= 10, "expected a meaningful sample, got {checked}");
    assert!(
        failures.is_empty(),
        "{} battle tokens disagree with their design: {failures:?}",
        failures.len()
    );
    assert!(
        exact > 0,
        "no unladen token matched its design's mass exactly"
    );
}
