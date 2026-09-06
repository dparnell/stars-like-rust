//! Planet detail: the list on the left, the selected planet on the right.
//!
//! Two things here are worth knowing rather than reinventing. The habitability
//! figure is **two numbers** — what the planet is worth now and what it would
//! be worth terraformed as far as this race can take it — and the production
//! queue shows a **running balance**: each entry's count is what is left to
//! build, not what was ordered.

use crate::views::{colonists, player_colour};
use crate::App;

/// Draw the planet screen.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let selected = app.selection.planet;
    let rows: Vec<(i16, String, bool, Option<i16>)> = app
        .visible_planets()
        .into_iter()
        .map(|(p, owned)| {
            (
                p.id,
                p.name.unwrap_or("unnamed").to_string(),
                owned,
                p.owner,
            )
        })
        .collect();

    egui::SidePanel::left("planet_list")
        .resizable(true)
        .default_width(220.0)
        .show_inside(ui, |ui| {
            ui.heading(format!("{} planets", rows.len()));
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (id, name, owned, owner) in &rows {
                    let mut label = egui::RichText::new(format!("{name} ({id})"));
                    if let Some(owner) = owner {
                        label = label.color(player_colour(*owner));
                    } else {
                        label = label.weak();
                    }
                    if !owned {
                        label = label.italics();
                    }
                    if ui.selectable_label(selected == Some(*id), label).clicked() {
                        app.selection.planet = Some(*id);
                    }
                }
            });
        });

    // Chosen inside the panel, applied after it: the planet is borrowed from
    // `app` for the whole closure.
    let mut add: Option<u16> = None;

    egui::CentralPanel::default().show_inside(ui, |ui| {
        let Some(planet) = app.selected_planet() else {
            ui.label("No planet selected.");
            return;
        };
        let game = app.game.as_ref();
        let race = planet
            .owner
            .and_then(|o| usize::try_from(o).ok())
            .and_then(|o| game.and_then(|g| g.players.get(o)))
            .map(|p| (&p.race, p.research.levels));

        ui.heading(planet.name.unwrap_or("unnamed"));
        ui.label(format!("planet {}", planet.id));
        ui.separator();

        egui::Grid::new("planet_facts")
            .num_columns(2)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                ui.label("owner");
                match planet.owner {
                    Some(o) => {
                        ui.colored_label(player_colour(o), format!("player {o}"));
                    }
                    None => {
                        ui.label("unclaimed");
                    }
                }
                ui.end_row();

                ui.label("what we know");
                ui.label(match planet.detail {
                    stars_core::planet::Detail::Full => "everything",
                    stars_core::planet::Detail::Scanned => "scanned only",
                    stars_core::planet::Detail::Minimal => "seen at a distance",
                });
                ui.end_row();

                ui.label("environment");
                ui.label(format!(
                    "gravity {}, temperature {}, radiation {}",
                    planet.env[0], planet.env[1], planet.env[2]
                ));
                ui.end_row();

                if let Some(orig) = planet.env_orig {
                    ui.label("before terraforming");
                    ui.label(format!("{}, {}, {}", orig[0], orig[1], orig[2]));
                    ui.end_row();
                }

                if let Some((race, tech)) = race {
                    // Two figures: what it is worth now, and terraformed.
                    let now = stars_core::hab::pct_planet_desirability(planet, race);
                    let optimal = stars_core::terraform::optimal_env(planet, race, tech);
                    let after =
                        stars_core::ai::colonise::pct_planet_opt_value(planet, race, optimal);
                    ui.label("habitability");
                    ui.label(format!("{now}% now, {after}% terraformed"));
                    ui.end_row();

                    let reach = stars_core::terraform::reachable_band(planet, race, tech);
                    ui.label("terraforming reach");
                    ui.label(format!(
                        "{}..{}, {}..{}, {}..{}",
                        reach[0].0, reach[0].1, reach[1].0, reach[1].1, reach[2].0, reach[2].1
                    ));
                    ui.end_row();
                }

                ui.label("concentration");
                ui.label(format!(
                    "ir {}, bo {}, ge {}",
                    planet.min_conc[0], planet.min_conc[1], planet.min_conc[2]
                ));
                ui.end_row();

                if planet.detail.is_full() {
                    ui.label("surface minerals");
                    ui.label(format!(
                        "ir {}, bo {}, ge {} kT",
                        planet.surface_min[0], planet.surface_min[1], planet.surface_min[2]
                    ));
                    ui.end_row();

                    ui.label("population");
                    ui.label(colonists(planet.pop));
                    ui.end_row();

                    ui.label("installations");
                    ui.label(format!(
                        "{} mines, {} factories, {} defences",
                        planet.mines, planet.factories, planet.defenses
                    ));
                    ui.end_row();

                    ui.label("starbase");
                    ui.label(if planet.starbase { "yes" } else { "no" });
                    ui.end_row();
                }
            });

        // The queue, which the player can change. Everything above is read
        // from the file; this is the one place a screen writes back.
        let owned_in_full = planet.detail.is_full() && planet.owner.is_some();
        let queue: Vec<(i32, u16, bool, i32)> = planet
            .queue
            .iter()
            .map(|e| (e.count, e.item, e.ship, e.completion))
            .collect();

        if owned_in_full {
            ui.separator();
            ui.heading("production queue");
            ui.label(
                egui::RichText::new(
                    "Counts are what is left to build, not what was ordered — a queue \
                     entry is a running balance. Items are worked from the top, so what \
                     is first takes its resources first.",
                )
                .weak()
                .small(),
            );

            let mut remove: Option<usize> = None;
            let mut shift: Option<(usize, isize)> = None;
            egui::Grid::new("queue")
                .num_columns(5)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    for (index, (count, item, ship, completion)) in queue.iter().enumerate() {
                        ui.label(format!("{count} x"));
                        ui.label(if *ship {
                            format!("design {item}")
                        } else {
                            item_name(*item)
                        });
                        ui.label(format!("{completion}% paid"));
                        ui.horizontal(|ui| {
                            if ui.small_button("▲").clicked() {
                                shift = Some((index, -1));
                            }
                            if ui.small_button("▼").clicked() {
                                shift = Some((index, 1));
                            }
                        });
                        if ui.small_button("remove").clicked() {
                            remove = Some(index);
                        }
                        ui.end_row();
                    }
                });
            if queue.is_empty() {
                ui.label(egui::RichText::new("nothing queued").weak());
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("add:");
                for (id, name) in app.buildable_items() {
                    if ui.button(name).clicked() {
                        add = Some(id);
                    }
                }
            });

            if let Some((index, delta)) = shift {
                app.queue_move(index, delta);
            }
            if let Some(index) = remove {
                app.queue_remove(index);
            }
        }
    });

    if let Some(item) = add {
        app.queue_add(item, 1);
    }
}

/// The name of a planetary production item.
pub(crate) fn item_name(item: u16) -> String {
    stars_core::production::item_name(item).to_string()
}
