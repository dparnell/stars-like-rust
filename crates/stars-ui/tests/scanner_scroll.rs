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
