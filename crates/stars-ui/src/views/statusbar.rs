//! The scanner's status bar.
//!
//! `DrawScannerSBar` (`1058:62d8`): two rows along the bottom of the scanner,
//! four sunken cells across the top one and a fifth across the bottom. See
//! `crate::statusbar` for what goes in them and `docs/ui/scanner.md` for the
//! layout the numbers here come from.

use crate::statusbar as sbar;
use crate::toolbar;
use crate::App;

/// How tall the bar is: `dySBar`, which is `(dyArial8 + 12) * 2`.
///
/// `dyArial8` is the height of a line of the original's Arial 8, so the
/// nearest thing here is the height of a line of the body font.
#[must_use]
pub fn height(ui: &egui::Ui) -> f32 {
    row_height(ui) * 2.0
}

/// One of the two rows.
fn row_height(ui: &egui::Ui) -> f32 {
    line(ui) + sbar::ROW_EXTRA
}

/// A line of the bar's font.
fn line(ui: &egui::Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Body)
}

/// The bar's font.
///
/// The original selects `rghfontArial8[1]` — Arial 8 **bold**. egui's default
/// font set has no bold proportional face, so this is the body font at the
/// same size; everything else about the bar is the original's.
fn font(ui: &egui::Ui) -> egui::FontId {
    egui::TextStyle::Body.resolve(ui.style())
}

fn colour([r, g, b]: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

/// Draw the bar across the bottom of `rect`, and handle the press in its
/// upper row that raises the pop-up summary.
pub fn view(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect) {
    let painter = ui.painter().with_clip_rect(rect);
    let bar = app.status_bar();
    let fill = |r: egui::Rect, c: egui::Color32| painter.rect_filled(r, 0.0, c);
    let at = |x: f32, y: f32, w: f32, h: f32| {
        egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
    };

    // The strip: filled with the button face, then a one-pixel highlight down
    // its left edge and along its top, which is what lifts it off the map.
    fill(rect, colour(toolbar::FACE));
    fill(
        at(rect.left(), rect.top(), 1.0, rect.height()),
        colour(toolbar::HILITE),
    );
    fill(
        at(rect.left(), rect.top(), rect.width(), 1.0),
        colour(toolbar::HILITE),
    );

    let font = font(ui);
    let text = colour(sbar::TEXT);
    let row = row_height(ui);
    let inset = sbar::CELL_INSET;

    // A cell: sunk into the face by a shadow along its left and top and a
    // highlight along its right and bottom (`DrawLockLight`, `1058:6b00`),
    // with its text three pixels in and two down, clipped to the cell less two
    // pixels all round.
    let cell = |left: f32, top: f32, width: f32, height: f32, label: &str| {
        let r = at(left, top, width, height);
        fill(
            at(r.left(), r.top(), 1.0, r.height()),
            colour(toolbar::SHADOW),
        );
        fill(
            at(r.left(), r.top(), r.width(), 1.0),
            colour(toolbar::SHADOW),
        );
        fill(
            at(r.right(), r.top(), 1.0, r.height() + 1.0),
            colour(toolbar::HILITE),
        );
        fill(
            at(r.left(), r.bottom(), r.width(), 1.0),
            colour(toolbar::HILITE),
        );
        if !label.is_empty() {
            painter.with_clip_rect(r.shrink(2.0).intersect(rect)).text(
                r.min + egui::vec2(sbar::TEXT_LEFT, sbar::TEXT_TOP),
                egui::Align2::LEFT_TOP,
                label,
                font.clone(),
                text,
            );
        }
    };

    // The top row. Its three left cells appear only when there is room for
    // them; below that width the name takes the whole row.
    let top = rect.top() + inset;
    let bottom = rect.top() + row - inset;
    let mut left = rect.left() + inset;
    if rect.width() > sbar::CELLS_WIDTH {
        let width = |sample: &str| {
            ui.fonts(|f| f.layout_no_wrap(sample.to_string(), font.clone(), text))
                .rect
                .width()
                .ceil()
                + sbar::CELL_PAD
        };
        let id_width = width(sbar::ID_SAMPLE);
        // The x and y cells are both sized from the same sample: the original
        // measures it once and reuses the figure.
        let coord_width = width(sbar::COORD_SAMPLE);
        cell(left, top, id_width, bottom - top, &bar.id);
        left += id_width + sbar::CELL_GAP;
        cell(left, top, coord_width, bottom - top, &bar.x);
        left += coord_width + sbar::CELL_GAP;
        cell(left, top, coord_width, bottom - top, &bar.y);
        left += coord_width + sbar::CELL_GAP;
    }
    cell(
        left,
        top,
        (rect.right() - left) - inset,
        bottom - top,
        &bar.name,
    );

    // The bottom row: one cell the whole way across.
    let wide = rect.width() > sbar::WIDE_UNIT_WIDTH;
    let label = bar.distance.map(|d| d.text(wide)).unwrap_or_default();
    cell(
        rect.left() + inset,
        rect.top() + row + inset,
        rect.width() - inset * 2.0,
        row - inset * 2.0,
        &label,
    );

    // Pressing the **upper** row raises the pop-up summary, and letting go
    // takes it away: `ScannerWndProc` (`1058:043a`) acts on `WM_LBUTTONDOWN`
    // and `PopupWndProc` destroys the window on `WM_LBUTTONUP`.
    let upper = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + row));
    let pointer = ui.input(|input| {
        (
            input.pointer.interact_pos(),
            input.pointer.primary_pressed(),
            input.pointer.any_released(),
        )
    });
    match pointer {
        (Some(at), true, _) if upper.contains(at) => {
            app.popup = app.status_bar_popup().map(|popup| (popup, (at.x, at.y)));
        }
        (_, _, true) => app.popup = None,
        _ => {}
    }
    crate::views::popup::view(app, ui);
}
