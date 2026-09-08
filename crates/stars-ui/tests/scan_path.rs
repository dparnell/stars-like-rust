//! What the selection draws for itself — `DrawShipScanPath`.
//!
//! See `docs/ui/scanner.md`.

use stars_core::fleet::{Fleet, ShipStack, Waypoint};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{App, ScanObject, ScanThing};

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "scan path".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.selection = Default::default();
    app
}

fn waypoint(at: Point) -> Waypoint {
    Waypoint {
        position: at,
        target: None,
        target_class: 4,
        warp: 6,
        task: 0,
        transport: None,
        task_data: Vec::new(),
    }
}

fn fleet(owner: i16, legs: &[Point]) -> Fleet {
    Fleet {
        id: 1,
        owner,
        position: legs.first().copied().unwrap_or(Point::new(0, 0)),
        orbiting: None,
        stacks: vec![ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: Default::default(),
        battle_plan: 0,
        warp: None,
        waypoints: legs.iter().copied().map(waypoint).collect(),
        name: None,
        repeat_orders: false,
        direction: None,
    }
}

/// A fleet seen at a distance has a course line: five years each way, marked
/// off a year at a time.
#[test]
fn a_scanned_fleet_gets_a_course_line() {
    let mut app = a_game();
    let mut theirs = fleet(1, &[Point::new(100, 100)]);
    theirs.direction = Some((1, 0));
    theirs.warp = Some(7);
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![theirs];
    }
    app.select_object(ScanObject::Fleet(0));

    let line = app.scan_scale_line().expect("a course");
    assert_eq!(line.warp, 7);
    assert_eq!(line.year(), 49, "a year's travel is the square of the warp");
    assert_eq!(line.reach(), 49 * 5, "and the line reaches five years");
}

/// One of your own has no recorded course — you read its waypoints instead.
#[test]
fn your_own_fleet_has_no_course_line() {
    let mut app = a_game();
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![fleet(0, &[Point::new(10, 10), Point::new(40, 40)])];
    }
    app.select_object(ScanObject::Fleet(0));
    assert!(
        app.scan_scale_line().is_none(),
        "no direction and no warp were recorded for it"
    );
    assert_eq!(app.selected_fleet_path().len(), 1, "but it has a path");
}

/// A packet's course points at the planet it was flung at, at its own warp.
#[test]
fn a_packet_aims_at_its_target() {
    let mut app = a_game();
    let (target, at) = {
        let game = app.game.as_ref().expect("a game");
        let planet = game.planets.first().expect("a planet");
        (
            u16::try_from(planet.id).expect("an id"),
            planet.position.expect("a position"),
        )
    };
    let here = Point::new(at.x - 30, at.y);
    if let Some(game) = app.game.as_mut() {
        game.packets = vec![stars_core::packet::Packet {
            id: 1,
            owner: 0,
            position: here,
            target,
            warp: 4,
            minerals: [10, 0, 0],
            decay_rate: 0,
            moved: false,
            include: true,
            turn: 0,
        }];
    }
    app.select_object(ScanObject::Thing(ScanThing::Packet(0)));

    let line = app.scan_scale_line().expect("a course");
    assert_eq!(line.warp, 8, "the stored nibble is the warp less four");
    assert_eq!(line.heading, (30, 0), "straight at the planet");

    // A packet sitting still is not going anywhere.
    if let Some(game) = app.game.as_mut() {
        game.packets[0].warp = 0;
    }
    assert!(app.scan_scale_line().is_none());
}

/// A leg the fleet travels twice is drawn once, in yellow.
#[test]
fn a_doubled_leg_is_drawn_once() {
    let mut app = a_game();
    let a = Point::new(10, 10);
    let b = Point::new(40, 10);
    let c = Point::new(70, 10);
    // Out to b, on to c, back to b: the b–c leg is travelled twice.
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![fleet(0, &[a, b, c, b])];
    }
    app.select_object(ScanObject::Fleet(0));

    let legs = app.selected_fleet_path();
    assert_eq!(legs.len(), 2, "three legs, one of them a repeat: {legs:?}");
    assert_eq!((legs[0].from, legs[0].to), (a, b));
    assert!(!legs[0].doubled);
    assert_eq!((legs[1].from, legs[1].to), (b, c));
    assert!(legs[1].doubled, "the leg it travels twice");
}

/// Nothing selected, nothing drawn.
#[test]
fn an_empty_selection_draws_nothing() {
    let app = a_game();
    assert!(app.scan_scale_line().is_none());
    assert!(app.selected_fleet_path().is_empty());
    assert!(app.planet_route_line().is_none());
    assert!(app.planet_driver_line().is_none());
}

/// A planet whose starbase has a mass driver aimed somewhere shows a line to
/// it, and a planet with a route as well shows both.
#[test]
fn a_mass_driver_and_a_route_are_two_lines() {
    let mut app = a_game();
    let (from_id, from_at, to_id, to_at) = {
        let game = app.game.as_ref().expect("a game");
        let mut all = game.planets.iter().filter(|p| p.position.is_some());
        let a = all.next().expect("a planet");
        let b = all.next().expect("another planet");
        (
            a.id,
            a.position.expect("a position"),
            b.id,
            b.position.expect("a position"),
        )
    };
    if let Some(game) = app.game.as_mut() {
        if let Some(planet) = game.planets.iter_mut().find(|p| p.id == from_id) {
            planet.starbase = true;
            planet.fling_dest = Some(to_id);
            planet.route_dest = Some(to_id);
        }
    }
    app.select_object(ScanObject::Planet(from_id));
    assert_eq!(app.planet_driver_line(), Some((from_at, to_at)));
    assert_eq!(
        app.planet_route_line(),
        Some((from_at, to_at)),
        "the route line is drawn as well as the driver's"
    );

    // No starbase, no driver line — the driver belongs to the base.
    if let Some(game) = app.game.as_mut() {
        if let Some(planet) = game.planets.iter_mut().find(|p| p.id == from_id) {
            planet.starbase = false;
        }
    }
    assert!(app.planet_driver_line().is_none());
}

/// A planet set to route its new fleets somewhere shows the line.
#[test]
fn a_route_is_a_line_between_two_planets() {
    let mut app = a_game();
    let (from_id, from_at, to_id, to_at) = {
        let game = app.game.as_ref().expect("a game");
        let mut owned = game.planets.iter().filter(|p| p.position.is_some());
        let a = owned.next().expect("a planet");
        let b = owned.next().expect("another planet");
        (
            a.id,
            a.position.expect("a position"),
            b.id,
            b.position.expect("a position"),
        )
    };
    if let Some(game) = app.game.as_mut() {
        if let Some(planet) = game.planets.iter_mut().find(|p| p.id == from_id) {
            planet.route_dest = Some(to_id);
        }
    }
    app.select_object(ScanObject::Planet(from_id));
    assert_eq!(app.planet_route_line(), Some((from_at, to_at)));

    // And none when the planet routes nowhere.
    if let Some(game) = app.game.as_mut() {
        if let Some(planet) = game.planets.iter_mut().find(|p| p.id == from_id) {
            planet.route_dest = None;
        }
    }
    assert!(app.planet_route_line().is_none());
}
