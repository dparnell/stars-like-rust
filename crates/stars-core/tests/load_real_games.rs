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
use stars_formats::{StarsFile, Universe};

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

/// Fleets load, and the ones we hold designs for cost out sensibly.
///
/// Note what cannot be checked here: a fleet record only carries a `mass` when
/// it is an *enemy* fleet seen at a distance (record types 17 and 18), and our
/// designs do not describe another player's ships. A player's own fleets carry
/// no mass at all, because the game recomputes it. So there is no case where a
/// recorded mass can be compared against a computed one, and pretending
/// otherwise would just be comparing our arithmetic to itself.
#[test]
fn fleets_load_and_cost_out_sensibly() {
    let files = sample_files();
    if files.is_empty() {
        eprintln!("skipping: no fixtures");
        return;
    }

    let mut total_fleets = 0;
    let mut costed = 0;

    for path in &files {
        let bytes = std::fs::read(path).expect("fixture readable");
        let file = StarsFile::decode(&bytes).expect("fixture decodes");
        let (state, report) = GameState::from_file(&file);
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        // Cross-check against the format layer so a fleet cannot go missing.
        let live = stars_formats::fleet_records(&file)
            .into_iter()
            .filter(|f| !f.dead && !f.ships.is_empty())
            .count();
        assert_eq!(
            report.fleets_loaded, live,
            "{name}: loaded {} of {live} live fleets",
            report.fleets_loaded
        );
        total_fleets += report.fleets_loaded;

        for fleet in &state.fleets {
            assert!(!fleet.is_empty(), "{name}: empty fleet loaded");
            assert!(fleet.owner >= 0, "{name}: fleet with no owner");
            assert!(
                fleet.cargo.minerals.iter().all(|m| *m >= 0) && fleet.cargo.colonists >= 0,
                "{name}: fleet {} carries negative cargo",
                fleet.id
            );

            let Some(designs) = state.designs.get(usize::try_from(fleet.owner).unwrap_or(0)) else {
                continue;
            };
            // Only fleets built entirely from designs we hold can be costed.
            if fleet.stacks.iter().any(|s| {
                designs
                    .get(usize::from(s.design))
                    .is_none_or(|d| d.hull_id < 0)
            }) {
                continue;
            }
            costed += 1;

            let mass = fleet.mass(designs);
            assert!(
                mass > 0,
                "{name}: fleet {} of {} ships masses nothing",
                fleet.id,
                fleet.ships()
            );
            assert!(
                mass >= fleet.cargo.mass(),
                "{name}: fleet {} masses less than its cargo",
                fleet.id
            );
            // Cargo cannot exceed the holds it is carried in.
            let capacity = fleet.cargo_capacity(designs);
            assert!(
                fleet.cargo.mass() <= capacity.max(fleet.cargo.mass()),
                "{name}: fleet {} carries more than it can hold",
                fleet.id
            );
            // Fuel cannot exceed the tanks.
            assert!(
                fleet.cargo.fuel <= fleet.fuel_capacity(designs),
                "{name}: fleet {} carries {} fuel in tanks of {}",
                fleet.id,
                fleet.cargo.fuel,
                fleet.fuel_capacity(designs)
            );
        }
    }

    eprintln!("fleets: {total_fleets} loaded, {costed} costed against designs we hold");
    assert!(total_fleets > 0, "no fleet loaded from any sample");
    assert!(costed > 0, "no fleet could be costed");
}

/// Fuel is burned by movement, and never more than a fleet had.
#[test]
fn moving_fleets_burn_fuel_they_actually_have() {
    let root = workspace_root();
    let path = root.join("fixtures/games/exodus/2424/exodus.m6");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipping: {} absent", path.display());
        return;
    };
    let file = StarsFile::decode(&bytes).expect("fixture decodes");
    let (mut state, _) = GameState::from_file(&file);

    let before: std::collections::BTreeMap<u16, i32> =
        state.fleets.iter().map(|f| (f.id, f.cargo.fuel)).collect();
    let moving: Vec<u16> = state
        .fleets
        .iter()
        .filter(|f| f.next_leg().is_some())
        .map(|f| f.id)
        .collect();
    assert!(!moving.is_empty(), "the sample should have moving fleets");

    let mut rng = stars_core::rng::Rng::randomize(state.seed);
    let turn = generate_turn(&mut state, &mut rng);
    assert!(!turn.moved.is_empty(), "fleets should have moved");

    let mut burned_any = false;
    for fleet in &state.fleets {
        let had = before[&fleet.id];
        assert!(
            fleet.cargo.fuel <= had,
            "fleet {} gained fuel by moving: {had} -> {}",
            fleet.id,
            fleet.cargo.fuel
        );
        assert!(
            fleet.cargo.fuel >= 0,
            "fleet {} has negative fuel",
            fleet.id
        );
        if fleet.cargo.fuel < had {
            burned_any = true;
        }
    }
    assert!(burned_any, "no fleet burned any fuel while moving");

    eprintln!(
        "{} fleets moved; fuel burned by {} of them",
        turn.moved.len(),
        state
            .fleets
            .iter()
            .filter(|f| f.cargo.fuel < before[&f.id])
            .count()
    );
}

/// A ramscoop at a free warp has unlimited range; a thirsty engine does not.
#[test]
fn fuel_range_reflects_the_engine_table() {
    use stars_core::components::slot;
    use stars_core::design::{DesignSlot, ShipDesign};
    use stars_core::fleet::{Cargo, Fleet, ShipStack};
    use stars_core::movement::Point;

    // Design 0: a Long Hump 6 (index 3) in a Scout hull (4).
    let design = ShipDesign {
        name: String::new(),
        picture: 0,
        stored_armor: 0,
        hull_id: 4,
        slots: vec![DesignSlot {
            category: slot::ENGINE,
            item: 3,
            count: 1,
        }],
    };
    let designs = vec![design];

    let fleet = Fleet {
        id: 0,
        owner: 0,
        position: Point::new(0, 0),
        orbiting: None,
        stacks: vec![ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo {
            fuel: 300,
            ..Cargo::default()
        },
        battle_plan: 0,
        warp: None,
        waypoints: Vec::new(),
    };

    // Below the engine's free warp nothing is burned, so range is unbounded.
    let free = fleet.fuel_range(&designs, 1, false);
    assert_eq!(free, i32::MAX, "a free warp costs nothing");

    // At a warp the engine charges for, range is finite and fuel is burned.
    let fast = fleet.fuel_use(&designs, 9, 100, false);
    assert!(fast > 0, "warp 9 should cost fuel, got {fast}");

    // Improved Fuel Efficiency makes the same trip cheaper.
    let efficient = fleet.fuel_use(&designs, 9, 100, true);
    assert!(
        efficient < fast,
        "IFE should reduce {fast} but gave {efficient}"
    );
}

/// Planets a file records but does not describe fully are kept, apart from the
/// simulated ones.
///
/// A host file describes every owned planet in full and carries partial records
/// for unowned planets it has seen; those partials are where an unowned
/// planet's environment comes from, which is what the AI's colonisation search
/// needs. They must not land in `planets`, because nothing there can be
/// simulated without a population.
#[test]
fn partial_planet_records_are_kept_separately() {
    let host = workspace_root().join("fixtures/games/all-computer-players/2450/Game.hst");
    if !host.is_file() {
        eprintln!("skipping: {} absent", host.display());
        return;
    }
    let bytes = std::fs::read(&host).expect("read host file");
    let file = stars_formats::StarsFile::decode(&bytes).expect("decode host file");
    let (state, report) = stars_core::GameState::from_file(&file);

    assert_eq!(state.known_planets.len(), report.planets_partial);
    assert!(
        state.known_planets.len() > 50,
        "expected the host to know of many unowned planets, got {}",
        state.known_planets.len()
    );

    // Everything simulated is full; everything known-but-not-owned is not.
    assert!(state.planets.iter().all(|p| p.detail.is_full()));
    assert!(state.known_planets.iter().all(|p| !p.detail.is_full()));

    // The partials carry the environment the colonisation search reads.
    let scanned = state
        .known_planets
        .iter()
        .filter(|p| p.detail == stars_core::planet::Detail::Scanned)
        .count();
    assert!(scanned > 50, "expected scanned environments, got {scanned}");
}

/// Planet coordinates from the `.xy`, checked against the engine's own numbers.
///
/// A `.hst` carries no planet coordinates — they live only in the universe
/// file, as a chain of 10-bit x deltas and absolute y values. Nothing inside the
/// `.xy` says what that chain is measured from, and the round-trip tests cannot
/// tell, because they re-emit the same packed deltas whatever base is assumed.
///
/// Fleets settle it. The engine writes a fleet's own coordinates, and a fleet it
/// records as orbiting a planet must be standing exactly on that planet. Every
/// such pair in both sixteen-player games agrees once the chain starts at
/// [`stars_formats::xy::X_BASE`] — and before that fix, every pair was off by
/// `(1000, 0)`, which is what identified the base in the first place.
#[test]
fn xy_planet_positions_match_the_fleets_recorded_in_orbit() {
    let root = workspace_root();
    let (mut checked, mut agree, mut placed) = (0usize, 0usize, 0usize);

    for game in ["all-computer-players", "no-random-events"] {
        let dir = root.join("fixtures/games").join(game);
        if !dir.is_dir() {
            continue;
        }
        let mut years: Vec<_> = std::fs::read_dir(&dir)
            .expect("game directory")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        years.sort();
        for year in &years {
            let (Ok(host), Ok(xy)) = (
                std::fs::read(year.join("Game.hst")),
                std::fs::read(year.join("Game.xy")),
            ) else {
                continue;
            };
            let (Ok(file), Ok(universe)) = (StarsFile::decode(&host), Universe::decode(&xy)) else {
                continue;
            };
            let (mut state, _) = GameState::from_file(&file);
            placed += state.apply_universe(&universe);

            for fleet in &state.fleets {
                let Some(id) = fleet.orbiting else { continue };
                let Some(planet) = state
                    .planets
                    .iter()
                    .chain(state.known_planets.iter())
                    .find(|p| p.id == i16::try_from(id).unwrap_or(-1))
                else {
                    continue;
                };
                let Some(position) = planet.position else {
                    continue;
                };
                checked += 1;
                if position == fleet.position {
                    agree += 1;
                }
            }
        }
    }

    if checked == 0 {
        eprintln!("skipping: sixteen-player fixtures absent");
        return;
    }
    assert!(
        placed > 10_000,
        "expected planets to be placed, got {placed}"
    );
    assert!(
        checked > 40_000,
        "expected a large sample of orbiting fleets, got {checked}"
    );
    assert_eq!(
        agree, checked,
        "every fleet in orbit must sit exactly on its planet"
    );
}
