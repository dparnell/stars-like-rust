//! The egui screens, written once and drawn by every frontend.
//!
//! Each view takes `&mut App` and an `&mut egui::Ui` and draws into it. They
//! hold no state of their own: everything the player has chosen lives in
//! [`crate::App`], so a frontend can save and restore it, and so the views stay
//! testable through that state rather than through the widgets.

pub mod battles;
pub mod fleets;
pub mod galaxy;
pub mod newgame;
pub mod planets;
pub mod players;

use crate::{App, Screen};

/// Draw whichever screen is selected.
///
/// Returns what the New Game wizard asked for, when it is open and the player
/// clicked something the shell has to act on (creating the game needs no
/// shell, but opening a race file does).
pub fn central(app: &mut App, ui: &mut egui::Ui) -> Option<newgame::Action> {
    if app.setup.is_some() {
        return newgame::view(app, ui);
    }
    if app.game.is_none() {
        empty(app, ui);
        return None;
    }
    match app.screen {
        Screen::Galaxy => galaxy::view(app, ui),
        Screen::Planets => planets::view(app, ui),
        Screen::Fleets => fleets::view(app, ui),
        Screen::Battles => battles::view(app, ui),
        Screen::Players => players::view(app, ui),
    }
    None
}

/// The title screen, shown before a game is opened.
fn empty(app: &mut App, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(80.0);
        ui.heading("Stars!");
        ui.add_space(8.0);
        ui.label("Open a save file to begin — a player file (.m1 …) or a host file (.hst).");
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Planet positions come from the .xy universe file, which is looked for \
                 beside the save and one directory up.",
            )
            .weak(),
        );
        ui.add_space(16.0);
        if ui.button("Start a new game…").clicked() {
            app.setup = Some(stars_core::newgame::NewGame::default());
        }
    });
}

/// The colour a player's things are drawn in.
///
/// Sixteen players, so sixteen hues far enough apart to tell at a glance.
#[must_use]
pub fn player_colour(player: i16) -> egui::Color32 {
    const PALETTE: [egui::Color32; 16] = [
        egui::Color32::from_rgb(0x4f, 0xa3, 0xff),
        egui::Color32::from_rgb(0xff, 0x6b, 0x6b),
        egui::Color32::from_rgb(0x5a, 0xd6, 0x8a),
        egui::Color32::from_rgb(0xff, 0xd1, 0x54),
        egui::Color32::from_rgb(0xc9, 0x7b, 0xff),
        egui::Color32::from_rgb(0x4a, 0xd9, 0xd9),
        egui::Color32::from_rgb(0xff, 0x9e, 0x4a),
        egui::Color32::from_rgb(0xa3, 0xbf, 0x5a),
        egui::Color32::from_rgb(0xf0, 0x7a, 0xc0),
        egui::Color32::from_rgb(0x8f, 0x9d, 0xff),
        egui::Color32::from_rgb(0x6f, 0xd0, 0x4f),
        egui::Color32::from_rgb(0xd6, 0xb0, 0x70),
        egui::Color32::from_rgb(0x70, 0xc4, 0xff),
        egui::Color32::from_rgb(0xe0, 0x60, 0x9c),
        egui::Color32::from_rgb(0x9a, 0xd8, 0xc0),
        egui::Color32::from_rgb(0xbb, 0xbb, 0xbb),
    ];
    let index = usize::try_from(player).unwrap_or(0) % PALETTE.len();
    PALETTE[index]
}

/// A population figure, in the units players expect.
///
/// The simulation stores population in units of 100 colonists, which is a
/// detail of the file format and not something to put in front of anyone.
#[must_use]
pub fn colonists(pop: i32) -> String {
    let people = i64::from(pop) * 100;
    if people >= 1_000_000 {
        format!("{:.2}M", people as f64 / 1_000_000.0)
    } else if people >= 1_000 {
        format!("{}k", people / 1_000)
    } else {
        people.to_string()
    }
}
