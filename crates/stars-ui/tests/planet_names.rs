//! How the scanner writes a planet's name — `DrawScanner`'s name pass.
//!
//! See `docs/ui/scanner.md`.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{App, ScanView};

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "names".to_string(),
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

/// Four fonts out of the jump table on the zoom, and the last two are bold.
#[test]
fn the_font_grows_with_the_zoom_and_turns_bold() {
    let mut app = a_game();
    let at = |app: &mut App, zoom: i8| {
        app.scan_zoom = zoom;
        app.planet_name_style()
    };

    let small = at(&mut app, -1);
    assert_eq!((small.points, small.bold), (6.0, false));
    for zoom in [0, 1, 2] {
        let style = at(&mut app, zoom);
        assert_eq!((style.points, style.bold), (8.0, false), "zoom {zoom}");
    }
    let three = at(&mut app, 3);
    assert_eq!(
        (three.points, three.bold),
        (8.0, true),
        "the same size in the bold face"
    );
    let four = at(&mut app, 4);
    assert_eq!((four.points, four.bold), (10.0, true));
}

/// Points become pixels the way the original converts them.
#[test]
fn a_point_size_is_four_thirds_of_a_pixel() {
    let mut app = a_game();
    app.scan_zoom = 0;
    let style = app.planet_name_style();
    assert!((style.pixels() - 8.0 * 4.0 / 3.0).abs() < f32::EPSILON);
}

/// Five pixels under the planet, and eleven more in the one view that draws
/// something there — but only once the zoom is in far enough for it.
#[test]
fn the_population_view_pushes_the_name_down() {
    let mut app = a_game();
    app.scan_view = ScanView::Normal;
    for zoom in [-1, 0, 3, 4] {
        app.scan_zoom = zoom;
        assert_eq!(app.planet_name_below(), 5, "zoom {zoom}");
    }

    app.scan_view = ScanView::Population;
    for zoom in [-1, 0, 1, 2] {
        app.scan_zoom = zoom;
        assert_eq!(
            app.planet_name_below(),
            5,
            "not zoomed in far enough: {zoom}"
        );
    }
    for zoom in [3, 4] {
        app.scan_zoom = zoom;
        assert_eq!(app.planet_name_below(), 16, "zoom {zoom}");
    }

    // And no other view moves it, however far in the zoom goes.
    for view in [
        ScanView::SurfaceMineral,
        ScanView::MineralConcentration,
        ScanView::PlanetValue,
        ScanView::NoPlayerInfo,
    ] {
        app.scan_view = view;
        app.scan_zoom = 4;
        assert_eq!(app.planet_name_below(), 5, "{view:?}");
    }
}

/// White unless Player Colors is on and the planet has an owner other than
/// this player — an unowned planet is never coloured at all.
#[test]
fn only_an_owned_planet_takes_a_colour() {
    let mut app = a_game();
    assert_eq!(app.planet_name_colour(Some(1)), None, "the setting is off");

    app.scan_overlays.player_colours = true;
    assert_eq!(app.planet_name_colour(None), None, "nobody owns it");
    assert_eq!(
        app.planet_name_colour(Some(0)),
        Some(None),
        "yours is white"
    );
    assert_eq!(app.planet_name_colour(Some(1)), Some(Some(1)));
}

/// The zoom cut-off: names stop below -1.
#[test]
fn names_stop_when_the_map_is_zoomed_out() {
    let mut app = a_game();
    app.scan_overlays.names = true;
    for zoom in [-1, 0, 4] {
        app.scan_zoom = zoom;
        assert!(app.planet_names_visible(), "zoom {zoom}");
    }
    for zoom in [-2, -3] {
        app.scan_zoom = zoom;
        assert!(!app.planet_names_visible(), "zoom {zoom}");
    }
}
