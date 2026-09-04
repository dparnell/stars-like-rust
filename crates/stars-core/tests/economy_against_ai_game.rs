//! The planetary economy, checked against the sixteen-player AI game.
//!
//! `fixtures/games/all-computer-players` is 101 turns with sixteen players
//! covering six primary traits, which is a far wider spread of races than the
//! single-player Exodus save the other differential tests use. These two tests
//! isolate the two inputs the AI's production decision reads: what a planet
//! mines, and what it earns.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use stars_core::mining::minerals_mined;
use stars_core::resources::resources_at_planet;
use stars_core::GameState;
use stars_formats::StarsFile;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

/// Every year directory across both sixteen-player AI games.
///
/// `all-computer-players` and `no-random-events` are the same shape — 101 turns
/// with sixteen computer players — and differ in that the second was created
/// with random events off, so it contains no Mystery Traders at all (85 in the
/// first, none in the second). Anything scored over AI behaviour should use
/// both.
fn ai_game_years() -> Vec<PathBuf> {
    let root = workspace_root();
    let mut out = Vec::new();
    for name in ["all-computer-players", "no-random-events"] {
        let dir = root.join("fixtures/games").join(name);
        if !dir.is_dir() {
            continue;
        }
        let mut years: Vec<_> = std::fs::read_dir(&dir)
            .expect("game directory")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        years.sort();
        out.extend(years);
    }
    out
}

fn load(path: &Path) -> Option<GameState> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    Some(GameState::from_file(&file).0)
}

/// What a planet mines, isolated from everything else that moves surface
/// minerals.
///
/// A planet that built nothing and had an empty queue should gain exactly what
/// it mined. The original rolls the leftover hundredths through the RNG, so the
/// recorded figure lands on the truncated estimate or one above it.
///
/// Readings outside that band by more than 4 are excluded: those are the other
/// things that move surface minerals — cargo transfers, mineral packets,
/// alchemy — which this test cannot see and does not model.
#[test]
fn mining_matches_on_planets_that_built_nothing() {
    let years = ai_game_years();
    if years.is_empty() {
        eprintln!("skipping: all-computer-players fixture absent");
        return;
    }

    struct Before {
        owner: i16,
        mines: i16,
        factories: i16,
        surface: [i32; 3],
        queue_clear: bool,
        predicted: [i32; 3],
    }

    let mut prev: HashMap<i16, Before> = HashMap::new();
    let (mut attributable, mut within_roll) = (0usize, 0usize);

    for year in &years {
        let Some(state) = load(&year.join("Game.hst")) else {
            continue;
        };
        for planet in &state.planets {
            if let Some(b) = prev.get(&planet.id) {
                let quiet = planet.owner == Some(b.owner)
                    && planet.mines == b.mines
                    && planet.factories == b.factories
                    && b.queue_clear;
                if quiet {
                    for i in 0..3 {
                        let err = (planet.surface_min[i] - b.surface[i]) - b.predicted[i];
                        if !(-4..=4).contains(&err) {
                            continue; // cargo, a packet, or alchemy
                        }
                        attributable += 1;
                        if err == 0 || err == 1 {
                            within_roll += 1;
                        }
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
                },
            );
        }
    }

    assert!(
        attributable > 10_000,
        "expected a large sample, got {attributable}"
    );
    let pct = within_roll * 100 / attributable;
    assert!(
        pct >= 97,
        "mining agreement fell to {pct}% of {attributable} readings (was 98%)"
    );
}

/// A player's resources are never less than the research they fed.
///
/// `PLAYER.lResLastYear` is not `resources * research_pct` — resources the
/// production queue did not consume also fall through to research, so a player
/// with `research_pct = 0` still shows a non-zero figure. It cannot measure
/// resource output directly, but it is a hard lower bound: a player-year where
/// the modelled output is below the recorded research is a definite
/// under-estimate.
///
/// This is what caught Alternate Reality planets producing nothing at all.
#[test]
fn modelled_resources_cover_the_research_that_came_from_them() {
    let years = ai_game_years();
    if years.is_empty() {
        eprintln!("skipping: all-computer-players fixture absent");
        return;
    }

    let (mut scored, mut covered) = (0usize, 0usize);
    let mut ar_scored = 0usize;

    for year in &years {
        let Some(state) = load(&year.join("Game.hst")) else {
            continue;
        };
        for (i, player) in state.players.iter().enumerate() {
            if player.research_last_year <= 0 {
                continue;
            }
            let owner = i16::try_from(i).unwrap_or(-1);
            let energy = i16::from(player.research.levels[0]);
            let total: i64 = state
                .planets
                .iter()
                .filter(|p| p.owner == Some(owner))
                .filter_map(|p| resources_at_planet(p, &player.race, energy))
                .map(i64::from)
                .sum();

            scored += 1;
            if player.race.is_ar() {
                ar_scored += 1;
            }
            if total >= i64::from(player.research_last_year) {
                covered += 1;
            }
        }
    }

    assert!(scored > 1000, "expected a large sample, got {scored}");
    assert!(
        ar_scored > 100,
        "expected the Alternate Reality players to be scored, got {ar_scored}"
    );
    let pct = covered * 100 / scored;
    assert!(
        pct >= 97,
        "only {pct}% of {scored} player-years produce enough resources to cover \
         the research recorded against them (was 99%)"
    );
}

/// The terraforming reach model, against every planet that records an original
/// environment.
///
/// A planet's environment can never have been moved further from its original
/// values than the owner's technology reaches — provided the owner is the one
/// who moved it. See the body for the two mechanisms that break that proviso
/// and how they are separated out.
#[test]
fn terraforming_never_exceeds_the_reach_we_compute() {
    use stars_core::terraform::terraform_reach;

    let years = ai_game_years();
    if years.is_empty() {
        eprintln!("skipping: all-computer-players fixture absent");
        return;
    }
    // Scored per axis, not per planet, and only on axes the planet's *current*
    // owner can actually terraform.
    //
    // `env - env_orig` is a historical record of what any actor ever did to a
    // planet, not a record of what its present owner did. Two mechanisms write
    // to it that no reach can account for:
    //
    // * **A previous owner.** Planet 260 of `all-computer-players` is the clean
    //   example. Player 7 (an all-immune HE race) holds it untouched from 2407;
    //   loses it in 2428; player 14, centred on 50/50/50, terraforms it from
    //   [47, 33, 49] to [50, 44, 50] between 2429 and 2441; player 7 retakes it
    //   in 2454 and keeps the offset forever.
    // * **Hostile action.** Planet 25 loses two clicks of temperature between
    //   2447 and 2448 and one of radiation between 2457 and 2458, each in the
    //   same year its population drops sharply, while an owner immune to all
    //   three axes holds it throughout.
    //
    // Both land overwhelmingly on axes their owner is immune to, because an
    // immune race never terraforms and so never overwrites the marks. Scoring
    // those axes measures the history of the galaxy rather than the formula, so
    // they are counted separately and reported.
    let (mut axes, mut within) = (0usize, 0usize);
    let (mut immune_axes, mut immune_within) = (0usize, 0usize);
    for year in &years {
        let Some(state) = load(&year.join("Game.hst")) else {
            continue;
        };
        for planet in &state.planets {
            let Some(owner) = planet.owner else { continue };
            let Some(player) = state.players.get(owner as usize) else {
                continue;
            };
            let Some(orig) = planet.env_orig else {
                continue;
            };
            let reach = terraform_reach(&player.race, player.research.levels);
            for v in 0..3 {
                let ok = i32::from(planet.env[v] - orig[v]).abs() <= i32::from(reach[v]);
                if player.race.is_immune(v) {
                    immune_axes += 1;
                    immune_within += usize::from(ok);
                } else {
                    axes += 1;
                    within += usize::from(ok);
                }
            }
        }
    }
    assert!(axes > 20_000, "expected a large sample, got {axes}");
    // 37712 of 37743 across both games: 99.9%. The 31 stragglers overshoot by
    // one to three clicks and are the same two mechanisms leaking onto an axis
    // the current owner happens not to be immune to.
    let per_mille = within * 1000 / axes;
    assert!(
        per_mille >= 995,
        "terraform reach agreement fell to {per_mille} per mille of {axes} axis-readings"
    );
    eprintln!(
        "terraform reach: {within}/{axes} terraformable axes within reach; \
         immune axes, for reference, {immune_within}/{immune_axes}"
    );
}

/// The AI's auto-terraform order, reconstructed from the file.
///
/// `FQueueAiTerraforming` queues `min(steps available, 4)`, where the count
/// comes from the production catalogue that `InitProduction` fills with
/// `IpctCanTerraformLppl`. Two things stand between that decision and what a
/// saved game shows, and both are the same rule — **a queue entry is a running
/// balance, not a record of what was chosen**:
///
/// * it counts down over following turns as production builds it, so only a
///   planet that carried no terraform order the turn before is a fresh
///   decision;
/// * `Produce` runs later in the *same* turn, so even a fresh order is already
///   short by what the planet built that year. Those clicks show up as
///   environment movement.
///
/// So the decision is `recorded + |env(Y) - env(Y-1)|`, and it matches
/// `min(terraform_steps, 4)` on every fresh order in both games.
#[test]
fn the_ai_queues_every_terraform_step_available_up_to_four() {
    use stars_core::ai::{AiPersonality, Control};
    use stars_core::terraform::terraform_steps;

    let years = ai_game_years();
    if years.is_empty() {
        eprintln!("skipping: all-computer-players fixture absent");
        return;
    }

    struct Before {
        env: [i8; 3],
        had_terraform: bool,
        predicted: i32,
    }

    let mut prev: HashMap<i16, Before> = HashMap::new();
    let (mut fresh, mut exact) = (0usize, 0usize);

    for year in &years {
        let Some(state) = load(&year.join("Game.hst")) else {
            continue;
        };
        for planet in &state.planets {
            let Some(owner) = planet.owner else { continue };
            let Some(player) = state.players.get(owner as usize) else {
                continue;
            };
            // Only the personalities FQueueAiTerraforming serves as their sole
            // source; Macinti and Cyber queue terraforming from elsewhere too.
            let Control::Computer { personality, .. } = player.control else {
                continue;
            };
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
            let Some(before) = prev.get(&planet.id) else {
                continue;
            };
            if before.had_terraform {
                continue;
            }
            let built: i32 = (0..3)
                .map(|v| i32::from(planet.env[v] - before.env[v]).abs())
                .sum();
            fresh += 1;
            if before.predicted == recorded + built {
                exact += 1;
            }
        }

        prev = state
            .planets
            .iter()
            .map(|planet| {
                let predicted = planet
                    .owner
                    .and_then(|o| state.players.get(o as usize))
                    .map_or(0, |pl| {
                        terraform_steps(planet, &pl.race, pl.research.levels).min(4)
                    });
                (
                    planet.id,
                    Before {
                        env: planet.env,
                        had_terraform: planet
                            .queue
                            .iter()
                            .any(|e| !e.ship && e.item == 12 && e.count > 0),
                        predicted,
                    },
                )
            })
            .collect();
    }

    assert!(fresh > 150, "expected a decent sample, got {fresh}");
    assert_eq!(
        exact, fresh,
        "{exact} of {fresh} fresh auto-terraform orders match min(steps, 4); \
         this was 100% when recovered"
    );
}
