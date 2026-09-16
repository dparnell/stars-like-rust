//! The About box — Help (About Stars!...). See `docs/ui/about.md`.

use std::path::PathBuf;

use stars_ui::views::about::About;
use stars_ui::App;

fn executable() -> Option<Vec<u8>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../binary");
    for name in ["stars.2.7j.exe", "stars.exe", "STARS!.EXE"] {
        if let Ok(bytes) = std::fs::read(root.join(name)) {
            return Some(bytes);
        }
    }
    None
}

/// One frame of the dialogs, as the shell draws them.
fn draw(app: &mut App, time: f64) {
    app.start_frame();
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        time: Some(time),
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |_ui| {});
        stars_ui::views::frame::dialogs(app, ctx);
    });
}

/// The roll: two pixels a tick, a line every `dyArial8`, from eleven
/// lines under the window round to the same again.
#[test]
fn the_credits_roll_as_the_timer_would() {
    let about = About {
        opened_at: 10.0,
        order_info: false,
    };
    // Before the first tick: the start, nothing scrolled.
    assert_eq!(about.position(10.0, 13), (About::FIRST, 0));
    assert_eq!(about.position(10.049, 13), (About::FIRST, 0));
    // One tick: two pixels of the first line gone.
    assert_eq!(about.position(10.05, 13), (About::FIRST, 2));
    // Seven ticks reach thirteen — the line steps and the partial resets.
    assert_eq!(about.position(10.0 + 6.0 * 0.05, 13), (About::FIRST, 12));
    assert_eq!(about.position(10.0 + 7.0 * 0.05, 13), (About::FIRST + 1, 0));
    // Ninety-six lines on, it starts over.
    let span = About::LAST - About::FIRST + 1;
    let one_round = f64::from(span * 7) * 0.05;
    assert_eq!(about.position(10.0 + one_round, 13), (About::FIRST, 0));
    assert_eq!(
        about.position(10.0 + one_round - 0.05, 13),
        (About::LAST, 12)
    );
}

/// With the executable, the version and credits are the game's own.
#[test]
fn the_box_reads_its_lines_from_the_executable() {
    let mut app = App::new();
    assert_eq!(app.about_version(), "Demo Version", "the template's own");
    assert!(app.about_credits().is_empty());
    let Some(exe) = executable() else { return };
    app.load_art(exe, "stars.exe")
        .expect("the game's own pictures");
    assert_eq!(app.about_version(), "Version 2.60j");
    let credits = app.about_credits();
    assert_eq!(credits.len(), 77);
    assert_eq!(credits[0], "Design and Programming");
    assert_eq!(credits[76], "Ross Youngs");
}

/// The menu opens it, OK closes it, and Order Info puts up the second box.
#[test]
fn the_box_opens_and_closes_through_its_buttons() {
    let mut app = App::new();
    if let Some(exe) = executable() {
        app.load_art(exe, "stars.exe")
            .expect("the game's own pictures");
    }
    app.open_about(1.0);
    assert!(app.about.is_some());
    draw(&mut app, 1.3);
    assert!(app.drawn_button("", "Order Info...").is_some());
    assert!(app.drawn_button("", "OK").is_some());
    app.about.as_mut().unwrap().order_info = true;
    draw(&mut app, 1.4);
    // Both boxes have an OK; the ordering box is drawn second.
    let oks = app
        .drawn
        .iter()
        .filter(|w| w.scope.is_empty() && w.label == "OK")
        .count();
    assert_eq!(oks, 2);
    app.close_about();
    assert!(app.about.is_none());
    draw(&mut app, 1.5);
    assert!(app.drawn_button("", "Order Info...").is_none());
}
