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
use stars_core::production::EtaMark;

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
    templates(app, ui);

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
    let schedule = app.production_schedule();
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
        // The year it will be finished, and the colour that goes with it.
        // `!` is the manual's red row: the item will practically never be
        // built (p. 7-7).
        let (when, mark) = schedule
            .get(index)
            .cloned()
            .unwrap_or_else(|| (String::new(), EtaMark::Ordinary));
        let mut text = egui::RichText::new(format!("{label}   {when}")).small();
        if auto {
            text = text.italics();
        }
        text = match mark {
            EtaMark::Never => text.color(egui::Color32::from_rgb(0xff, 0x6b, 0x6b)),
            EtaMark::AllNextYear => text.color(egui::Color32::from_rgb(0x5a, 0xd6, 0x8a)),
            EtaMark::FirstNextYear => text.color(egui::Color32::from_rgb(0xa3, 0xbf, 0x5a)),
            EtaMark::Idle => text.weak(),
            EtaMark::Ordinary => text,
        };
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

/// The production templates, reached from the **blue diamond**.
///
/// `DrawProductionDlg` puts a small raised blue diamond at the bottom left of
/// the dialog with `Apply or define a production template` beside it, remembers
/// its rectangle in `rcProdDiamond`, and `ProductionDlg` hit-tests that:
/// hovering shows the help cursor, a **left** click explains what to do, and a
/// **right** click brings up the menu.
fn templates(app: &mut App, ui: &mut egui::Ui) {
    /// What the original says when the diamond is left-clicked.
    const HELP: &str = "Right click on the blue diamond to apply a production template to \
                        this queue, or choose <Customize> to define a template based on the \
                        auto build items in the current queue.";

    let names: Vec<String> = (0..stars_formats::TEMPLATE_SLOTS)
        .map(|slot| app.production_template_name(slot))
        .collect();
    let usable: Vec<bool> = app
        .production_templates()
        .iter()
        .map(|t| t.queue.is_some())
        .collect();

    let mut apply = None;
    let mut customize = false;

    ui.horizontal(|ui| {
        let size = ui.text_style_height(&egui::TextStyle::Body);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
        diamond(ui, rect);
        // The original swaps in the arrow-and-question-mark cursor over it.
        let response = response.on_hover_cursor(egui::CursorIcon::Help);

        // A left click only tells you to use the right button.
        if response.clicked() {
            ui.memory_mut(|m| m.open_popup(egui::Id::new("diamond-help")));
        }
        egui::popup_below_widget(
            ui,
            egui::Id::new("diamond-help"),
            &response,
            egui::PopupCloseBehavior::CloseOnClick,
            |ui| {
                ui.set_max_width(320.0);
                ui.label(egui::RichText::new(HELP).small());
            },
        );

        // The right button brings up the menu: every template that has
        // something in it, then <Customize>.
        response.context_menu(|ui| {
            for (slot, name) in names.iter().enumerate() {
                if !usable.get(slot).copied().unwrap_or(false) {
                    continue;
                }
                if ui.button(name).clicked() {
                    apply = Some(slot);
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button("<Customize>").clicked() {
                customize = true;
                ui.close_menu();
            }
        });

        ui.label(
            egui::RichText::new("Apply or define a production template")
                .small()
                .strong(),
        );
    });

    if let Some(slot) = apply {
        app.production_apply_template(slot);
    }
    if customize {
        app.production_customize_open(0);
    }
    if app.production_customize_slot().is_some() {
        customize_panel(app, ui);
    }
}

/// The diamond itself: a small raised blue lozenge.
///
/// `DrawDiamond` (`1028:4b60`) walks it scanline by scanline, laying a
/// highlight along the upper-left edges and a shadow along the lower-right
/// before filling the middle. This paints the same three pieces as polygons.
fn diamond(ui: &egui::Ui, rect: egui::Rect) {
    let painter = ui.painter();
    let c = rect.center();
    let w = rect.width() / 2.0;
    let top = egui::pos2(c.x, rect.top());
    let bottom = egui::pos2(c.x, rect.bottom());
    let left = egui::pos2(c.x - w, c.y);
    let right = egui::pos2(c.x + w, c.y);

    painter.add(egui::Shape::convex_polygon(
        vec![top, right, bottom, left],
        egui::Color32::from_rgb(0x30, 0x60, 0xff),
        egui::Stroke::NONE,
    ));
    // The highlight runs up the left side, the shadow down the right.
    painter.line_segment(
        [left, top],
        egui::Stroke::new(1.0_f32, ui.visuals().widgets.inactive.fg_stroke.color),
    );
    painter.line_segment(
        [top, right],
        egui::Stroke::new(1.0_f32, ui.visuals().widgets.inactive.fg_stroke.color),
    );
    painter.line_segment(
        [right, bottom],
        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(0x40)),
    );
    painter.line_segment(
        [bottom, left],
        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(0x40)),
    );
}

/// The `<Customize>` dialog (`ZipProdDlg`, `10d0:5490`), drawn inline.
fn customize_panel(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut slot) = app.production_customize_slot() else {
        return;
    };
    let names: Vec<String> = (0..stars_formats::TEMPLATE_SLOTS)
        .map(|s| app.production_template_name(s))
        .collect();

    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(
            egui::RichText::new("Customize Production Templates")
                .small()
                .strong(),
        );
        ui.horizontal(|ui| {
            for (index, name) in names.iter().enumerate() {
                ui.radio_value(&mut slot, index, egui::RichText::new(name).small());
            }
        });
        app.production_customize_select(slot);

        // What the chosen template holds, and its research setting.
        let templates = app.production_templates();
        let queue = templates.get(slot).and_then(|t| t.queue.as_ref());
        for line in stars_core::production::template_lines(queue) {
            ui.label(egui::RichText::new(line).small());
        }
        ui.label(
            egui::RichText::new(if queue.is_some_and(|q| q.no_research) {
                "Don't contribute to research"
            } else {
                "Contribute to research"
            })
            .small()
            .weak(),
        );

        let editable = app.production_template_editable(slot);
        ui.horizontal(|ui| {
            if ui
                .button(egui::RichText::new("Import").small())
                .on_hover_text(
                    "Takes the auto-build items out of this planet's queue, in order, \
                     and makes them this template.",
                )
                .clicked()
            {
                let name = app.production_template_name(slot);
                app.production_import_template(slot, &name);
            }
            if ui
                .add_enabled(
                    editable,
                    egui::Button::new(egui::RichText::new("Delete").small()),
                )
                .on_disabled_hover_text(
                    "The default template cannot be deleted. To empty it, import an \
                     empty queue over it.",
                )
                .clicked()
            {
                app.production_delete_template(slot);
            }
            if ui.button(egui::RichText::new("OK").small()).clicked() {
                app.production_customize_close(true);
            }
            if ui
                .button(egui::RichText::new("Cancel").small())
                .on_hover_text("Puts every template back as it was.")
                .clicked()
            {
                app.production_customize_close(false);
            }
        });

        // Renaming, for a slot that has one to rename.
        if editable {
            let mut name = app.production_template_name(slot);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Name").small());
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut name)
                            .desired_width(160.0)
                            .char_limit(stars_formats::TEMPLATE_NAME_MAX),
                    )
                    .changed()
                {
                    app.production_rename_template(slot, &name);
                }
            });
        }
    });
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
