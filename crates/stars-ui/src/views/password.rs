//! The Change Password dialog.
//!
//! `NewPasswordDlg`, `IDD_NEW_PASSWORD` (141) — Commands (Change Password...),
//! the last item of that menu.
//!
//! ```text
//!   New Password:     [                ]      [ OK     ]
//!   Retype Password:  [                ]      [ Cancel ]
//!                                             [ Help   ]
//!   Note: ...
//! ```
//!
//! There is no *old* password box, because there is nothing to check it
//! against: the game keeps a salt of the password rather than the password, so
//! anyone who can open the file can change it. See
//! `docs/ui/change-password.md`.

use crate::App;

/// Draw the dialog.
pub fn view(app: &mut App, ui: &mut egui::Ui) {
    if app.password_dialog.is_none() {
        return;
    }
    let template = &crate::dialog::CHANGE_PASSWORD;
    let (rect, at, caption) = crate::views::dialog_frame(ui, template);
    let host = app.password_dialog.as_ref().is_some_and(|d| d.host);

    // In host mode the caption changes (string `0x35e`).
    ui.painter().text(
        rect.min + egui::vec2(6.0, 2.0),
        egui::Align2::LEFT_TOP,
        if host {
            crate::dialog::CHANGE_HOST_PASSWORD
        } else {
            template.caption
        },
        egui::TextStyle::Small.resolve(ui.style()),
        ui.visuals().text_color(),
    );

    let mut submit = false;
    let mut cancel = false;
    let mut clear = false;

    // The two boxes, each with its own label and each `ES_PASSWORD`.
    for (label_id, edit_id) in [(0xffffu16, 0x10cu16), (0xfffe, 0x10d)] {
        ui.put(
            at(label_id),
            egui::Label::new(egui::RichText::new(caption(label_id)).small()),
        );
        let Some(dialog) = app.password_dialog.as_mut() else {
            return;
        };
        let text = if edit_id == 0x10c {
            &mut dialog.new
        } else {
            &mut dialog.retype
        };
        let field = ui.put(
            at(edit_id),
            egui::TextEdit::singleline(text)
                .password(true)
                .char_limit(stars_formats::PASSWORD_FIELD_LIMIT),
        );
        submit |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    }

    // The static across the foot, which the original fills in at run time: it
    // says **when** the new password starts to bind, and that differs for a
    // player and for the host.
    let note = at(0x7e2);
    let error = app.password_dialog.as_ref().and_then(|d| d.error.clone());
    {
        let mut child = ui.child_ui(note, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(note);
        if let Some(error) = error {
            child.colored_label(egui::Color32::from_rgb(0xff, 0x6b, 0x6b), error);
        }
        child.label(egui::RichText::new(app.password_note()).small().weak());
    }

    submit |= crate::views::dialog_button(ui, at(0x1), &caption(0x1), true).clicked();
    cancel |= crate::views::dialog_button(ui, at(0x2), &caption(0x2), true).clicked();
    // The Help button has nothing behind it here.
    crate::views::dialog_button(ui, at(0x76), &caption(0x76), false);

    // An empty password is how the original clears one, so `Clear` is only a
    // shortcut for typing nothing in both boxes — it is this project's, not
    // the template's, and sits under the buttons rather than among them.
    let set = if host {
        app.has_host_password()
    } else {
        app.has_password()
    };
    let below = egui::Rect::from_min_size(
        egui::pos2(at(0x76).left(), at(0x76).bottom() + 2.0),
        at(0x76).size(),
    );
    clear |= ui
        .put(
            below,
            egui::Button::new(egui::RichText::new("Clear").small()).sense(if set {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            }),
        )
        .on_hover_text("Play without a password. The same as leaving both boxes empty.")
        .clicked();

    if clear && set {
        if host {
            app.set_host_password("");
        } else {
            app.set_password("");
        }
        app.close_password_dialog();
    } else if cancel {
        app.close_password_dialog();
    } else if submit {
        app.submit_password();
    }
}

/// Draw the prompt that asks for a turn password.
///
/// `PasswordDlg`, `IDD_PASSWORD` (140): one box, the question above it, OK and
/// Cancel. Cancelling gives up on the file, as the original's loader does.
///
/// `now` is the frontend's clock in seconds, which the retry delay is measured
/// against.
pub fn prompt(app: &mut App, ui: &mut egui::Ui, now: f64) {
    if app.password_prompt.is_none() {
        return;
    }
    let template = &crate::dialog::PASSWORD_PROMPT;
    let (_, at, caption) = crate::views::dialog_frame(ui, template);

    // The static, filled in at run time (string `0x35f`).
    ui.put(
        at(0x7e2),
        egui::Label::new(egui::RichText::new(crate::dialog::PASSWORD_PROMPT_LABEL).small()),
    );

    let mut submit = false;
    let field = {
        let Some(state) = app.password_prompt.as_mut() else {
            return;
        };
        ui.put(
            at(0x10c),
            egui::TextEdit::singleline(&mut state.typed)
                .password(true)
                .char_limit(PROMPT_FIELD_LIMIT),
        )
    };
    if ui.memory(|m| m.focused().is_none()) {
        field.request_focus();
    }
    submit |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let error = app.password_prompt.as_ref().and_then(|s| s.error.clone());

    // A wrong password costs a wait before the next attempt, which grows with
    // how many have been wrong. See `App::password_retry_delay_ms`.
    let wait = app.password_wait_left(now);
    let under = egui::Rect::from_min_max(
        egui::pos2(at(0x10c).left(), at(0x10c).bottom() + 2.0),
        egui::pos2(at(0x1).left() - 4.0, at(0x76).bottom()),
    );
    if under.height() > 4.0 {
        let mut child = ui.child_ui(under, egui::Layout::top_down(egui::Align::Min), None);
        child.set_clip_rect(under);
        if let Some(error) = error {
            child.colored_label(egui::Color32::from_rgb(0xff, 0x6b, 0x6b), error);
        }
        if wait > 0.0 {
            child.label(
                egui::RichText::new(format!("Try again in {:.0}s.", wait.ceil()))
                    .small()
                    .weak(),
            );
        }
    }

    submit |= crate::views::dialog_button(ui, at(0x1), &caption(0x1), wait <= 0.0).clicked();
    let cancel = crate::views::dialog_button(ui, at(0x2), &caption(0x2), true).clicked();
    crate::views::dialog_button(ui, at(0x76), &caption(0x76), false);

    if cancel {
        app.cancel_password_prompt();
    } else if submit && wait <= 0.0 {
        app.submit_password_prompt(now);
    }
}

/// What the prompt's box will take.
///
/// `PasswordDlg` limits it to fifteen characters — one fewer than the Change
/// Password dialog, which is its own inconsistency and not this project's.
const PROMPT_FIELD_LIMIT: usize = 15;
