//! Which way a fleet's arrow points.
//!
//! See `docs/ui/scanner.md`.

use std::path::PathBuf;

use stars_core::fleet::{Fleet, ShipStack, Waypoint};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{fleet_arrow, App};

fn executable() -> Option<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    for name in ["stars.2.7j.exe", "stars.exe", "STARS!.EXE"] {
        if let Ok(bytes) = std::fs::read(root.join(name)) {
            return Some(bytes);
        }
    }
    None
}

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "arrows".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app
}

fn fleet(owner: i16, at: Point) -> Fleet {
    Fleet {
        id: 1,
        owner,
        position: at,
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
        waypoints: Vec::new(),
        name: None,
        repeat_orders: false,
        direction: None,
    }
}

fn waypoint(at: Point, warp: u8) -> Waypoint {
    Waypoint {
        position: at,
        target: None,
        target_class: 4,
        warp,
        task: 0,
        transport: Default::default(),
        task_data: Vec::new(),
    }
}

/// The eight octants, anticlockwise from south-west.
#[test]
fn the_eight_octants_are_in_the_sheet_s_order() {
    for (dx, dy, want, name) in [
        (-1, -1, 0, "SW"),
        (-1, 0, 1, "W"),
        (-1, 1, 2, "NW"),
        (0, 1, 3, "N"),
        (1, 1, 4, "NE"),
        (1, 0, 5, "E"),
        (1, -1, 6, "SE"),
        (0, -1, 7, "S"),
    ] {
        assert_eq!(fleet_arrow(dx, dy), want, "{name} ({dx}, {dy})");
        // Distance does not matter, only direction.
        assert_eq!(fleet_arrow(dx * 97, dy * 97), want, "{name}, far away");
    }

    // Every direction lands on one of the eight, and each of the eight is
    // reachable.
    let mut seen = [false; 8];
    for degrees in 0..360 {
        let radians = f64::from(degrees).to_radians();
        #[allow(clippy::cast_possible_truncation)]
        let (dx, dy) = (
            (radians.cos() * 1000.0) as i16,
            (radians.sin() * 1000.0) as i16,
        );
        let arrow = fleet_arrow(dx, dy);
        assert!(arrow < 8, "{degrees}° gave {arrow}");
        seen[usize::from(arrow)] = true;
    }
    assert!(seen.iter().all(|s| *s), "all eight are used: {seen:?}");
}

/// A fleet going nowhere gets arrow 0 — **the same picture as one heading
/// south-west**. The original sets the index to zero before it looks at the
/// angle and never tells the two apart.
#[test]
fn a_fleet_going_nowhere_shares_the_south_west_arrow() {
    assert_eq!(fleet_arrow(0, 0), 0);
    assert_eq!(fleet_arrow(-1, -1), 0, "and so does south-west");
}

/// A player's own fleet points along its next leg, and only when the leg has
/// a warp set.
#[test]
fn your_own_fleet_points_along_its_next_leg() {
    let app = a_game();
    let here = Point::new(100, 100);
    let mut mine = fleet(0, here);

    // No orders at all: nothing to point along.
    assert_eq!(app.fleet_arrow_of(&mine), 0);

    // A leg going east.
    mine.waypoints = vec![waypoint(here, 0), waypoint(Point::new(140, 100), 5)];
    assert_eq!(app.fleet_arrow_of(&mine), 5, "east");

    // North-west.
    mine.waypoints[1] = waypoint(Point::new(60, 140), 5);
    assert_eq!(app.fleet_arrow_of(&mine), 2, "north-west");

    // A leg with no warp is not a course: the original checks the warp nibble
    // before it reads the waypoint.
    mine.waypoints[1] = waypoint(Point::new(140, 100), 0);
    assert_eq!(app.fleet_arrow_of(&mine), 0);
}

/// Another player's fleet points the way the sighting recorded, not along
/// waypoints — which a fleet seen at a distance does not have anyway.
#[test]
fn another_players_fleet_uses_the_recorded_direction() {
    let app = a_game();
    let mut theirs = fleet(1, Point::new(100, 100));
    assert_eq!(app.fleet_arrow_of(&theirs), 0, "nothing recorded");

    theirs.direction = Some((40, 0));
    assert_eq!(app.fleet_arrow_of(&theirs), 5, "east");
    theirs.direction = Some((0, -40));
    assert_eq!(app.fleet_arrow_of(&theirs), 7, "south");

    // Waypoints on somebody else's fleet are ignored: the original reads the
    // direction field for them and never the orders.
    theirs.direction = None;
    theirs.waypoints = vec![
        waypoint(Point::new(100, 100), 0),
        waypoint(Point::new(140, 100), 5),
    ];
    assert_eq!(app.fleet_arrow_of(&theirs), 0);
}

/// The arrows are two columns of eight in a 16-by-72 sheet: nine pixels at
/// life size and seven once zoomed out.
#[test]
fn the_arrows_are_two_columns_of_eight() {
    let mut app = a_game();
    app.scan_zoom = 0;
    assert_eq!(app.fleet_arrow_cell(0), (7, 0, 9));
    assert_eq!(app.fleet_arrow_cell(7), (7, 63, 9));
    app.scan_zoom = -1;
    assert_eq!(app.fleet_arrow_cell(0), (0, 0, 7));
    assert_eq!(app.fleet_arrow_cell(7), (0, 49, 7));

    let Some(exe) = executable() else { return };
    let sheet = stars_formats::resources::read_bitmap(
        &exe,
        &stars_formats::resources::Name::Id(stars_ui::ARROW_SHEET),
    )
    .expect("the arrow sheet");
    assert_eq!((sheet.width, sheet.height), (16, 72));
    for zoom in [-1, 0] {
        app.scan_zoom = zoom;
        for arrow in 0..8u8 {
            let (x, y, side) = app.fleet_arrow_cell(arrow);
            assert!(
                sheet.crop(x, y, side, side).is_some(),
                "arrow {arrow} at zoom {zoom}"
            );
            // Each cell really has an arrow in it: the shape is the black
            // pixel, and no cell is blank.
            let cell = sheet.crop(x, y, side, side).expect("a cell");
            assert!(
                cell.pixels.chunks(4).any(|p| p[..3] == [0, 0, 0]),
                "arrow {arrow} at zoom {zoom} is blank"
            );
        }
    }
}

/// The map draws the arrows, with the game's sheet and without it.
#[test]
fn the_map_draws_the_arrows() {
    let mut app = a_game();
    let here = Point::new(100, 100);
    let mut mine = fleet(0, here);
    mine.waypoints = vec![waypoint(here, 0), waypoint(Point::new(140, 140), 6)];
    let mut theirs = fleet(1, Point::new(120, 120));
    theirs.direction = Some((-30, 20));
    app.game.as_mut().expect("a game").fleets = vec![mine, theirs];

    let draw = |app: &mut App| {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                stars_ui::views::galaxy::view(app, ui);
            });
        });
    };
    draw(&mut app);
    if let Some(exe) = executable() {
        app.load_art(exe, "the test's own copy").expect("pictures");
        draw(&mut app);
        app.scan_zoom = -2;
        draw(&mut app);
    }
}
