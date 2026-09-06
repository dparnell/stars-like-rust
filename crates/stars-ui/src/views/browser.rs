//! The Technology Browser.
//!
//! `BrowserDlg` (`10d8:1ed8`), reached with **F2**, with `DisplayComponentInfo`
//! (`10d8:2ac6`) painting the panel. It shows one component at a time: a
//! category dropdown, a checkbox limiting the walk to what can be built now,
//! Prev and Next, and the details underneath.
//!
//! See `docs/ui/technology-browser.md`.

use crate::App;

/// Draw the browser's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some(browser) = app.browser else {
        return;
    };

    // The category dropdown and the filter.
    let names: Vec<&str> = stars_core::browser::CATEGORIES
        .iter()
        .map(|(_, name)| *name)
        .collect();
    let mut category = browser.category;
    let mut only = browser.buildable_only;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_source("browser-category")
            .width(150.0)
            .show_index(ui, &mut category, names.len(), |i| names[i].to_string());
        ui.checkbox(&mut only, "Only show what I can build");
    });
    if category != browser.category {
        app.browser_set_category(category);
    }
    if only != browser.buildable_only {
        app.browser_set_buildable_only(only);
    }

    ui.separator();
    detail(app, ui);

    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("◀ Prev").clicked() {
            app.browser_step(false);
        }
        if ui.button("Next ▶").clicked() {
            app.browser_step(true);
        }
        if ui.button("Close").clicked() {
            app.close_browser();
        }
    });
}

/// The panel: what the component is, what it costs, what it needs and what it
/// does.
fn detail(app: &mut App, ui: &mut egui::Ui) {
    let Some(detail) = app.browser_detail() else {
        ui.label(
            egui::RichText::new("nothing to show in this category")
                .weak()
                .small(),
        );
        return;
    };

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(detail.name).strong());
        ui.label(
            egui::RichText::new(format!("({})", detail.category_name))
                .small()
                .weak(),
        );
        if !detail.buildable() {
            // The original's `UnAvail` marker.
            ui.label(
                egui::RichText::new("UnAvail")
                    .small()
                    .color(egui::Color32::from_rgb(0xff, 0x8a, 0x8a)),
            );
        }
    });

    egui::Grid::new("browser-detail")
        .num_columns(2)
        .spacing([12.0, 1.0])
        .show(ui, |ui| {
            let names = ["Ironium", "Boranium", "Germanium", "Resources"];
            ui.label(egui::RichText::new("Cost:").small());
            ui.label(
                egui::RichText::new(
                    detail
                        .cost
                        .iter()
                        .enumerate()
                        .filter(|(_, value)| **value != 0)
                        .map(|(i, value)| {
                            if i == 3 {
                                format!("{value} resources")
                            } else {
                                format!("{value}kT {}", names[i].to_lowercase())
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                )
                .small(),
            );
            ui.end_row();

            if let Some(mass) = detail.mass {
                ui.label(egui::RichText::new("Mass:").small());
                ui.label(egui::RichText::new(format!("{mass}kT")).small());
                ui.end_row();
            }

            for (label, value) in &detail.stats {
                ui.label(egui::RichText::new(label).small());
                ui.label(egui::RichText::new(value).small());
                ui.end_row();
            }
        });

    // The technology it needs, each field red when the player is short of it
    // and ordinary when they are not (`MANUAL.PDF` p. 8-3).
    if detail.tech.is_empty() {
        ui.label(egui::RichText::new("Tech Req: none").small().weak());
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("Tech Req:").small());
            for need in &detail.tech {
                let text =
                    egui::RichText::new(format!("{} {}", need.field.short_name(), need.level))
                        .small();
                ui.label(if need.met {
                    text
                } else {
                    text.color(egui::Color32::from_rgb(0xff, 0x8a, 0x8a))
                });
            }
        });
    }

    for note in &detail.notes {
        ui.label(
            egui::RichText::new(note)
                .small()
                .color(egui::Color32::from_rgb(0xff, 0xc0, 0x60)),
        );
    }
}
