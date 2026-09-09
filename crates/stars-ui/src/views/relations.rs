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
    let line = ui.text_style_height(&egui::TextStyle::Small);

    // `&Player:` over the listbox.
    ui.put(
        at(0xffff),
        egui::Label::new(egui::RichText::new(caption(0xffff)).small()),
    );

    // The listbox. Plain black on white, in player-index order with the local
    // player left out, and with no owner-draw behind it: this is the only
    // control on the dialog that keeps the window background, because
    // `WM_CTLCOLOR` (`10f0:02aa`) answers with the button-face brush for
    // everything else and falls through for this one.
    let box_ = at(0x7d3);
    ui.painter().rect_filled(box_, 0.0, egui::Color32::WHITE);
    crate::views::draw_3d_frame(ui.painter(), box_);
    let inner = box_.shrink(2.0);
    let mut child = ui.child_ui(inner, egui::Layout::top_down(egui::Align::Min), None);
    child.set_clip_rect(inner);
    child.spacing_mut().item_spacing.y = 0.0;
    egui::ScrollArea::vertical()
        .id_source("relations-players")
        .show(&mut child, |ui| {
            for other in app.relations_others() {
                let name = app.psz_player_name(other);
                let row = ui.add(egui::SelectableLabel::new(
                    other == selected,
                    egui::RichText::new(name)
                        .small()
                        .color(if other == selected {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::BLACK
                        }),
                ));
                if row.clicked() {
                    app.relations_select(other);
                }
            }
        });

    // The `Relation` frame, drawn round the radios rather than placed with
    // them, and its caption written over its top edge.
    let frame = crate::dialog::relation_group(at(0x7d5), at(0x7d6), line);
    crate::views::draw_3d_frame(ui.painter(), frame);
    ui.painter().text(
        crate::dialog::relation_group_caption(frame, line),
        egui::Align2::LEFT_TOP,
        crate::dialog::RELATION_GROUP,
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().text_color(),
    );

    // The three radios, each where the template puts it — Friend at the top,
    // Neutral under it, Enemy at the bottom, which is not the order of their
    // values.
    let now = app.regard(selected);
    for (id, relation) in [
        (0x7d5u16, Relation::Friend),
        (0x7d4, Relation::Neutral),
        (0x7d6, Relation::Enemy),
    ] {
        if ui
            .put(
                at(id),
                egui::RadioButton::new(now == relation, egui::RichText::new(caption(id)).small()),
            )
            .clicked()
        {
            app.set_regard(selected, relation);
        }
    }

    // Close commits — there is no Cancel. Help goes to context `0x43b` in the
    // original and has nothing behind it here.
    if crate::views::dialog_button(ui, at(0x2), &caption(0x2), true).clicked() {
        app.close_relations();
    }
    crate::views::dialog_button(ui, at(0x76), &caption(0x76), false);
}
