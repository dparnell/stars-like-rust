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
            crate::views::fleets_here(app, ui);
            production(ui, &app.planet_production_rows());
            // The tile's own button, which is how the original opens the
            // Production dialog.
            if ui
                .add_enabled(
                    app.selected_planet().is_some(),
                    egui::Button::new(egui::RichText::new("Change").small()),
                )
                .clicked()
            {
                app.open_production();
            }
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

/// The production tile, whose rows are coloured by when they will be built —
/// red for one that practically never will be, which is the manual's warning
/// on p. 7-7.
fn production(ui: &mut egui::Ui, rows: &[(String, stars_core::production::EtaMark)]) {
    use stars_core::production::EtaMark;
    tile(ui, "Production", |ui| {
        if rows.is_empty() {
            ui.label(egui::RichText::new("--- Queue is Empty ---").weak().small());
            return;
        }
        for (line, mark) in rows {
            let text = egui::RichText::new(line).small();
            ui.label(match mark {
                EtaMark::Never => text.color(egui::Color32::from_rgb(0xff, 0x6b, 0x6b)),
                EtaMark::AllNextYear => text.color(egui::Color32::from_rgb(0x5a, 0xd6, 0x8a)),
                EtaMark::FirstNextYear => text.color(egui::Color32::from_rgb(0xa3, 0xbf, 0x5a)),
                EtaMark::Idle => text.weak(),
                EtaMark::Ordinary => text,
            });
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
    // The planet's own face, when the game's pictures have been found. The
    // original draws it 64 pixels square in a sunken frame at the top of the
    // pane, and picks it from the planet's id, so a planet keeps the same face
    // all game.
    let picture = app
        .pane_planet()
        .map(|planet| planet.id)
        .and_then(|id| app.planet_picture(id));
    tile(ui, &title, |ui| {
        if let Some(cell) = picture {
            egui::Frame::none()
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .inner_margin(1.0)
                .show(ui, |ui| {
                    crate::art::draw(app, ui, cell, 64.0);
                });
        }
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
