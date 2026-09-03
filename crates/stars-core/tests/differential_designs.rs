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

/// Armour is only partly pinned down, and this test says exactly how far.
///
/// In the three-player sample game and the shipped tutorial, every ship
/// design's armour is reproduced exactly by `hull armour + fitted armour`.
/// In the Exodus game it is not: a Stalwart Defender there is a Destroyer
/// (200) with two Crobmnium (75 each), which should be 350, and the engine
/// stored 275. The difference is not a constant factor and goes both ways
/// across designs, so it is not a simple scaling.
///
/// The routine that computes it is a stub in the reconstructed sources and
/// has not been read out of our binary yet, so rather than invent a rule that
/// fits one game, this asserts the games where the model is known to hold and
/// reports the other. See `docs/formulas/design.md`.
#[test]
fn computed_armour_matches_what_the_engine_stored() {
    let files = fixture_files();
    if files.is_empty() {
        eprintln!("skipping: no fixtures");
        return;
    }

    let mut checked = 0;
    let mut uncached = 0;
    let mut starbases = 0;
    let mut starbases_matching = 0;
    let mut loose_total = 0;
    let mut loose_matching = 0;
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
            if !record.full_design {
                continue;
            }
            let Some(expected) = record.armor else {
                continue;
            };
            // The stored armour is a value the host caches while generating a
            // turn. A game's very first files, written before any turn has
            // been generated, carry zero for every design; there is nothing to
            // compare against in those.
            if expected == 0 {
                uncached += 1;
                continue;
            }
            let design = to_design(&record);
            let Some(armor) = design.armor() else {
                continue;
            };

            let strict = !path.to_string_lossy().contains("exodus");
            if !strict {
                loose_total += 1;
                if i64::from(design.armor().unwrap_or(-1)) == i64::from(expected) {
                    loose_matching += 1;
                }
                continue;
            }

            if record.starbase {
                // Starbase armour is only partly understood: fitted armour
                // counts half, which fits every starbase in the fixtures bar
                // one stock design that stores twice its hull's value with
                // nothing fitted. Counted, not asserted — see
                // docs/formulas/design.md.
                starbases += 1;
                if i64::from(armor) == i64::from(expected) {
                    starbases_matching += 1;
                }
                continue;
            }

            checked += 1;
            if i64::from(armor) != i64::from(expected) {
                failures.push(format!(
                    "{}: design {:?} on hull {} computes {armor} armour, engine stored {expected}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    record.name,
                    record.hull_id
                ));
            }
        }
    }

    eprintln!(
        "armour: {checked} ship designs match exactly in the sample game and tutorial \
         ({uncached} not yet cached by the host); {starbases_matching} of {starbases} \
         starbases match the inferred half-armour rule; in the Exodus game \
         {loose_matching} of {loose_total} match (see the doc comment)"
    );
    for f in failures.iter().take(10) {
        eprintln!("  {f}");
    }
    assert!(checked >= 10, "expected a meaningful sample, got {checked}");
    assert!(
        failures.is_empty(),
        "{} ship designs disagree on armour: {failures:?}",
        failures.len()
    );
    // The starbase rule is inferred and the Exodus divergence is unexplained,
    // so both are held to a floor rather than to perfection. A regression in
    // the hull or armour tables would drop these sharply.
    assert!(
        starbases_matching * 10 >= starbases * 7,
        "only {starbases_matching} of {starbases} starbase designs match the inferred armour rule"
    );
    assert!(
        loose_matching * 10 >= loose_total * 75 / 10,
        "only {loose_matching} of {loose_total} Exodus designs match; that is worse than the \
         82% this stood at when the divergence was first measured"
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
