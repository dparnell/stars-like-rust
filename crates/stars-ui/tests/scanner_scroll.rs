//! The scanner's scroll bars: past 100% the map is bigger than its panel,
//! and a bar along the foot and another down the right edge scroll it —
//! the original's window has `WS_HSCROLL | WS_VSCROLL`.

use std::path::PathBuf;

use stars_ui::App;

fn a_game() -> Option<App> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut app = App::new();
    app.open(&root.join("fixtures/games/exodus/2418/exodus.m6"))
        .ok()?;
    Some(app)
}

/// At 100% the map fits and keeps the whole panel; at 400% it gives up
/// sixteen pixels along the foot and the right edge to the bars, and a
/// click on the foot bar's right arrow scrolls the map to the right.
#[test]
fn the_bars_appear_past_a_hundred_percent_and_scroll() {
    let Some(app) = a_game() else {
        eprintln!("skipping: no fixtures");
        return;
    };
    let mut shell = stars_ui::autopilot::Shell::new(app);
    shell.frame();
    shell.frame();
    let fitted = shell.app.map_frame.expect("the map was drawn").rect;

    shell.app.scan_zoom_by(4);
    assert_eq!(shell.app.scan_zoom_percent(), 400);
    shell.frame();
    shell.frame();
    let zoomed = shell.app.map_frame.expect("the map was drawn").rect;
    assert_eq!(
        fitted.right() - zoomed.right(),
        stars_ui::views::scrollbar::WIDTH,
        "the vertical bar takes the right edge"
    );
    assert_eq!(
        fitted.bottom() - zoomed.bottom(),
        stars_ui::views::scrollbar::WIDTH,
        "the horizontal bar takes the foot"
    );

    // The map is drawn about `scan_center`; the foot bar's right arrow
    // moves it one line to the right.
    let before = shell.app.map_frame.expect("drawn").origin.x;
    let arrow = egui::pos2(zoomed.right() - 8.0, zoomed.bottom() + 8.0);
    shell.click_at(arrow);
    shell.frame();
    let after = shell.app.map_frame.expect("drawn").origin.x;
    assert!(
        after < before,
        "the galaxy's corner moved left, {before} -> {after}: the map scrolled right"
    );
    assert!(shell.app.scan_center.is_some());
}

/// The map spans the whole universe, not the planets the player happens
/// to know: a first-year player file knows three of the tutorial's
/// twenty-four, and a map fitted to those three would be zoomed onto a
/// corner with the rest off its edges and no bar to reach them.
#[test]
fn the_map_spans_the_universe_not_the_known_planets() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut app = App::new();
    app.prompt_for_password = false;
    if app
        .open(&root.join("fixtures/games/tutorial/tutorial.m1"))
        .is_err()
    {
        eprintln!("skipping: no fixtures");
        return;
    }
    let game = app.game.as_ref().expect("a game");
    let known = game.planets.len() + game.known_planets.len();
    let all = app
        .universe
        .as_ref()
        .expect("a universe")
        .planets_resolved();
    assert!(known < all.len(), "{known} of {} known", all.len());
    // `dGal` for a tiny universe is 400, from 1000.
    assert_eq!(app.extent(), Some((1000.0, 1000.0, 1400.0, 1400.0)));
    // Every planet of the universe lies inside it.
    for planet in all {
        assert!((1000..=1400).contains(&planet.x) && (1000..=1400).contains(&planet.y));
    }
    // And at 100% the whole of it fits the panel, so there are no bars.
    let mut shell = stars_ui::autopilot::Shell::new(app);
    shell.frame();
    shell.frame();
    let frame = shell.app.map_frame.expect("the map was drawn");
    let to = |x: f32, y: f32| frame.to_screen(x as i16, y as i16);
    let far = to(1400.0, 1000.0);
    assert!(frame.rect.contains(far), "the far corner is on the panel");
    assert!(frame.rect.contains(to(1000.0, 1400.0)));
}
