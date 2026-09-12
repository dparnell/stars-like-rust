//! The Research dialog.
//!
//! `ResearchDlg` (`10d8:0000`), reached with **F5**. Two columns: the six
//! fields of study with the levels held and what the next levels will bring on
//! the left, and what is being researched now with the resource allocation on
//! the right.
//!
//! Nothing is committed until **Done**: the original edits three globals —
//! `pctResGlob`, `iResTechNow` and the dropdown — and writes all three at once,
//! logging a single two-byte `rtLogResearch` order. There is no Cancel: the
//! dialog template (`0x3480ee` in the executable) has just the two buttons,
//! **Done** (control 2, the one the tutorial's first year tells you to press)
//! and **Help**.
//!
//! See `docs/ui/research.md`.

use crate::App;

/// Draw the dialog's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    if app.research_dialog.is_none() {
        return;
    }
    app.drawn_scope = "research";

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(260.0);
            technology_status(app, ui);
            ui.add_space(6.0);
            benefits(app, ui);
        });
        // Not a separator: a vertical one inside a horizontal layout takes
        // the window's whole available height, buttons and all, and an
        // auto-sized window then grows to fit it, thirty pixels a frame,
        // until it runs off the screen with Done out of sight.
        ui.add_space(12.0);
        ui.vertical(|ui| {
            ui.set_min_width(360.0);
            currently_researching(app, ui);
            ui.add_space(6.0);
            allocation(app, ui);
            // The note under the allocation box. It is three lines tall in
            // the original and pressing it raises `grPopupString` with one of
            // two sentences, chosen by which half was pressed — see
            // `App::research_note_text`.
            let notes = app.research_notes();
            let mut raise = None;
            for (index, note) in notes.iter().enumerate() {
                let response = ui.add(
                    egui::Label::new(egui::RichText::new(note).small().weak())
                        .sense(egui::Sense::click()),
                );
                if response.is_pointer_button_down_on() {
                    if let Some(at) = ui.ctx().pointer_latest_pos() {
                        let lower = index > 0;
                        if let Some(text) = app.research_note_text(lower) {
                            raise = Some((crate::popup::Popup::Note(text), (at.x, at.y)));
                        }
                    }
                }
            }
            if let Some(popup) = raise {
                app.popup = Some(popup);
            }
        });
    });

    ui.separator();
    ui.horizontal(|ui| {
        if crate::views::flow_button(app, ui, "Done", true).clicked() {
            app.research_ok();
        }
        // The original's second button opens the help file, which this
        // project has no reader for yet; it is drawn where it belongs and
        // left dead rather than dropped.
        crate::views::flow_button(app, ui, "Help", false);
    });
}

/// **Technology Status**: the six fields, their levels, and which is being
/// studied. The radio buttons and the level column are one table in the
/// original, headed `Field Of Study` and `Level`.
fn technology_status(app: &mut App, ui: &mut egui::Ui) {
    let rows = app.research_levels();
    let Some(mut field) = app.research_dialog.map(|d| d.field) else {
        return;
    };
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(egui::RichText::new("Technology Status").small().strong());
        egui::Grid::new("research-levels")
            .num_columns(2)
            .spacing([12.0, 1.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Field Of Study").small().strong());
                ui.label(egui::RichText::new("Level").small().strong());
                ui.end_row();
                for (index, (name, level)) in rows.iter().enumerate() {
                    ui.radio_value(&mut field, index, egui::RichText::new(*name).small());
                    ui.label(egui::RichText::new(level.to_string()).small());
                    ui.end_row();
                }
            });
    });
    if let Some(dialog) = app.research_dialog.as_mut() {
        dialog.field = field;
    }
}

/// **Expected Research Benefits**: what the coming levels will unlock.
///
/// The original colours the list by how soon: the next level green, the three
/// after it red, and everything further off black. It is not a preview of the
/// selected field — see `stars_core::research::expected_benefits`.
fn benefits(app: &mut App, ui: &mut egui::Ui) {
    let benefits = app.research_benefits();
    let mut raise: Option<(crate::popup::Popup, (f32, f32))> = None;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(
            egui::RichText::new("Expected Research Benefits")
                .small()
                .strong(),
        );
        if benefits.is_empty() {
            ui.label(egui::RichText::new("nothing within reach").small().weak());
            return;
        }
        egui::ScrollArea::vertical()
            .id_source("research-benefits")
            .max_height(220.0)
            .show(ui, |ui| {
                for benefit in &benefits {
                    let colour = match benefit.levels_away {
                        1 => egui::Color32::from_rgb(0x5a, 0xd6, 0x8a),
                        2..=4 => egui::Color32::from_rgb(0xff, 0x8a, 0x8a),
                        _ => ui.visuals().text_color(),
                    };
                    let response = ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!(
                                "{}  ({})",
                                benefit.name, benefit.levels_away
                            ))
                            .small()
                            .color(colour),
                        )
                        .sense(egui::Sense::click()),
                    );
                    // Pressing a line puts up that component's details:
                    // `FTrackResearchDlg` (`10d8:1b5f`) fills `GlobalPD.part`
                    // from the entry's own `grhst` and `iItem` and raises
                    // `grPopupComponent`, which `DrawPopup` paints with the
                    // very routine the Technology Browser's panel uses.
                    if response.is_pointer_button_down_on() {
                        if let Some(at) = ui.ctx().pointer_latest_pos() {
                            raise = Some((
                                crate::popup::Popup::Component((benefit.category, benefit.item)),
                                (at.x, at.y),
                            ));
                        }
                    }
                }
            });
    });
    if let Some(popup) = raise {
        app.popup = Some(popup);
    } else if ui.input(|i| i.pointer.any_released()) {
        app.popup = None;
    }
    crate::views::popup::view(app, ui);
}

/// **Currently Researching**: the level in hand, its remaining cost, how long
/// it will take, and what to study next.
fn currently_researching(app: &mut App, ui: &mut egui::Ui) {
    let rows = app.research_status();
    let choices = App::research_next_choices();
    let Some(mut next) = app.research_dialog.map(|d| d.next) else {
        return;
    };
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(
            egui::RichText::new("Currently Researching")
                .small()
                .strong(),
        );
        egui::Grid::new("research-status")
            .num_columns(2)
            .spacing([10.0, 1.0])
            .show(ui, |ui| {
                for (label, value) in &rows {
                    ui.label(egui::RichText::new(label).small());
                    ui.label(egui::RichText::new(value).small());
                    ui.end_row();
                }
            });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Next field to research:").small());
            let selected = choices
                .iter()
                .position(|(choice, _)| *choice == next)
                .unwrap_or(0);
            let mut index = selected;
            egui::ComboBox::from_id_source("research-next")
                .width(160.0)
                .show_index(ui, &mut index, choices.len(), |i| choices[i].1.clone());
            if index != selected {
                next = choices[index].0;
            }
        });
    });
    if let Some(dialog) = app.research_dialog.as_mut() {
        dialog.next = next;
    }
}

/// **Resource Allocation**: what the empire makes, what it spent last year,
/// the share going to research, and what that will be worth next year.
fn allocation(app: &mut App, ui: &mut egui::Ui) {
    let rows = app.research_allocation();
    let Some(mut percent) = app.research_dialog.map(|d| d.percent) else {
        return;
    };
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(egui::RichText::new("Resource Allocation").small().strong());
        egui::Grid::new("research-allocation")
            .num_columns(2)
            .spacing([10.0, 1.0])
            .show(ui, |ui| {
                for (label, value) in &rows {
                    ui.label(egui::RichText::new(label).small());
                    ui.label(egui::RichText::new(value).small());
                    ui.end_row();
                }
            });
        // The original has a pair of spin buttons beside the percentage; a
        // slider is the same control with the whole range on show.
        ui.add(egui::Slider::new(&mut percent, 0..=100).suffix("%"));
    });
    if let Some(dialog) = app.research_dialog.as_mut() {
        dialog.percent = percent;
    }
}
