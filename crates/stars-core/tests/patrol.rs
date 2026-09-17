//! The Patrol waypoint task.
//!
//! A patrolling fleet finds something to intercept at the end of the year, in
//! the same place the original decides it: as each player's turn file is
//! written. See `docs/formulas/waypoint-tasks.md`.

use stars_core::design::ShipDesign;
use stars_core::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use stars_core::movement::Point;
use stars_core::patrol::{attacks, matches_target, patrol, patrol_range};
use stars_core::race::Race;
use stars_core::{generate_turn, GameState, Player, Rng};

/// Two players, each with one design, and a fleet apiece.
fn a_game() -> GameState {
    let mut state = GameState::new(3);
    state.players = vec![Player::new(Race::humanoid()), Player::new(Race::humanoid())];
    // Hull 4 is the Scout, whose category is 2: one of the armed hulls, which
    // is what the default battle plan hunts.
    let design = ShipDesign {
        name: "Scout".to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: 4,
        // A Rhino Scanner: a patrol can only target a fleet on its map.
        slots: vec![stars_core::design::DesignSlot {
            category: stars_core::components::slot::SCANNER,
            item: 1,
            count: 1,
        }],
    };
    state.designs = vec![vec![design.clone()], vec![design]];
    state
}

fn fleet(id: u16, owner: i16, at: Point, task: u8) -> Fleet {
    Fleet {
        name: None,
        repeat_orders: false,
        direction: None,
        id,
        owner,
        position: at,
        orbiting: None,
        stacks: vec![ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo::default(),
        battle_plan: 0,
        warp: Some(6),
        waypoints: vec![Waypoint {
            position: at,
            target: None,
            target_class: 4,
            warp: 0,
            task,
            transport: None,
            task_data: vec![0, 0, 0, 0],
        }],
    }
}

/// Fifty light years a step, and the tenth setting means as far as it takes.
#[test]
fn the_range_steps_in_fifties() {
    assert_eq!(patrol_range(0), 50);
    assert_eq!(patrol_range(1), 100);
    assert_eq!(patrol_range(9), 500);
    assert_eq!(patrol_range(10), 10_000, "as far as it takes");
}

/// A patrol takes the nearest enemy inside its range, and leaves alone one
/// outside it.
#[test]
fn a_patrol_intercepts_the_nearest_enemy_in_range() {
    let mut state = a_game();
    // Everyone attacks everyone, so the battle plan is not in the way.
    state.players[0].battle_plans[0].attack_who = 3;
    state.fleets = vec![
        fleet(1, 0, Point::new(1000, 1000), stars_formats::task::PATROL),
        fleet(2, 1, Point::new(1040, 1000), 0), // 40 ly away: inside 50
        fleet(3, 1, Point::new(1020, 1000), 0), // 20 ly away: nearer
    ];

    let ordered = patrol(&mut state);
    assert_eq!(ordered, vec![(1, 3)], "the nearer of the two");
    let leg = &state.fleets[0].waypoints[1];
    assert_eq!(leg.target, Some((1 << 9) | 3), "player 1's fleet 3");
    assert_eq!(leg.target_class, 2, "a fleet, not a planet");
    assert_eq!(leg.position, Point::new(1020, 1000));

    // Move the targets out of range and nothing is ordered.
    let mut state = a_game();
    state.players[0].battle_plans[0].attack_who = 3;
    state.fleets = vec![
        fleet(1, 0, Point::new(1000, 1000), stars_formats::task::PATROL),
        fleet(2, 1, Point::new(1060, 1000), 0), // 60 ly: outside 50
    ];
    assert!(patrol(&mut state).is_empty());
    assert_eq!(state.fleets[0].waypoints.len(), 1);
}

/// Two patrols do not chase the same fleet while there is another to chase.
#[test]
fn patrols_spread_out() {
    let mut state = a_game();
    state.players[0].battle_plans[0].attack_who = 3;
    state.fleets = vec![
        fleet(1, 0, Point::new(1000, 1000), stars_formats::task::PATROL),
        fleet(2, 0, Point::new(1000, 1000), stars_formats::task::PATROL),
        fleet(3, 1, Point::new(1010, 1000), 0),
        fleet(4, 1, Point::new(1030, 1000), 0),
    ];

    let ordered = patrol(&mut state);
    assert_eq!(ordered, vec![(1, 3), (2, 4)], "one patroller per target");
}

/// The battle plan decides who is worth chasing.
#[test]
fn a_patrol_obeys_its_battle_plan() {
    // "Attack nobody" chases nothing; "enemies only" needs an enemy.
    for (attack_who, relation, expect) in [(0u8, 2u8, false), (1, 0, false), (1, 2, true)] {
        let mut state = a_game();
        state.players[0].battle_plans[0].attack_who = attack_who;
        state.players[0].relations = vec![0, relation];
        state.fleets = vec![
            fleet(1, 0, Point::new(1000, 1000), stars_formats::task::PATROL),
            fleet(2, 1, Point::new(1010, 1000), 0),
        ];
        assert_eq!(
            !patrol(&mut state).is_empty(),
            expect,
            "attack {attack_who}, relation {relation}"
        );
    }
}

/// `FAttackPlayer` in its own right, since combat will want it too.
#[test]
fn who_a_fleet_will_attack() {
    let mut state = a_game();
    state.players[0].relations = vec![0, 1, 2]; // neutral, friend, enemy
    state.fleets = vec![fleet(1, 0, Point::new(0, 0), 0)];
    let mut check = |attack_who: u8, other: i16| {
        state.players[0].battle_plans[0].attack_who = attack_who;
        let fleet = state.fleets[0].clone();
        attacks(&state, &fleet, other)
    };
    assert!(!check(0, 2), "nobody");
    assert!(check(1, 2), "enemies");
    assert!(!check(1, 1), "a friend is not an enemy");
    assert!(
        check(2, 0),
        "a neutral counts when it is anyone but a friend"
    );
    assert!(!check(2, 1));
    assert!(check(3, 1), "everyone");
    assert!(check(4 + 2, 2), "that player and no other");
    assert!(!check(4 + 2, 0));
}

/// Target classes come from the **hull's** category, not from what is bolted
/// to it.
#[test]
fn what_counts_as_a_target() {
    use stars_core::battle::TargetClass;

    let mut state = a_game();
    state.fleets = vec![fleet(1, 0, Point::new(0, 0), 0)];
    let scout = state.fleets[0].clone();
    // The Scout hull's category is 2, one of the armed ones.
    assert!(matches_target(&state, &scout, TargetClass::ArmedShips));
    assert!(!matches_target(&state, &scout, TargetClass::UnarmedShips));
    // And everything matches the classes the original's fall-through covers.
    assert!(matches_target(&state, &scout, TargetClass::Any));
}

/// Through a whole generated year: the patrol is set up at the end of it, so
/// the fleet flies at the target the following year.
#[test]
fn a_patrol_is_set_up_by_the_turn() {
    let mut state = a_game();
    state.players[0].battle_plans[0].attack_who = 3;
    state.fleets = vec![
        fleet(1, 0, Point::new(1000, 1000), stars_formats::task::PATROL),
        fleet(2, 1, Point::new(1030, 1000), 0),
    ];
    // Nobody may move: the point is what the patrol does, not the flying.
    state.fleets[0].warp = None;
    state.fleets[1].warp = None;

    let mut rng = Rng::from_seeds(1, 2);
    let report = generate_turn(&mut state, &mut rng);
    assert_eq!(report.patrols, vec![(1, 2)]);
    // The leg names the fleet by its owner and number.
    assert_eq!(state.fleets[0].waypoints[1].target, Some((1 << 9) | 2));

    // Out of the scanner's fifty light years, nothing is found.
    let mut state = a_game();
    state.players[0].battle_plans[0].attack_who = 3;
    state.fleets = vec![
        fleet(1, 0, Point::new(1000, 1000), stars_formats::task::PATROL),
        fleet(2, 1, Point::new(1080, 1000), 0),
    ];
    state.fleets[0].warp = None;
    state.fleets[1].warp = None;
    let report = generate_turn(&mut state, &mut Rng::from_seeds(1, 2));
    assert!(report.patrols.is_empty(), "unseen, untargeted");
}
