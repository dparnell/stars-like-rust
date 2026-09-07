//! The egui screens, written once and drawn by every frontend.
//!
//! Each view takes `&mut App` and an `&mut egui::Ui` and draws into it. They
//! hold no state of their own: everything the player has chosen lives in
//! [`crate::App`], so a frontend can save and restore it, and so the views stay
//! testable through that state rather than through the widgets.

pub mod battles;
pub mod browser;
pub mod designer;
pub mod fleet;
pub mod fleets;
pub mod galaxy;
pub mod messages;
pub mod newgame;
pub mod parameters;
pub mod planet;
pub mod planets;
pub mod players;
pub mod production;
pub mod race;
pub mod relations;
pub mod research;
pub mod score;
pub mod survey;
pub mod toolbar;

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
/// The game's own sixteen (`rgcrPlrHistory`) — see
/// [`stars_core::scoresheet::PLAYER_COLOURS`]. `MANUAL.PDF` p. 5-16 sends a
/// player to the Score sheet's history graph to find out which colour is
/// theirs, so these are the colours that answer has to match.
#[must_use]
pub fn player_colour(player: i16) -> egui::Color32 {
    let index = usize::try_from(player).unwrap_or(0);
    let [r, g, b] = stars_core::scoresheet::player_colour(index);
    egui::Color32::from_rgb(r, g, b)
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
