//! Score the terraforming model against a recorded game.
//!
//! Two checks. A planet's environment can never have moved further from its
//! original values than the owner's technology reaches — that tests
//! [`terraform_reach`] directly. And the AI's recorded auto-terraform order is
//! `min(steps available, 4)`, which tests [`terraform_steps`], the quantity
//! that until now had to be stubbed at zero.

use std::collections::BTreeMap;

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

    let mut queued = 0usize;
    let mut queue_exact = 0usize;
    let mut queue_diff: BTreeMap<i32, usize> = BTreeMap::new();
    let mut shown = 0usize;
    let mut shown2 = 0usize;

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
                with_orig += 1;
                let worst = (0..3)
                    .map(|v| i32::from(planet.env[v] - orig[v]).abs() - i32::from(reach[v]))
                    .max()
                    .unwrap_or(0);
                if worst <= 0 {
                    within_reach += 1;
                } else {
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
    }

    println!("{with_orig} planet-turns record an original environment");
    println!(
        "  movement within the reach we compute: {within_reach} ({}%)",
        pct(within_reach, with_orig)
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
}

fn pct(n: usize, total: usize) -> usize {
    n.checked_mul(100)
        .and_then(|x| x.checked_div(total))
        .unwrap_or(0)
}
