//! The scanner's status bar — `DrawScannerSBar` (`1058:62d8`).
//!
//! See `docs/ui/scanner.md`.

use stars_core::fleet::{Fleet, ShipStack, Waypoint};
use stars_core::minefield::Minefield;
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::packet::Packet;
use stars_core::{opponents, Race};
use stars_ui::statusbar::{Distance, StatusBar};
use stars_ui::{App, ScanObject, ScanThing, Selection};

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "sbar".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.selection = Selection::default();
    app
}

fn a_fleet(owner: i16, at: Point, legs: &[Point]) -> Fleet {
    Fleet {
        id: 0,
        owner,
        position: at,
        orbiting: None,
        stacks: vec![ShipStack {
            design: 0,
            count: 1,
            damaged_pct: 0,
            damage_pct: 0,
        }],
        cargo: stars_core::fleet::Cargo::default(),
        battle_plan: 0,
        warp: Some(6),
        waypoints: legs
            .iter()
            .copied()
            .map(|position| Waypoint {
                position,
                target: None,
                target_class: 4,
                warp: 6,
                task: 0,
                transport: None,
                task_data: Vec::new(),
            })
            .collect(),
        name: None,
        repeat_orders: false,
        direction: None,
    }
}

/// Only a planet fills the id cell, and the number it shows is the stored
/// index **plus one** (`"ID #%d"` with `idpl + 1`).
///
/// `MANUAL.PDF` p. 5-16 says the same from the outside: an ID# for a planet,
/// only coordinates and a name for a fleet or an object.
#[test]
fn only_a_planet_gets_an_id() {
    let mut app = a_game();
    let (home, at) = {
        let game = app.game.as_ref().expect("a game");
        let home = game
            .planets
            .iter()
            .find(|p| p.homeworld && p.owner == Some(0))
            .expect("a home world");
        (home.id, home.position.expect("a position"))
    };

    app.select_object(ScanObject::Planet(home));
    let bar = app.status_bar();
    assert_eq!(bar.id, format!("ID #{}", home + 1));
    assert_eq!(bar.name, app.planet_name(home));
    assert_eq!(bar.x, format!("X: {}", at.x));
    assert_eq!(bar.y, format!("Y: {}", at.y));
    assert!(bar.distance.is_none(), "nothing to measure from");

    // A fleet out in space: coordinates and a name, and no id.
    let out = Point::new(at.x + 60, at.y + 60);
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![a_fleet(0, out, &[out])];
    }
    app.select_object(ScanObject::Fleet(0));
    let bar = app.status_bar();
    assert_eq!(bar.id, "");
    assert_eq!(bar.name, app.fleet_display_name(0));
    assert_eq!(bar.x, format!("X: {}", out.x));
}

/// A fleet in orbit shows the **planet's** information, which is what
/// `ChangeScanSel` forces by turning the scan into the planet whenever the
/// point has one (`MANUAL.PDF` p. 5-16).
#[test]
fn a_fleet_in_orbit_shows_the_planet() {
    let mut app = a_game();
    let (home, at) = {
        let game = app.game.as_ref().expect("a game");
        let home = game
            .planets
            .iter()
            .find(|p| p.homeworld && p.owner == Some(0))
            .expect("a home world");
        (home.id, home.position.expect("a position"))
    };
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![a_fleet(0, at, &[at])];
    }
    app.select_object(ScanObject::Fleet(0));
    let bar = app.status_bar();
    assert_eq!(bar.id, format!("ID #{}", home + 1));
    assert_eq!(bar.name, app.planet_name(home));
}

/// A coordinate is printed only when it is **positive**: the original's
/// guards are `if (0 < x)` and `if (0 < y)`.
#[test]
fn a_zero_coordinate_leaves_its_cell_empty() {
    let mut bar = StatusBar::default();
    bar.place(Point::new(0, 40));
    assert_eq!(bar.x, "");
    assert_eq!(bar.y, "Y: 40");
}

/// The unit is chosen by the scanner's width, not by the abbreviation
/// `PszGetDistance` itself prints, and the `from` clause is added only when
/// the caller did not say where it was measuring from.
#[test]
fn the_distance_is_worded_by_the_windows_width() {
    let plain = Distance {
        figure: "12.5".to_string(),
        from: None,
    };
    assert_eq!(plain.text(false), "12.5 ly");
    assert_eq!(plain.text(true), "12.5 light years");

    let named = Distance {
        figure: "12.5".to_string(),
        from: Some("Home World".to_string()),
    };
    assert_eq!(named.text(false), "12.5 ly from Home World");
}

/// Dragging a waypoint puts `WP #n` in the id cell, `Deep Space Waypoint` in
/// the name cell, and the distance back to the fleet — named, because
/// `FHandleWayPointDrag` passes no anchor scan.
#[test]
fn a_dragged_waypoint_measures_back_to_the_fleet() {
    let mut app = a_game();
    let from = Point::new(100, 100);
    let to = Point::new(160, 180);
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![a_fleet(0, from, &[from, to])];
    }
    app.select_object(ScanObject::Fleet(0));
    app.dragging_waypoint = Some(1);

    let bar = app.status_bar();
    assert_eq!(bar.id, "WP #1");
    assert_eq!(bar.name, "Deep Space Waypoint");
    assert_eq!(bar.x, "X: 160");
    let measured = bar.distance.expect("a distance");
    // 60-80-100: the leg is exactly a hundred light years.
    assert_eq!(measured.figure, "100.0");
    assert_eq!(
        measured.from.as_deref(),
        Some(app.fleet_display_name(0)).as_deref()
    );
    assert_eq!(
        measured.text(true),
        format!("100.0 light years from {}", app.fleet_display_name(0))
    );
}

/// Space objects are named the way `PszGetThingName` names them: the kind is
/// an adjective in front of `Mine Field`, a packet aimed at no planet is
/// `Salvage`, and the owner is named only when it is somebody else's.
#[test]
fn space_objects_are_named_the_way_the_game_names_them() {
    let mut app = a_game();
    let at = Point::new(200, 200);
    if let Some(game) = app.game.as_mut() {
        game.minefields = vec![
            Minefield {
                id: 1,
                owner: 0,
                position: at,
                mines: 400,
                kind: 0,
                detonating: false,
                detected_by: 0,
                visible_to: 0,
                turn: 0,
            },
            Minefield {
                id: 2,
                owner: 1,
                position: Point::new(210, 200),
                mines: 400,
                kind: 2,
                detonating: false,
                detected_by: 0,
                visible_to: 0,
                turn: 0,
            },
        ];
        game.packets = vec![
            Packet {
                id: 3,
                owner: 0,
                position: Point::new(220, 200),
                target: 7,
                warp: 3,
                minerals: [100, 0, 0],
                decay_rate: 0,
                moved: false,
                include: true,
                turn: 0,
            },
            Packet {
                id: 4,
                owner: 0,
                position: Point::new(230, 200),
                target: 0,
                warp: 0,
                minerals: [10, 0, 0],
                decay_rate: 0,
                moved: false,
                include: true,
                turn: 0,
            },
        ];
    }
    let other = app.player_name(1);
    assert_eq!(
        app.thing_name(ScanThing::Minefield(0)),
        "Standard Mine Field"
    );
    assert_eq!(
        app.thing_name(ScanThing::Minefield(1)),
        format!("{other} Speed Bump Mine Field")
    );
    assert_eq!(app.thing_name(ScanThing::Packet(0)), "Mineral Packet");
    assert_eq!(app.thing_name(ScanThing::Packet(1)), "Salvage");

    // And the bar names one with no id, since only planets and waypoints get
    // the id cell.
    app.select_object(ScanObject::Thing(ScanThing::Minefield(0)));
    let bar = app.status_bar();
    assert_eq!(bar.id, "");
    assert_eq!(bar.name, "Standard Mine Field");
    assert_eq!(bar.x, "X: 200");
}
