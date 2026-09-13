//! What a player can see: `SetVisPFPlanets` and `SetVisPFFleets` as
//! `stars_core::visibility` transcribes them, checked on the tutorial's
//! world. See `docs/formulas/scanning.md`.

use stars_core::visibility::{fleet_scan, view};

fn tutorial_world() -> stars_core::GameState {
    let (config, seed) = stars_core::newgame::tutorial();
    let mut rng = stars_core::rng::Rng::randomize(seed);
    stars_core::newgame::generate(&config, &mut rng)
        .expect("generates")
        .state
}

/// In 2400 the player knows their home world and nothing else: the Scoper
/// 150 does not penetrate, and every fleet is at home.
#[test]
fn the_tutorial_opens_knowing_only_home() {
    let state = tutorial_world();
    let seen = view(&state, 0);
    assert_eq!(seen.planets.iter().copied().collect::<Vec<_>>(), vec![13]);
    let own: Vec<usize> = (0..state.fleets.len())
        .filter(|i| state.fleets[*i].owner == 0)
        .collect();
    assert_eq!(seen.fleets.iter().copied().collect::<Vec<_>>(), own);
    // The other player's home world and fleets are out of sight.
    assert!(!seen.planets.contains(&10));
}

/// The Armed Probe's scanners: a Rhino (50, no penetration) and, in the
/// tutorial, the Jack of All Trades' fixed 40/20 built into the Scout hull.
/// The two combine by fourth powers: `(50⁴ + 40⁴)^¼ = 54`, and 20 deep.
#[test]
fn the_armed_probe_scans_fifty_four_and_twenty_deep() {
    let state = tutorial_world();
    let probe = state
        .fleets
        .iter()
        .find(|f| f.owner == 0 && f.id == 0)
        .expect("Armed Probe #1");
    let range = fleet_scan(&state, probe);
    assert_eq!((range.normal, range.penetrating), (54, 20));
}

/// Outside the tutorial the built-in scanner scales with Electronics:
/// `20 × 3 = 60` and `10 × 3 = 30` at level 3, so `(50⁴ + 60⁴)^¼ = 66` with the Rhino.
#[test]
fn outside_the_tutorial_the_built_in_scanner_follows_electronics() {
    let mut state = tutorial_world();
    state.tutorial_game = false;
    let probe = state
        .fleets
        .iter()
        .find(|f| f.owner == 0 && f.id == 0)
        .expect("Armed Probe #1");
    let range = fleet_scan(&state, probe);
    assert_eq!(state.players[0].research.levels[4], 3);
    assert_eq!((range.normal, range.penetrating), (66, 30));
}

/// A planet is learned within penetrating range, and a fleet in orbit is
/// seen only within it; a fleet in open space within normal range.
#[test]
fn planets_take_penetration_and_orbiting_fleets_too() {
    let mut state = tutorial_world();
    // Park the probe nineteen light years from Prune, in open space.
    let prune = state
        .planets
        .iter()
        .find(|p| p.id == 12)
        .and_then(|p| p.position)
        .expect("Prune");
    let probe = state
        .fleets
        .iter()
        .position(|f| f.owner == 0 && f.id == 0)
        .expect("Armed Probe #1");
    state.fleets[probe].position = stars_core::movement::Point::new(prune.x + 19, prune.y);
    state.fleets[probe].orbiting = None;
    // An enemy fleet in orbit at Prune, and one in open space thirty away.
    let enemy = state
        .fleets
        .iter()
        .position(|f| f.owner == 1)
        .expect("a Berserker fleet");
    state.fleets[enemy].position = prune;
    state.fleets[enemy].orbiting = Some(12);
    let mut loose = state.fleets[enemy].clone();
    loose.id = 9;
    loose.position = stars_core::movement::Point::new(prune.x + 19, prune.y + 30);
    loose.orbiting = None;
    state.fleets.push(loose);
    let last = state.fleets.len() - 1;

    let seen = view(&state, 0);
    assert!(seen.planets.contains(&12), "Prune within twenty");
    assert!(seen.fleets.contains(&enemy), "in orbit, within twenty");
    assert!(seen.fleets.contains(&last), "in space, within fifty-four");

    // Twenty-one out: the planet and the orbiting fleet drop away, the loose
    // one stays.
    state.fleets[probe].position = stars_core::movement::Point::new(prune.x + 21, prune.y);
    let seen = view(&state, 0);
    assert!(!seen.planets.contains(&12));
    assert!(!seen.fleets.contains(&enemy));
    assert!(seen.fleets.contains(&last));
}
