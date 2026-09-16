//! The Player Relations dialog.
//!
//! `RelationsDlg` (`10f0:0088`), reached from Commands (Player Relations) or
//! **F7**. Modal, and the whole of it is a list of the other players beside
//! three radio buttons saying how this player regards whichever is selected.
//!
//! The layout is the dialog resource's — 2008, [`crate::dialog::RELATIONS`] —
//! rather than anything arranged here. The one thing on it that is *not* a
//! control is the `Relation` frame around the radios, which `WM_PAINT` draws
//! by hand from where Windows put the first and last of them.
//!
//! See `docs/ui/player-relations.md`.

use crate::App;
use stars_core::relations::Relation;

/// Draw the dialog's contents.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    let Some(selected) = app.relations_dialog else {
        return;
    };
    let template = &crate::dialog::RELATIONS;
    let (_, at, caption) = crate::views::dialog_frame(ui, template);

    // The original's `dyArial8`, at whatever scale the template came out at.
    // The dialog's font is what defines a dialog unit, so a line of it is
    // eight vertical units by construction, and any placed control gives the
    // scale away — the radios are twelve units tall.
    let line = at(0x7d5).height() / 12.0 * 8.0;
    let font = egui::FontId::proportional(line * 0.8);

    // `&Player:` over the listbox. `SS_LEFT`, so it starts at the static's
    // left edge rather than sitting in the middle of it.
    left_aligned(
        ui,
        at(0xffff),
        egui::Label::new(text(&caption(0xffff), &font)),
    );

    // The listbox. `WS_BORDER` and nothing else: a single dark line round a
    // white field. This is the only control the dialog leaves alone —
    // `WM_CTLCOLOR` (`10f0:02aa`) hands back the button-face brush for every
    // other one — so it is the only white thing on it, and with no owner-draw
    // behind it the rows are the system's: black on white, and white on
    // `COLOR_HIGHLIGHT` for the selection.
    //
    // Its 65 units start 16 down on a dialog only 80 tall, so it overruns the
    // bottom by one unit. That is the resource as shipped.
    let box_ = at(0x7d3);
    let painter = ui.painter().with_clip_rect(box_);
    painter.rect_filled(box_, 0.0, WINDOW);
    painter.rect_stroke(box_, 0.0, egui::Stroke::new(1.0_f32, WINDOW_FRAME));
    let inner = box_.shrink(1.0);
    let mut child = ui.child_ui(inner, egui::Layout::top_down(egui::Align::Min), None);
    child.set_clip_rect(inner);
    egui::ScrollArea::vertical()
        .id_source("relations-players")
        .show(&mut child, |ui| {
            for other in app.relations_others() {
                let (row, response) =
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), line), CLICK);
                let chosen = other == selected;
                if chosen {
                    ui.painter().rect_filled(row, 0.0, HIGHLIGHT);
                }
                ui.painter().text(
                    egui::pos2(row.left() + 2.0, row.center().y),
                    egui::Align2::LEFT_CENTER,
                    app.psz_player_name(other),
                    font.clone(),
                    if chosen { WINDOW } else { WINDOW_TEXT },
                );
                if response.clicked() {
                    app.relations_select(other);
                }
            }
        });

    // The `Relation` frame, drawn round the radios rather than placed with
    // them, and its caption written over its top edge in the bold face.
    let frame = crate::dialog::relation_group(at(0x7d5), at(0x7d6), line);
    crate::views::draw_3d_frame(ui.painter(), frame);
    let where_ = crate::dialog::relation_group_caption(frame, line);
    let group = crate::dialog::RELATION_GROUP;
    let width = ui
        .painter()
        .layout_no_wrap(group.to_string(), font.clone(), WINDOW_TEXT)
        .rect
        .width();
    // The caption sits on the frame's top edge, so the edge is cleared out
    // from under it first, the way `TextOut` with an opaque background does.
    ui.painter().rect_filled(
        egui::Rect::from_min_size(where_, egui::vec2(width + 2.0, line)),
        0.0,
        FACE,
    );
    ui.painter().text(
        where_,
        egui::Align2::LEFT_TOP,
        group,
        font.clone(),
        WINDOW_TEXT,
    );

    // The three radios, each where the template puts it — Friend at the top,
    // Neutral under it, Enemy at the bottom, which is not the order of their
    // values. The button is at the left edge of its rectangle, not centred in
    // it: none of them carries `BS_RIGHTBUTTON`.
    let now = app.regard(selected);
    for (id, relation) in [
        (0x7d5u16, Relation::Friend),
        (0x7d4, Relation::Neutral),
        (0x7d6, Relation::Enemy),
    ] {
        let button = egui::RadioButton::new(now == relation, text(&caption(id), &font));
        if left_aligned(ui, at(id), button).clicked() {
            app.set_regard(selected, relation);
        }
    }

    // Close commits — there is no Cancel. Help goes to context `0x43b`
    // (`10f0:0434`), the Player Relations page.
    if crate::views::dialog_button(ui, at(0x2), &caption(0x2), true).clicked() {
        app.close_relations();
    }
    if crate::views::dialog_button(ui, at(0x76), &caption(0x76), true).clicked() {
        app.help_context(crate::help::context::RELATIONS);
    }
}

/// `COLOR_WINDOW`, which is also `COLOR_HIGHLIGHTTEXT`.
const WINDOW: egui::Color32 = egui::Color32::WHITE;
/// `COLOR_WINDOWTEXT`.
const WINDOW_TEXT: egui::Color32 = egui::Color32::BLACK;
/// What `WS_BORDER` draws round a control — `COLOR_WINDOWFRAME`.
const WINDOW_FRAME: egui::Color32 = egui::Color32::BLACK;
/// `COLOR_HIGHLIGHT`, the listbox's selection bar.
const HIGHLIGHT: egui::Color32 = egui::Color32::from_rgb(0x00, 0x00, 0x80);
/// `COLOR_BTNFACE`, which everything but the listbox sits on.
const FACE: egui::Color32 = egui::Color32::from_rgb(
    crate::toolbar::FACE[0],
    crate::toolbar::FACE[1],
    crate::toolbar::FACE[2],
);
/// A listbox row answers to a click and to nothing else.
const CLICK: egui::Sense = egui::Sense {
    click: true,
    drag: false,
    focusable: false,
};

/// The dialog's own text, at the dialog's own size.
fn text(caption: &str, font: &egui::FontId) -> egui::RichText {
    egui::RichText::new(caption)
        .font(font.clone())
        .color(WINDOW_TEXT)
}

/// Put a control where the template says, against the **left** edge of its
/// rectangle rather than in the middle of it.
///
/// Windows draws a `SS_LEFT` static and a radio button's circle at the left of
/// the control, and these templates leave slack on the right — a 64-unit
/// static holding `Player:` most of all — so centring visibly moves them.
fn left_aligned(ui: &mut egui::Ui, rect: egui::Rect, widget: impl egui::Widget) -> egui::Response {
    let mut child = ui.child_ui(rect, egui::Layout::left_to_right(egui::Align::Center), None);
    child.set_clip_rect(rect);
    child.add(widget)
}
