//! The minefield subsystem, end to end through a generated turn.
//!
//! Laying is a waypoint task, so it runs where the original runs it — after
//! movement, in the same pass as remote mining — and what it produces is a
//! field that other players' fleets then have to fly around. See
//! `docs/formulas/minefields.md`.

use stars_core::components::slot;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_core::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use stars_core::minefield::Minefield;
use stars_core::movement::Point;
use stars_core::planet::Planet;
use stars_core::race::{Prt, Race};
use stars_core::{generate_turn, GameState, Player, Rng};

/// A one-player game with a minelayer sitting still, ordered to lay.
fn a_layer(years: u16, prt: Prt) -> GameState {
    let mut state = GameState::new(7);
    let mut race = Race::humanoid();
    race.attrs[stars_core::race::RaceStat::MajorAdv as usize] = prt as i16;
    state.players = vec![Player::new(race)];
    state.designs = vec![vec![ShipDesign {
        name: "Layer".to_string(),
        picture: 0,
        stored_armor: 0,
        hull_id: 0,
        slots: vec![
            DesignSlot {
                category: slot::ENGINE,
                item: 1, // something that will actually fly
                count: 1,
            },
            DesignSlot {
                category: slot::MINES,
                item: 0, // Mine Dispenser 40
                count: 1,
            },
        ],
    }]];
    let mut planet = Planet::unowned(1);
    planet.owner = Some(0);
    planet.position = Some(Point::new(1000, 1000));
    planet.pop = 25_000;
    state.planets = vec![planet];

    state.fleets = vec![Fleet {
        name: None,
        repeat_orders: false,
        id: 1,
        owner: 0,
        position: Point::new(1000, 1000),
        orbiting: Some(1),
        stacks: vec![ShipStack {
            design: 0,
            count: 2,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo::default(),
        battle_plan: 0,
        warp: None,
        waypoints: vec![Waypoint {
            position: Point::new(1000, 1000),
            target: Some(1),
            target_class: 1,
            warp: 0,
            task: stars_formats::task::LAY_MINES,
            transport: None,
            task_data: years.to_le_bytes().to_vec(),
        }],
    }];
    state
}

/// Two ships with a Mine Dispenser 40 apiece lay eighty mines a year, into the
/// same field year after year.
#[test]
fn a_fleet_lays_mines_where_it_sits() {
    let mut state = a_layer(5, Prt::Joat);
    let mut rng = Rng::from_seeds(1, 2);

    let report = generate_turn(&mut state, &mut rng);
    assert_eq!(report.mines_laid, vec![(1, 0, 80)]);
    assert_eq!(state.minefields.len(), 1);
    assert_eq!(state.minefields[0].mines, 80);
    assert_eq!(state.minefields[0].owner, 0);
    assert_eq!(state.minefields[0].position, Point::new(1000, 1000));

    // A second year adds to the same field rather than starting another.
    let report = generate_turn(&mut state, &mut rng);
    assert_eq!(report.mines_laid, vec![(1, 0, 80)]);
    assert_eq!(state.minefields.len(), 1);
    assert_eq!(state.minefields[0].mines, 160);
    // "Indefinitely" is never spent.
    assert_eq!(
        state.fleets[0].waypoints[0].task,
        stars_formats::task::LAY_MINES
    );
}

/// A countdown of years runs out and the order clears itself.
#[test]
fn a_counted_order_runs_out() {
    let mut state = a_layer(2, Prt::Joat);
    let mut rng = Rng::from_seeds(1, 2);

    generate_turn(&mut state, &mut rng);
    assert_eq!(&state.fleets[0].waypoints[0].task_data[0..2], &[1, 0]);
    generate_turn(&mut state, &mut rng);
    assert_eq!(&state.fleets[0].waypoints[0].task_data[0..2], &[0, 0]);
    // The next year finds a zero and gives up the order.
    generate_turn(&mut state, &mut rng);
    assert_eq!(state.fleets[0].waypoints[0].task, stars_formats::task::NONE);
    assert_eq!(state.minefields[0].mines, 240, "three years of laying");
}

/// Everybody but Space Demolition has to sit still to lay.
#[test]
fn only_space_demolition_lays_on_the_move() {
    for (prt, expect) in [(Prt::Joat, 0), (Prt::Sd, 40)] {
        let mut state = a_layer(5, prt);
        // A fleet with a warp set is one that moved this year.
        state.fleets[0].warp = Some(6);
        let mut rng = Rng::from_seeds(1, 2);
        let report = generate_turn(&mut state, &mut rng);
        let laid: i32 = report.mines_laid.iter().map(|(_, _, n)| n).sum();
        assert_eq!(laid, expect, "{prt:?} laying while moving");
        // And the Space Demolition fleet lays half of the eighty it would
        // standing still.
    }
}

/// A fleet crossing somebody else's field at speed is stopped and damaged.
#[test]
fn a_field_stops_a_fleet_that_flies_into_it() {
    let mut state = a_layer(5, Prt::Joat);
    state.players.push(Player::new(Race::humanoid()));
    state.designs.push(state.designs[0].clone());
    // Player 1's field lies across player 0's course.
    state.minefields.push(Minefield {
        id: 0,
        owner: 1,
        position: Point::new(1100, 1000),
        mines: 10_000, // radius 100
        kind: 1,       // heavy: 500 a ship, and a 3% chance a light year at warp 9
        detonating: false,
        detected_by: 0,
        visible_to: 0,
        turn: 0,
    });
    // Send the fleet 200 light years east at warp 9, straight through it.
    let fleet = &mut state.fleets[0];
    fleet.waypoints[0].task = stars_formats::task::NONE;
    fleet.warp = Some(9);
    fleet.orbiting = None;
    fleet.cargo.fuel = 10_000;
    fleet.waypoints.push(Waypoint {
        position: Point::new(1200, 1000),
        target: None,
        target_class: 4,
        warp: 9,
        task: stars_formats::task::NONE,
        transport: None,
        task_data: Vec::new(),
    });

    let mut rng = Rng::from_seeds(11, 22);
    let report = generate_turn(&mut state, &mut rng);
    // Eighty-one light years at warp 9 through a heavy field is a 3% roll a
    // light year, so a miss is under a ten-thousandth as likely as a hit; the
    // seed is fixed regardless.
    let hit = report
        .mine_hits
        .first()
        .expect("81 light years at warp 9 through a hundred-light-year field");
    assert_eq!(hit.1.field_owner, 1);
    assert_eq!(hit.1.kind, 1);
    assert_eq!(hit.1.damage, 2000, "two ships, so the minimum applies");
    // It stopped where the mines caught it, short of its waypoint.
    assert!(state.fleets[0].position.x < 1200);
}
