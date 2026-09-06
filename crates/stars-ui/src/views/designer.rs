//! The Ship and Starbase Designer.
//!
//! `ShipBuilder` (`10c8:008c`) opens dialog 0x5c and `SlotDlg` (`10c8:0550`)
//! runs it. One window with two faces:
//!
//! * a **browser** — two radio groups, a dropdown, and Copy / Edit / Delete —
//!   over the player's designs, the hulls they have researched, or the designs
//!   they have seen an opponent fly;
//! * an **editor**, reached by Copy or Edit, with the parts list on the left
//!   and the hull's schematic on the right, and OK / Cancel at the bottom.
//!
//! The schematic is not drawn by code: each hull carries a table of cells
//! (`HULDEF.rgbrc`) saying where its slots sit on a grid, and the designer
//! lays them out from that (`UpdateSlotGlobals`, `10c8:6528`). This view uses
//! the same table, so every hull comes out in its own shape.
//!
//! See `docs/ui/ship-design.md` for what is and is not reproduced.

use crate::{App, DesignView, DesignerDrag, PartRow};

/// One cell of the schematic grid, in points. The original's is 32 pixels and
/// a slot is two cells square.
const CELL: f32 = 17.0;

/// Draw the designer's contents into whatever the shell gives it.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    if app.designer.is_none() {
        return;
    }
    let editing = app.designer.as_ref().is_some_and(|d| d.editing.is_some());

    if editing {
        editor(app, ui);
    } else {
        browser(app, ui);
    }
}

// --- browsing ------------------------------------------------------------

fn browser(app: &mut App, ui: &mut egui::Ui) {
    // Components view is the odd one out: there is no design to show, so the
    // parts list fills the space the schematic had and the dropdown becomes
    // the category filter (`DrawSlotDlg` returns early on `mdBuildComp`).
    let components = app
        .designer
        .as_ref()
        .is_some_and(|d| d.view == DesignView::Components);

    ui.horizontal_top(|ui| {
        // The left of the window is the design itself: its picture and its
        // schematic, with the plaque underneath.
        ui.vertical(|ui| {
            ui.set_min_width(340.0);
            if components {
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| parts_list(app, ui));
                return;
            }
            picture(app, ui, false);
            schematic(app, ui, false);
            if let Some((alive, built)) = app.designer_plaque() {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(format!("{alive} of {built}"))
                        .small()
                        .strong(),
                );
            }
        });

        ui.separator();

        ui.vertical(|ui| {
            ui.set_min_width(250.0);
            radios(app, ui);
            ui.add_space(6.0);
            if components {
                filter_dropdown(app, ui);
            } else {
                dropdown(app, ui);
            }
            ui.add_space(6.0);
            buttons(app, ui);
        });
    });

    ui.separator();
    stats(app, ui);

    if let Some(question) = app.designer.as_ref().and_then(|d| d.confirm.clone()) {
        ui.separator();
        ui.label(egui::RichText::new(question).color(egui::Color32::from_rgb(0xff, 0xc0, 0x60)));
        ui.horizontal(|ui| {
            if ui.button("Yes, delete it").clicked() {
                app.designer_delete();
            }
            if ui.button("No").clicked() {
                if let Some(designer) = app.designer.as_mut() {
                    designer.confirm = None;
                }
            }
        });
    }

    ui.separator();
    ui.horizontal(|ui| {
        // In the browser the right-hand button reads Done, not Cancel
        // (`ShowMainControls`).
        if ui.button("Done").clicked() {
            app.close_designer();
        }
    });
}

/// The two radio groups: **Design** (ship or starbase) and **View**.
fn radios(app: &mut App, ui: &mut egui::Ui) {
    let Some(designer) = app.designer.as_ref() else {
        return;
    };
    let mut starbase = designer.starbase;
    let mut view = designer.view;

    ui.group(|ui| {
        ui.label(egui::RichText::new("Design").small().strong());
        ui.horizontal(|ui| {
            ui.radio_value(&mut starbase, false, "Ship");
            ui.radio_value(&mut starbase, true, "Starbase");
        });
    });
    ui.group(|ui| {
        ui.label(egui::RichText::new("View").small().strong());
        for choice in DesignView::ALL {
            ui.radio_value(&mut view, choice, choice.title());
        }
    });

    if let Some(designer) = app.designer.as_mut() {
        // Either radio resets the selection, as the original does by refilling
        // the dropdown and sending it back to the top.
        if designer.starbase != starbase || designer.view != view {
            designer.starbase = starbase;
            designer.view = view;
            designer.selected = 0;
            designer.filter = 0;
            designer.confirm = None;
        }
    }
}

fn dropdown(app: &mut App, ui: &mut egui::Ui) {
    let list = app.designer_list();
    let Some(designer) = app.designer.as_mut() else {
        return;
    };
    if list.is_empty() {
        ui.label(egui::RichText::new("No Designs").weak());
        return;
    }
    designer.selected = designer.selected.min(list.len() - 1);
    let mut selected = designer.selected;
    egui::ComboBox::from_id_source("designer-dd")
        .width(230.0)
        .show_index(ui, &mut selected, list.len(), |i| list[i].clone());
    if selected != designer.selected {
        designer.selected = selected;
        designer.confirm = None;
    }
}

fn buttons(app: &mut App, ui: &mut egui::Ui) {
    let can_copy = app.designer_can_copy();
    let can_edit = app.designer_can_edit();
    let can_delete = app.designer_can_delete();

    if ui
        .add_enabled(can_copy, egui::Button::new("Copy Selected Design"))
        .on_disabled_hover_text(
            "Either nothing is selected, or every design slot is full — sixteen ships \
             and ten starbases. Delete one first.",
        )
        .clicked()
    {
        app.designer_copy();
    }
    if ui
        .add_enabled(can_edit, egui::Button::new("Edit Selected Design"))
        .on_disabled_hover_text(
            "Only a design no ship has been built to can be edited. Copy it instead.",
        )
        .clicked()
    {
        app.designer_edit();
    }
    if ui
        .add_enabled(can_delete, egui::Button::new("Delete Design"))
        .clicked()
    {
        // Deleting destroys every ship built to the design and returns no
        // minerals, so the original asks first and so does this.
        match app.designer_delete_warning() {
            Some(question) => {
                if let Some(designer) = app.designer.as_mut() {
                    designer.confirm = Some(question);
                }
            }
            None => app.designer_delete(),
        }
    }
}

// --- editing -------------------------------------------------------------

fn editor(app: &mut App, ui: &mut egui::Ui) {
    // A drag that finished this frame, resolved after the lists are drawn so
    // both ends have been laid out.
    let mut dropped_on_list: Option<DesignerDrag> = None;
    let mut dropped_on_slot: Option<usize> = None;

    ui.horizontal_top(|ui| {
        // Left: the category filter over the parts list. Dropping a part here
        // takes it off the design.
        ui.vertical(|ui| {
            ui.set_min_width(240.0);
            filter_dropdown(app, ui);
            let frame = egui::Frame::default().inner_margin(4.0);
            let (_, payload) = ui.dnd_drop_zone::<DesignerDrag, ()>(frame, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| parts_list(app, ui));
            });
            // `dnd_drop_zone` takes the payload itself, so this is the one
            // chance to read it.
            dropped_on_list = payload.map(|p| *p);
        });

        ui.separator();

        // Right: the name, the picture with its two arrows, and the schematic.
        ui.vertical(|ui| {
            ui.set_min_width(340.0);
            name_field(app, ui);
            picture(app, ui, true);
            dropped_on_slot = schematic(app, ui, true);
        });
    });

    if let Some(drag) = dropped_on_list {
        app.designer_drop_on_list(drag);
    }
    if let Some(target) = dropped_on_slot {
        if let Some(drag) = taken_payload(ui) {
            app.designer_drop_on_slot(drag, target);
        }
    }

    ui.separator();
    stats(app, ui);

    if let Some(complaint) = app.designer.as_ref().and_then(|d| d.complaint.clone()) {
        ui.separator();
        ui.label(egui::RichText::new(complaint).color(egui::Color32::from_rgb(0xff, 0x8a, 0x8a)));
    }

    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("OK").clicked() {
            app.designer_ok();
        }
        if ui.button("Cancel").clicked() {
            app.designer_cancel();
        }
    });
}

/// The payload of the drag that just ended, if it was one of ours.
fn taken_payload(ui: &egui::Ui) -> Option<DesignerDrag> {
    egui::DragAndDrop::take_payload::<DesignerDrag>(ui.ctx()).map(|p| *p)
}

fn filter_dropdown(app: &mut App, ui: &mut egui::Ui) {
    let names: Vec<&'static str> = app
        .designer_filters()
        .iter()
        .map(|(_, name)| *name)
        .collect();
    let Some(designer) = app.designer.as_mut() else {
        return;
    };
    designer.filter = designer.filter.min(names.len().saturating_sub(1));
    let mut selected = designer.filter;
    egui::ComboBox::from_id_source("designer-filter")
        .width(230.0)
        .show_index(ui, &mut selected, names.len(), |i| names[i].to_string());
    designer.filter = selected;
}

fn parts_list(app: &mut App, ui: &mut egui::Ui) {
    let parts = app.designer_parts();
    let held = held_count(ui);
    for (index, part) in parts.iter().enumerate() {
        let id = egui::Id::new(("designer-part", index));
        let drag = DesignerDrag {
            category: part.category,
            item: part.item,
            count: held,
            from_slot: None,
        };
        ui.dnd_drag_source(id, drag, |ui| {
            part_row(app, ui, part, index);
        });
    }
    if parts.is_empty() {
        ui.label(
            egui::RichText::new("nothing you can build yet")
                .weak()
                .small(),
        );
    }
}

fn part_row(app: &mut App, ui: &mut egui::Ui, part: &PartRow, index: usize) {
    let selected = app
        .designer
        .as_ref()
        .is_some_and(|d| d.selected_part == Some(index));
    let label = format!("{}  {}kT", part.name, part.mass);
    let cell = stars_core::parts::picture_cell(part.category, part.picture, 0);
    let mut clicked = false;
    ui.horizontal(|ui| {
        // The original lists a part with its picture beside it, at half the
        // size the browser shows.
        if let Some(cell) = cell {
            crate::art::draw(app, ui, cell, 32.0);
        }
        clicked = ui
            .selectable_label(selected, egui::RichText::new(label).small())
            .clicked();
    });
    if clicked {
        if let Some(designer) = app.designer.as_mut() {
            designer.selected_part = Some(index);
        }
    }
}

/// How many a drag from the list carries, given what is held down.
fn held_count(ui: &egui::Ui) -> u8 {
    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
    App::designer_drag_count(None, 1, ctrl, shift)
}

fn name_field(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut name) = app
        .designer
        .as_ref()
        .and_then(|d| d.editing.as_ref())
        .map(|e| e.design.name.clone())
    else {
        return;
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Name").small());
        if ui
            .add(
                egui::TextEdit::singleline(&mut name)
                    .desired_width(230.0)
                    .char_limit(stars_core::design::MAX_NAME),
            )
            .changed()
        {
            app.designer_rename(&name);
        }
    });
}

// --- the picture and the schematic ---------------------------------------

/// The hull picture, with the two arrows under it while editing.
///
/// `DrawFleetBitmap` blits the ship 64 pixels square in a sunken frame, with
/// the owner's race emblem over its bottom-left corner, and the arrows spin
/// between the four pictures the hull owns. Without a copy of the original to
/// read the bitmaps out of, the frame holds the picture's number instead —
/// which is still what the arrows change.
fn picture(app: &mut App, ui: &mut egui::Ui, editing: bool) {
    let Some(design) = app.designer_subject() else {
        return;
    };
    let hull = App::designer_hull(&design);
    let ship = stars_formats::resources::art::ship(
        u16::from(design.picture),
        stars_formats::resources::art::ShipSize::Large,
    );
    let emblem = app.emblem_of(
        app.local_player(),
        stars_formats::resources::art::EmblemSize::Medium,
    );
    ui.horizontal(|ui| {
        if app.has_art() {
            // The emblem sits over the ship's bottom-left corner, as the
            // original overlays it: sixteen pixels on the sixty-four.
            let corner = ui.cursor().min;
            crate::art::draw(app, ui, ship, 64.0);
            if let Some(emblem) = emblem {
                let ctx = ui.ctx().clone();
                if let Some(art) = app.art.as_mut() {
                    if let Some(image) = art.sprite(&ctx, emblem, 16.0) {
                        let at = egui::Rect::from_min_size(
                            corner + egui::vec2(0.0, 48.0),
                            egui::vec2(16.0, 16.0),
                        );
                        image.paint_at(ui, at);
                    }
                }
            }
        } else {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(72.0, 48.0), egui::Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(rect, 2.0, ui.visuals().faint_bg_color);
            painter.rect_stroke(rect, 2.0, ui.visuals().widgets.noninteractive.bg_stroke);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("#{}", design.picture),
                egui::FontId::proportional(14.0),
                ui.visuals().weak_text_color(),
            );
        }
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(hull.map_or("", |h| h.name))
                    .small()
                    .strong(),
            );
            if editing {
                ui.horizontal(|ui| {
                    if ui.small_button("◀").clicked() {
                        app.designer_next_picture(false);
                    }
                    if ui.small_button("▶").clicked() {
                        app.designer_next_picture(true);
                    }
                });
            }
        });
    });
}

/// The hull schematic. Returns the slot a drag was dropped on, if any.
fn schematic(app: &mut App, ui: &mut egui::Ui, editing: bool) -> Option<usize> {
    let slots = app.designer_schematic();
    let design = app.designer_subject()?;
    let hull = App::designer_hull(&design)?;
    if slots.is_empty() {
        return None;
    }

    // The grid is as wide and tall as the furthest cell any slot or the hold
    // reaches, plus the two cells a slot occupies.
    let mut cols = 0;
    let mut rows = 0;
    for s in &slots {
        cols = cols.max(s.cell.0 + 2);
        rows = rows.max(s.cell.1 + 2);
    }
    if let Some((_, (right, bottom))) = hull.cargo_cells() {
        cols = cols.max(right);
        rows = rows.max(bottom);
    }

    let size = egui::vec2(cols as f32 * CELL, rows as f32 * CELL);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let origin = rect.min;
    let cell_rect = |(col, row): (i32, i32)| {
        egui::Rect::from_min_size(
            origin + egui::vec2(col as f32 * CELL, row as f32 * CELL),
            egui::vec2(CELL * 2.0, CELL * 2.0),
        )
    };

    cargo_box(app, ui, hull, &design, origin);

    let mut dropped = None;
    for (index, slot) in slots.iter().enumerate() {
        let target = cell_rect(slot.cell);
        let response = slot_widget(app, ui, target, slot, index, editing);
        if response {
            dropped = Some(index);
        }
    }
    dropped
}

/// The cargo space: three lines in a box, or a circle on the two starbases the
/// original draws round.
fn cargo_box(
    app: &App,
    ui: &mut egui::Ui,
    hull: &'static stars_core::components::Hull,
    design: &stars_core::design::ShipDesign,
    origin: egui::Pos2,
) {
    let Some(((left, top), (right, bottom))) = hull.cargo_cells() else {
        return;
    };
    let rect = egui::Rect::from_min_max(
        origin + egui::vec2(left as f32 * CELL, top as f32 * CELL),
        origin + egui::vec2(right as f32 * CELL, bottom as f32 * CELL),
    );
    let painter = ui.painter();
    let stroke = ui.visuals().widgets.noninteractive.fg_stroke;

    let starbase = design.is_starbase();
    // `DrawSlotDlg` draws the dock round for the Space Dock and the Death Star
    // and square for everything else — including the Space Station and the
    // Ultra Station, which also have docks. That is what the binary compares
    // (`ihuldef == 33 || ihuldef == 36`), so it is what happens here.
    let round = starbase && (design.hull_id == 33 || design.hull_id == 36);
    if round {
        painter.circle_stroke(rect.center(), rect.width().min(rect.height()) / 2.0, stroke);
    } else {
        painter.rect_stroke(rect, 0.0, stroke);
    }

    let capacity = if hull.unlimited_cargo() {
        "Unlimited".to_string()
    } else if starbase {
        format!("{}kT", hull.cargo_max)
    } else {
        // A ship's hold grows with cargo pods, so the figure is the design's.
        format!("{}kT", design.cargo_capacity().unwrap_or(0))
    };
    // A ship reads Cargo / <n>kT / max; a starbase reads <n>kT / Space / Dock.
    let lines = if starbase {
        [capacity, "Space".into(), "Dock".into()]
    } else {
        ["Cargo".into(), capacity, "max".into()]
    };
    let colour = ui.visuals().text_color();
    let font = egui::FontId::proportional(9.0);
    for (i, line) in lines.iter().enumerate() {
        let y = rect.top() + rect.height() * (i as f32 + 0.5) / 3.0;
        painter.text(
            egui::pos2(rect.center().x, y),
            egui::Align2::CENTER_CENTER,
            line,
            font.clone(),
            colour,
        );
    }
    let _ = app;
}

/// One slot. Returns whether a drag was dropped on it.
fn slot_widget(
    app: &mut App,
    ui: &mut egui::Ui,
    rect: egui::Rect,
    slot: &crate::SchematicSlot,
    index: usize,
    editing: bool,
) -> bool {
    let id = egui::Id::new(("designer-slot", index));
    let response = ui.interact(rect, id, egui::Sense::click_and_drag());
    let selected = app
        .designer
        .as_ref()
        .is_some_and(|d| d.selected_slot == Some(index));

    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, ui.visuals().extreme_bg_color);
    let stroke = if selected {
        egui::Stroke::new(2.0_f32, ui.visuals().selection.bg_fill)
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    painter.rect_stroke(rect, 1.0, stroke);

    let title = match &slot.fitted {
        Some((name, _)) => name.clone(),
        None => App::designer_slot_kinds(slot.allowed),
    };
    painter.text(
        egui::pos2(rect.center().x, rect.top() + rect.height() * 0.35),
        egui::Align2::CENTER_CENTER,
        shorten(&title),
        egui::FontId::proportional(9.0),
        ui.visuals().text_color(),
    );
    painter.text(
        egui::pos2(rect.center().x, rect.bottom() - 8.0),
        egui::Align2::CENTER_CENTER,
        &slot.label,
        egui::FontId::proportional(9.0),
        ui.visuals().weak_text_color(),
    );

    if response.clicked() {
        if let Some(designer) = app.designer.as_mut() {
            designer.selected_slot = Some(index);
        }
    }

    if !editing {
        return false;
    }

    // Dragging a fitted component off its slot, and dropping one on.
    if let Some((_, count)) = &slot.fitted {
        if response.drag_started() {
            let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
            if let Some(part) = app
                .designer_subject()
                .and_then(|d| d.slots.get(index).copied())
                .and_then(|s| stars_core::design::slot_part(&s))
            {
                let drag = DesignerDrag {
                    category: part.category,
                    item: part.item,
                    count: App::designer_drag_count(Some(index), *count, ctrl, shift),
                    from_slot: Some(index),
                };
                egui::DragAndDrop::set_payload(ui.ctx(), drag);
            }
        }
    }

    let hovered = ui.rect_contains_pointer(rect);
    let released = ui.input(|i| i.pointer.any_released());
    hovered && released && egui::DragAndDrop::has_any_payload(ui.ctx())
}

/// Slot pictures are small; a long list of categories will not fit.
fn shorten(text: &str) -> String {
    if text.chars().count() <= 18 {
        return text.to_string();
    }
    text.chars().take(17).chain(std::iter::once('…')).collect()
}

// --- the numbers ---------------------------------------------------------

fn stats(app: &mut App, ui: &mut egui::Ui) {
    let Some(design) = app.designer_subject() else {
        return;
    };
    let hulls = app
        .designer
        .as_ref()
        .is_some_and(|d| d.view == DesignView::Hulls && d.editing.is_none());
    // `Cost of one <name>`, with " Hull" appended while browsing bare hulls.
    let suffix = if hulls { " Hull" } else { "" };
    ui.label(
        egui::RichText::new(format!("Cost of one {}{suffix}", design.name))
            .small()
            .strong(),
    );

    let cost = app.designer_cost_rows();
    let stats = app.designer_stat_rows();
    ui.horizontal_top(|ui| {
        egui::Grid::new("designer-cost")
            .num_columns(2)
            .spacing([10.0, 1.0])
            .show(ui, |ui| {
                for (label, value) in &cost {
                    ui.label(egui::RichText::new(label).small());
                    ui.label(egui::RichText::new(value).small());
                    ui.end_row();
                }
            });
        ui.add_space(20.0);
        egui::Grid::new("designer-stats")
            .num_columns(2)
            .spacing([10.0, 1.0])
            .show(ui, |ui| {
                for (label, value) in &stats {
                    ui.label(egui::RichText::new(label).small());
                    ui.label(egui::RichText::new(value).small());
                    ui.end_row();
                }
            });
    });
}
