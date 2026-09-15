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
    // The Berserkers are Super Stealth — 300 cloaking points bare, 75% —
    // which is its own test below; here they are made plain to see.
    state.players[1].race.attrs[stars_core::race::RaceStat::MajorAdv as usize] = 9;
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

/// A cloaked fleet is seen at a shorter range: a Stealth Cloak's 35% on an
/// empty ship brings the probe's fifty-four down to thirty-five light years
/// (`SetVisPFFleets`, `1070:a1d0`), and a Tachyon Detector on the scanning
/// ship claws five per cent of the cloak back.
#[test]
fn a_cloak_shortens_the_range_a_fleet_is_seen_at() {
    use stars_core::components::slot;
    use stars_core::design::DesignSlot;

    let mut state = tutorial_world();
    // Bare, the Super Stealth Berserkers are 75% cloaked: 300 points. With
    // a Stealth Cloak's 70 on top, 370 is 77%.
    let enemy = state
        .fleets
        .iter()
        .position(|f| f.owner == 1)
        .expect("a Berserker fleet");
    assert_eq!(
        state.fleets[enemy].cloak_pct(&state.designs[1], &state.players[1].race),
        75,
        "Super Stealth alone"
    );
    state.players[1].race.attrs[stars_core::race::RaceStat::MajorAdv as usize] = 9;
    let probe = state
        .fleets
        .iter()
        .position(|f| f.owner == 0 && f.id == 0)
        .expect("Armed Probe #1");
    let at = stars_core::movement::Point::new(1500, 1500);
    state.fleets[probe].position = at;
    state.fleets[probe].orbiting = None;
    // The Berserker fleet, fitted with a Stealth Cloak and emptied of cargo,
    // forty light years off.
    let design = usize::from(state.fleets[enemy].stacks[0].design);
    state.designs[1][design].slots.push(DesignSlot {
        category: slot::SPECIAL_E,
        item: 1, // Stealth Cloak, 70 points
        count: 1,
    });
    state.fleets[enemy].cargo = stars_core::fleet::Cargo::default();
    state.fleets[enemy].orbiting = None;
    state.fleets[enemy].position = stars_core::movement::Point::new(at.x + 40, at.y);
    assert_eq!(
        state.fleets[enemy].cloak_pct(&state.designs[1], &state.players[1].race),
        35
    );

    let seen = view(&state, 0);
    assert!(!seen.fleets.contains(&enemy), "forty is past 54 × 65%");
    state.fleets[enemy].position = stars_core::movement::Point::new(at.x + 35, at.y);
    let seen = view(&state, 0);
    assert!(seen.fleets.contains(&enemy), "thirty-five is not");

    // Cargo aboard dilutes the cloak: the same ship loaded to twice its
    // mass shows at more like 60%-of-70 points.
    let mass = state.designs[1][design].mass().expect("a hull");
    state.fleets[enemy].cargo.minerals[0] = mass;
    let diluted = state.fleets[enemy].cloak_pct(&state.designs[1], &state.players[1].race);
    assert_eq!(diluted, 35 / 2, "70 points over twice the mass");

    // A Tachyon Detector on the probe leaves 95% of the cloak: 33%, and the
    // reach is 54 × 67% = 36.
    state.fleets[enemy].cargo = stars_core::fleet::Cargo::default();
    state.fleets[enemy].position = stars_core::movement::Point::new(at.x + 36, at.y);
    assert!(!view(&state, 0).fleets.contains(&enemy));
    let probe_design = usize::from(state.fleets[probe].stacks[0].design);
    state.designs[0][probe_design].slots.push(DesignSlot {
        category: slot::SPECIAL_E,
        item: 15, // Tachyon Detector
        count: 1,
    });
    assert_eq!(state.designs[0][probe_design].tachyon_pct(), 95);
    assert!(view(&state, 0).fleets.contains(&enemy));
}

/// The points-to-percent table, as the manual prints it (p. 24-3).
#[test]
fn cloak_points_read_off_the_manuals_table() {
    use stars_core::design::cloak_pct_of_points;
    assert_eq!(cloak_pct_of_points(0), 0);
    assert_eq!(cloak_pct_of_points(70), 35, "a Stealth Cloak");
    assert_eq!(cloak_pct_of_points(100), 50);
    assert_eq!(cloak_pct_of_points(140), 55, "a Super-Stealth Cloak");
    assert_eq!(cloak_pct_of_points(300), 75, "a Transport Cloak");
    assert_eq!(cloak_pct_of_points(540), 85, "an Ultra-Stealth Cloak");
    assert_eq!(cloak_pct_of_points(612), 88);
    assert_eq!(cloak_pct_of_points(1124), 96);
    assert_eq!(cloak_pct_of_points(1380), 97);
    assert_eq!(cloak_pct_of_points(1612), 98);
    assert_eq!(cloak_pct_of_points(30_000), 0, "the overflow guard");
}

/// A Packet Physics race's packets scan as they fly, penetrating, to the
/// square of their warp; a Space Demolition race's minefields show the
/// fleets loose inside them (`SetVisPFThings`, `1070:b9ee`).
#[test]
fn packets_and_minefields_scan_for_the_races_that_own_them() {
    let mut state = tutorial_world();
    state.players[1].race.attrs[stars_core::race::RaceStat::MajorAdv as usize] = 9;
    // Everything of ours parked far from the corner in question.
    for fleet in state.fleets.iter_mut().filter(|f| f.owner == 0) {
        fleet.position = stars_core::movement::Point::new(1600, 1600);
        fleet.orbiting = None;
    }
    let corner = stars_core::movement::Point::new(1050, 1050);
    let enemy = state
        .fleets
        .iter()
        .position(|f| f.owner == 1)
        .expect("a Berserker fleet");
    state.fleets[enemy].position = corner;
    state.fleets[enemy].orbiting = None;
    assert!(
        !view(&state, 0).fleets.contains(&enemy),
        "nothing of ours near"
    );

    // A warp 6 packet of ours thirty light years off: 36 reaches it.
    state.packets.push(stars_core::packet::Packet {
        id: 0,
        owner: 0,
        position: stars_core::movement::Point::new(corner.x + 30, corner.y),
        target: 0,
        warp: 2,
        minerals: [100, 0, 0],
        decay_rate: 0,
        moved: false,
        include: true,
        turn: 0,
    });
    assert!(
        !view(&state, 0).fleets.contains(&enemy),
        "only for Packet Physics"
    );
    state.players[0].race.attrs[stars_core::race::RaceStat::MajorAdv as usize] = 6;
    assert!(view(&state, 0).fleets.contains(&enemy));
    state.packets[0].warp = 1; // warp 5: 25, short of 30
    assert!(!view(&state, 0).fleets.contains(&enemy));
    state.packets.clear();

    // A minefield of ours over the corner shows the fleet for Space
    // Demolition alone.
    state.minefields.push(stars_core::minefield::Minefield {
        id: 0,
        owner: 0,
        position: corner,
        mines: 400,
        kind: 0,
        detonating: false,
        detected_by: 0,
        visible_to: 0,
        turn: 0,
    });
    assert!(!view(&state, 0).fleets.contains(&enemy));
    state.players[0].race.attrs[stars_core::race::RaceStat::MajorAdv as usize] = 5;
    assert!(view(&state, 0).fleets.contains(&enemy));
    // Not one in orbit, though.
    state.fleets[enemy].orbiting = Some(0);
    assert!(!view(&state, 0).fleets.contains(&enemy));
}
