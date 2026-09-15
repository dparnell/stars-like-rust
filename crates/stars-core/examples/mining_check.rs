//! Score `minerals_mined` in isolation against a recorded game.
//!
//! A planet's surface minerals change for many reasons — building, cargo
//! transfers, packets, alchemy. To isolate mining, this looks only at
//! planet-year pairs where the planet built nothing and had an empty queue, so
//! the change in surface minerals should be exactly what was mined.
//!
//! The original rolls the leftover hundredths through the RNG, so the recorded
//! figure should land on either the truncated estimate or one above it.

use std::collections::{BTreeMap, HashMap};

use stars_core::mining::minerals_mined;
use stars_core::GameState;
use stars_formats::StarsFile;

struct Before {
    owner: i16,
    mines: i16,
    factories: i16,
    surface: [i32; 3],
    queue_clear: bool,
    predicted: [i32; 3],
    conc: [u8; 3],
    pop: i32,
}

fn main() {
    let dir = std::env::args().nth(1).expect("usage: mining_check <dir>");
    let verbose = std::env::args().any(|a| a == "-v");
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    let mut prev: HashMap<i16, Before> = HashMap::new();
    let mut scored = 0usize;
    let mut exact = 0usize;
    let mut within_roll = 0usize;
    let mut all_three = 0usize;
    let mut errors: BTreeMap<i32, usize> = BTreeMap::new();
    let mut shown = 0usize;

    for year in &years {
        let Ok(bytes) = std::fs::read(year.join("Game.hst")) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let (state, _) = GameState::from_file(&file);

        for planet in &state.planets {
            if let Some(b) = prev.get(&planet.id) {
                let unchanged = planet.owner == Some(b.owner)
                    && planet.mines == b.mines
                    && planet.factories == b.factories
                    && b.queue_clear;
                if unchanged {
                    scored += 1;
                    let mut good = 0;
                    for i in 0..3 {
                        let actual = planet.surface_min[i] - b.surface[i];
                        let want = b.predicted[i];
                        let err = actual - want;
                        *errors.entry(err.clamp(-5, 5)).or_default() += 1;
                        if err == 0 {
                            exact += 1;
                        }
                        if err == 0 || err == 1 {
                            within_roll += 1;
                            good += 1;
                        }
                    }
                    if good == 3 {
                        all_three += 1;
                    } else if verbose && shown < 12 {
                        shown += 1;
                        let actual: Vec<i32> = (0..3)
                            .map(|i| planet.surface_min[i] - b.surface[i])
                            .collect();
                        println!(
                            "  {} planet {:>3}: predicted {:?} actual {:?} \
                             (mines {} conc {:?} pop {})",
                            state.year(),
                            planet.id,
                            b.predicted,
                            actual,
                            b.mines,
                            b.conc,
                            b.pop
                        );
                    }
                }
            }
        }

        prev.clear();
        for planet in &state.planets {
            let Some(owner) = planet.owner else { continue };
            let Some(player) = state.players.get(owner as usize) else {
                continue;
            };
            prev.insert(
                planet.id,
                Before {
                    owner,
                    mines: planet.mines,
                    factories: planet.factories,
                    surface: planet.surface_min,
                    queue_clear: planet.queue.is_empty(),
                    predicted: minerals_mined(planet, &player.race, None, None),
                    conc: planet.min_conc,
                    pop: planet.pop,
                },
            );
        }
    }

    let total = scored * 3;
    println!("{scored} quiet planet-year pairs ({total} mineral readings)");
    println!("  exact (no roll):      {exact} ({}%)", pct(exact, total));
    println!(
        "  within the RNG roll:  {within_roll} ({}%)",
        pct(within_roll, total)
    );
    println!(
        "  all three minerals:   {all_three} ({}%)",
        pct(all_three, scored)
    );
    println!("\nerror distribution (actual - predicted, clamped to +/-5):");
    for (err, n) in &errors {
        println!("  {err:>3}: {n}");
    }
}

fn pct(n: usize, total: usize) -> usize {
    n.checked_mul(100)
        .and_then(|x| x.checked_div(total))
        .unwrap_or(0)
}
