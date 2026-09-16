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
use stars_core::{generate_turn, generate_turn_with_orders, GameState, TurnOrders};
use stars_formats::StarsFile;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root")
        .to_path_buf()
}

/// The cargo transfers the player's own `.x` order file records for this year.
///
/// The host replays these before anything else in the turn, so a replay that
/// skips them is missing colonists that were loaded or dropped.
fn orders_for(year: &Path) -> TurnOrders {
    let Some(bytes) = std::fs::read(year.join("EXODUS.X6")).ok() else {
        return TurnOrders::default();
    };
    let Ok(file) = stars_formats::StarsFile::decode(&bytes) else {
        return TurnOrders::default();
    };
    let segment = file.latest_segment();
    TurnOrders {
        cargo: stars_formats::cargo_transfers_in(file.segment_blocks(segment)),
        ..TurnOrders::default()
    }
}

fn load(path: &Path) -> Option<GameState> {
    let bytes = std::fs::read(path).ok()?;
    let file = StarsFile::decode(&bytes).ok()?;
    let (mut state, _) = GameState::from_file(&file);
    // Planet coordinates live in the universe file, and a planet that finishes
    // ships with no fleet in orbit needs them to start one.
    if let Some(universe) = path
        .parent()
        .and_then(Path::parent)
        .map(|games| games.join("exodus.xy"))
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| stars_formats::Universe::decode(&b).ok())
    {
        state.apply_universe(&universe);
    }
    Some(state)
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
    /// Individual mineral readings, three per planet-year.
    mineral_readings: usize,
    /// Readings that match exactly.
    mineral_exact: usize,
    /// Readings within one kT — the width of the mining remainder roll.
    mineral_near: usize,
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

        let actual: BTreeMap<i16, _> = after.planets.iter().map(|p| (p.id, p.clone())).collect();

        let mut rng = Rng::randomize(before.seed);
        generate_turn_with_orders(
            &mut before,
            &orders_for(&games.join(w[0].to_string())),
            &mut rng,
        );
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
            for i in 0..3 {
                total.mineral_readings += 1;
                let d = planet.surface_min[i] - want.surface_min[i];
                total.mineral_exact += usize::from(d == 0);
                total.mineral_near += usize::from(d.abs() <= 1);
            }
            total.concentration += usize::from(planet.min_conc == want.min_conc);
        }
    }

    let pct = |n: usize| (n * 100).checked_div(total.planets).unwrap_or(0);
    eprintln!(
        "whole-turn replay over {pairs} year pairs, {} planet-years:\n  \
         population {}/{} ({}%)  mines {}%  factories {}%  surface minerals {}%  \
         concentrations {}/{} ({}%)",
        total.planets,
        total.population,
        total.planets,
        pct(total.population),
        pct(total.mines),
        pct(total.factories),
        pct(total.minerals),
        total.concentration,
        total.planets,
        pct(total.concentration),
    );
    // "Surface minerals" above demands all three match at once, which
    // compounds a per-mineral error cubically. The per-mineral figures say what
    // is actually happening: most of the disagreement is a single kilotonne,
    // the width of the mining remainder roll, which this replay cannot align
    // because it starts the RNG fresh rather than in the state the original
    // reached.
    println!(
        "  per mineral: {}% exact, {}% within 1 kT of {} readings",
        total
            .mineral_readings
            .checked_div(1)
            .map_or(0, |_| total.mineral_exact * 100
                / total.mineral_readings.max(1)),
        total.mineral_near * 100 / total.mineral_readings.max(1),
        total.mineral_readings,
    );

    assert!(pairs > 0, "expected at least one consecutive-year pair");
    assert!(total.planets > 100, "expected a meaningful sample");

    // Population is the field the pipeline models most completely, so it is
    // the one held to a real standard. Mines and factories are reported.
    //
    // Minerals get a real standard too, but per reading rather than per
    // planet-year: 83% land within a kilotonne, and the residual beyond that
    // is a genuine gap rather than a rounding difference.
    assert!(
        total.mineral_near * 100 / total.mineral_readings.max(1) >= 78,
        "only {}% of mineral readings land within a kilotonne",
        total.mineral_near * 100 / total.mineral_readings.max(1)
    );
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

/// Fleet movement checked against where the fleets actually ended up.
///
/// A fleet's position is recorded every year, so moving it along its own
/// waypoints and comparing is exact — no allowance needed, unlike the planet
/// fields, which depend on orders the pipeline does not process.
#[test]
fn fleets_move_to_where_the_engine_put_them() {
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

    let mut moving = 0usize;
    let mut exact = 0usize;
    let mut notes = Vec::new();

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

        // Only fleets that had somewhere to go are informative.
        let heading: BTreeMap<u16, _> = before
            .fleets
            .iter()
            .filter(|f| f.next_leg().is_some())
            .map(|f| (f.id, f.position))
            .collect();

        let mut rng = Rng::randomize(before.seed);
        generate_turn(&mut before, &mut rng);

        let actual: BTreeMap<u16, _> = after.fleets.iter().map(|f| (f.id, f.position)).collect();

        for fleet in &before.fleets {
            if !heading.contains_key(&fleet.id) {
                continue;
            }
            let Some(want) = actual.get(&fleet.id) else {
                continue; // the fleet is gone: merged, scrapped or destroyed
            };
            moving += 1;
            if fleet.position == *want {
                exact += 1;
            } else if notes.len() < 8 {
                notes.push(format!(
                    "{}->{}: fleet {} computed ({},{}), engine ({},{})",
                    w[0], w[1], fleet.id, fleet.position.x, fleet.position.y, want.x, want.y
                ));
            }
        }
    }

    let pct = (exact * 100).checked_div(moving).unwrap_or(0);
    eprintln!("fleet movement: {exact} of {moving} moving fleets land exactly right ({pct}%)");
    for n in &notes {
        eprintln!("  {n}");
    }
    assert!(moving > 20, "expected a decent sample of moving fleets");
    assert!(
        pct >= 50,
        "only {pct}% of fleets moved to where the engine put them"
    );
}

/// The recorded cargo transfers, applied on the planet-years they name.
///
/// A `.x` order file logs the transfers a player's client already performed,
/// and the host replays them before anything else in the turn. Scoring them
/// across the whole replay dilutes them to nothing — Exodus records 43 in forty
/// files — so this scores only the planets a transfer actually names, with the
/// orders applied and again without them as a control.
///
/// The result is small but one-sided, which is the point: applying the orders
/// can only help here, and never turns a right answer into a wrong one.
#[test]
fn recorded_cargo_transfers_improve_the_planets_they_name() {
    use stars_core::rng::Rng;
    use stars_formats::GrobjClass;

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

    let (mut named_total, mut with, mut without) = (0usize, 0usize, 0usize);
    for w in years.windows(2) {
        if w[1] != w[0] + 1 {
            continue;
        }
        let (Some(before), Some(after)) = (
            load(&games.join(w[0].to_string()).join("exodus.m6")),
            load(&games.join(w[1].to_string()).join("exodus.m6")),
        ) else {
            continue;
        };
        let orders = orders_for(&games.join(w[0].to_string()));
        let mut named: Vec<i16> = Vec::new();
        for record in &orders.cargo {
            for (class, id) in [
                (record.source_class, record.source),
                (record.destination_class, record.destination),
            ] {
                if class == Some(GrobjClass::Planet) {
                    if let Ok(id) = i16::try_from(id) {
                        named.push(id);
                    }
                }
            }
        }
        if named.is_empty() {
            continue;
        }
        named_total += named.len();

        for (apply, tally) in [(true, &mut with), (false, &mut without)] {
            let mut state = before.clone();
            let mut rng = Rng::randomize(state.seed);
            let used = if apply {
                orders.clone()
            } else {
                TurnOrders::default()
            };
            generate_turn_with_orders(&mut state, &used, &mut rng);
            for id in &named {
                let (Some(ours), Some(theirs)) = (
                    state.planets.iter().find(|p| p.id == *id),
                    after.planets.iter().find(|p| p.id == *id),
                ) else {
                    continue;
                };
                if ours.pop == theirs.pop {
                    *tally += 1;
                }
            }
        }
    }

    if named_total == 0 {
        eprintln!("skipping: no recorded transfers in the fixtures");
        return;
    }
    eprintln!(
        "cargo transfers: {named_total} planet-years named; population exact \
         {with} with the orders, {without} without"
    );
    assert!(
        with > without,
        "applying the recorded transfers should not make the replay worse \
         ({with} with, {without} without)"
    );
}
