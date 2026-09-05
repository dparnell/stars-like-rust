//! The message pane.
//!
//! The original shows the year's news **one message at a time** in a pane of
//! its own, below the planet pane: a title bar saying which message of how
//! many, the message itself, and three buttons — Prev, Goto and Next
//! (`MessageWndProc`, `1030:5c92`).
//!
//! Two of the title bar's decorations are controls (`HtMsgBox`, `1030:7d8c`):
//! the square at the **left** silences the kind of message being shown, and the
//! square at the **right** shows the silenced ones anyway. The right one is
//! drawn only when something is actually hidden, which is the same condition
//! the original draws its own under.
//!
//! The keys are the original's: Up and Down step, Home and End jump to the
//! ends, Enter is Goto, `+` filters the message being shown and `-` toggles
//! showing the filtered ones.
//!
//! What is not the original: its title bar carries little bitmaps where this
//! has short labelled buttons, and its "FILTERED" watermark is drawn diagonally
//! across the message text where this puts it above.

use crate::App;

/// Draw the message pane.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    keys(app, ui);

    let filtered_here = app
        .current_message()
        .is_some_and(|m| app.message_filter().hidden(m.id));
    let has_filtered = app.has_filtered_messages();

    // --- the title bar, and the two controls in it
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if app.current_message().is_some() {
                    // `+` both ways, as in the original.
                    let (label, hint) = if filtered_here {
                        ("show", "stop filtering messages like this one  (+)")
                    } else {
                        ("filter", "stop showing messages like this one  (+)")
                    };
                    if ui.small_button(label).on_hover_text(hint).clicked() {
                        app.toggle_message_filter();
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if has_filtered {
                        let label = if app.view_filtered {
                            "hide filtered"
                        } else {
                            "view filtered"
                        };
                        if ui
                            .small_button(label)
                            .on_hover_text("show the messages you have filtered  (-)")
                            .clicked()
                        {
                            app.toggle_view_filtered();
                        }
                    }
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.label(app.message_title());
                    });
                });
            });
        });

    // --- the message itself
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            ui.set_min_height(48.0);
            ui.vertical(|ui| {
                if filtered_here && app.view_filtered {
                    // The original writes this across the text, corner to
                    // corner.
                    ui.label(egui::RichText::new("FILTERED").weak().strong());
                }
                egui::ScrollArea::vertical()
                    .max_height(72.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.label(app.message_body());
                    });
            });
        });

    // --- Prev, Goto, Next
    ui.horizontal(|ui| {
        let has_previous = app.message_previous(false).is_some();
        let has_next = app.message_next(false).is_some();
        if ui
            .add_enabled(has_previous, egui::Button::new("Prev"))
            .clicked()
        {
            app.show_previous_message();
        }
        let goto = app.message_goto() != stars_core::message::Goto::None;
        let label = app.message_goto_label();
        if ui.add_enabled(goto, egui::Button::new(label)).clicked() {
            app.message_goto_follow();
        }
        if ui
            .add_enabled(has_next, egui::Button::new("Next"))
            .clicked()
        {
            app.show_next_message();
        }
    });
}

/// The keys the original's pane answers to.
///
/// In the original the arrows belong to the message window, while `+`, `-` and
/// Enter are forwarded to it from the frame wherever the player is
/// (`mdi.c:1880`), so they work without clicking on the pane first. Here they
/// all work whenever no widget has taken the keyboard — which is what stops a
/// `-` typed into a text box from hiding messages.
fn keys(app: &mut App, ui: &egui::Ui) {
    if ui.memory(|m| m.focused()).is_some() {
        return;
    }
    ui.input(|i| {
        if i.key_pressed(egui::Key::ArrowDown) {
            app.show_next_message();
        }
        if i.key_pressed(egui::Key::ArrowUp) {
            app.show_previous_message();
        }
        if i.key_pressed(egui::Key::Home) {
            app.show_first_message();
        }
        if i.key_pressed(egui::Key::End) {
            app.show_last_message();
        }
        if i.key_pressed(egui::Key::Enter) {
            app.message_goto_follow();
        }
        if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
            app.toggle_message_filter();
        }
        if i.key_pressed(egui::Key::Minus) {
            app.toggle_view_filtered();
        }
    });
}
