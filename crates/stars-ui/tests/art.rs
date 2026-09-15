//! Drawing with the game's own pictures.
//!
//! Every one of these has to pass **without** a copy of the original too: the
//! pictures are an improvement, never a requirement, and a screen that cannot
//! find them must draw exactly as it did before.

use std::path::PathBuf;

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_formats::resources::art::EmblemSize;
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

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "art".to_string(),
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

/// Draw the screens that use a picture, with and without one.
fn draw_everything(app: &mut App) {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::planet::view(app, ui);
        });
    });
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::players::view(app, ui);
        });
    });
}

/// Without a copy of the original there are no pictures, and everything draws
/// anyway.
#[test]
fn the_screens_draw_without_the_pictures() {
    let mut app = a_game();
    assert!(!app.has_art());
    assert!(
        app.emblem_of(0, EmblemSize::Large).is_some(),
        "the cell is known"
    );
    draw_everything(&mut app);
}

/// With one, the pictures load and the same screens draw.
#[test]
fn the_screens_draw_with_the_pictures() {
    let Some(exe) = executable() else { return };
    let mut app = a_game();
    app.load_art(exe, "the test's own copy")
        .expect("the executable has pictures in it");
    assert!(app.has_art());
    assert_eq!(app.art.as_ref().expect("art").bitmaps(), 38);
    draw_everything(&mut app);
    // Drawing again reuses the textures rather than uploading them twice.
    draw_everything(&mut app);
}

/// A planet's face comes from its own id, so it never changes and neighbours
/// do not share one.
#[test]
fn a_planet_keeps_the_same_face() {
    let app = a_game();
    let first = app.planet_picture(0).expect("a cell");
    assert_eq!(app.planet_picture(0), Some(first), "the same every time");
    assert_ne!(app.planet_picture(1), Some(first));
    // Twenty-eight faces, so the twenty-ninth planet takes the first again.
    assert_eq!(app.planet_picture(28), Some(first));
    // `(id + 8) % 28`, which for planet 0 is the ninth cell — second row,
    // second column, counting up from the bottom of the sheet.
    assert_eq!((first.x, first.y), (64, 128));
}

/// Every race's emblem has a cell, and a logo the file should never hold has
/// none rather than a wrong one.
#[test]
fn every_race_has_an_emblem() {
    let mut app = a_game();
    for size in [EmblemSize::Large, EmblemSize::Medium, EmblemSize::Small] {
        assert!(app.emblem_of(0, size).is_some());
    }
    if let Some(game) = app.game.as_mut() {
        game.players[0].logo = 31;
    }
    assert!(app.emblem_of(0, EmblemSize::Large).is_some());
    if let Some(game) = app.game.as_mut() {
        game.players[0].logo = 200;
    }
    assert!(app.emblem_of(0, EmblemSize::Large).is_none());
    // And a screen that would have drawn it copes.
    draw_everything(&mut app);
}

/// Anything that is not the game is refused with something to show the player.
#[test]
fn rubbish_is_refused() {
    let mut app = a_game();
    assert!(app.load_art(vec![0u8; 1024], "not-a-program").is_err());
    assert!(!app.has_art(), "and nothing is left half-loaded");
}

/// The toolbar's sheet comes out of the app's pictures with the button
/// face where the artist keyed magenta — `FGetSystemColors` writes
/// `COLOR_BTNFACE` into that entry of the palette, and so does `Art`.
#[test]
fn the_toolbar_wears_the_button_face_not_magenta() {
    let Some(exe) = executable() else { return };
    let mut app = App::new();
    app.load_art(exe, "the test's copy")
        .expect("the pictures load");
    let art = app.art.as_mut().expect("loaded");
    let sheet = art
        .sheet(&stars_formats::resources::Name::Id(178))
        .expect("the toolbar");
    assert_eq!(
        sheet.pixel(0, 0).map(|(r, g, b, _)| [r, g, b]),
        Some(stars_ui::toolbar::FACE)
    );
    let magenta = (0..sheet.height)
        .flat_map(|y| (0..sheet.width).map(move |x| (x, y)))
        .filter(|&(x, y)| sheet.pixel(x, y).map(|(r, g, b, _)| (r, g, b)) == Some((255, 0, 255)))
        .count();
    assert_eq!(magenta, 0);
}
