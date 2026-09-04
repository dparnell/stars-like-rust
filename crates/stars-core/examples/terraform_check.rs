//! Score the terraforming model against a recorded game.
//!
//! Two checks. A planet's environment can never have moved further from its
//! original values than the owner's technology reaches — that tests
//! [`terraform_reach`] directly. And the AI's recorded auto-terraform order is
//! `min(steps available, 4)`, which tests [`terraform_steps`], the quantity
//! that until now had to be stubbed at zero.
//!
//! The order has to be reconstructed rather than read. A queue entry is a
//! running balance: it counts down as production builds it, and production runs
//! later in the *same* turn the AI queued it, so the saved file already shows
//! the order short by whatever was built. Adding back the clicks the
//! environment moved that year recovers the decision.
//!
//! The first check is scored **per axis, and only on axes the current owner can
//! terraform at all**. `env - env_orig` records what any actor ever did to a
//! planet, not what its present owner did: a previous owner's work survives a
//! change of hands, and hostile action moves the environment too. Both land
//! almost entirely on axes whose owner is immune — an immune race never
//! terraforms, so it never overwrites the marks — so those axes are counted
//! and reported separately rather than folded into the score. See
//! `docs/formulas/terraforming.md`.

use std::collections::{BTreeMap, HashMap};

use stars_core::ai::{AiPersonality, Control};
use stars_core::terraform::{terraform_reach, terraform_steps};
use stars_core::GameState;
use stars_formats::StarsFile;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: terraform_check <dir>");
    let verbose = std::env::args().any(|a| a == "-v");
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();

    let mut with_orig = 0usize;
    let mut within_reach = 0usize;
    let mut overshoot: BTreeMap<i32, usize> = BTreeMap::new();
    let mut immune_axes = 0usize;
    let mut immune_within = 0usize;

    let mut queued = 0usize;
    let mut queue_exact = 0usize;
    let mut queue_diff: BTreeMap<i32, usize> = BTreeMap::new();
    let mut shown = 0usize;
    let mut shown2 = 0usize;
    let mut shown3 = 0usize;
    // A terraform entry persists and counts down as production builds it, so
    // only an order that was not there last turn is a fresh decision — and even
    // a fresh one has already been drawn down by the production that ran later
    // in the same turn. Both need last year's state: what the AI decided from,
    // and the environment the clicks built this turn are measured against.
    struct Prev {
        env: [i8; 3],
        had_terraform: bool,
        predicted: i32,
    }
    let mut prev: HashMap<i16, Prev> = HashMap::new();
    let mut next: HashMap<i16, Prev> = HashMap::new();
    let mut fresh = 0usize;
    let mut fresh_exact = 0usize;
    let mut fresh_diff: BTreeMap<i32, usize> = BTreeMap::new();

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
            let tech = player.research.levels;
            let reach = terraform_reach(&player.race, tech);

            if let Some(orig) = planet.env_orig {
                let worst = (0..3)
                    .filter(|v| !player.race.is_immune(*v))
                    .map(|v| i32::from(planet.env[v] - orig[v]).abs() - i32::from(reach[v]))
                    .max()
                    .unwrap_or(0);
                for v in 0..3 {
                    let ok = i32::from(planet.env[v] - orig[v]).abs() <= i32::from(reach[v]);
                    if player.race.is_immune(v) {
                        immune_axes += 1;
                        immune_within += usize::from(ok);
                    } else {
                        with_orig += 1;
                        within_reach += usize::from(ok);
                    }
                }
                if worst > 0 {
                    *overshoot.entry(worst.min(9)).or_default() += 1;
                    if verbose && shown < 10 {
                        shown += 1;
                        println!(
                            "  {} planet {:>3}: env {:?} orig {:?} reach {reach:?} tech {tech:?}",
                            state.year(),
                            planet.id,
                            planet.env,
                            orig
                        );
                    }
                }
            }

            // The AI's auto-terraform order, against what we say is available.
            let Control::Computer { personality, .. } = player.control else {
                continue;
            };
            // Only the personalities served by FQueueAiTerraforming, which caps
            // at four. Macinti and Cyber have other sources.
            if !matches!(
                personality,
                Some(AiPersonality::TurinDrone) | Some(AiPersonality::Automitron)
            ) {
                continue;
            }
            let recorded: i32 = planet
                .queue
                .iter()
                .filter(|e| !e.ship && e.item == 12)
                .map(|e| e.count)
                .sum();
            if recorded == 0 {
                continue;
            }
            queued += 1;
            let predicted = terraform_steps(planet, &player.race, tech).min(4);
            if let Some(before) = prev.get(&planet.id) {
                if !before.had_terraform {
                    // What the AI actually decided, recovered from the file: the
                    // count still queued plus the clicks production spent during
                    // the same turn, which show up as environment movement.
                    let built: i32 = (0..3)
                        .map(|v| i32::from(planet.env[v] - before.env[v]).abs())
                        .sum();
                    fresh += 1;
                    if before.predicted == recorded + built {
                        fresh_exact += 1;
                    }
                    *fresh_diff
                        .entry((before.predicted - (recorded + built)).clamp(-4, 4))
                        .or_default() += 1;
                    if verbose && before.predicted != recorded + built && shown3 < 14 {
                        shown3 += 1;
                        let band =
                            stars_core::terraform::reachable_band(planet, &player.race, tech);
                        println!(
                            "  F {} p{:>3}: pred {} rec {recorded} built {built} env {:?} \
                             was {:?} orig {:?} ideal {:?} reach {reach:?} band {band:?}",
                            state.year(),
                            planet.id,
                            before.predicted,
                            planet.env,
                            before.env,
                            planet.env_orig,
                            player.race.env_center
                        );
                    }
                }
            }
            if predicted == recorded {
                queue_exact += 1;
            }
            *queue_diff
                .entry((predicted - recorded).clamp(-4, 4))
                .or_default() += 1;
            if verbose && predicted != recorded && shown2 < 10 {
                shown2 += 1;
                println!(
                    "  Q {} planet {:>3}: predicted {predicted} recorded {recorded} \
                     env {:?} orig {:?} ideal {:?} reach {reach:?}",
                    state.year(),
                    planet.id,
                    planet.env,
                    planet.env_orig,
                    player.race.env_center
                );
            }
        }
        // Carry this year's state forward as next year's "before".
        next.clear();
        for planet in &state.planets {
            let predicted = planet
                .owner
                .and_then(|o| state.players.get(o as usize))
                .map_or(0, |pl| {
                    terraform_steps(planet, &pl.race, pl.research.levels).min(4)
                });
            next.insert(
                planet.id,
                Prev {
                    env: planet.env,
                    had_terraform: planet
                        .queue
                        .iter()
                        .any(|e| !e.ship && e.item == 12 && e.count > 0),
                    predicted,
                },
            );
        }
        std::mem::swap(&mut prev, &mut next);
    }

    println!(
        "{with_orig} terraformable axis-readings on planets that record an original environment"
    );
    println!(
        "  movement within the reach we compute: {within_reach} ({}%)",
        pct(within_reach, with_orig)
    );
    println!(
        "  axes the owner is immune to, for reference: {immune_within}/{immune_axes} ({}%) \
         — previous owners and hostile action, which no reach explains",
        pct(immune_within, immune_axes)
    );
    if !overshoot.is_empty() {
        println!("  overshoot by (clicks): {overshoot:?}");
    }

    println!("\n{queued} AI auto-terraform orders from the personalities the routine serves");
    println!(
        "  count matches min(steps, 4): {queue_exact} ({}%)",
        pct(queue_exact, queued)
    );
    println!("  predicted - recorded: {queue_diff:?}");
    println!("\n{fresh} of those are fresh orders (none queued the turn before):");
    println!(
        "  min(steps, 4) == recorded + built that turn: {fresh_exact} ({}%)",
        pct(fresh_exact, fresh)
    );
    println!("  predicted - (recorded + built): {fresh_diff:?}");
}

fn pct(n: usize, total: usize) -> usize {
    n.checked_mul(100)
        .and_then(|x| x.checked_div(total))
        .unwrap_or(0)
}
