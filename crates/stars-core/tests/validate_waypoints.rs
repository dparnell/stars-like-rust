//! `ValidateWaypoints` (`1038:68c6`) at the year's end: a leg aimed at a
//! fleet that is no longer where it was is aimed at another of that
//! player's fleets still there — the heaviest the chaser's plan would
//! target — and a leg aimed at a wormhole follows it.

use stars_core::components::slot;
use stars_core::design::{DesignSlot, ShipDesign};
use stars_core::fleet::{grobj, Cargo, Fleet, ShipStack, Waypoint};
use stars_core::movement::Point;
use stars_core::race::Race;
use stars_core::wormhole::Wormhole;
use stars_core::{generate_turn, GameState, Player, Rng};

fn design(name: &str, hull_id: i16) -> ShipDesign {
    ShipDesign {
        name: name.to_string(),
        picture: 0,
        stored_armor: 0,
        obsolete: false,
        designed: 0,
        built: 0,
        hull_id,
        slots: vec![DesignSlot {
            category: slot::ENGINE,
            item: 3,
            count: 1,
        }],
    }
}

fn fleet(owner: i16, id: u16, at: Point, design: u8, ships: i32) -> Fleet {
    Fleet {
        name: None,
        repeat_orders: false,
        direction: None,
        id,
        owner,
        position: at,
        orbiting: None,
        stacks: vec![ShipStack {
            design,
            count: ships,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Cargo {
            minerals: [0, 0, 0],
            colonists: 0,
            fuel: 500,
        },
        battle_plan: 0,
        warp: None,
        waypoints: vec![Waypoint {
            position: at,
            target: None,
            target_class: grobj::POSITION,
            warp: 0,
            task: 0,
            transport: None,
            task_data: Vec::new(),
        }],
    }
}

fn two_players() -> GameState {
    let mut state = GameState::new(11);
    state.players = vec![Player::new(Race::humanoid()), Player::new(Race::humanoid())];
    // Player 1's designs: a Scout (armed class) and a Small Freighter.
    state.designs = vec![
        vec![design("Chaser", 4)],
        vec![design("Scout", 4), design("Freighter", 0)],
    ];
    state
}

/// The chaser's quarry has moved on; the heaviest fleet of the quarry's
/// owner still at the spot that the chaser's plan targets is chased
/// instead.
#[test]
fn a_lost_quarry_is_replaced_by_the_heaviest_fleet_left_there() {
    let mut state = two_players();
    let spot = Point::new(1200, 1200);
    let mut chaser = fleet(0, 1, Point::new(1000, 1000), 0, 1);
    // Aimed at player 1's fleet 5, which is not at the spot any more.
    chaser.waypoints.push(Waypoint {
        position: spot,
        target: Some((1 << 9) | 5),
        target_class: grobj::FLEET,
        warp: 0,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    });
    let gone = fleet(1, 5, Point::new(1400, 1400), 0, 1);
    let light = fleet(1, 6, spot, 0, 1);
    let heavy = fleet(1, 7, spot, 0, 5);
    // A freighter there is not what the Default plan (armed ships) wants.
    let freighter = fleet(1, 8, spot, 1, 20);
    state.fleets = vec![chaser, gone, light, heavy, freighter];

    generate_turn(&mut state, &mut Rng::randomize(1));

    let leg = &state.fleets[0].waypoints[1];
    assert_eq!(leg.target_class, grobj::FLEET);
    assert_eq!(leg.target, Some((1 << 9) | 7), "the five-ship fleet");
    assert_eq!(leg.position, spot);
}

/// A leg aimed at a wormhole the player has seen follows it when it
/// jumps; one aimed at a wormhole never seen becomes a point where it was.
#[test]
fn a_leg_at_a_wormhole_follows_it_or_is_left_where_it_was() {
    let mut state = two_players();
    state.galaxy_size = 1;
    let was = Point::new(1300, 1300);
    let now = Point::new(1500, 1100);
    state.wormholes = vec![Wormhole {
        id: 1,
        position: now,
        stability: 3,
        years_still: 0,
        dest_known: false,
        include: true,
        detected_by: 1,
        traversed_by: 0,
        partner: (2 << 13) | 2,
        turn: 0,
    }];
    let mut seen = fleet(0, 1, Point::new(1000, 1000), 0, 1);
    seen.waypoints.push(Waypoint {
        position: was,
        target: Some((2 << 13) | 1),
        target_class: grobj::THING,
        warp: 0,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    });
    let mut unseen = fleet(1, 2, Point::new(1000, 1000), 0, 1);
    unseen.waypoints.push(seen.waypoints[1].clone());
    state.fleets = vec![seen, unseen];

    generate_turn(&mut state, &mut Rng::randomize(1));

    let hole_at = state.wormholes[0].position;
    let followed = &state.fleets[0].waypoints[1];
    assert_eq!(followed.target_class, grobj::THING);
    assert_eq!(followed.position, hole_at, "followed to the hole");
    let left = &state.fleets[1].waypoints[1];
    assert_eq!(left.target_class, grobj::POSITION);
    assert_eq!(left.position, was, "left where it was");
    assert!(state
        .messages
        .iter()
        .any(|m| m.id == stars_core::message::id::WORMHOLE_VANISHED && m.player == 1));
}
