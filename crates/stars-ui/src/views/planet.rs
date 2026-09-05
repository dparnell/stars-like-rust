//! The planet pane.
//!
//! The original's top-left pane: the selected planet as **six tiles in two
//! columns**, each with its own little title bar (`PlanetWndProc`,
//! `1048:0000`, over the `rgtilePlanet` table at `1120:07fc`).
//!
//! ```text
//! ┌───────────────────────┬───────────────────────┐
//! │ the planet, drawn     │ the fleets in orbit   │
//! ├───────────────────────┼───────────────────────┤
//! │ Minerals On Hand      │ Production            │
//! ├───────────────────────┼───────────────────────┤
//! │ Status                │ Starbase              │
//! └───────────────────────┴───────────────────────┘
//! ```
//!
//! The column each tile belongs to, its order and its height are read from that
//! table rather than guessed; see `docs/ui/planet-pane.md`. Every label and
//! every number format here is the original's.
//!
//! What is not the original: it draws the planet itself as a picture and its
//! tiles can be collapsed by clicking their title bars, and neither is
//! reproduced.

use crate::App;

/// Draw the planet pane.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_top(|ui| {
        let width = (ui.available_width() - 8.0) / 2.0;
        ui.vertical(|ui| {
            ui.set_width(width);
            summary(app, ui);
            rows(ui, "Minerals On Hand", &app.planet_minerals_tile());
            rows(ui, "Status", &app.planet_status_tile());
        });
        ui.vertical(|ui| {
            ui.set_width(width);
            lines(ui, "Fleets in Orbit", &app.planet_fleets_tile(), "none");
            lines(
                ui,
                "Production",
                &app.planet_production_tile(),
                "--- Queue is Empty ---",
            );
            let (title, starbase) = app.planet_starbase_tile();
            rows(ui, &title, &starbase);
        });
    });
}

/// One tile: a title bar and a two-column grid of label and value.
fn rows(ui: &mut egui::Ui, title: &str, rows: &[(String, String)]) {
    tile(ui, title, |ui| {
        if rows.is_empty() {
            ui.label(egui::RichText::new("no data").weak().small());
            return;
        }
        egui::Grid::new(title)
            .num_columns(2)
            .spacing([8.0, 1.0])
            .striped(false)
            .show(ui, |ui| {
                for (label, value) in rows {
                    ui.label(egui::RichText::new(label).small());
                    // The original right-aligns every value against the tile's
                    // inside edge.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(value).small());
                    });
                    ui.end_row();
                }
            });
    });
}

/// One tile holding a list rather than label/value pairs.
fn lines(ui: &mut egui::Ui, title: &str, lines: &[String], empty: &str) {
    tile(ui, title, |ui| {
        if lines.is_empty() {
            ui.label(egui::RichText::new(empty).weak().small());
            return;
        }
        for line in lines {
            ui.label(egui::RichText::new(line).small());
        }
    });
}

/// The frame every tile shares: a title bar above a body.
fn tile(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(title).small().strong());
                ui.separator();
                body(ui);
            });
        });
}

/// The tallest tile: the planet itself.
///
/// The original draws the planet as a picture here, sized to the tile. This
/// says in words what that picture says at a glance — whose it is, and whether
/// anybody has been.
fn summary(app: &mut App, ui: &mut egui::Ui) {
    let title = app.planet_pane_title();
    tile(ui, &title, |ui| {
        let Some(planet) = app.pane_planet() else {
            ui.label(egui::RichText::new("no planet selected").weak().small());
            return;
        };
        let owner = match planet.owner {
            Some(owner) => format!("player {}", owner + 1),
            None => "unowned".to_string(),
        };
        let detail = match planet.detail {
            stars_core::planet::Detail::Full => "yours",
            stars_core::planet::Detail::Scanned => "scanned",
            stars_core::planet::Detail::Minimal => "not surveyed",
        };
        ui.label(egui::RichText::new(format!("{owner} — {detail}")).small());
        if planet.homeworld {
            ui.label(egui::RichText::new("homeworld").small());
        }
        if planet.starbase {
            ui.label(egui::RichText::new("starbase in orbit").small());
        }
    });
}
