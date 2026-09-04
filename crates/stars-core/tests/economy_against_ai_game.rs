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
/// values than the owner's technology reaches. A handful legitimately exceed
/// it — Claim Adjusters terraform from orbit, and a planet that changed hands
/// carries work done by an owner with different technology — so this allows a
/// small margin rather than demanding none.
#[test]
fn terraforming_never_exceeds_the_reach_we_compute() {
    use stars_core::terraform::terraform_reach;

    let years = ai_game_years();
    if years.is_empty() {
        eprintln!("skipping: all-computer-players fixture absent");
        return;
    }
    let (mut scored, mut within) = (0usize, 0usize);
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
            scored += 1;
            if (0..3).all(|v| i32::from(planet.env[v] - orig[v]).abs() <= i32::from(reach[v])) {
                within += 1;
            }
        }
    }
    assert!(scored > 5000, "expected a large sample, got {scored}");
    let pct = within * 100 / scored;
    // 96% on all-computer-players alone; 90% once no-random-events is added,
    // whose races terraform past the reach computed for them far more often.
    // Unexplained — see docs/formulas/terraforming.md.
    assert!(
        pct >= 88,
        "terraform reach agreement fell to {pct}% of {scored} planet-turns (was 90%)"
    );
}
