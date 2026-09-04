//! Score the AI production transcription against a recorded game.
//!
//! `FFillProdMinesAndFactories` queues auto-build mines and factories, and the
//! next turn's production step builds them and empties the queue. The recorded
//! queue is therefore almost always empty even though the AI is building
//! constantly — across `all-computer-players` there are 7684 planet-year pairs
//! where mines or factories grew while the queue was empty, against 80 where a
//! queue entry was visible. The observable to score against is the **change**
//! in a planet's mine and factory counts, not the queue.
//!
//! ```text
//! cargo run --release -p stars-core --example ai_production -- \
//!     fixtures/games/all-computer-players
//! ```

use std::collections::HashMap;

use stars_core::ai::production::{fill_prod_mines_and_factories, Context};
use stars_core::ai::Control;
use stars_core::rng::Rng;
use stars_core::GameState;
use stars_formats::StarsFile;

/// What we predicted for a planet in one year, kept until the next year shows
/// what actually happened.
struct Predicted {
    mines: i32,
    factories: i32,
    from: (i16, i16),
    /// Whether anything else was in the queue competing for the same
    /// resources when the decision was made.
    queue_was_clear: bool,
    /// Whether the planet could build ships at all. A planet with a starbase
    /// competes with the AI's shipbuilding for the same resources, and those
    /// orders are gone from the queue by the time the turn is recorded.
    starbase: bool,
}

fn main() {
    let dir = std::env::args().nth(1).expect("usage: ai_production <dir>");
    let verbose = std::env::args().any(|a| a == "-v");

    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    let mut pending: HashMap<i16, Predicted> = HashMap::new();
    let mut scored = 0usize;
    let mut both_exact = 0usize;
    let mut factories_exact = 0usize;
    let mut mines_exact = 0usize;
    let mut within_one = 0usize;
    let mut shown = 0usize;

    // Chance control: score each prediction against a *different* planet's
    // recorded outcome from the same year. A rule no better than this is not a
    // rule.
    let mut rng = Rng::randomize(12345);
    let mut chance_both = 0usize;
    let mut chance_total = 0usize;
    // The other control that matters: predicting nothing at all. Most planets
    // build nothing on most turns, so a rule has to beat this to be worth
    // anything.
    let mut zero_both = 0usize;
    // Precision and recall on "did this planet build anything at all".
    let mut built = 0usize;
    let mut predicted_some = 0usize;
    let mut true_positive = 0usize;
    let mut built_exact = 0usize;
    let mut clear_built = 0usize;
    let mut clear_exact = 0usize;
    let (mut nb_scored, mut nb_exact) = (0usize, 0usize);
    let (mut nb_built, mut nb_built_exact) = (0usize, 0usize);
    // The clean subset: no starbase (so nothing else competes for resources)
    // and an empty queue (so nothing was carried over). On these the AI's
    // decision and the year's building are the same quantity.
    let (mut clean_scored, mut clean_exact) = (0usize, 0usize);
    let (mut clean_built, mut clean_built_exact) = (0usize, 0usize);
    let (mut clean_nothing, mut clean_chance, mut clean_chance_total) = (0usize, 0usize, 0usize);
    let mut ferr: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
    let mut merr: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
    let mut cferr: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
    let mut cmerr: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();

    for year in &years {
        let Ok(bytes) = std::fs::read(year.join("Game.hst")) else {
            continue;
        };
        let Ok(file) = StarsFile::decode(&bytes) else {
            continue;
        };
        let (state, _) = GameState::from_file(&file);
        let turn = state.year() - 2400;

        // Score last year's predictions against this year's counts.
        let mut deltas: Vec<(i32, i32)> = Vec::new();
        let mut clean_deltas: Vec<(i32, i32)> = Vec::new();
        let mut clean_preds: Vec<(i32, i32)> = Vec::new();
        for planet in &state.planets {
            let Some(p) = pending.get(&planet.id) else {
                continue;
            };
            let d_mines = i32::from(planet.mines - p.from.0);
            let d_factories = i32::from(planet.factories - p.from.1);
            // A planet that lost buildings changed hands or was bombed; that is
            // not a production outcome.
            if d_mines < 0 || d_factories < 0 {
                continue;
            }
            scored += 1;
            deltas.push((d_mines, d_factories));

            let m = p.mines == d_mines;
            let f = p.factories == d_factories;
            if m {
                mines_exact += 1;
            }
            if f {
                factories_exact += 1;
            }
            if m && f {
                both_exact += 1;
            }
            if d_mines == 0 && d_factories == 0 {
                zero_both += 1;
            } else {
                built += 1;
                if m && f {
                    built_exact += 1;
                }
                if p.queue_was_clear {
                    clear_built += 1;
                    if m && f {
                        clear_exact += 1;
                    }
                }
            }
            *ferr
                .entry((p.factories - d_factories).clamp(-9, 9))
                .or_default() += 1;
            *merr.entry((p.mines - d_mines).clamp(-9, 9)).or_default() += 1;
            if !p.starbase && p.queue_was_clear {
                clean_deltas.push((d_mines, d_factories));
                clean_preds.push((p.mines, p.factories));
                clean_scored += 1;
                *cferr
                    .entry((p.factories - d_factories).clamp(-9, 9))
                    .or_default() += 1;
                *cmerr.entry((p.mines - d_mines).clamp(-9, 9)).or_default() += 1;
                if d_mines == 0 && d_factories == 0 {
                    clean_nothing += 1;
                }
                if m && f {
                    clean_exact += 1;
                }
                if d_mines != 0 || d_factories != 0 {
                    clean_built += 1;
                    if m && f {
                        clean_built_exact += 1;
                    }
                }
            }
            if !p.starbase {
                nb_scored += 1;
                if m && f {
                    nb_exact += 1;
                }
                if d_mines + d_factories > 0 {
                    nb_built += 1;
                    if m && f {
                        nb_built_exact += 1;
                    }
                }
            }
            let says_some = p.mines + p.factories > 0;
            let did_some = d_mines + d_factories > 0;
            if says_some {
                predicted_some += 1;
                if did_some {
                    true_positive += 1;
                }
            }
            if (p.mines - d_mines).abs() <= 1 && (p.factories - d_factories).abs() <= 1 {
                within_one += 1;
            } else if verbose && shown < 15 {
                shown += 1;
                println!(
                    "  {} planet {:>3}: predicted {}m {}f, actual {d_mines}m {d_factories}f \
                     (had {}m {}f)",
                    state.year(),
                    planet.id,
                    p.mines,
                    p.factories,
                    p.from.0,
                    p.from.1
                );
            }
        }

        // The same predictions against someone else's outcome.
        for planet in &state.planets {
            if deltas.is_empty() {
                break;
            }
            let Some(p) = pending.get(&planet.id) else {
                continue;
            };
            let i = (rng.next_raw().unsigned_abs() as usize) % deltas.len();
            let (dm, df) = deltas[i];
            chance_total += 1;
            if p.mines == dm && p.factories == df {
                chance_both += 1;
            }
        }

        // The same control, drawn from and applied to the clean subset only, so
        // the 72% below is compared against a like-for-like baseline.
        for (mines, factories) in &clean_preds {
            if clean_deltas.is_empty() {
                break;
            }
            let i = (rng.next_raw().unsigned_abs() as usize) % clean_deltas.len();
            let (dm, df) = clean_deltas[i];
            clean_chance_total += 1;
            if *mines == dm && *factories == df {
                clean_chance += 1;
            }
        }

        // Predict for this year, to be scored against the next.
        pending.clear();
        for planet in &state.planets {
            let Some(owner) = planet.owner else { continue };
            let Some(player) = state.players.get(owner as usize) else {
                continue;
            };
            let Control::Computer { personality, .. } = player.control else {
                continue;
            };
            let mut tech = [0u8; 6];
            for (slot, level) in tech.iter_mut().zip(player.research.levels.iter()) {
                *slot = *level;
            }
            let ctx = Context {
                personality,
                research_pct: player.research_pct,
                tech,
                turn,
                terraform_steps: 0,
                factories_cost_all_minerals: false,
            };
            let d = fill_prod_mines_and_factories(planet, &player.race, &ctx);
            pending.insert(
                planet.id,
                Predicted {
                    mines: d.mines,
                    factories: d.factories,
                    from: (planet.mines, planet.factories),
                    queue_was_clear: planet.queue.is_empty(),
                    starbase: planet.starbase,
                },
            );
        }
    }

    println!("{scored} AI planet-year pairs scored");
    println!(
        "  mines exact:      {mines_exact} ({}%)",
        pct(mines_exact, scored)
    );
    println!(
        "  factories exact:  {factories_exact} ({}%)",
        pct(factories_exact, scored)
    );
    println!(
        "  both exact:       {both_exact} ({}%)",
        pct(both_exact, scored)
    );
    println!(
        "  both within 1:    {within_one} ({}%)",
        pct(within_one, scored)
    );
    println!(
        "  chance control:   {chance_both} of {chance_total} ({}%) — same predictions, \
         another planet's outcome",
        pct(chance_both, chance_total)
    );
    println!(
        "  predict-nothing:  {zero_both} ({}%) — how often the planet built nothing",
        pct(zero_both, scored)
    );
    println!("\non the {built} pairs where the planet actually built something:");
    println!(
        "  exact:          {built_exact} ({}%)",
        pct(built_exact, built)
    );
    println!(
        "  recall:         {true_positive} of {built} ({}%) — we said it would build",
        pct(true_positive, built)
    );
    println!(
        "  precision:      {true_positive} of {predicted_some} ({}%) — we said build, it did",
        pct(true_positive, predicted_some)
    );
    println!(
        "  exact, and nothing else was queued: {clear_exact} of {clear_built} ({}%)",
        pct(clear_exact, clear_built)
    );
    println!(
        "\non the {clean_scored} pairs with no starbase AND an empty queue — the only \
         pairs where the AI's decision and the year's building are the same quantity:"
    );
    println!(
        "  both exact:     {clean_exact} ({}%)",
        pct(clean_exact, clean_scored)
    );
    println!(
        "  exact where it built: {clean_built_exact} of {clean_built} ({}%)",
        pct(clean_built_exact, clean_built)
    );
    println!(
        "  chance control:  {clean_chance} of {clean_chance_total} ({}%) — same \
         predictions, another clean planet's outcome",
        pct(clean_chance, clean_chance_total)
    );
    println!(
        "  predict-nothing: {clean_nothing} ({}%)",
        pct(clean_nothing, clean_scored)
    );
    println!("  clean factory error: {cferr:?}");
    println!("  clean mine error:    {cmerr:?}");
    println!(
        "\non the {nb_scored} pairs at planets with no starbase (no shipbuilding \
         competing for resources):"
    );
    println!(
        "  both exact:     {nb_exact} ({}%)",
        pct(nb_exact, nb_scored)
    );
    println!(
        "  exact where it built: {nb_built_exact} of {nb_built} ({}%)",
        pct(nb_built_exact, nb_built)
    );
    println!("\nfactory error (predicted - actual): {ferr:?}");
    println!("mine error    (predicted - actual): {merr:?}");
}

fn pct(n: usize, total: usize) -> usize {
    n.checked_mul(100)
        .and_then(|x| x.checked_div(total))
        .unwrap_or(0)
}
