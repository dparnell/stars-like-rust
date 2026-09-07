//! The Change Password dialog — Commands (Change Password...).
//!
//! See `docs/ui/change-password.md`.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::App;

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "password".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app.open_password_dialog();
    app
}

/// What the local player's block holds.
fn salt(app: &App) -> u32 {
    app.game.as_ref().expect("a game").players[app.local_player()].password
}

#[test]
fn a_new_game_has_no_password() {
    let app = a_game();
    assert!(!app.has_password());
    assert_eq!(salt(&app), 0);
    let dialog = app.password_dialog.as_ref().expect("open");
    assert!(dialog.new.is_empty() && dialog.retype.is_empty());
    assert!(dialog.error.is_none());
}

#[test]
fn typing_the_same_password_twice_sets_it() {
    let mut app = a_game();
    if let Some(dialog) = app.password_dialog.as_mut() {
        dialog.new = "hyperion".to_string();
        dialog.retype = "hyperion".to_string();
    }
    assert!(app.submit_password(), "it should close");
    assert!(app.password_dialog.is_none());
    assert!(app.has_password());
    assert_eq!(salt(&app), stars_formats::password_salt("hyperion"));
}

#[test]
fn a_mismatch_is_refused_and_clears_both_boxes() {
    let mut app = a_game();
    if let Some(dialog) = app.password_dialog.as_mut() {
        dialog.new = "hyperion".to_string();
        dialog.retype = "hyperlon".to_string();
    }
    assert!(!app.submit_password(), "it should stay open");
    let dialog = app.password_dialog.as_ref().expect("still open");
    assert!(dialog.error.is_some());
    assert!(dialog.new.is_empty() && dialog.retype.is_empty());
    assert!(!app.has_password(), "nothing should have been set");
}

/// The original compares the **salts** of the two boxes, not their text, so
/// two strings that fold together are the same password to it.
#[test]
fn the_two_boxes_are_compared_by_salt() {
    // A trailing NUL ends the string the original folds, so these two salt
    // identically while differing as text.
    let with_nul = "orion\0extra";
    assert_eq!(
        stars_formats::password_salt(with_nul),
        stars_formats::password_salt("orion")
    );
    let mut app = a_game();
    if let Some(dialog) = app.password_dialog.as_mut() {
        dialog.new = "orion".to_string();
        dialog.retype = with_nul.to_string();
    }
    assert!(app.submit_password(), "same salt, so the same password");
    assert_eq!(salt(&app), stars_formats::password_salt("orion"));
}

#[test]
fn an_empty_pair_clears_the_password() {
    let mut app = a_game();
    if let Some(dialog) = app.password_dialog.as_mut() {
        dialog.new = "hyperion".to_string();
        dialog.retype = "hyperion".to_string();
    }
    app.submit_password();
    assert!(app.has_password());

    app.open_password_dialog();
    assert!(app.submit_password(), "two empty boxes are a valid pair");
    assert!(!app.has_password());
    assert_eq!(salt(&app), 0);
}

/// Setting the password logs one `rtChgPassword` record, and a run of changes
/// collapses to the last of them — the host applies whichever survives.
#[test]
fn changing_it_twice_logs_one_record() {
    let mut app = a_game();
    let before = app.orders.len();
    for text in ["first", "second"] {
        app.open_password_dialog();
        if let Some(dialog) = app.password_dialog.as_mut() {
            dialog.new = text.to_string();
            dialog.retype = text.to_string();
        }
        app.submit_password();
    }
    let logged: Vec<_> = app.orders[before..]
        .iter()
        .filter(|r| r.record_type == stars_formats::LogRecordType::ChangePassword)
        .collect();
    assert_eq!(logged.len(), 1);
    assert_eq!(salt(&app), stars_formats::password_salt("second"));
}

#[test]
fn cancelling_keeps_the_old_password() {
    let mut app = a_game();
    if let Some(dialog) = app.password_dialog.as_mut() {
        dialog.new = "keepme".to_string();
        dialog.retype = "keepme".to_string();
    }
    app.submit_password();
    let before = salt(&app);

    app.open_password_dialog();
    if let Some(dialog) = app.password_dialog.as_mut() {
        dialog.new = "changed".to_string();
        dialog.retype = "changed".to_string();
    }
    app.close_password_dialog();
    assert_eq!(salt(&app), before);
}

/// The dialog's boxes stop at sixteen characters, which is what the original
/// limits them to; the buffer behind them would take one more.
#[test]
fn the_field_limit_is_shorter_than_the_buffer() {
    assert_eq!(stars_formats::PASSWORD_FIELD_LIMIT, 16);
    assert_eq!(stars_formats::MAX_PASSWORD_LEN, 17);
}
