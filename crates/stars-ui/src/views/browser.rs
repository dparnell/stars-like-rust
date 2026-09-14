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
    app.drawn_scope = "browser";
    let template = &crate::dialog::BROWSER;
    let (rect, at, caption) = crate::views::dialog_frame(ui, template);
    let line = ui.text_style_height(&egui::TextStyle::Small);

    // Prev, the dropdown and Next in a row along the top, each where the
    // template puts it.
    if crate::views::placed_button(app, ui, at(0x42e), &caption(0x42e), true).clicked() {
        app.browser_step(false);
    }
    let names: Vec<&str> = stars_core::browser::CATEGORIES
        .iter()
        .map(|(_, name)| *name)
        .collect();
    let mut category = browser.category;
    {
        let where_ = at(0x10b);
        let mut child = ui.child_ui(where_, egui::Layout::top_down(egui::Align::Min), None);
        egui::ComboBox::from_id_source("browser-category")
            .width(where_.width())
            .show_index(&mut child, &mut category, names.len(), |i| {
                names[i].to_string()
            });
    }
    if crate::views::placed_button(app, ui, at(0x42f), &caption(0x42f), true).clicked() {
        app.browser_step(true);
    }
    if category != browser.category {
        app.browser_set_category(category);
    }

    // The panel is a child window the dialog creates rather than a control the
    // template places, and it is sized from the font (`BrowserDlg`,
    // `10d8:21ce`).
    let line10 = ui.text_style_height(&egui::TextStyle::Body);
    let (panel_at, panel_size) = crate::dialog::browser_panel(line, line10);
    let panel = egui::Rect::from_min_size(rect.min + panel_at.to_vec2(), panel_size).intersect(
        egui::Rect::from_min_max(
            egui::pos2(rect.left(), at(0x42e).bottom() + 2.0),
            egui::pos2(rect.right() - 4.0, at(0x10a).top() - 4.0),
        ),
    );
    if panel.height() > 8.0 {
        ui.painter().rect_stroke(
            panel,
            0.0,
            egui::Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
        );
        let inner = panel.shrink(4.0);
        let mut child = ui.child_ui(inner, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(inner);
        egui::ScrollArea::vertical()
            .id_source("browser-panel")
            .show(&mut child, |ui| detail(app, ui));
    }

    // The filter and Close along the foot.
    let mut only = browser.buildable_only;
    if ui
        .put(
            at(0x10a),
            egui::Checkbox::new(&mut only, egui::RichText::new(caption(0x10a)).small()),
        )
        .changed()
    {
        app.browser_set_buildable_only(only);
    }
    if crate::views::placed_button(app, ui, at(0x2), &caption(0x2), true).clicked() {
        app.close_browser();
    }
}

/// The same panel for an arbitrary component, which is what the
/// `grPopupComponent` hover help puts up.
///
/// `DrawPopup` calls the very routine the browser calls — the pop-up and the
/// browser's own middle are one panel — so this shows the given component and
/// then puts the browser back where it was.
pub fn component_panel(app: &mut App, ui: &mut egui::Ui, showing: (u16, usize)) {
    let was = app.browser;
    let opened = was.is_none();
    if opened {
        app.open_browser();
    }
    if let Some(browser) = app.browser.as_mut() {
        browser.showing = showing;
    }
    detail(app, ui);
    if opened {
        app.browser = None;
    } else {
        app.browser = was;
    }
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

    // The component's own picture, when the game's pictures have been found.
    // The original draws it 64 pixels square beside the figures.
    if let Some(cell) = stars_core::browser::picture(&detail) {
        ui.horizontal_top(|ui| {
            crate::art::draw(app, ui, cell, 64.0);
            ui.add_space(4.0);
        });
    }

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
