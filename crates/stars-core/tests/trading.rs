//! Going through a wormhole, and trading with the Mystery Trader.
//!
//! The two things the wanderers are *for*, both run from a generated turn: a
//! fleet ordered to a wormhole comes out of the far end during movement, and a
//! fleet that has come to rest on the Trader is absorbed by it in exchange for
//! technology. See `docs/formulas/wanderers.md`.

use stars_core::components::slot;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_core::fleet::{Cargo, Fleet, ShipStack, Waypoint};
use stars_core::message::id;
use stars_core::movement::Point;
use stars_core::wormhole::{part, tech_levels, Gift, MysteryTrader, Wormhole};
use stars_core::{generate_turn, GameState, Player, Race, Rng};

/// A one-player galaxy with a single ship that can fly.
fn a_galaxy() -> GameState {
    let mut state = GameState::new(7);
    state.players = vec![Player::new(Race::humanoid())];
    state.designs = vec![vec![ShipDesign {
        name: "Scout".to_string(),
        picture: 0,
        stored_armor: 0,
        hull_id: 0,
        slots: vec![DesignSlot {
            category: slot::ENGINE,
            item: 1,
            count: 1,
        }],
    }]];
    state.galaxy_planets = 2;
    state
}

/// A fleet of one ship, at `at`.
fn a_fleet(at: Point) -> Fleet {
    Fleet {
        name: None,
        repeat_orders: false,
        id: 1,
        owner: 0,
        position: at,
        orbiting: None,
        stacks: vec![ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo {
            fuel: 10_000,
            ..Cargo::default()
        },
        battle_plan: 0,
        warp: None,
        waypoints: vec![Waypoint {
            position: at,
            target: None,
            target_class: 4,
            warp: 0,
            task: stars_formats::task::NONE,
            transport: None,
            task_data: Vec::new(),
        }],
    }
}

/// A wormhole end.
fn an_end(id: u16, at: Point, partner: u16) -> Wormhole {
    Wormhole {
        id,
        position: at,
        stability: 0,
        years_still: 0,
        dest_known: false,
        include: true,
        detected_by: 0,
        traversed_by: 0,
        partner,
        turn: 0,
    }
}

/// The Trader, carrying `part`.
fn a_trader(at: Point, part: u16) -> MysteryTrader {
    MysteryTrader {
        id: 1,
        position: at,
        destination: at,
        warp: 13, // fast enough that it never changes course
        include: true,
        detected_by: 0,
        part,
        turn: 0,
    }
}

/// A fleet ordered to a wormhole comes out of the far end.
#[test]
fn a_fleet_goes_through_a_wormhole() {
    let mut state = a_galaxy();
    let near = Point::new(1000, 1000);
    let far = Point::new(3000, 3000);
    state.wormholes = vec![an_end(1, near, 2), an_end(2, far, 1)];

    let mut fleet = a_fleet(Point::new(950, 1000));
    fleet.warp = Some(8);
    fleet.waypoints.push(Waypoint {
        position: near,
        target: Some(1),
        // `grobj` 8: the waypoint names a thing, not a planet.
        target_class: 8,
        warp: 8,
        task: stars_formats::task::NONE,
        transport: None,
        task_data: Vec::new(),
    });
    state.fleets = vec![fleet];

    let mut rng = Rng::from_seeds(1, 2);
    let report = generate_turn(&mut state, &mut rng);

    assert_eq!(report.wormhole_trips, vec![(1, 1, 2)]);
    // It came out where the far end was when it went in. The ends wander
    // afterwards; the fleet does not go with them.
    assert_eq!(state.fleets[0].position, far);
    assert_eq!(state.fleets[0].orbiting, None);
    // Both ends remember the traveller, and the far end is now in view.
    assert_eq!(state.wormholes[0].traversed_by, 1);
    assert_eq!(state.wormholes[1].traversed_by, 1);
    assert_eq!(state.wormholes[1].detected_by, 1);
}

/// A fleet that stops short of the wormhole stays where it is.
#[test]
fn a_fleet_that_has_not_arrived_stays_put() {
    let mut state = a_galaxy();
    let near = Point::new(2000, 1000);
    state.wormholes = vec![an_end(1, near, 2), an_end(2, Point::new(3000, 3000), 1)];

    let mut fleet = a_fleet(Point::new(1000, 1000));
    fleet.warp = Some(4);
    fleet.waypoints.push(Waypoint {
        position: near,
        target: Some(1),
        target_class: 8,
        warp: 4,
        task: stars_formats::task::NONE,
        transport: None,
        task_data: Vec::new(),
    });
    state.fleets = vec![fleet];

    let mut rng = Rng::from_seeds(1, 2);
    let report = generate_turn(&mut state, &mut rng);
    assert!(report.wormhole_trips.is_empty());
    assert!(state.fleets[0].position.x < 2000);
    assert_eq!(state.wormholes[0].traversed_by, 0);
}

/// The Trader hands over the technology it is carrying, and keeps the fleet.
#[test]
fn the_trader_gives_what_it_carries() {
    let mut state = a_galaxy();
    let at = Point::new(2000, 2000);
    state.trader = Some(a_trader(at, part::SHIELD));
    let mut fleet = a_fleet(at);
    fleet.cargo.minerals = [2000, 2000, 1000];
    state.fleets = vec![fleet];

    let mut rng = Rng::from_seeds(1, 2);
    let report = generate_turn(&mut state, &mut rng);

    assert_eq!(report.trades, vec![(1, Gift::Part(part::SHIELD))]);
    assert_eq!(state.players[0].trader_parts, part::SHIELD);
    // The fleet is gone: the Trader absorbed it, cargo and all.
    assert!(state.fleets[0].stacks.is_empty());
    assert_eq!(state.fleets[0].cargo.minerals, [0, 0, 0]);
    // And the Trader will not trade with this player again.
    assert_eq!(state.trader.as_ref().unwrap().detected_by, 1);

    let message = state
        .messages
        .iter()
        .find(|m| m.id == id::TRADER_GAVE_PART)
        .expect("the player is told what they were given");
    // 0xC206: a shield, the seventh of its kind.
    assert_eq!(message.object as u16, 0xC206);
}

/// A Trader carrying nothing in particular gives research instead.
#[test]
fn the_trader_gives_technology() {
    let mut state = a_galaxy();
    let at = Point::new(2000, 2000);
    state.trader = Some(a_trader(at, 0));
    let mut fleet = a_fleet(at);
    // Ten thousand kilotons: five thousand buys the audience and the rest buys
    // four more levels.
    fleet.cargo.minerals = [4000, 3000, 3000];
    state.fleets = vec![fleet];

    let mut rng = Rng::from_seeds(1, 2);
    let report = generate_turn(&mut state, &mut rng);

    assert_eq!(tech_levels(10_000, 0), 10);
    assert_eq!(report.trades, vec![(1, Gift::Tech(10))]);
    let levels: i16 = state.players[0]
        .research
        .levels
        .iter()
        .map(|l| i16::from(*l))
        .sum();
    assert_eq!(levels, 10, "ten levels, spread over the six fields");
    assert!(state
        .messages
        .iter()
        .any(|m| m.id == id::TRADER_GAVE_TECH && m.params.get(1) == Some(&10)));
}

/// One Trader trades once with each player, and turns away a fleet that has
/// not brought enough.
#[test]
fn the_trader_trades_once_and_wants_paying() {
    let mut state = a_galaxy();
    let at = Point::new(2000, 2000);
    state.trader = Some(a_trader(at, part::MINER));

    let mut rich = a_fleet(at);
    rich.cargo.minerals = [2000, 2000, 1000];
    let mut poor = a_fleet(at);
    poor.id = 2;
    poor.cargo.minerals = [2000, 2000, 1000];
    let mut empty = a_fleet(at);
    empty.id = 3;
    empty.cargo.minerals = [10, 10, 10];
    // It arrived this year, so it is told; a fleet already sitting here is not
    // told again.
    empty.warp = Some(5);
    state.fleets = vec![rich, poor, empty];

    let mut rng = Rng::from_seeds(1, 2);
    let report = generate_turn(&mut state, &mut rng);

    assert_eq!(report.trades[0], (1, Gift::Part(part::MINER)));
    assert_eq!(report.trades[1], (2, Gift::AlreadyMet));
    assert_eq!(report.trades[2], (3, Gift::Refused));
    // The second fleet was not taken.
    assert!(!state.fleets[1].stacks.is_empty());
    assert!(state
        .messages
        .iter()
        .any(|m| m.id == id::TRADER_REFUSED && m.params.first() == Some(&3)));
}
