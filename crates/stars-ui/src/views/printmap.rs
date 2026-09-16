//! The Print Map dialog and the preview of what it prints.
//!
//! `PrintMapDlg` (`1108:a1c2`), resource 214 ([`crate::dialog::PRINT_MAP`]):
//! two one-character boxes in Arial 8 bold — pages across `by` pages down
//! — and Print, Cancel and Help. A key that is not `1` to `9` beeps and is
//! taken out again; Print complains about a box that is not a digit and
//! prints regardless, as the original does. The preview that follows is
//! this project's own, there being no printer: each page as the arm
//! would have drawn it, with a way to save them. See `docs/ui/print-map.md`.

use crate::App;

/// The button face the dialog is painted with.
const BUTTON_FACE: egui::Color32 = egui::Color32::from_rgb(0xc0, 0xc0, 0xc0);

/// Draw the dialog's contents. Returns the complaint Print raised, for
/// the shell to show.
pub fn view(app: &mut App, ui: &mut egui::Ui) -> Option<String> {
    let template = &crate::dialog::PRINT_MAP;
    let (rect, at, caption) = crate::views::dialog_frame(ui, template);
    let scale = template.scale(rect);
    ui.painter().rect_filled(rect, 0.0, BUTTON_FACE);
    let line_px = crate::dialog::DLU_Y * 8.0 * scale;
    let font = egui::FontId::proportional(line_px * 0.8);

    // The two boxes, one character each, Arial 8 bold in the original.
    for (index, id) in [0x10c_u16, 0x10d].into_iter().enumerate() {
        let mut text = app
            .print_map_dialog
            .as_ref()
            .map(|d| d.fields[index].clone())
            .unwrap_or_default();
        let field = ui.put(
            at(id),
            egui::TextEdit::singleline(&mut text)
                .font(font.clone())
                .char_limit(1)
                .horizontal_align(egui::Align::Center),
        );
        let name = if index == 0 {
            "pages across"
        } else {
            "pages down"
        };
        crate::views::record(app, ui, name, &field);
        if field.changed() {
            app.print_map_type(index, &text);
        }
    }
    for id in [0xffff_u16, 0xfffe] {
        let text = caption(id);
        let where_ = at(id);
        ui.painter().text(
            egui::pos2(where_.left(), where_.center().y),
            egui::Align2::LEFT_CENTER,
            text,
            font.clone(),
            egui::Color32::BLACK,
        );
    }

    let mut complaint = None;
    if crate::views::placed_button(app, ui, at(0x1), &caption(0x1), true).clicked() {
        complaint = app.print_map_ok();
    }
    if crate::views::placed_button(app, ui, at(0x2), &caption(0x2), true).clicked() {
        app.close_print_map();
    }
    // `WINHELP(HELP_CONTEXT, 0xc3c)` (`1108:a3c0`): the guide's page on
    // printing the map.
    if crate::views::placed_button(app, ui, at(0x76), &caption(0x76), true).clicked() {
        app.help_context(crate::help::context::PRINT_MAP);
    }
    complaint
}

/// The preview: the page on show, scaled to fit, with the pages stepped
/// through and a way to save them.
pub fn preview(app: &mut App, ui: &mut egui::Ui) {
    let Some(preview) = app.print_preview.clone() else {
        return;
    };
    let pages = preview.pages();
    ui.horizontal(|ui| {
        if crate::views::flow_button(app, ui, "Previous page", preview.page > 0).clicked() {
            if let Some(p) = app.print_preview.as_mut() {
                p.page = p.page.saturating_sub(1);
            }
        }
        ui.label(egui::RichText::new(format!("Page {} of {pages}", preview.page + 1)).small());
        if crate::views::flow_button(app, ui, "Next page", preview.page + 1 < pages).clicked() {
            if let Some(p) = app.print_preview.as_mut() {
                p.page = (p.page + 1).min(pages - 1);
            }
        }
        ui.separator();
        if crate::views::flow_button(app, ui, "Save pages…", true)
            .on_hover_text("Write every page out as a picture.")
            .clicked()
        {
            app.print_save_requested = true;
        }
        if crate::views::flow_button(app, ui, "Close", true).clicked() {
            app.close_print_preview();
        }
    });
    ui.separator();
    // The page, as large as the window allows at the sheet's proportions.
    let page = crate::printmap::PAGE;
    let available = ui.available_size();
    let scale = (available.x / page.width as f32)
        .min(available.y / page.height as f32)
        .max(0.05);
    let size = egui::vec2(page.width as f32 * scale, page.height as f32 * scale);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let (column, row) = preview.at();
    app.paint_print_page(ui.painter(), rect, scale, column, row);
    ui.painter().rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0_f32, egui::Color32::DARK_GRAY),
    );
}
