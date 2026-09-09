//! The pop-up summary the scanner's status bar raises.
//!
//! `Popup` (`10c0:0c7c`) sizes and places it, `DrawPopup` (`10c0:01c0`) paints
//! it, and `PopupWndProc` (`10c0:0000`) takes it away on the next button-up.
//! See `crate::popup` for what goes in it and `docs/ui/scanner.md`.

use crate::popup::{self, FleetSummary, PlanetSummary, Popup};
use crate::App;

/// How wide a `grPopupString` note wraps to. The original takes the width
/// from the caller — the research dialog passes `dxResRight`, the width of its
/// own right-hand column.
const NOTE_WIDTH: f32 = 260.0;

fn colour([r, g, b]: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

/// The pop-up's font: `rghfontArial8[0]`, with `[1]` — the bold face — for the
/// planet summary's labels and the fleet list's header. egui's default font
/// set has no bold proportional face, so both are the body font here.
fn font(ui: &egui::Ui) -> egui::FontId {
    egui::TextStyle::Body.resolve(ui.style())
}

/// A line of it, the original's `dyArial8`.
fn line(ui: &egui::Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Body)
}

/// Draw the pop-up, if one is up.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some((popup, (x, y))) = app.popup.clone() else {
        return;
    };
    let popup = &popup;
    let font = font(ui);
    let text = colour(popup::TEXT);
    let width = |s: &str| {
        ui.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), text))
            .rect
            .width()
            .ceil()
    };
    let line = line(ui);
    let margin = popup::MARGIN;

    let size = match popup {
        Popup::Planet(_) => {
            let labels = popup::PLANET_LABELS
                .iter()
                .map(|label| width(label))
                .fold(0.0_f32, f32::max);
            let Popup::Planet(PlanetSummary { values }) = popup else {
                unreachable!()
            };
            // The value column is the widest of the four values against
            // `idsN9999`, and the label column adds eight when sizing.
            let value = values
                .iter()
                .map(|v| width(v))
                .fold(width(popup::VALUE_SAMPLE), f32::max);
            egui::vec2(
                labels + popup::LABEL_GAP_SIZE + value,
                line * 4.0 + margin * 2.0,
            )
        }
        // The component pop-up is the Technology Browser's own panel, at the
        // size that panel uses (`crate::popup::component_size`).
        Popup::Component(_) => {
            let (w, h) =
                crate::popup::component_size(line, ui.text_style_height(&egui::TextStyle::Body));
            egui::vec2(w, h)
        }
        // A paragraph, word-wrapped to a width the caller chose.
        Popup::Note(note) => {
            let galley = ui.fonts(|f| f.layout(note.clone(), font.clone(), text, NOTE_WIDTH));
            galley.rect.size() + egui::vec2(margin * 2.0, margin * 2.0)
        }
        Popup::Fleet(fleet) => {
            let names = fleet
                .rows
                .iter()
                .map(|row| width(&row.name))
                .fold(width(popup::HEADER_NAME), f32::max);
            let counts = fleet
                .rows
                .iter()
                .map(|row| width(&row.count))
                .fold(0.0_f32, f32::max);
            let damage = if fleet.damage_column() {
                width(popup::DAMAGE_SAMPLE) + popup::DAMAGE_GAP
            } else {
                0.0
            };
            #[allow(clippy::cast_precision_loss)]
            let rows = fleet.rows.len() as f32;
            egui::vec2(
                names + counts + popup::COLUMN_GAP + damage,
                line * (rows + 1.0) + margin * 2.0,
            )
        }
    };

    // The window's **bottom-right** corner goes at the pointer, and is then
    // pulled back inside the screen.
    let screen = ui.ctx().screen_rect();
    let left = (x - size.x).clamp(screen.left(), (screen.right() - size.x).max(screen.left()));
    let top = (y - size.y).clamp(screen.top(), (screen.bottom() - size.y).max(screen.top()));
    let rect = egui::Rect::from_min_size(egui::pos2(left, top), size);

    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("stars-scanner-popup"),
    ));
    painter.rect_filled(rect, 0.0, colour(popup::BACKGROUND));
    painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0_f32, colour(popup::FRAME)));
    let painter = painter.with_clip_rect(rect);
    let put = |at: egui::Pos2, align: egui::Align2, label: &str, fill: egui::Color32| {
        painter.text(at, align, label, font.clone(), fill);
    };

    match popup {
        Popup::Planet(PlanetSummary { values }) => {
            // Labels right-aligned at the label column's edge, values left
            // from the same x — which is why the pair look joined up.
            let labels = popup::PLANET_LABELS
                .iter()
                .map(|label| width(label))
                .fold(0.0_f32, f32::max);
            let column = rect.left() + labels + popup::LABEL_GAP;
            for (row, (label, value)) in popup::PLANET_LABELS.iter().zip(values).enumerate() {
                #[allow(clippy::cast_precision_loss)]
                let y = rect.top() + margin + row as f32 * line;
                put(egui::pos2(column, y), egui::Align2::RIGHT_TOP, label, text);
                put(egui::pos2(column, y), egui::Align2::LEFT_TOP, value, text);
            }
        }
        Popup::Fleet(fleet) => fleet_rows(&painter, rect, fleet, &font, line, put),
        Popup::Component(showing) => {
            let inner = rect.shrink(margin);
            let mut child = ui.child_ui(inner, egui::Layout::top_down(egui::Align::Min), None);
            child.set_clip_rect(inner);
            crate::views::browser::component_panel(app, &mut child, *showing);
        }
        Popup::Note(note) => {
            painter.text(
                rect.min + egui::vec2(margin, margin),
                egui::Align2::LEFT_TOP,
                note,
                font.clone(),
                text,
            );
        }
    }
}

/// The fleet list: a header, a rule under it when the damage column is there,
/// and one row per design.
fn fleet_rows(
    painter: &egui::Painter,
    rect: egui::Rect,
    fleet: &FleetSummary,
    font: &egui::FontId,
    line: f32,
    put: impl Fn(egui::Pos2, egui::Align2, &str, egui::Color32),
) {
    let margin = popup::MARGIN;
    let text = colour(popup::TEXT);
    let left = rect.left() + margin;
    let top = rect.top() + margin;
    // A fleet with nothing in it says so and stops.
    if fleet.rows.is_empty() {
        put(
            egui::pos2(left, top),
            egui::Align2::LEFT_TOP,
            popup::NONE,
            text,
        );
        return;
    }
    let damage = fleet.damage_column();
    let damage_width = if damage {
        painter
            .layout_no_wrap(popup::DAMAGE_SAMPLE.to_string(), font.clone(), text)
            .rect
            .width()
            .ceil()
            + popup::DAMAGE_GAP
    } else {
        0.0
    };
    let counts_at = rect.right() - margin - damage_width;

    put(
        egui::pos2(left, top),
        egui::Align2::LEFT_TOP,
        popup::HEADER_NAME,
        text,
    );
    put(
        egui::pos2(counts_at, top),
        egui::Align2::RIGHT_TOP,
        popup::HEADER_COUNT,
        text,
    );
    if damage {
        put(
            egui::pos2(rect.right() - margin, top),
            egui::Align2::RIGHT_TOP,
            popup::HEADER_DAMAGE,
            text,
        );
        // `PatBlt(4, dyArial8 + 2, right - 8, 1, BLACKNESS)`.
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(left, rect.top() + line + 2.0),
                egui::vec2(rect.width() - margin * 2.0, 1.0),
            ),
            0.0,
            colour(popup::FRAME),
        );
    }

    for (row, entry) in fleet.rows.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let y = top + (row + 1) as f32 * line;
        let fill = if damage && entry.damage.is_some() {
            colour(popup::DAMAGED)
        } else {
            text
        };
        put(
            egui::pos2(left, y),
            egui::Align2::LEFT_TOP,
            &entry.name,
            fill,
        );
        put(
            egui::pos2(counts_at, y),
            egui::Align2::RIGHT_TOP,
            &entry.count,
            fill,
        );
        if damage {
            if let Some(damage) = &entry.damage {
                put(
                    egui::pos2(rect.right() - margin, y),
                    egui::Align2::RIGHT_TOP,
                    damage,
                    fill,
                );
            }
        }
    }
}
