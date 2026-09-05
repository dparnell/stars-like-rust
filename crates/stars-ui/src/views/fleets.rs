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
    // Chosen inside the panels, applied after: the game is borrowed for the
    // whole of each closure.
    let mut transfer: Option<(usize, usize, i32)> = None;
    let mut destination: Option<(usize, i16)> = None;
    let mut task: Option<(usize, u8)> = None;
    let mut transport: Option<(usize, usize, stars_formats::XferAction)> = None;
    let mut split: Option<(usize, u8, i32)> = None;
    let mut merge: Option<(usize, usize)> = None;
    let mut rename: Option<usize> = None;
    let mut plan: Option<(usize, u8)> = None;
    let mut repeat: Option<(usize, bool)> = None;
    let mut warp = app.warp;
    let mut name = app.fleet_name.clone();

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
        match &fleet.name {
            Some(given) => ui.heading(format!("{given} (fleet {})", fleet.id)),
            None => ui.heading(format!("Fleet {}", fleet.id)),
        };
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
        let designs = game
            .designs
            .get(usize::try_from(fleet.owner).unwrap_or(usize::MAX));
        let index = app.selection.fleet.unwrap_or(0);
        egui::Grid::new("stacks")
            .num_columns(3)
            .spacing([16.0, 2.0])
            .show(ui, |ui| {
                for stack in &fleet.stacks {
                    let label = designs
                        .and_then(|d| d.get(usize::from(stack.design)))
                        .filter(|d| !d.name.is_empty())
                        .map_or_else(|| format!("design {}", stack.design), |d| d.name.clone());
                    ui.label(label);
                    ui.label(format!("{}", stack.count));
                    // Splitting one ship off is the whole of what a split is;
                    // the game writes it as a fleet-to-fleet ship transfer.
                    if stack.count > 1 && ui.small_button("split one off").clicked() {
                        split = Some((index, stack.design, 1));
                    }
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

        // --- orders
        let orbiting = fleet.orbiting;
        // Fleets of the same owner sitting on the same spot can be merged.
        let mergeable: Vec<(usize, String)> = game
            .fleets
            .iter()
            .enumerate()
            .filter(|(i, f)| *i != index && f.owner == fleet.owner && f.position == fleet.position)
            .map(|(i, f)| (i, format!("fleet {} ({} ships)", f.id, f.ships())))
            .collect();
        let destinations: Vec<(i16, String)> = game
            .planets
            .iter()
            .chain(game.known_planets.iter())
            .filter(|p| p.position.is_some())
            .map(|p| (p.id, format!("{} ({})", p.name.unwrap_or("unnamed"), p.id)))
            .collect();

        ui.separator();
        ui.heading("orders");

        if let Some(planet) = orbiting {
            ui.label(format!("cargo, to and from planet {planet}"));
            ui.label(
                egui::RichText::new(
                    "A transfer happens at once and is logged, which is what the game \
                     does: an order file records what the client already did.",
                )
                .weak()
                .small(),
            );
            for (kind, name) in [
                (0usize, "ironium"),
                (1, "boranium"),
                (2, "germanium"),
                (3, "colonists"),
            ] {
                ui.horizontal(|ui| {
                    ui.label(format!("{name}:"));
                    if ui.small_button("load 100").clicked() {
                        transfer = Some((index, kind, 100));
                    }
                    if ui.small_button("load 10").clicked() {
                        transfer = Some((index, kind, 10));
                    }
                    if ui.small_button("unload 10").clicked() {
                        transfer = Some((index, kind, -10));
                    }
                    if ui.small_button("unload all").clicked() {
                        let held = match kind {
                            3 => fleet.cargo.colonists,
                            k => fleet.cargo.minerals[k],
                        };
                        transfer = Some((index, kind, -held));
                    }
                });
            }
        } else {
            ui.label(egui::RichText::new("not in orbit, so nothing to load from").weak());
        }

        ui.add_space(6.0);
        // What to do on arrival. A task rides the waypoint the fleet is
        // heading to and is consumed when it runs.
        let current = fleet
            .waypoints
            .last()
            .map_or(stars_formats::task::NONE, |w| w.task);
        ui.horizontal(|ui| {
            ui.label("on arrival:");
            for id in [
                stars_formats::task::NONE,
                stars_formats::task::COLONIZE,
                stars_formats::task::TRANSPORT,
                stars_formats::task::REMOTE_MINING,
                stars_formats::task::SCRAP,
                stars_formats::task::ROUTE,
                stars_formats::task::LAY_MINES,
                stars_formats::task::PATROL,
            ] {
                if ui
                    .selectable_label(current == id, stars_formats::task::name(id))
                    .clicked()
                {
                    task = Some((index, id));
                }
            }
        });
        if current == stars_formats::task::TRANSPORT {
            let orders = fleet.waypoints.last().and_then(|w| w.transport);
            for (kind, name) in [
                (0usize, "ironium"),
                (1, "boranium"),
                (2, "germanium"),
                (3, "colonists"),
            ] {
                let now = orders.map_or(stars_formats::XferAction::None, |o| o.items[kind].action);
                ui.horizontal(|ui| {
                    ui.label(format!("  {name}:"));
                    for action in [
                        stars_formats::XferAction::None,
                        stars_formats::XferAction::LoadAll,
                        stars_formats::XferAction::UnloadAll,
                    ] {
                        let label = match action {
                            stars_formats::XferAction::LoadAll => "load all",
                            stars_formats::XferAction::UnloadAll => "unload all",
                            _ => "nothing",
                        };
                        if ui.selectable_label(now == action, label).clicked() {
                            transport = Some((index, kind, action));
                        }
                    }
                });
            }
        }
        ui.label(
            egui::RichText::new(
                "Colonise, transport, scrap and route are performed on arrival; \
                 remote mining and laying mines in their own passes; patrol at the \
                 end of the year. Merging into another fleet is a task too, but it \
                 needs a fleet to merge into, so it is set from the list below \
                 rather than here. Giving a fleet away can be set but is not \
                 simulated.",
            )
            .weak()
            .small(),
        );

        ui.add_space(6.0);
        // The plans are the owner's own, names included: a player can rename,
        // retune and add them, and the file carries what they chose.
        let plans: Vec<String> = game
            .players
            .get(usize::try_from(fleet.owner).unwrap_or(usize::MAX))
            .map(|p| p.battle_plans.as_slice())
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(slot, plan)| {
                if plan.name.is_empty() {
                    format!("plan {slot}")
                } else {
                    plan.name.clone()
                }
            })
            .collect();
        ui.horizontal(|ui| {
            ui.label("battle plan:");
            for (slot, name) in plans.iter().enumerate() {
                let Ok(id) = u8::try_from(slot) else { continue };
                if ui.selectable_label(fleet.battle_plan == id, name).clicked() {
                    plan = Some((index, id));
                }
            }
        });
        {
            let mut repeating = fleet.repeat_orders;
            if ui
                .checkbox(&mut repeating, "repeat orders")
                .on_hover_text(
                    "The fleet returns to its first waypoint once it reaches its last. \
                     Carried in the file and in the order log; the turn generator does \
                     not act on it yet.",
                )
                .changed()
            {
                repeat = Some((index, repeating));
            }
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("name:");
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(160.0));
            if ui.button("rename").clicked() {
                rename = Some(index);
            }
        });

        if !mergeable.is_empty() {
            ui.horizontal(|ui| {
                ui.label("merge in:");
                egui::ComboBox::from_id_source("merge")
                    .selected_text("a fleet in the same place")
                    .show_ui(ui, |ui| {
                        for (other, label) in &mergeable {
                            if ui.selectable_label(false, label).clicked() {
                                merge = Some((index, *other));
                            }
                        }
                    });
            });
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("send to:");
            egui::ComboBox::from_id_source("destination")
                .selected_text("choose a planet")
                .show_ui(ui, |ui| {
                    for (id, label) in &destinations {
                        if ui.selectable_label(false, label).clicked() {
                            destination = Some((index, *id));
                        }
                    }
                });
            ui.label("at warp");
            ui.add(egui::DragValue::new(&mut warp).range(1..=10));
        });
    });

    if let Some((fleet, kind, amount)) = transfer {
        app.transfer_cargo(fleet, kind, amount);
    }
    if let Some((fleet, planet)) = destination {
        app.set_destination(fleet, planet, warp);
    }
    if let Some((fleet, id)) = task {
        app.set_task(fleet, id);
    }
    if let Some((fleet, kind, action)) = transport {
        app.set_transport(fleet, kind, action);
    }
    if let Some((fleet, design, count)) = split {
        app.split_fleet(fleet, design, count);
    }
    if let Some((survivor, absorbed)) = merge {
        app.merge_fleets(survivor, &[absorbed]);
    }
    if let Some(fleet) = rename {
        app.rename_fleet(fleet, name.trim());
    }
    if let Some((fleet, id)) = plan {
        app.set_battle_plan(fleet, id);
    }
    if let Some((fleet, repeating)) = repeat {
        app.set_repeat_orders(fleet, repeating);
    }
    app.warp = warp;
    app.fleet_name = name;
}
