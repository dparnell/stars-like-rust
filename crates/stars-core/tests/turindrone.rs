//! The TurinDrone's turn on the tutorial's world — the Berserkers, whom
//! `tutorial.hst` marks `0x27`. See `docs/formulas/ai.md`, *The TurinDrone
//! turn*. Only what is written so far is asked about.

use stars_core::ai::turindrone;

fn tutorial_world() -> stars_core::GameState {
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state
}

/// In year 0 the Berserkers send their scouts to planets they have not
/// seen, and queue a scout — one per thirty planets of a twenty-four
/// planet universe — at home.
#[test]
fn year_zero_sends_the_scouts_out() {
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(1);
    let report = turindrone::turn(&mut state, 1, &mut rng);
    assert!(
        !report.scouted.is_empty(),
        "the scouts have somewhere to go"
    );
    for (fleet, planet) in &report.scouted {
        let fleet = state
            .fleets
            .iter()
            .find(|f| f.owner == 1 && f.id == *fleet)
            .expect("a Berserker fleet");
        assert_eq!(fleet.waypoints.len(), 2);
        assert_eq!(fleet.waypoints[1].target, Some(*planet as u16));
        assert!(fleet.waypoints[1].warp > 0, "at a warp of its own");
        // Not one of the planets in their scanner's view.
        assert!(!state.players[1].explored.contains(planet));
    }
    assert_eq!(
        report
            .queued
            .iter()
            .filter(|(_, d, _)| *d == turindrone::SCOUT_SLOT)
            .map(|(_, _, n)| n)
            .sum::<i32>(),
        1,
        "one scout for twenty-four planets"
    );
}

/// The turn runs inside the year: after a few years the Berserkers' scouts
/// are out among the planets and their knowledge has grown.
#[test]
fn the_berserkers_explore_as_the_years_go_by() {
    let mut state = tutorial_world();
    let mut rng = stars_core::rng::Rng::randomize(1);
    let home = state.players[1].explored.len();
    for _ in 0..6 {
        stars_core::generate_turn(&mut state, &mut rng);
    }
    let berserker_fleets: Vec<_> = state.fleets.iter().filter(|f| f.owner == 1).collect();
    assert!(
        berserker_fleets.iter().any(|f| f.orbiting != Some(10)),
        "somebody has left home: {:?}",
        berserker_fleets
            .iter()
            .map(|f| (f.id, f.position, f.orbiting))
            .collect::<Vec<_>>()
    );
    assert!(
        state.players[1].explored.len() > home.max(1),
        "and seen more of the galaxy"
    );
}

/// The new game gives the Berserkers what `tutorial.hst` gives them: three
/// designs — a Smaugarian Peeping Tom, a Santa Maria, a Potato Bug — and
/// four fleets, one scout, one colony ship and two miners.
#[test]
fn the_berserkers_start_as_the_host_file_has_them() {
    let state = tutorial_world();
    let designs = &state.designs[1];
    let names: Vec<&str> = designs.iter().take(3).map(|d| d.name.as_str()).collect();
    assert_eq!(
        names,
        ["Smaugarian Peeping Tom", "Santa Maria", "Potato Bug"]
    );
    assert!(designs
        .get(3)
        .is_none_or(|d| d.hull().is_none() || d.is_starbase()));
    let mut fleets: Vec<(u16, u8)> = state
        .fleets
        .iter()
        .filter(|f| f.owner == 1)
        .map(|f| (f.id, f.stacks[0].design))
        .collect();
    fleets.sort_unstable();
    assert_eq!(fleets, [(0, 0), (1, 1), (2, 2), (3, 2)]);
    assert_eq!(state.players[1].research.levels, [0, 0, 0, 0, 5, 0]);
}
