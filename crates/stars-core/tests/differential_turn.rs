//! Whole-turn differential: generate a year and compare against the file the
//! original engine wrote for that year.
//!
//! This is the broadest check in the project. Everything the pipeline does —
//! mining, the resource split, the build queue, population growth, research —
//! runs together on a real game, and the result is compared field by field
//! against what actually happened.
//!
//! It is expected to be *partly* wrong, because the pipeline does not yet do
//! everything a turn does: fleets do not move, cargo is not transferred, no
//! terraforming happens and ships are not built. The value is in measuring how
//! much each field agrees, and in noticing when that gets worse.

use std::collections::BTreeMap;
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

fn load(path: &Path) -> Option<GameState> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    Some(GameState::from_file(&file).0)
}

/// How often each field of a planet comes out right.
#[derive(Default)]
struct Tally {
    planets: usize,
    population: usize,
    mines: usize,
    factories: usize,
    minerals: usize,
    concentration: usize,
}

#[test]
fn generating_a_year_reproduces_much_of_the_next_file() {
    let root = workspace_root();
    let games = root.join("fixtures/games/exodus");
    if !games.is_dir() {
        eprintln!("skipping: no Exodus fixtures");
        return;
    }
    let mut years: Vec<i32> = std::fs::read_dir(&games)
        .expect("readable fixture dir")
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    years.sort_unstable();

    let mut total = Tally::default();
    let mut pairs = 0;

    for w in years.windows(2) {
        if w[1] != w[0] + 1 {
            continue;
        }
        let (Some(mut before), Some(after)) = (
            load(&games.join(w[0].to_string()).join("exodus.m6")),
            load(&games.join(w[1].to_string()).join("exodus.m6")),
        ) else {
            continue;
        };

        // Terraforming happens before growth and this crate does not do it, so
        // feed in the environment the engine recorded — the same allowance the
        // population test makes, for the same reason.
        let actual: BTreeMap<i16, _> = after.planets.iter().map(|p| (p.id, p.clone())).collect();
        for planet in &mut before.planets {
            if let Some(next) = actual.get(&planet.id) {
                planet.env = next.env;
            }
        }

        let mut rng = Rng::randomize(before.seed);
        generate_turn(&mut before, &mut rng);
        pairs += 1;

        for planet in &before.planets {
            let Some(want) = actual.get(&planet.id) else {
                continue;
            };
            total.planets += 1;
            total.population += usize::from(planet.pop == want.pop);
            total.mines += usize::from(planet.mines == want.mines);
            total.factories += usize::from(planet.factories == want.factories);
            total.minerals += usize::from(planet.surface_min == want.surface_min);
            total.concentration += usize::from(planet.min_conc == want.min_conc);
        }
    }

    let pct = |n: usize| (n * 100).checked_div(total.planets).unwrap_or(0);
    eprintln!(
        "whole-turn replay over {pairs} year pairs, {} planet-years:\n  \
         population {}%  mines {}%  factories {}%  surface minerals {}%  concentrations {}%",
        total.planets,
        pct(total.population),
        pct(total.mines),
        pct(total.factories),
        pct(total.minerals),
        pct(total.concentration),
    );

    assert!(pairs > 0, "expected at least one consecutive-year pair");
    assert!(total.planets > 100, "expected a meaningful sample");

    // Population is the field the pipeline models most completely, so it is
    // the one held to a real standard. The others are reported: mines and
    // factories depend on a build queue the pipeline reads but whose ship
    // items it cannot build, and minerals depend on cargo the pipeline does
    // not move.
    assert!(
        pct(total.population) >= 70,
        "population agreed on only {}% of planet-years",
        pct(total.population)
    );
    assert!(
        pct(total.concentration) >= 70,
        "mineral concentrations agreed on only {}% of planet-years",
        pct(total.concentration)
    );
}
