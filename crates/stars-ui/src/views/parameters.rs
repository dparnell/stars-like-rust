//! The Game Parameters window.
//!
//! View (Game Parameters), menu id `0x9e`. What the game was set up with —
//! and, on its own page, what has to be done to win it, which is where
//! `MANUAL.PDF` p. 2-3 sends a player: *"To view the winning conditions once
//! the game has begun, choose the View (Race) menu item, then turn to page 3
//! of the View Game Parameters dialog that appears."*
//!
//! See `docs/ui/view-menu.md`.

use crate::App;

/// Draw the window's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let rows = app.game_parameters_rows();
    if rows.is_empty() {
        ui.label(
            egui::RichText::new(
                "The game's settings live in its .xy universe file, which was not \
                 found beside this save or one directory up.",
            )
            .weak()
            .small(),
        );
        return;
    }

    egui::Grid::new("game-parameters")
        .num_columns(2)
        .spacing([14.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for (label, value) in &rows {
                ui.label(egui::RichText::new(label).small());
                ui.label(egui::RichText::new(value).small());
                ui.end_row();
            }
        });

    ui.separator();
    ui.label(egui::RichText::new("Victory Conditions").strong().small());
    let conditions = app.game_parameters_conditions();
    if conditions.is_empty() {
        ui.label(egui::RichText::new("none recorded").weak().small());
        return;
    }
    for condition in &conditions {
        // A condition the game is not playing for is listed greyed, with its
        // setting, exactly as the Score sheet lists it.
        let text = egui::RichText::new(&condition.text).small();
        ui.label(if condition.active || !condition.scoreable {
            text
        } else {
            text.color(egui::Color32::from_rgb(0x9a, 0x9a, 0x9a))
        });
    }
}
