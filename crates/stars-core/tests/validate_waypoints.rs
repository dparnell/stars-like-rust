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
        slots: vec![
            DesignSlot {
                category: slot::ENGINE,
                item: 3,
                count: 1,
            },
            // A Peerless Scanner, 500 light years: the year's end lets a
            // leg chase only what is on the player's map.
            DesignSlot {
                category: slot::SCANNER,
                item: 15,
                count: 1,
            },
        ],
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
    // A tiny universe, so the hole stays within the scanners' reach
    // wherever it wanders: the year's end keeps a leg aimed at a wormhole
    // only while the wormhole is on the player's map.
    state.galaxy_size = 0;
    let was = Point::new(1300, 1300);
    let now = Point::new(1350, 1100);
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
    let mut seen = fleet(0, 1, Point::new(1200, 1200), 0, 1);
    seen.waypoints.push(Waypoint {
        position: was,
        target: Some((2 << 13) | 1),
        target_class: grobj::THING,
        warp: 0,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    });
    let mut unseen = fleet(1, 2, Point::new(1200, 1200), 0, 1);
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

/// The look a player's orders get as their file is written
/// (`FWriteDataFile`, `1070:5964`): a leg chasing a fleet that has slipped
/// off their map ends where the fleet was last seen — at the planet it
/// ducked behind, if that is where it is — and one chasing a fleet that is
/// gone likewise, each with a word to the player.
#[test]
fn a_chase_ends_where_the_quarry_was_last_seen() {
    use stars_core::message::id;
    use stars_core::planet::Planet;

    let mut state = two_players();
    state.galaxy_size = 4;
    // Player 1's designs carry no scanner at all, so player 0's fleets
    // are never on their map; player 0's Peerless sees five hundred.
    for design in &mut state.designs[1] {
        design.slots.retain(|s| s.category != slot::SCANNER);
    }
    let mut hidden = Planet::unowned(3);
    hidden.position = Some(Point::new(2400, 1000));
    hidden.owner = Some(1);
    hidden.pop = 1000;
    state.planets = vec![hidden];

    let chase = |at: Point, target: u16| Waypoint {
        position: at,
        target: Some(target),
        target_class: grobj::FLEET,
        warp: 0,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    };
    // Player 1 chasing player 0's fleet 1, which is in space and out of
    // their (nonexistent) scanners' reach: outrun.
    let mut outrun = fleet(1, 1, Point::new(1000, 1000), 0, 1);
    outrun.waypoints.push(chase(Point::new(1200, 1000), 1));
    // Player 1 chasing player 0's fleet 2, sitting at player 1's own
    // planet — which puts it on their map; the chase goes on.
    let mut kept = fleet(1, 2, Point::new(1000, 1000), 0, 1);
    kept.waypoints.push(chase(Point::new(2400, 1000), 2));
    // Player 0 chasing player 1's fleet 4, which is gone.
    let mut bereft = fleet(0, 3, Point::new(1000, 1000), 0, 1);
    bereft
        .waypoints
        .push(chase(Point::new(1300, 1300), (1 << 9) | 4));
    let quarry_in_space = fleet(0, 1, Point::new(1200, 1000), 0, 1);
    let mut quarry_at_planet = fleet(0, 2, Point::new(2400, 1000), 0, 1);
    quarry_at_planet.orbiting = Some(3);
    state.fleets = vec![outrun, kept, bereft, quarry_in_space, quarry_at_planet];

    generate_turn(&mut state, &mut Rng::randomize(1));

    let outrun = &state.fleets[0].waypoints[1];
    assert_eq!(outrun.target_class, grobj::POSITION);
    assert_eq!(outrun.position, Point::new(1200, 1000));
    let kept = &state.fleets[1].waypoints[1];
    assert_eq!(kept.target_class, grobj::FLEET);
    let bereft = &state.fleets[2].waypoints[1];
    assert_eq!(bereft.target_class, grobj::POSITION);
    let ids: Vec<(usize, u16)> = state
        .messages
        .iter()
        .filter(|m| {
            [
                id::CHASED_FLEET_OUTRUN,
                id::CHASED_FLEET_DUCKED,
                id::CHASED_FLEET_GONE,
            ]
            .contains(&m.id)
        })
        .map(|m| (m.player, m.id))
        .collect();
    assert_eq!(
        ids,
        vec![(0, id::CHASED_FLEET_GONE), (1, id::CHASED_FLEET_OUTRUN)]
    );

    // The same fleet ducking behind a planet the chaser cannot see into:
    // the leg is aimed at the planet.
    let mut state = two_players();
    state.galaxy_size = 4;
    for design in &mut state.designs[1] {
        design.slots.retain(|s| s.category != slot::SCANNER);
    }
    let mut planet = Planet::unowned(3);
    planet.position = Some(Point::new(1200, 1000));
    state.planets = vec![planet];
    let mut chaser = fleet(1, 1, Point::new(1000, 1000), 0, 1);
    chaser.waypoints.push(chase(Point::new(1200, 1000), 1));
    let mut quarry = fleet(0, 1, Point::new(1200, 1000), 0, 1);
    quarry.orbiting = Some(3);
    state.fleets = vec![chaser, quarry];
    generate_turn(&mut state, &mut Rng::randomize(1));
    let leg = &state.fleets[0].waypoints[1];
    assert_eq!(leg.target_class, grobj::PLANET);
    assert_eq!(leg.target, Some(3));
    assert!(state
        .messages
        .iter()
        .any(|m| m.id == id::CHASED_FLEET_DUCKED
            && m.player == 1
            && m.params == vec![stars_core::message::fleet_object(1), 3]));
}

/// A quarry that goes through a wormhole is not followed: the chaser's leg
/// is marked `fNoAutoTrack` and fixed where the quarry went in
/// (`NoAutoTrackFleet`, `1080:1d32`), the year's end does not re-aim it,
/// and the player is told the fleet has outrun them.
#[test]
fn a_quarry_through_a_wormhole_is_not_followed() {
    use stars_core::message::id;

    let mut state = two_players();
    state.galaxy_size = 4;
    let near = Point::new(1100, 1000);
    let far = Point::new(2500, 2500);
    state.wormholes = vec![
        Wormhole {
            id: 1,
            position: near,
            stability: 3,
            years_still: 0,
            dest_known: false,
            include: true,
            detected_by: 0b11,
            traversed_by: 0,
            partner: (2 << 13) | 2,
            turn: 0,
        },
        Wormhole {
            id: 2,
            position: far,
            stability: 3,
            years_still: 0,
            dest_known: false,
            include: true,
            detected_by: 0,
            traversed_by: 0,
            partner: (2 << 13) | 1,
            turn: 0,
        },
    ];
    let mut quarry = fleet(1, 5, Point::new(1090, 1000), 0, 1);
    quarry.warp = Some(6);
    quarry.waypoints.push(Waypoint {
        position: near,
        target: Some((2 << 13) | 1),
        target_class: grobj::THING,
        warp: 6,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    });
    let mut chaser = fleet(0, 1, Point::new(1000, 1000), 0, 1);
    chaser.warp = Some(6);
    chaser.waypoints.push(Waypoint {
        position: quarry.position,
        target: Some((1 << 9) | 5),
        target_class: grobj::FLEET,
        warp: 6,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    });
    state.fleets = vec![chaser, quarry];

    let report = generate_turn(&mut state, &mut Rng::randomize(1));
    assert_eq!(report.wormhole_trips.len(), 1, "{report:?}");
    assert_eq!(state.fleets[1].position, far);
    let leg = &state.fleets[0].waypoints[1];
    assert_eq!(leg.position, near, "fixed where the quarry went in");
    assert_eq!(leg.target_class, grobj::POSITION);
    assert!(state
        .messages
        .iter()
        .any(|m| m.id == id::CHASED_FLEET_OUTRUN && m.player == 0));
    assert!(state.no_auto_track.is_empty(), "the mark is spent");
}
