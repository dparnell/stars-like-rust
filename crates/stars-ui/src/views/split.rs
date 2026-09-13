//! The Ship Transfer dialog — `TransferDlg` in its ship mode (`mdXferDlg
//! == 1`), the **Split** button on the Fleet Composition tile.
//!
//! The same template as the Cargo Transfer dialog (`transfer.rs`), titled
//! `Ship Transfer` (`idsShipTransfer`), with the fleet on the left and the
//! new fleet on the right. Each side is a framed square with a title bar,
//! then a row per design aboard (`rgXferValidHulls`): the design's name
//! right-aligned and its count in a sunken frame
//! (`DrawFleetShipsXferSide`). Between them an arrow pair a row
//! (`FSetupXferBtns`), moving one ship, ten with Shift, a hundred with
//! Ctrl, a thousand with both (`FTrackXfer`), as far as the giver has. OK
//! makes the new fleet from what crossed; Cancel makes nothing. Page 26 of
//! the tutorial: "press the Split button in the Fleet Composition tile.
//! Move one Santa Maria across to Fleet #10 and press OK."
//!
//! Recorded under the scope `"split"`: the arrows as `Santa Maria <` and
//! `Santa Maria >`, and the three buttons.

use crate::dialog::Control;
use crate::App;

/// Draw the dialog's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    if app.split.is_none() {
        return;
    }
    app.drawn_scope = "split";
    let template = &crate::dialog::TRANSFER;
    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
    let step = match (ctrl, shift) {
        (false, false) => 1,
        (false, true) => 10,
        (true, false) => 100,
        (true, true) => 1000,
    };

    let want = template.pixels();
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(
            ui.available_width(),
            want.y
                * template.scale(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    ui.available_size(),
                )),
        ),
        egui::Sense::hover(),
    );
    let scale = template.scale(rect);
    let line = ui.text_style_height(&egui::TextStyle::Small).ceil();
    let font = egui::TextStyle::Small.resolve(ui.style());
    let painter = ui.painter_at(rect);
    let text = ui.visuals().text_color();

    let foot = rect.top()
        + f32::from(template.control(1).map_or(145, |c| c.at.1)) * crate::dialog::DLU_Y * scale;
    let whole = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), foot));
    let mid = whole.center().x;
    let margin = 4.0 + line + 3.0;
    let left = egui::Rect::from_min_max(
        egui::pos2(whole.left() + margin - (line + 1.0), whole.top() + 4.0),
        egui::pos2(mid - margin, whole.bottom() - 4.0),
    );
    let right = egui::Rect::from_min_max(
        egui::pos2(mid + margin, whole.top() + 4.0),
        egui::pos2(whole.right() - margin + line + 1.0, whole.bottom() - 4.0),
    );

    let Some(dialog) = app.split.clone() else {
        return;
    };
    let names = (
        app.fleet_display_name(dialog.fleet),
        format!("Fleet #{}", dialog.new_id + 1),
    );

    // A side: the whole half framed, a title bar, and the rows.
    let row_step = line + 6.0;
    let side = |painter: &egui::Painter, rect: egui::Rect, title: &str, counts: &[i32]| {
        crate::views::transfer::frame_3d(painter, rect);
        let bar = egui::Rect::from_min_size(
            rect.min + egui::vec2(1.0, 1.0),
            egui::vec2(rect.width() - 2.0, line + 2.0),
        );
        crate::views::transfer::frame_3d(painter, bar);
        painter.text(
            bar.center(),
            egui::Align2::CENTER_CENTER,
            title,
            font.clone(),
            text,
        );
        let value_width = 36.0;
        let value_left = rect.right() - value_width - 6.0;
        let mut y = bar.bottom() + 3.0;
        for ((_, name), count) in dialog.designs.iter().zip(counts) {
            painter.text(
                egui::pos2(value_left - 8.0, y),
                egui::Align2::RIGHT_TOP,
                name,
                font.clone(),
                text,
            );
            let frame = egui::Rect::from_min_max(
                egui::pos2(value_left - 2.0, y - 1.0),
                egui::pos2(value_left + value_width + 2.0, y + line + 1.0),
            );
            crate::views::transfer::frame_3d(painter, frame);
            painter.text(
                egui::pos2(value_left + value_width, y),
                egui::Align2::RIGHT_TOP,
                count.to_string(),
                font.clone(),
                text,
            );
            y += row_step;
        }
        bar.bottom() + 3.0
    };
    let top = side(&painter, left, &names.0, &dialog.left);
    side(&painter, right, &names.1, &dialog.right);

    // The arrows between: the left one brings ships back to the fleet, the
    // right one sends them across.
    let mut moves: Vec<(usize, i32)> = Vec::new();
    let size = line + 3.0;
    for (row, (_, name)) in dialog.designs.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let y = top + row as f32 * row_step - 2.0;
        let back =
            egui::Rect::from_min_size(egui::pos2(mid - size + 1.0, y), egui::vec2(size, size));
        let across = egui::Rect::from_min_size(egui::pos2(mid + 3.0, y), egui::vec2(size, size));
        let label = format!("{name} <");
        if crate::views::placed_button_named(app, ui, back, "<", &label, true).clicked() {
            moves.push((row, -step));
        }
        let label = format!("{name} >");
        if crate::views::placed_button_named(app, ui, across, ">", &label, true).clicked() {
            moves.push((row, step));
        }
    }
    for (row, delta) in moves {
        app.split_move(row, delta);
    }

    // The row along the foot, from the template.
    let at = |id: u16| -> egui::Rect {
        let control = template.control(id);
        let (x, y, w, h) = control.map_or((0, 0, 0, 0), |c| c.at);
        egui::Rect::from_min_size(
            rect.min
                + egui::vec2(
                    f32::from(x) * crate::dialog::DLU_X * scale,
                    f32::from(y) * crate::dialog::DLU_Y * scale,
                ),
            egui::vec2(
                f32::from(w) * crate::dialog::DLU_X * scale,
                f32::from(h) * crate::dialog::DLU_Y * scale,
            ),
        )
    };
    let caption = |id: u16| -> String {
        template
            .control(id)
            .map_or_else(String::new, Control::label)
    };
    if crate::views::placed_button(app, ui, at(0x1), &caption(0x1), true).clicked() {
        app.split_ok();
    }
    if crate::views::placed_button(app, ui, at(0x2), &caption(0x2), true).clicked() {
        app.split_cancel();
    }
    crate::views::placed_button(app, ui, at(0x76), &caption(0x76), false);
}
