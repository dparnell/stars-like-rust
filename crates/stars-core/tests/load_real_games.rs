//! Loading real save files into a simulation state.
//!
//! This exercises the bridge between the format layer and the engine on every
//! sample game, and then runs a turn on the result — the closest thing to
//! "play the real game" the crate can currently do.
//!
//! Fixtures are optional; each test skips when they are absent.

use std::path::{Path, PathBuf};

use stars_core::rng::Rng;
use stars_core::{generate_turn, GameState};
use stars_formats::StarsFile;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

fn sample_files() -> Vec<PathBuf> {
    let root = workspace_root();
    let mut out = Vec::new();
    for rel in [
        "fixtures/incoming/turn0/Game.hst",
        "fixtures/incoming/turn1/Game.hst",
        "fixtures/incoming/turn1/Game.m1",
        "fixtures/incoming/turn1/Game.m2",
        "fixtures/incoming/turn1/Game.m3",
        "fixtures/games/tutorial/tutorial.m1",
        "fixtures/games/exodus/2424/exodus.m6",
        "fixtures/games/exodus/2450/exodus.m6",
    ] {
        let p = root.join(rel);
        if p.is_file() {
            out.push(p);
        }
    }
    out
}

#[test]
fn every_sample_game_loads() {
    let files = sample_files();
    if files.is_empty() {
        eprintln!("skipping: no fixtures");
        return;
    }

    for path in &files {
        let bytes = std::fs::read(path).expect("fixture readable");
        let file = StarsFile::decode(&bytes).expect("fixture decodes");
        let (state, report) = GameState::from_file(&file);
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        eprintln!(
            "{name}: year {}, {} players, {} planets ({} too sparse), {} designs",
            state.year(),
            report.players_loaded,
            report.planets_loaded,
            report.planets_partial,
            report.designs_loaded
        );

        assert!(report.players_loaded > 0, "{name}: no player loaded");
        assert!(report.planets_loaded > 0, "{name}: no planet loaded");
        assert!(
            state.year() >= 2400 && state.year() < 2600,
            "{name}: implausible year {}",
            state.year()
        );

        // Every loaded planet must be internally consistent.
        for planet in &state.planets {
            assert!(planet.owner.is_some(), "{name}: unowned planet loaded");
            assert!(planet.pop > 0, "{name}: planet with no population loaded");
            assert!(
                state.owner_race(planet).is_some(),
                "{name}: planet {} has no race for its owner",
                planet.id
            );
            assert!(planet.delta_pop < 100, "{name}: bad growth accumulator");
        }
    }
}

#[test]
fn a_real_game_can_generate_a_turn() {
    let root = workspace_root();
    let path = root.join("fixtures/games/exodus/2424/exodus.m6");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipping: {} absent", path.display());
        return;
    };
    let file = StarsFile::decode(&bytes).expect("fixture decodes");
    let (mut state, report) = GameState::from_file(&file);

    let year_before = state.year();
    let pop_before: i64 = state.planets.iter().map(|p| i64::from(p.pop)).sum();
    let minerals_before: i64 = state
        .planets
        .iter()
        .map(|p| p.surface_min.iter().map(|m| i64::from(*m)).sum::<i64>())
        .sum();

    let mut rng = Rng::randomize(state.seed);
    let turn = generate_turn(&mut state, &mut rng);

    eprintln!(
        "generated {} -> {}: {} planets mined, {} grew, research {:?}",
        year_before,
        state.year(),
        turn.mined.len(),
        turn.population.len(),
        turn.research_spending
    );

    assert_eq!(state.year(), year_before + 1);
    assert_eq!(
        turn.mined.len(),
        report.planets_loaded,
        "every planet mines"
    );

    let pop_after: i64 = state.planets.iter().map(|p| i64::from(p.pop)).sum();
    assert!(pop_after > pop_before, "a healthy empire should grow");

    let minerals_after: i64 = state
        .planets
        .iter()
        .map(|p| p.surface_min.iter().map(|m| i64::from(*m)).sum::<i64>())
        .sum();
    assert!(
        minerals_after > minerals_before,
        "mining should add minerals"
    );

    // Research must have received something, and be spent sensibly.
    assert!(
        turn.research_spending.iter().any(|r| *r > 0),
        "research should be funded"
    );
    for player in &state.players {
        for level in player.research.levels {
            assert!(level <= 26, "tech level {level} is out of range");
        }
    }

    // The steps that are genuinely not implemented are still declared.
    assert!(!turn.skipped.is_empty(), "a partial turn must say so");
}
