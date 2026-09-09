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
    let mut open: Vec<bool> = app.open_ship_tiles.to_vec();
    crate::views::tile_pane(
        app,
        ui,
        &crate::tiles::SHIP_TILES,
        &mut open,
        tile_title,
        tile_body,
    );
    for (index, value) in open.iter().enumerate() {
        app.open_ship_tiles[index] = *value;
    }
}

/// What a tile's title bar says. Three of the seven take their title from the
/// game rather than the table: the fleet's own name, the planet it is at, and
/// the fleets-here tile, which titles itself by which pane it is in.
fn tile_title(app: &mut App, index: usize) -> String {
    match index {
        0 => app
            .survey_subject()
            .fleet_index()
            .map_or_else(|| "Fleet".to_string(), |i| app.fleet_display_name(i)),
        1 => app.fleet_location_title(),
        6 => app.pane_fleets_title().to_string(),
        other => crate::tiles::SHIP_TILES[other].title.to_string(),
    }
}

/// What goes inside one tile.
fn tile_body(app: &mut App, ui: &mut egui::Ui, index: usize) {
    match index {
        0 => summary(app, ui),
        1 => location(app, ui),
        2 => crate::views::planet::grid(ui, "waypoints", &app.fleet_waypoints_tile(), false),
        3 => {
            let task = app.fleet_task_tile();
            ui.label(egui::RichText::new(task).small());
        }
        4 => crate::views::planet::grid(ui, "fuel-cargo", &app.fleet_cargo_tile(), false),
        5 => crate::views::planet::grid(ui, "composition", &app.fleet_composition_tile(), false),
        _ => crate::views::fleets_here_body(app, ui),
    }
}

/// The tallest tile: the fleet itself, which the original draws as a picture.
///
/// `DrawFleetBitmap` blits the primary design's ship 64 pixels square with the
/// owner's race emblem over its bottom-left corner. Without the game's own
/// pictures the tile is the same words without the ship.
fn summary(app: &mut App, ui: &mut egui::Ui) {
    let Some(fleet) = app.pane_fleet() else {
        ui.label(egui::RichText::new("no fleet selected").weak().small());
        return;
    };
    let ships: i32 = fleet.stacks.iter().map(|s| s.count).sum();
    let owner = fleet.owner;
    let position = fleet.position;
    let picture = app.fleet_picture();
    let emblem = app.fleet_emblem(stars_formats::resources::art::EmblemSize::Medium);
    {
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
    }
}

/// The planet the fleet is at, or deep space.
fn location(app: &App, ui: &mut egui::Ui) {
    let at_planet = app.pane_fleet().is_some_and(|f| f.orbiting.is_some());
    ui.label(
        egui::RichText::new(if at_planet { "in orbit" } else { "under way" })
            .weak()
            .small(),
    );
}
