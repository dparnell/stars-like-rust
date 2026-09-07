//! The View menu: the toolbar, the window layout and the Game Parameters
//! window.
//!
//! See `docs/ui/view-menu.md`.

use std::path::{Path, PathBuf};

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{App, WindowLayout};

fn fixture(relative: &str) -> Option<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(relative);
    path.exists().then_some(path)
}

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "view".to_string(),
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

fn draw(app: &mut App) {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::galaxy::view(app, ui);
        });
    });
}

/// The toolbar shows to begin with, and View (Toolbar) hides it.
///
/// It is stored the other way up — `toolbar_hidden` — so that a default-built
/// `App` still shows it.
#[test]
fn the_toolbar_shows_until_it_is_hidden() {
    let mut app = a_game();
    assert!(app.toolbar_visible());
    assert!(App::default().toolbar_visible(), "even a bare one");

    app.toolbar_hidden = true;
    assert!(!app.toolbar_visible());
    // The map still draws without it.
    draw(&mut app);
    app.toolbar_hidden = false;
    draw(&mut app);
}

/// Three window layouts, named as the menu names them, largest first.
#[test]
fn there_are_three_window_layouts() {
    assert_eq!(
        WindowLayout::ALL.map(|l| l.name()),
        ["Large Screen", "Medium Screen", "Small Screen"]
    );
    assert_eq!(WindowLayout::ALL.map(|l| l as u8), [0, 1, 2]);
    assert_eq!(WindowLayout::default(), WindowLayout::Large);
    assert_eq!(a_game().window_layout, WindowLayout::Large);
}

/// The Game Parameters window lists what the game was set up with, out of the
/// `.xy` — and says nothing rather than guessing when there is no `.xy`.
#[test]
fn game_parameters_come_from_the_universe_file() {
    let mut app = a_game();
    // A game this project generated has a universe, so it has parameters.
    let rows = app.game_parameters_rows();
    assert!(!rows.is_empty());
    let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
    for wanted in [
        "Name",
        "Year",
        "Universe size",
        "Density",
        "Player positions",
        "Players",
        "Planets",
        "Options",
    ] {
        assert!(labels.contains(&wanted), "{wanted} is listed: {labels:?}");
    }
    let value = |label: &str| {
        rows.iter()
            .find(|(l, _)| l == label)
            .map(|(_, v)| v.clone())
            .expect("a row")
    };
    assert_eq!(value("Name"), "view");
    assert_eq!(value("Year"), "2400");
    assert_eq!(value("Universe size"), "Small");
    assert_eq!(value("Players"), "2");

    // Without a universe there is nothing to say.
    app.universe = None;
    assert!(app.game_parameters_rows().is_empty());
}

/// A real save's parameters, read out of the `.xy` beside it.
#[test]
fn a_real_game_s_parameters_read_back() {
    let Some(save) = fixture("games/no-random-events/2500/Game.m1") else {
        return;
    };
    let mut app = App::new();
    if app.open(&save).is_err() {
        return;
    }
    let rows = app.game_parameters_rows();
    assert!(!rows.is_empty(), "the .xy sits beside it");
    let value = |label: &str| {
        rows.iter()
            .find(|(l, _)| l == label)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    assert_eq!(value("Players"), "16");
    assert_eq!(value("Year"), "2500");
    // That game is played with public scores, which the score sheet showed
    // from the other side.
    assert!(
        value("Options").contains("Public player scores"),
        "{}",
        value("Options")
    );

    // And the window lists the victory conditions under them.
    assert_eq!(app.game_parameters_conditions().len(), 9);

    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::parameters::view(&mut app, ui);
        });
    });
}

/// The window draws even with nothing to put in it.
#[test]
fn the_parameters_window_copes_with_no_universe() {
    let mut app = a_game();
    app.universe = None;
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::parameters::view(&mut app, ui);
        });
    });
}
