//! The scanner's toolbar.
//!
//! `TbWndProc` (`1068:0000`) and `DrawToolbar` (`1068:06f0`): a single row of
//! eighteen buttons and one combo box, laid out by the table in
//! [`crate::toolbar::LAYOUT`]. Six of the buttons are a radio group — the
//! scanner's views — and the rest toggle an overlay, open a menu, or zoom.
//!
//! Each button is drawn as a 3-D frame with one cell of the toolbar bitmap
//! inside it, offset by a pixel when it is pressed, which is what
//! `DrawBitmapButton` does. Without a copy of the original to read the bitmap
//! out of, the frame holds a short label instead.
//!
//! See `docs/ui/toolbar.md`.

use crate::toolbar::{self, Button, Item};
use crate::App;

/// How tall a button is, border and all.
const HEIGHT: f32 = 28.0;

/// Draw the toolbar. Returns whether anything was pressed.
pub fn view(app: &mut App, ui: &mut egui::Ui) -> bool {
    let mut acted = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(4.0);
        for item in toolbar::items() {
            match item {
                Item::Gap(width) => ui.add_space(width as f32),
                Item::Combo => coverage(app, ui),
                Item::Button(button) => {
                    let (clicked, response) = draw_button(app, ui, button);
                    acted |= clicked;
                    if button == Button::Zoom {
                        acted |= zoom_menu(app, ui, &response, clicked);
                    }
                }
            }
        }
    });
    acted
}

/// One button: the frame, the picture, and the click.
fn draw_button(app: &mut App, ui: &mut egui::Ui, button: Button) -> (bool, egui::Response) {
    let enabled = app.toolbar_enabled(button);
    let down = app.toolbar_down(button);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(button.width() as f32, HEIGHT),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );

    let visuals = ui.visuals();
    let face = visuals.widgets.inactive.bg_fill;
    let (top_left, bottom_right) = if down {
        (
            visuals.widgets.inactive.bg_stroke.color,
            visuals.extreme_bg_color,
        )
    } else {
        (
            visuals.extreme_bg_color,
            visuals.widgets.inactive.bg_stroke.color,
        )
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, if down { visuals.faint_bg_color } else { face });
    // The original draws the lit edge top-left and the shadow bottom-right,
    // and swaps them when the button is down.
    painter.line_segment(
        [rect.left_bottom(), rect.left_top()],
        egui::Stroke::new(1.0_f32, top_left),
    );
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1.0_f32, top_left),
    );
    painter.line_segment(
        [rect.right_top(), rect.right_bottom()],
        egui::Stroke::new(1.0_f32, bottom_right),
    );
    painter.line_segment(
        [rect.right_bottom(), rect.left_bottom()],
        egui::Stroke::new(1.0_f32, bottom_right),
    );

    // The picture, nudged a pixel when the button is down as the original
    // nudges it.
    let nudge = if down { 1.0 } else { 0.0 };
    let art = egui::Rect::from_min_size(
        rect.min + egui::vec2(2.0 + nudge, 2.0 + nudge),
        egui::vec2(button.art_width() as f32, toolbar::ART.1 as f32),
    );
    let ctx = ui.ctx().clone();
    let drawn = app
        .art
        .as_mut()
        .and_then(|art_source| art_source.sprite_rect(&ctx, button.cell()))
        .map(|image| {
            image.paint_at(ui, art);
            true
        })
        .unwrap_or(false);
    if !drawn {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            short_label(button),
            egui::FontId::proportional(9.0),
            if enabled {
                ui.visuals().text_color()
            } else {
                ui.visuals().weak_text_color()
            },
        );
    }
    if !enabled {
        ui.painter()
            .rect_filled(rect, 1.0, egui::Color32::from_black_alpha(80));
    }

    let response = response.on_hover_text(if enabled {
        button.name().to_string()
    } else {
        format!("{} — not wired up yet", button.name())
    });
    // Zoom opens a menu rather than doing anything itself, so the caller
    // handles it.
    if response.clicked() && button != Button::Zoom {
        app.toolbar_click(button);
        return (true, response);
    }
    (false, response)
}

/// The Zoom button's menu: the nine sizes the original offers, which are the
/// nine steps the scanner has.
fn zoom_menu(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, _clicked: bool) -> bool {
    let id = ui.make_persistent_id("toolbar-zoom-menu");
    if response.clicked() {
        ui.memory_mut(|memory| memory.toggle_popup(id));
    }
    let mut chosen = None;
    egui::popup::popup_below_widget(
        ui,
        id,
        response,
        egui::popup::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(70.0);
            for (step, percent) in App::ZOOM_PERCENT.iter().enumerate() {
                let zoom = i8::try_from(step).unwrap_or(4) - 4;
                if ui
                    .selectable_label(app.scan_zoom == zoom, format!("{percent}%"))
                    .clicked()
                {
                    chosen = Some(zoom);
                }
            }
        },
    );
    if let Some(zoom) = chosen {
        app.scan_zoom = zoom;
        return true;
    }
    false
}

/// The scanner-coverage combo: a percentage, typed or chosen.
fn coverage(app: &mut App, ui: &mut egui::Ui) {
    let mut chosen = app.scan_coverage_pct;
    egui::ComboBox::from_id_source("toolbar-coverage")
        .width(toolbar::COMBO_WIDTH as f32 - 8.0)
        .selected_text(format!("{chosen}%"))
        .show_ui(ui, |ui| {
            for step in toolbar::COVERAGE_STEPS {
                ui.selectable_value(&mut chosen, step, format!("{step}%"));
            }
        })
        .response
        .on_hover_text(
            "How effective the coverage overlay pretends every scanner is, \
             so a cloaked ship's reach can be seen",
        );
    if chosen != app.scan_coverage_pct {
        app.set_scan_coverage(&chosen.to_string());
    }
}

/// What goes in the frame when there is no picture to put there.
fn short_label(button: Button) -> &'static str {
    match button {
        Button::Normal => "Nml",
        Button::SurfaceMinerals => "Surf",
        Button::MineralConcentration => "Conc",
        Button::PlanetValue => "%",
        Button::Population => "Pop",
        Button::NoPlayerInfo => "None",
        Button::AddWaypoints => "+WP",
        Button::ScannerCoverage => "Scan",
        Button::MineFields => "Mine",
        Button::FleetPaths => "Path",
        Button::IdleFleets => "Idle",
        Button::PlanetNames => "Abc",
        Button::ShipDesignFilter => "Ship",
        Button::ShipDesignMenu | Button::EnemyClassMenu => "▾",
        Button::EnemyClassFilter => "Enmy",
        Button::Zoom => "Zoom",
        Button::ShipCount => "123",
    }
}
