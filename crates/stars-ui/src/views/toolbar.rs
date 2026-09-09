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

/// How tall a button is, bevel and all — [`toolbar::BUTTON_HEIGHT`] as a
/// float, since every rectangle here is one.
const HEIGHT: f32 = toolbar::BUTTON_HEIGHT as f32;

/// Draw the toolbar. Returns whether anything was pressed.
pub fn view(app: &mut App, ui: &mut egui::Ui) -> bool {
    let mut acted = false;
    // What the pointer is resting on, and how long it has been there: the
    // tooltip's own timing wants both. See `crate::toolbar::Tooltip`.
    let now = ui.input(|input| input.time);
    let mut over: Option<toolbar::Tip> = None;
    // The strip itself: `WM_ERASEBKGND` fills its whole client rectangle with
    // the button face, and the largest window layout puts a black line down
    // its left edge (`PatBlt(0, 0, 1, rc.bottom, BLACKNESS)`).
    let [r, g, b] = toolbar::FACE;
    let top_left = ui.cursor().min;
    let strip = egui::Rect::from_min_size(
        top_left,
        egui::vec2(ui.available_width(), toolbar::ROW_HEIGHT as f32),
    );
    ui.painter()
        .rect_filled(strip, 0.0, egui::Color32::from_rgb(r, g, b));
    if app.window_layout == crate::WindowLayout::Large {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(strip.min, egui::vec2(1.0, strip.height())),
            0.0,
            egui::Color32::BLACK,
        );
    }
    ui.add_space(toolbar::MARGIN as f32);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(toolbar::MARGIN as f32);
        for item in toolbar::items() {
            match item {
                Item::Gap(width) => ui.add_space(width as f32),
                Item::Combo => {
                    let response = coverage(app, ui);
                    if response.hovered() {
                        over = Some(toolbar::Tip::Combo);
                    }
                }
                Item::Button(button) => {
                    let (clicked, response) = draw_button(app, ui, button);
                    acted |= clicked;
                    if response.hovered() {
                        over = Some(toolbar::Tip::Button(button));
                    }
                    match button {
                        Button::Zoom => acted |= zoom_menu(app, ui, &response, clicked),
                        Button::ShipDesignMenu => acted |= filter_menu(app, ui, &response, false),
                        Button::EnemyClassMenu => acted |= filter_menu(app, ui, &response, true),
                        Button::MineFields => acted |= minefield_menu(app, ui, &response),
                        _ => {}
                    }
                }
            }
        }
    });
    ui.add_space(toolbar::MARGIN as f32);

    // Any click takes the tooltip away, which is what the original's
    // `WM_LBUTTONDOWN` arm does before it acts on the button.
    if acted || ui.input(|input| input.pointer.any_pressed()) {
        app.tooltip.dismiss(now);
    } else {
        app.tooltip.hover(over, now);
    }
    tooltip(app, ui, now);
    acted
}

/// The tooltip itself: a pale yellow box with a one-pixel frame, its top-left
/// at the pointer's x and a line and a half below it, pulled back from the
/// right edge when it would not fit.
fn tooltip(app: &mut App, ui: &egui::Ui, now: f64) {
    let Some(tip) = app.tooltip.showing() else {
        return;
    };
    let Some(pointer) = ui.ctx().pointer_latest_pos() else {
        app.tooltip.dismiss(now);
        return;
    };
    let text = tip.text();
    let font = egui::FontId::proportional(11.0);
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        font,
        egui::Color32::from_rgb(
            toolbar::TOOLTIP_FRAME[0],
            toolbar::TOOLTIP_FRAME[1],
            toolbar::TOOLTIP_FRAME[2],
        ),
    );
    let margin = toolbar::TOOLTIP_MARGIN;
    let size = galley.size() + egui::vec2(margin * 2.0, margin * 2.0);
    // `pt.y + dyArial8 * 3 / 2`, and pulled back by five when the box would
    // run off the frame's right edge.
    let mut at = pointer + egui::vec2(0.0, galley.size().y * 1.5);
    let limit = ui.ctx().screen_rect().right();
    if at.x + size.x > limit {
        at.x = limit - size.x - 5.0;
    }
    let rect = egui::Rect::from_min_size(at, size);
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("stars-toolbar-tooltip"),
    ));
    let [r, g, b] = toolbar::TOOLTIP_BACKGROUND;
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(r, g, b));
    let [r, g, b] = toolbar::TOOLTIP_FRAME;
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(r, g, b)),
    );
    painter.galley(
        rect.min + egui::vec2(margin, margin),
        galley,
        egui::Color32::BLACK,
    );
}

/// One button: the frame, the picture, and the click.
///
/// `DrawBitmapButton` (`1068:078c`) draws the bevel with eight `PatBlt`s and a
/// couple of fills, and this follows it rectangle for rectangle: a one-pixel
/// ring, lit along the top and left and shadowed along the bottom and right —
/// the two swapping over when the button is down — with the four corner
/// pixels laid in separately so the ring reads as rounded, and a one-pixel
/// face inside it. The picture then goes in at `+2, +2`, pushed by another
/// pixel for each step of [`Press`].
fn draw_button(app: &mut App, ui: &mut egui::Ui, button: Button) -> (bool, egui::Response) {
    let enabled = app.toolbar_enabled(button);
    let width = button.width() as f32;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, HEIGHT),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );

    let press = if enabled && response.is_pointer_button_down_on() {
        toolbar::Press::Held
    } else if app.toolbar_down(button) {
        toolbar::Press::Latched
    } else {
        toolbar::Press::Up
    };
    bevel(ui, rect, width, press);

    // The picture, pushed in by however far the button is pressed.
    let nudge = press.offset();
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
            rect.center() + egui::vec2(nudge, nudge),
            egui::Align2::CENTER_CENTER,
            short_label(button),
            egui::FontId::proportional(9.0),
            egui::Color32::BLACK,
        );
    }
    if !enabled {
        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_black_alpha(80));
    }

    // The three buttons that open a menu do nothing themselves; the caller
    // handles them.
    let opens_a_menu = matches!(
        button,
        Button::Zoom | Button::ShipDesignMenu | Button::EnemyClassMenu | Button::MineFields
    );
    if response.clicked() && !opens_a_menu {
        app.toolbar_click(button);
        return (true, response);
    }
    (false, response)
}

/// The eight rectangles of a button's bevel, in the order and the places
/// `DrawBitmapButton` lays them.
fn bevel(ui: &egui::Ui, rect: egui::Rect, width: f32, press: toolbar::Press) {
    let painter = ui.painter();
    let colour = |[r, g, b]: [u8; 3]| egui::Color32::from_rgb(r, g, b);
    let face = colour(toolbar::FACE);
    let (lit, dark) = if press.is_down() {
        (colour(toolbar::SHADOW), colour(toolbar::HILITE))
    } else {
        (colour(toolbar::HILITE), colour(toolbar::SHADOW))
    };
    let at = |x: f32, y: f32, w: f32, h: f32, fill: egui::Color32| {
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min + egui::vec2(x, y), egui::vec2(w, h)),
            0.0,
            fill,
        );
    };

    // The face first, since the original leaves the middle of the button to
    // whatever the strip was filled with and this has no strip behind it.
    at(1.0, 1.0, width - 2.0, HEIGHT - 2.0, face);

    // Lit along the top and the left, with the two corners it touches.
    at(2.0, 0.0, width - 4.0, 1.0, lit);
    at(0.0, 2.0, 1.0, 24.0, lit);
    at(1.0, 1.0, 1.0, 1.0, lit);
    at(1.0, 26.0, 1.0, 1.0, lit);
    // Shadowed along the bottom and the right, likewise.
    at(2.0, 27.0, width - 4.0, 1.0, dark);
    at(width - 1.0, 2.0, 1.0, 24.0, dark);
    at(width - 2.0, 1.0, 1.0, 1.0, dark);
    at(width - 2.0, 26.0, 1.0, 1.0, dark);

    match press {
        // Up: the face runs two pixels wide down the inner bottom and right,
        // which is what gives the button its thickness.
        toolbar::Press::Up => {
            at(2.0, 25.0, width - 4.0, 2.0, face);
            at(width - 3.0, 2.0, 2.0, 24.0, face);
        }
        // Down: the face moves to the inner top and left instead, as many
        // pixels wide as the button is pressed.
        press => {
            let deep = press.offset();
            at(2.0, 2.0, width - 4.0, deep, face);
            at(2.0, 2.0, deep, 24.0, face);
            if press == toolbar::Press::Latched {
                at(2.0, 26.0, width - 4.0, 1.0, face);
                at(width - 2.0, 2.0, 1.0, 24.0, face);
            } else {
                // Held: the bottom-right corner is lit again.
                at(width - 2.0, 26.0, 1.0, 1.0, colour(toolbar::HILITE));
            }
        }
    }
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
fn coverage(app: &mut App, ui: &mut egui::Ui) -> egui::Response {
    let mut chosen = app.scan_coverage_pct;
    let response = egui::ComboBox::from_id_source("toolbar-coverage")
        .width(toolbar::COMBO_WIDTH as f32 - 8.0)
        .selected_text(format!("{chosen}%"))
        .show_ui(ui, |ui| {
            for step in toolbar::COVERAGE_STEPS {
                ui.selectable_value(&mut chosen, step, format!("{step}%"));
            }
        })
        .response;
    if chosen != app.scan_coverage_pct {
        app.set_scan_coverage(&chosen.to_string());
    }
    response
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

/// One of the two ship filters' menus: three commands, a rule, then a tick per
/// entry.
///
/// Both menus are built from the same three strings in the original, so the
/// enemy one says "All Designs" too. That is the game's own wording reused,
/// not a slip here.
fn filter_menu(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, enemy: bool) -> bool {
    let id = ui.make_persistent_id(if enemy {
        "toolbar-class-filter"
    } else {
        "toolbar-design-filter"
    });
    if response.clicked() {
        ui.memory_mut(|memory| memory.toggle_popup(id));
    }

    let entries = if enemy {
        app.class_filter_entries()
    } else {
        app.design_filter_entries()
    };
    let mut command = None;
    let mut toggled = None;
    egui::popup::popup_below_widget(
        ui,
        id,
        response,
        egui::popup::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(140.0);
            for (which, label) in crate::FilterCommand::ALL {
                if ui.button(label).clicked() {
                    command = Some(which);
                }
            }
            ui.separator();
            if entries.is_empty() {
                ui.label(egui::RichText::new("no designs yet").weak().small());
            }
            for entry in &entries {
                if ui.selectable_label(entry.on, &entry.name).clicked() {
                    toggled = Some(entry.bit);
                }
            }
        },
    );

    match (command, toggled) {
        (Some(command), _) => {
            if enemy {
                app.class_filter_command(command);
            } else {
                app.design_filter_command(command);
            }
            true
        }
        (_, Some(bit)) => {
            if enemy {
                app.toggle_class_filter(bit);
            } else {
                app.toggle_design_filter(bit);
            }
            true
        }
        _ => false,
    }
}

/// The Mine Fields menu: two commands, a rule, then a tick for each of the
/// four groups.
///
/// Unlike the ship filters this menu has no invert, and the overlay follows
/// the filter exactly — unticking the last group turns the overlay off.
fn minefield_menu(app: &mut App, ui: &mut egui::Ui, response: &egui::Response) -> bool {
    let id = ui.make_persistent_id("toolbar-minefield-menu");
    if response.clicked() {
        // Opening it with the overlay off empties the filter first, as the
        // original empties it.
        app.open_minefield_menu();
        ui.memory_mut(|memory| memory.toggle_popup(id));
    }

    let entries = app.minefield_filter_entries();
    let all_on = app.scan_minefield_filter == 0xf;
    let none_on = app.scan_minefield_filter == 0;
    let mut command = None;
    let mut toggled = None;
    egui::popup::popup_below_widget(
        ui,
        id,
        response,
        egui::popup::PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(170.0);
            if ui.selectable_label(all_on, "All Mine Fields").clicked() {
                command = Some(true);
            }
            if ui.selectable_label(none_on, "No Mine Fields").clicked() {
                command = Some(false);
            }
            ui.separator();
            for entry in &entries {
                if ui.selectable_label(entry.on, &entry.name).clicked() {
                    toggled = Some(entry.bit);
                }
            }
        },
    );

    match (command, toggled) {
        (Some(all), _) => {
            app.minefield_filter_command(all);
            true
        }
        (_, Some(bit)) => {
            app.toggle_minefield_filter(bit);
            true
        }
        _ => false,
    }
}
