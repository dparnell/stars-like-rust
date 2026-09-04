//! Recover how many torpedoes hit in each recorded volley, and check the
//! accuracy formula against them.
//!
//! A volley writes two kill records. The misses splash the shields for
//! `cTorpMiss * dp / 8` and are marked `bitFTorp | 0xC0`; the hits do
//! `cTorpFire * dp / 2` to the shields and the same again to the armour, and
//! are marked `bitFTorp`. Where the shields absorbed the whole of a record, its
//! `dpShield` is that figure exactly — so both counts can be divided back out,
//! which recovers the roll's outcome without needing the generator.
use stars_core::battle::torpedo_accuracy;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_formats::{
    battle_records_in_with, design_records, ActionLayout, DesignRecord, StarsFile,
};
use std::collections::BTreeMap;

/// `bitFTorp`.
const TORP: u8 = 0x04;
/// The `0xC0` `FDamageTok` adds to a torpedo record that stopped at shields.
const SHIELDS_ONLY: u8 = 0xC0;

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let mut years: Vec<i32> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let (mut volleys, mut usable) = (0usize, 0usize);
    let (mut fired_total, mut hit_total) = (0i64, 0i64);
    let mut expected_total = 0f64;
    let mut by_accuracy: BTreeMap<i32, (i64, i64)> = BTreeMap::new();

    for year in &years {
        let path = std::path::Path::new(&dir)
            .join(year.to_string())
            .join("exodus.m6");
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let blocks = file.segment_blocks(file.latest_segment());
        let to_design = |r: &DesignRecord| ShipDesign {
            name: String::new(),
            picture: 0,
            stored_armor: 0,
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
        };
        let Ok(records) = design_records(&file) else {
            continue;
        };
        let designs: BTreeMap<u8, ShipDesign> = records
            .iter()
            .filter(|r| r.full_design && !r.starbase)
            .map(|r| (r.design_number, to_design(r)))
            .collect();
        let header = &file.latest_segment().header;
        let layout = ActionLayout::for_version(header.version_major, header.version_minor);

        for battle in battle_records_in_with(blocks, layout) {
            for action in &battle.actions {
                let torp_kills: Vec<_> = action
                    .kills
                    .iter()
                    .filter(|k| k.weapon & TORP != 0)
                    .collect();
                if torp_kills.is_empty() {
                    continue;
                }
                volleys += 1;

                // The firing token, and the torpedo it fired.
                let Some(attacker) = battle.tokens.get(usize::from(action.token)) else {
                    continue;
                };
                let Some(design) = designs.get(&attacker.design) else {
                    continue;
                };
                let Some(torp) = design.weapons().into_iter().find(|w| w.torpedo) else {
                    continue;
                };
                if torp.dp <= 0 {
                    continue;
                }
                let launchers = design
                    .weapons()
                    .iter()
                    .filter(|w| w.torpedo)
                    .map(|w| w.count)
                    .sum::<i32>();
                let available = launchers * i32::from(attacker.ships);

                // Divide the two records back out.
                let mut misses = 0i64;
                let mut hits = 0i64;
                let mut clean = true;
                for kill in &torp_kills {
                    let dp_shield = i64::from(kill.shield_damage);
                    if kill.weapon & SHIELDS_ONLY == SHIELDS_ONLY {
                        // Misses: dpCol = cTorpMiss * dp / 8.
                        if dp_shield * 8 % i64::from(torp.dp) != 0 {
                            clean = false;
                        }
                        misses += dp_shield * 8 / i64::from(torp.dp);
                    } else {
                        // Hits: dpT = cTorpFire * dp / 2, but only if the
                        // shields took all of it; otherwise the record is
                        // capped at the pool and says nothing.
                        if dp_shield == 0 {
                            clean = false;
                        }
                        hits += dp_shield * 2 / i64::from(torp.dp);
                    }
                }
                if !clean || hits + misses == 0 {
                    continue;
                }
                // The recovered counts must not exceed what the token carries.
                if hits + misses > i64::from(available) {
                    continue;
                }
                usable += 1;
                fired_total += hits + misses;
                hit_total += hits;

                let accuracy = torpedo_accuracy(
                    torp.accuracy,
                    i32::from(attacker.pct_jam),
                    i32::from(attacker.pct_computer),
                );
                expected_total += (hits + misses) as f64 * f64::from(accuracy) / 100.0;
                let e = by_accuracy.entry(accuracy).or_insert((0, 0));
                e.0 += hits + misses;
                e.1 += hits;
            }
        }
    }

    println!("{volleys} recorded torpedo volleys, {usable} with both counts recoverable");
    println!("  torpedoes fired: {fired_total}, hits recovered: {hit_total}");
    if fired_total > 0 {
        println!(
            "  observed hit rate {:.1}%, accuracy formula predicts {:.1}%",
            hit_total as f64 * 100.0 / fired_total as f64,
            expected_total * 100.0 / fired_total as f64
        );
    }
    println!("  by predicted accuracy — accuracy: (fired, hit):");
    for (acc, (fired, hit)) in &by_accuracy {
        println!(
            "    {acc}% -> {hit} of {fired} ({:.0}%)",
            *hit as f64 * 100.0 / *fired as f64
        );
    }
}
