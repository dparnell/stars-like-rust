//! Check `QueueAiStarbases`'s claims against a recorded game.
//!
//! `QueueAiStarbases` (`1090:8524`) queues a starbase as design slot
//! `n + 0x10`, one at a time, never for Macinti, and only on a planet whose
//! population is above 79.

use std::collections::BTreeMap;

use stars_core::ai::{AiPersonality, Control};
use stars_core::GameState;
use stars_formats::StarsFile;

/// Queue entries at or above this design slot are starbases
/// (`AddItemToQueue(ishdefSBLatest + 0x10, ...)`).
const STARBASE_SLOT_BASE: u16 = 0x10;

fn main() {
    let dir = std::env::args().nth(1).expect("usage: ai_ships <dir>");
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    let mut sb_entries = 0usize;
    let mut sb_multiple = 0usize;
    let mut sb_count_not_one = 0usize;
    let mut sb_low_pop = 0usize;
    let mut sb_min_pop = i32::MAX;
    let mut by_personality: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut ship_counts: BTreeMap<i32, usize> = BTreeMap::new();
    let mut ship_entries = 0usize;
    let mut sb_design: BTreeMap<Option<u8>, usize> = BTreeMap::new();
    let mut queued_when_has_sb = 0usize;
    let mut upgrade: BTreeMap<i32, usize> = BTreeMap::new();

    for year in &years {
        let Ok(bytes) = std::fs::read(year.join("Game.hst")) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let (state, _) = GameState::from_file(&file);

        for planet in &state.planets {
            let Some(owner) = planet.owner else { continue };
            let Some(player) = state.players.get(owner as usize) else {
                continue;
            };
            let Control::Computer { personality, .. } = player.control else {
                continue;
            };
            let name = personality.map_or("none", AiPersonality::name);

            let starbases: Vec<_> = planet
                .queue
                .iter()
                .filter(|e| e.ship && e.item >= STARBASE_SLOT_BASE)
                .collect();
            if planet.starbase {
                *sb_design.entry(planet.starbase_design).or_default() += 1;
            }
            if !starbases.is_empty() {
                sb_entries += 1;
                if planet.starbase {
                    queued_when_has_sb += 1;
                    let have = i32::from(planet.starbase_design.unwrap_or(0));
                    for e in &starbases {
                        let want = i32::from(e.item - STARBASE_SLOT_BASE);
                        *upgrade.entry((want - have).signum()).or_default() += 1;
                    }
                }
                *by_personality.entry(name).or_default() += 1;
                if starbases.len() > 1 {
                    sb_multiple += 1;
                }
                if starbases.iter().any(|e| e.count != 1) {
                    sb_count_not_one += 1;
                }
                sb_min_pop = sb_min_pop.min(planet.pop);
                if planet.pop <= 79 {
                    sb_low_pop += 1;
                }
            }
            for e in planet.queue.iter().filter(|e| e.ship) {
                ship_entries += 1;
                if e.item < STARBASE_SLOT_BASE {
                    *ship_counts.entry(e.count).or_default() += 1;
                }
            }
        }
    }

    println!("{ship_entries} ship queue entries, {sb_entries} planet-turns with a starbase queued");
    println!("  more than one starbase queued at once: {sb_multiple}");
    println!("  starbase entry with count != 1:        {sb_count_not_one}");
    println!("  population at or below 79:             {sb_low_pop}");
    println!("  lowest population seen:                {sb_min_pop} (gate is > 79)");
    println!("  queued while the planet already had a starbase: {queued_when_has_sb}");
    println!("  of those, queued design vs current (-1 older, 0 same, 1 newer): {upgrade:?}");
    println!("\nstarbase design index on planets that have one: {sb_design:?}");
    println!("\nstarbase entries by personality: {by_personality:?}");
    println!("\ncounts for ordinary ship entries: {ship_counts:?}");
}
