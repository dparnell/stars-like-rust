//! The fleet pane.
//!
//! The **same window as the planet pane**, with a different tile table when a
//! fleet is selected rather than a planet (`rgtileShip`, `1120:090e`). Seven
//! tiles in two columns:
//!
//! ```text
//! ┌───────────────────────┬───────────────────────┐
//! │ the fleet, drawn      │ Fuel & Cargo          │
//! ├───────────────────────┼───────────────────────┤
//! │ <planet> / In Deep    │ Fleet Composition     │
//! │ Space                 │                       │
//! ├───────────────────────┼───────────────────────┤
//! │ Fleet Waypoints       │ the other fleets here │
//! ├───────────────────────┤                       │
//! │ Waypoint Task         │                       │
//! └───────────────────────┴───────────────────────┘
//! ```
//!
//! See `docs/ui/fleet-pane.md`.

use crate::App;

/// Draw the fleet pane.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_top(|ui| {
        let width = (ui.available_width() - 8.0) / 2.0;
        ui.vertical(|ui| {
            ui.set_width(width);
            summary(app, ui);
            location(app, ui);
            rows(ui, "Fleet Waypoints", &app.fleet_waypoints_tile());
            let task = app.fleet_task_tile();
            tile(ui, "Waypoint Task", |ui| {
                ui.label(egui::RichText::new(task).small());
            });
        });
        ui.vertical(|ui| {
            ui.set_width(width);
            rows(ui, "Fuel & Cargo", &app.fleet_cargo_tile());
            rows(ui, "Fleet Composition", &app.fleet_composition_tile());
            let others = app.planet_fleets_tile();
            tile(ui, "Fleets Here", |ui| {
                if others.is_empty() {
                    ui.label(egui::RichText::new("none").weak().small());
                }
                for line in others {
                    ui.label(egui::RichText::new(line).small());
                }
            });
        });
    });
}

/// The tallest tile: the fleet itself, which the original draws as a picture.
///
/// `DrawFleetBitmap` blits the primary design's ship 64 pixels square with the
/// owner's race emblem over its bottom-left corner. Without the game's own
/// pictures the tile is the same words without the ship.
fn summary(app: &mut App, ui: &mut egui::Ui) {
    let Some(fleet) = app.pane_fleet() else {
        tile(ui, "Fleet", |ui| {
            ui.label(egui::RichText::new("no fleet selected").weak().small());
        });
        return;
    };
    let name = app
        .survey_subject()
        .fleet_index()
        .map_or_else(String::new, |index| app.fleet_display_name(index));
    let ships: i32 = fleet.stacks.iter().map(|s| s.count).sum();
    let owner = fleet.owner;
    let position = fleet.position;
    let picture = app.fleet_picture();
    let emblem = app.fleet_emblem(stars_formats::resources::art::EmblemSize::Medium);
    tile(ui, &name, |ui| {
        if let Some((cell, distinct)) = picture {
            ui.horizontal_top(|ui| {
                let corner = ui.cursor().min;
                if crate::art::draw(app, ui, cell, 64.0) {
                    if let Some(emblem) = emblem {
                        let ctx = ui.ctx().clone();
                        if let Some(art) = app.art.as_mut() {
                            if let Some(image) = art.sprite(&ctx, emblem, 16.0) {
                                image.paint_at(
                                    ui,
                                    egui::Rect::from_min_size(
                                        corner + egui::vec2(0.0, 48.0),
                                        egui::vec2(16.0, 16.0),
                                    ),
                                );
                            }
                        }
                    }
                    // The original marks a mixed fleet beside the picture
                    // rather than drawing every design in it.
                    if distinct > 1 {
                        ui.label(egui::RichText::new(format!("+{}", distinct - 1)).small());
                    }
                }
            });
        }
        ui.label(egui::RichText::new(format!("player {}", owner + 1)).small());
        ui.label(egui::RichText::new(format!("{ships} ships")).small());
        ui.label(egui::RichText::new(format!("({}, {})", position.x, position.y)).small());
    });
}

/// The planet the fleet is at, or deep space.
fn location(app: &App, ui: &mut egui::Ui) {
    let title = app.fleet_location_title();
    tile(ui, &title, |ui| {
        let at_planet = app.pane_fleet().is_some_and(|f| f.orbiting.is_some());
        ui.label(
            egui::RichText::new(if at_planet { "in orbit" } else { "under way" })
                .weak()
                .small(),
        );
    });
}

/// One tile: a title bar and a two-column grid of label and value.
fn rows(ui: &mut egui::Ui, title: &str, rows: &[(String, String)]) {
    tile(ui, title, |ui| {
        if rows.is_empty() {
            ui.label(egui::RichText::new("none").weak().small());
            return;
        }
        egui::Grid::new(title)
            .num_columns(2)
            .spacing([8.0, 1.0])
            .show(ui, |ui| {
                for (label, value) in rows {
                    ui.label(egui::RichText::new(label).small());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(value).small());
                    });
                    ui.end_row();
                }
            });
    });
}

/// The frame every tile shares.
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
