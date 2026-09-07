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
    let Some(dialog) = app.password_dialog.as_mut() else {
        return;
    };
    let mut submit = false;
    let mut cancel = false;

    egui::Grid::new("password-fields")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for (label, text) in [
                ("New Password:", &mut dialog.new),
                ("Retype Password:", &mut dialog.retype),
            ] {
                ui.label(label);
                let field = ui.add(
                    egui::TextEdit::singleline(text)
                        .password(true)
                        .char_limit(stars_formats::PASSWORD_FIELD_LIMIT)
                        .desired_width(140.0),
                );
                submit |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.end_row();
            }
        });

    if let Some(error) = dialog.error.clone() {
        ui.colored_label(egui::Color32::from_rgb(0xff, 0x6b, 0x6b), error);
    }
    ui.label(egui::RichText::new(app.password_note()).small().weak());

    ui.separator();
    let mut clear = false;
    ui.horizontal(|ui| {
        submit |= ui.button("OK").clicked();
        cancel |= ui.button("Cancel").clicked();
        // An empty password is how the original clears one, so this is only a
        // shortcut for typing nothing in both boxes.
        clear = ui
            .add_enabled(app.has_password(), egui::Button::new("Clear"))
            .on_hover_text("Play without a password. The same as leaving both boxes empty.")
            .clicked();
    });

    if clear {
        app.set_password("");
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
    let Some(state) = app.password_prompt.as_mut() else {
        return;
    };
    // String `0x035f`.
    ui.label("Enter the password:");
    let mut submit = false;
    let field = ui.add(
        egui::TextEdit::singleline(&mut state.typed)
            .password(true)
            .char_limit(PROMPT_FIELD_LIMIT)
            .desired_width(160.0),
    );
    if ui.memory(|m| m.focused().is_none()) {
        field.request_focus();
    }
    submit |= field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let error = state.error.clone();

    // A wrong password costs a wait before the next attempt, which grows with
    // how many have been wrong. See `App::password_retry_delay_ms`.
    let wait = app.password_wait_left(now);
    if let Some(error) = error {
        ui.colored_label(egui::Color32::from_rgb(0xff, 0x6b, 0x6b), error);
    }
    if wait > 0.0 {
        ui.label(
            egui::RichText::new(format!("Try again in {:.0}s.", wait.ceil()))
                .small()
                .weak(),
        );
    }

    ui.separator();
    let mut cancel = false;
    ui.horizontal(|ui| {
        submit |= ui
            .add_enabled(wait <= 0.0, egui::Button::new("OK"))
            .clicked();
        cancel = ui.button("Cancel").clicked();
    });

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
