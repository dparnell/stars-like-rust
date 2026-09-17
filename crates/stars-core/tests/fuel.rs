//! Running dry.
//!
//! `MoveFleets` (`10b0:42c3`): a fleet whose tank empties short of its
//! waypoint flies as far as the fuel took it, and the leg is then slowed to
//! the fastest warp its engines run free at, with a message saying so
//! (`idmHasRunFuelFleetsSpeedHasDecreased`). See
//! `docs/formulas/movement.md`.

use stars_core::components::slot;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_core::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use stars_core::message::id;
use stars_core::movement::Point;
use stars_core::planet::Planet;
use stars_core::race::Race;
use stars_core::{generate_turn, GameState, Player, Rng};

/// A Medium Freighter on a Long Hump 6 — the tutorial's Teamster — with
/// `cargo` kT of ironium and `fuel` mg aboard, ordered 49 light years east
/// into empty space at warp 7.
fn a_freighter(fuel: i32, cargo: i32) -> GameState {
    let mut state = GameState::new(11);
    state.players = vec![Player::new(Race::humanoid())];
    state.designs = vec![vec![ShipDesign {
        name: "Teamster".to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id: 1,
        slots: vec![DesignSlot {
            category: slot::ENGINE,
            item: 3, // Long Hump 6: warp 1 free, warp 2 costs
            count: 1,
        }],
    }]];
    let mut planet = Planet::unowned(1);
    planet.owner = Some(0);
    planet.position = Some(Point::new(1000, 1000));
    planet.pop = 25_000;
    state.planets = vec![planet];
    state.fleets = vec![Fleet {
        name: None,
        repeat_orders: false,
        direction: None,
        id: 1,
        owner: 0,
        position: Point::new(1000, 1000),
        orbiting: Some(1),
        stacks: vec![ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo {
            minerals: [cargo, 0, 0],
            colonists: 0,
            fuel,
        },
        battle_plan: 0,
        warp: None,
        waypoints: vec![
            Waypoint {
                position: Point::new(1000, 1000),
                target: Some(1),
                target_class: 1,
                warp: 0,
                task: 0,
                transport: None,
                task_data: Vec::new(),
            },
            Waypoint {
                position: Point::new(1049, 1000),
                target: None,
                target_class: 0,
                warp: 7,
                task: 0,
                transport: None,
                task_data: Vec::new(),
            },
        ],
    }];
    state
}

/// A loaded freighter at warp 7 cannot make 49 light years on 200 mg: it
/// gets as far as the fuel takes it, and the leg drops to warp 1 — the one
/// warp a Long Hump 6 runs free at.
#[test]
fn a_dry_tank_slows_the_leg_to_a_free_warp() {
    let mut state = a_freighter(200, 210);
    let designs = state.designs[0].clone();
    let whole_leg = state.fleets[0].fuel_use(&designs, 7, 49, false);
    assert!(
        whole_leg > 200,
        "the leg costs {whole_leg} mg, more than the tank"
    );

    let mut rng = Rng::randomize(1);
    generate_turn(&mut state, &mut rng);

    let fleet = &state.fleets[0];
    assert_eq!(fleet.cargo.fuel, 0, "the tank is empty");
    assert!(
        fleet.position.x > 1000 && fleet.position.x < 1049,
        "went part of the way: {:?}",
        fleet.position
    );
    assert_eq!(fleet.waypoints.len(), 2, "the leg is still to fly");
    assert_eq!(fleet.waypoints[1].warp, 1, "slowed to the free warp");
    let told: Vec<_> = state
        .messages
        .iter()
        .filter(|m| m.id == id::OUT_OF_FUEL_SLOWED)
        .collect();
    assert_eq!(told.len(), 1, "{:?}", state.messages);
    assert_eq!(told[0].player, 0);
    assert_eq!(told[0].object, stars_core::message::fleet_object(1));
    assert_eq!(told[0].params, vec![1, 1], "the fleet and its new warp");
    assert!(
        !state.messages.iter().any(|m| m.id == id::OUT_OF_FUEL),
        "not stranded, only slowed"
    );
}

/// With a full tank and no cargo the same leg is flown in the year, and
/// nobody is told anything.
#[test]
fn a_tank_that_covers_the_leg_is_left_alone() {
    let mut state = a_freighter(450, 0);
    let mut rng = Rng::randomize(1);
    generate_turn(&mut state, &mut rng);

    let fleet = &state.fleets[0];
    assert_eq!(fleet.position, Point::new(1049, 1000), "arrived");
    assert_eq!(fleet.waypoints.len(), 1, "the leg is done with");
    assert!(fleet.cargo.fuel > 0);
    assert!(!state
        .messages
        .iter()
        .any(|m| m.id == id::OUT_OF_FUEL || m.id == id::OUT_OF_FUEL_SLOWED));
    // And, having nothing to do there, it says its orders are complete —
    // `KillUsedWaypoints` (`1080:189a`).
    let done: Vec<_> = state
        .messages
        .iter()
        .filter(|m| m.id == id::ORDERS_COMPLETE)
        .collect();
    assert_eq!(done.len(), 1, "{:?}", state.messages);
    assert_eq!(done[0].object, stars_core::message::fleet_object(1));
    assert_eq!(done[0].params, vec![1]);
}

/// A last waypoint with a task that reports for itself — Transport here —
/// does not also announce the arrival.
#[test]
fn a_task_at_the_last_waypoint_keeps_the_arrival_quiet() {
    let mut state = a_freighter(450, 0);
    state.fleets[0].waypoints[1].task = stars_formats::task::TRANSPORT;
    let mut rng = Rng::randomize(1);
    generate_turn(&mut state, &mut rng);
    assert_eq!(state.fleets[0].position, Point::new(1049, 1000), "arrived");
    assert!(
        !state.messages.iter().any(|m| m.id == id::ORDERS_COMPLETE),
        "{:?}",
        state.messages
    );
}

/// A tank that empties exactly on the waypoint is an arrival, not a
/// breakdown: the fleet is where it was going, so there is no leg to slow.
#[test]
fn arriving_on_the_last_drop_is_not_running_dry() {
    let mut state = a_freighter(450, 0);
    let designs = state.designs[0].clone();
    let exact = state.fleets[0].fuel_use(&designs, 7, 49, false);
    state.fleets[0].cargo.fuel = exact;
    let mut rng = Rng::randomize(1);
    generate_turn(&mut state, &mut rng);

    let fleet = &state.fleets[0];
    assert_eq!(fleet.position, Point::new(1049, 1000), "arrived");
    assert!(!state
        .messages
        .iter()
        .any(|m| m.id == id::OUT_OF_FUEL || m.id == id::OUT_OF_FUEL_SLOWED));
}
