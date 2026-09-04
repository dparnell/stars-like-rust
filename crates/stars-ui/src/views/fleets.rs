//! The fleet list and the selected fleet's detail.

use crate::views::{colonists, player_colour};
use crate::App;

/// Draw the fleet screen.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some(game) = app.game.as_ref() else {
        return;
    };
    let rows: Vec<(usize, String, i16)> = game
        .fleets
        .iter()
        .enumerate()
        .map(|(i, f)| (i, format!("fleet {} — {} ships", f.id, f.ships()), f.owner))
        .collect();
    let selected = app.selection.fleet;

    egui::SidePanel::left("fleet_list")
        .resizable(true)
        .default_width(220.0)
        .show_inside(ui, |ui| {
            ui.heading(format!("{} fleets", rows.len()));
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (index, label, owner) in &rows {
                    let text = egui::RichText::new(label).color(player_colour(*owner));
                    if ui
                        .selectable_label(selected == Some(*index), text)
                        .clicked()
                    {
                        app.selection.fleet = Some(*index);
                    }
                }
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        let Some(game) = app.game.as_ref() else {
            return;
        };
        let Some(fleet) = app.selection.fleet.and_then(|i| game.fleets.get(i)) else {
            ui.label("No fleet selected.");
            return;
        };
        ui.heading(format!("Fleet {}", fleet.id));
        ui.separator();

        egui::Grid::new("fleet_facts")
            .num_columns(2)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                ui.label("owner");
                ui.colored_label(
                    player_colour(fleet.owner),
                    format!("player {}", fleet.owner),
                );
                ui.end_row();

                ui.label("position");
                ui.label(format!("({}, {})", fleet.position.x, fleet.position.y));
                ui.end_row();

                ui.label("in orbit of");
                match fleet.orbiting {
                    Some(id) => {
                        let name = game
                            .planets
                            .iter()
                            .chain(game.known_planets.iter())
                            .find(|p| p.id == i16::try_from(id).unwrap_or(-1))
                            .and_then(|p| p.name)
                            .unwrap_or("unnamed");
                        ui.label(format!("{name} ({id})"));
                    }
                    None => {
                        ui.label("deep space");
                    }
                }
                ui.end_row();

                ui.label("cargo");
                ui.label(format!(
                    "ir {}, bo {}, ge {} kT, {} colonists, {} fuel",
                    fleet.cargo.minerals[0],
                    fleet.cargo.minerals[1],
                    fleet.cargo.minerals[2],
                    colonists(fleet.cargo.colonists),
                    fleet.cargo.fuel
                ));
                ui.end_row();

                if let Some(warp) = fleet.warp {
                    ui.label("warp");
                    ui.label(warp.to_string());
                    ui.end_row();
                }
            });

        ui.separator();
        ui.heading("ships");
        egui::Grid::new("stacks")
            .num_columns(2)
            .spacing([16.0, 2.0])
            .show(ui, |ui| {
                for stack in &fleet.stacks {
                    ui.label(format!("design {}", stack.design));
                    ui.label(format!("{}", stack.count));
                    ui.end_row();
                }
            });

        if fleet.waypoints.len() > 1 {
            ui.separator();
            ui.heading("waypoints");
            for (i, waypoint) in fleet.waypoints.iter().enumerate() {
                ui.label(format!(
                    "{i}: ({}, {}) warp {} task {}",
                    waypoint.position.x, waypoint.position.y, waypoint.warp, waypoint.task
                ));
            }
        }
    });
}
