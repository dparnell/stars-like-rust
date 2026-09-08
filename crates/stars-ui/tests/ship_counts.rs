//! The numbers the scanner writes over fleets — `DrawScanFleetCount`.
//!
//! See `docs/ui/scanner.md`.

use stars_core::fleet::{Fleet, ShipStack};
use stars_core::movement::Point;
use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{App, ScanView};

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "counts".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.selection.fleet = None;
    app.selection.planet = None;
    app
}

fn fleet(owner: i16, at: Point, orbiting: Option<u16>, ships: i32) -> Fleet {
    Fleet {
        id: 1,
        owner,
        position: at,
        orbiting,
        stacks: vec![ShipStack {
            design: 0,
            count: ships,
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

/// The layout is by hand and the three cases do not share a left edge: the
/// digits are five pixels apart and the number ends up roughly centred.
#[test]
fn the_digits_are_laid_out_the_way_the_original_lays_them_out() {
    assert_eq!(App::ship_count_digits(7), vec![(-1, 7)]);
    assert_eq!(App::ship_count_digits(42), vec![(-4, 4), (1, 2)]);
    assert_eq!(App::ship_count_digits(999), vec![(-6, 9), (-1, 9), (4, 9)]);
    // A zero inside the number keeps its place.
    assert_eq!(App::ship_count_digits(101), vec![(-6, 1), (-1, 0), (4, 1)]);
    assert_eq!(App::ship_count_digits(10), vec![(-4, 1), (1, 0)]);
}

/// Nothing to write for an empty spot, and never more than three digits.
#[test]
fn nothing_is_written_for_nothing() {
    assert!(App::ship_count_digits(0).is_empty());
    assert!(App::ship_count_digits(-3).is_empty());
    assert_eq!(App::ship_count_digits(50_000).len(), 3);
}

/// Each of `DrawScanner`'s three arms hands the routine a different `y`.
#[test]
fn a_number_sits_above_whichever_mark_the_fleet_got() {
    let mut app = a_game();
    let here = Point::new(50, 50);

    // Deep space: half the arrow's height, plus two.
    app.scan_zoom = 0;
    let arrow = fleet(0, here, None, 3);
    assert_eq!(app.ship_count_anchor(&arrow), Some(9 / 2 + 2));
    app.scan_zoom = -1;
    assert_eq!(
        app.ship_count_anchor(&arrow),
        Some(7 / 2 + 2),
        "the smaller arrow when zoomed out"
    );
    app.scan_zoom = 0;

    // In orbit: above the ring, and above the larger ring when the planet is
    // the selected point.
    let orbiting = fleet(0, here, Some(3), 3);
    assert_eq!(app.ship_count_anchor(&orbiting), Some(5 + 2));

    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![orbiting.clone()];
    }
    app.selection.fleet = Some(0);
    app.selection.on_fleet = true;
    assert_eq!(app.ship_count_anchor(&orbiting), Some(9 + 2));
    // And a deep-space fleet on the selected point gets the glyph's own y.
    assert_eq!(app.ship_count_anchor(&arrow), Some(7));
}

/// The orbit call sits inside the ring arm, so the views that draw no rings
/// write no numbers over planets either — but deep space still gets one.
#[test]
fn a_fleet_in_orbit_writes_no_number_in_the_last_three_views() {
    let mut app = a_game();
    let here = Point::new(50, 50);
    let orbiting = fleet(0, here, Some(3), 3);
    let deep = fleet(0, here, None, 3);

    for view in [
        ScanView::Normal,
        ScanView::SurfaceMineral,
        ScanView::MineralConcentration,
    ] {
        app.scan_view = view;
        assert!(app.ship_count_anchor(&orbiting).is_some(), "{view:?}");
    }
    for view in [
        ScanView::PlanetValue,
        ScanView::Population,
        ScanView::NoPlayerInfo,
    ] {
        app.scan_view = view;
        assert_eq!(app.ship_count_anchor(&orbiting), None, "{view:?}");
        assert!(
            app.ship_count_anchor(&deep).is_some(),
            "a fleet in deep space still writes one: {view:?}"
        );
    }
}

/// A location whose only fleets are in orbit loses its number in those views;
/// one with a fleet in deep space keeps it, at that fleet's own height.
#[test]
fn a_location_takes_the_first_arm_that_writes_one() {
    let mut app = a_game();
    let here = Point::new(50, 50);
    if let Some(game) = app.game.as_mut() {
        game.fleets = vec![fleet(0, here, Some(3), 4)];
    }
    app.scan_view = ScanView::Normal;
    let counts = app.ship_counts();
    assert_eq!(counts.len(), 1);
    assert_eq!(counts[0].ships, 4);
    assert_eq!(counts[0].above, 7);

    app.scan_view = ScanView::Population;
    assert!(
        app.ship_counts().is_empty(),
        "no ring, so no number over the planet"
    );

    // Add a fleet in deep space on the same point and the number comes back,
    // above the arrow, still counting both.
    if let Some(game) = app.game.as_mut() {
        game.fleets.push(fleet(0, here, None, 2));
    }
    let counts = app.ship_counts();
    assert_eq!(counts.len(), 1);
    assert_eq!(counts[0].ships, 6, "both fleets, added");
    assert_eq!(counts[0].above, 9 / 2 + 2);
}
