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

fn game_years() -> Vec<PathBuf> {
    let dir = workspace_root().join("fixtures/games/all-computer-players");
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut years: Vec<_> = std::fs::read_dir(&dir)
        .expect("game directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    years.sort();
    years
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
    let years = game_years();
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
    let years = game_years();
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
