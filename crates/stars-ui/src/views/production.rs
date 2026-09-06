//! The Production dialog.
//!
//! `ChangeProduction` (`10d0:0000`) opens it and `ProdCommandHandler`
//! (`10d0:1994`) runs it. Two lists side by side — the **inventory** of
//! everything this planet can build, and the planet's **queue** — with Add and
//! Remove between them, Item Up and Item Down beside the queue, and the cost of
//! whatever is selected underneath.
//!
//! See `docs/ui/production.md`.

use crate::App;

/// Draw the dialog's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    if app.production.is_none() {
        return;
    }
    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
    let step = App::production_step(ctrl, shift);

    title(app, ui);
    ui.separator();

    ui.horizontal_top(|ui| {
        // Left: what the planet can build.
        ui.vertical(|ui| {
            ui.set_min_width(240.0);
            ui.label(egui::RichText::new("Inventory").small().strong());
            egui::ScrollArea::vertical()
                .id_source("production-inventory")
                .max_height(260.0)
                .show(ui, |ui| inventory(app, ui, step));
        });

        // Middle: the two buttons that move items between the lists. The
        // modifiers change how many, so the labels say so.
        ui.vertical(|ui| {
            ui.add_space(24.0);
            if ui
                .button("Add ▶")
                .on_hover_text(
                    "Shift for ten, Ctrl for a hundred, both for as many as possible. \
                     The item goes under whichever queue row is selected.",
                )
                .clicked()
            {
                app.production_add(step);
            }
            if ui
                .button("◀ Remove")
                .on_hover_text("Shift for ten, Ctrl for a hundred, both for all of them.")
                .clicked()
            {
                app.production_remove(step);
            }
            if step != 1 {
                ui.label(egui::RichText::new(format!("×{step}")).small().weak());
            }
        });

        // Right: the queue.
        ui.vertical(|ui| {
            ui.set_min_width(240.0);
            ui.label(egui::RichText::new("Production queue").small().strong());
            egui::ScrollArea::vertical()
                .id_source("production-queue")
                .max_height(260.0)
                .show(ui, |ui| queue(app, ui, step));
        });

        ui.vertical(|ui| {
            ui.add_space(24.0);
            if ui.button("Item Up").clicked() {
                app.production_move(true);
            }
            if ui.button("Item Down").clicked() {
                app.production_move(false);
            }
            ui.add_space(6.0);
            if ui
                .button("Clear")
                .on_hover_text("Empties the queue. Anything part-built loses what it has spent.")
                .clicked()
            {
                app.production_clear();
            }
        });
    });

    ui.separator();
    cost(app, ui);

    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("OK").clicked() {
            app.production_ok();
        }
        if ui.button("Cancel").clicked() {
            app.production_cancel();
        }
        ui.separator();
        // Prev and Next write this planet's queue out and move on, which is
        // what `FinishProduction(1)` does before `SelectAdjPlanet`.
        let hint = "Shift jumps to the next planet with a starbase.";
        if ui.button("◀ Prev").on_hover_text(hint).clicked() {
            app.production_step_planet(false, shift);
        }
        if ui.button("Next ▶").on_hover_text(hint).clicked() {
            app.production_step_planet(true, shift);
        }
    });
}

/// The planet's name, and the checkbox that shares the dialog's bottom row in
/// the original.
fn title(app: &mut App, ui: &mut egui::Ui) {
    let name = app
        .production
        .as_ref()
        .map(|d| d.planet)
        .and_then(|id| {
            app.game
                .as_ref()?
                .planets
                .iter()
                .find(|p| p.id == id)
                .map(|p| {
                    p.name
                        .map_or_else(|| format!("Planet {id}"), str::to_string)
                })
        })
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(name).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let Some(dialog) = app.production.as_mut() else {
                return;
            };
            ui.checkbox(
                &mut dialog.no_research,
                "Contribute only leftover resources to research",
            );
        });
    });
}

fn inventory(app: &mut App, ui: &mut egui::Ui, step: i32) {
    let rows = app.production_inventory();
    if rows.is_empty() {
        ui.label(egui::RichText::new("nothing to build").weak().small());
        return;
    }
    let selected = app.production.as_ref().map_or(0, |d| d.inventory_index);
    let mut clicked = None;
    let mut added = false;
    for (index, row) in rows.iter().enumerate() {
        // An auto-build item is italic and labelled, as `FillProdSrcLB` draws
        // it; a unique item shows how many are left.
        let mut text = egui::RichText::new(if row.auto {
            format!("{} (Auto Build)", row.name)
        } else if row.unlimited() {
            row.name.clone()
        } else {
            format!("{}  ({})", row.name, row.count)
        })
        .small();
        if row.auto {
            text = text.italics();
        }
        let response = ui.selectable_label(index == selected, text);
        if response.clicked() {
            clicked = Some(index);
        }
        if response.double_clicked() {
            clicked = Some(index);
            added = true;
        }
    }
    if let Some(index) = clicked {
        if let Some(dialog) = app.production.as_mut() {
            dialog.inventory_index = index;
        }
    }
    if added {
        app.production_add(step);
    }
}

fn queue(app: &mut App, ui: &mut egui::Ui, step: i32) {
    let rows = app.production_queue_rows();
    let selected = app.production.as_ref().and_then(|d| d.queue_index);

    // The original's first line, which is where "add to the front" lives.
    let mut clicked: Option<Option<usize>> = None;
    let mut removed = false;
    if ui
        .selectable_label(
            selected.is_none(),
            egui::RichText::new("— Top of the Queue —").small().weak(),
        )
        .clicked()
    {
        clicked = Some(None);
    }
    for (index, (count, name)) in rows.iter().enumerate() {
        let auto = app
            .production
            .as_ref()
            .and_then(|d| d.queue.get(index))
            .is_some_and(|e| e.is_auto());
        // An auto-build row reads "up to N"; auto alchemy reads "as needed",
        // because its count is only a placeholder.
        let label = if auto {
            if app
                .production
                .as_ref()
                .and_then(|d| d.queue.get(index))
                .is_some_and(|e| e.item == stars_core::production::item::AUTO_ALCHEMY)
            {
                format!("{name} as needed")
            } else {
                format!("{name} up to {count}")
            }
        } else {
            format!("{count}  {name}")
        };
        let mut text = egui::RichText::new(label).small();
        if auto {
            text = text.italics();
        }
        let response = ui.selectable_label(selected == Some(index), text);
        if response.clicked() {
            clicked = Some(Some(index));
        }
        if response.double_clicked() {
            clicked = Some(Some(index));
            removed = true;
        }
    }
    if rows.is_empty() {
        ui.label(egui::RichText::new("nothing queued").weak().small());
    }
    if let Some(index) = clicked {
        if let Some(dialog) = app.production.as_mut() {
            dialog.queue_index = index;
        }
    }
    if removed {
        app.production_remove(step);
    }
}

/// What the selected item costs, against what the planet has on the surface.
fn cost(app: &mut App, ui: &mut egui::Ui) {
    let rows = app.production_cost_rows();
    if rows.is_empty() {
        return;
    }
    egui::Grid::new("production-cost")
        .num_columns(2)
        .spacing([12.0, 1.0])
        .show(ui, |ui| {
            for (label, value) in &rows {
                ui.label(egui::RichText::new(label).small());
                ui.label(egui::RichText::new(value).small());
                ui.end_row();
            }
        });
}
