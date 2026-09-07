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

// --- The prompt --------------------------------------------------------
//
// `PasswordDlg` / `IDD_PASSWORD`: what `FCheckPassword` puts up before a
// guarded turn is opened.

/// Write a two-player game to a temporary directory and hand back the path of
/// the first player's turn file, with `password` on it.
fn a_saved_game(name: &str, password: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    use stars_core::newgame::{NewGame, NewPlayer, Size};

    let mut app = App::new();
    app.new_game(&NewGame {
        name: name.to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    if !password.is_empty() {
        app.set_password(password);
    }

    let dir = std::env::temp_dir().join(format!("stars-ui-password-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let written = app
        .save_new_game(&dir.join(format!("{name}.hst")))
        .expect("writes the game");
    let turn = written
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "m1"))
        .expect("player 1's turn file")
        .clone();
    (dir, turn)
}

#[test]
fn a_turn_with_no_password_opens_straight_away() {
    let (dir, turn) = a_saved_game("Open", "");
    let mut app = asking();
    app.open(&turn).expect("opens");
    assert!(app.password_prompt.is_none());
    assert!(app.game.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// An app with somebody at the keyboard, which is what makes it ask.
fn asking() -> App {
    let mut app = App::new();
    app.prompt_for_password = true;
    app
}

#[test]
fn a_turn_with_a_password_waits_for_it() {
    let (dir, turn) = a_saved_game("Guarded", "cassini");
    let mut app = asking();
    app.open(&turn).expect("reads the file");

    // Read, but not opened: the game is not there until the password is.
    assert!(app.game.is_none(), "the save should be waiting");
    let prompt = app.password_prompt.as_ref().expect("a prompt");
    assert_eq!(prompt.salt, stars_formats::password_salt("cassini"));

    // A wrong one is refused, counted, and costs a wait.
    if let Some(prompt) = app.password_prompt.as_mut() {
        prompt.typed = "cassino".to_string();
    }
    assert!(!app.submit_password_prompt(0.0));
    assert!(app.game.is_none());
    assert_eq!(app.password_failures, 1);
    assert!(app.password_wait_left(0.0) > 0.0);
    assert!(app.password_wait_left(2.0) == 0.0, "a second is enough");
    let prompt = app.password_prompt.as_ref().expect("still asking");
    assert!(prompt.typed.is_empty(), "the box is cleared");
    assert!(prompt.error.is_some());

    // The right one opens it.
    if let Some(prompt) = app.password_prompt.as_mut() {
        prompt.typed = "cassini".to_string();
    }
    assert!(app.submit_password_prompt(2.0));
    assert!(app.password_prompt.is_none());
    assert!(app.game.is_some(), "the save should be open now");
    assert_eq!(app.password_wait_left(2.0), 0.0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancelling_the_prompt_gives_up_on_the_file() {
    let (dir, turn) = a_saved_game("Cancelled", "titan");
    let mut app = asking();
    app.open(&turn).expect("reads the file");
    app.cancel_password_prompt();
    assert!(app.password_prompt.is_none());
    assert!(app.game.is_none(), "the file should have been dropped");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The same password is asked for once a session, not once a file.
#[test]
fn a_password_already_given_is_not_asked_for_again() {
    let (dir, turn) = a_saved_game("Again", "europa");
    let mut app = asking();
    app.open(&turn).expect("reads the file");
    if let Some(prompt) = app.password_prompt.as_mut() {
        prompt.typed = "europa".to_string();
    }
    assert!(app.submit_password_prompt(0.0));

    app.open(&turn).expect("opens");
    assert!(
        app.password_prompt.is_none(),
        "asked twice for the same one"
    );
    assert!(app.game.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A password kept in `stars.ini` is offered before the prompt is.
#[test]
fn a_matching_default_password_skips_the_prompt() {
    let (dir, turn) = a_saved_game("Default", "ganymede");
    let mut app = asking();
    app.default_password = "ganymede".to_string();
    app.open(&turn).expect("opens");
    assert!(app.password_prompt.is_none());
    assert!(app.game.is_some());

    // One that does not match is no help.
    let mut app = asking();
    app.default_password = "callisto".to_string();
    app.open(&turn).expect("reads the file");
    assert!(app.password_prompt.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The wait after a wrong password grows with how many have been wrong.
#[test]
fn the_wait_grows_with_the_failures() {
    let mut app = App::new();
    assert_eq!(app.password_retry_delay_ms(), 1_000);
    app.password_failures = 9;
    assert_eq!(app.password_retry_delay_ms(), 1_000);
    app.password_failures = 10;
    assert_eq!(app.password_retry_delay_ms(), 5_000);
    app.password_failures = 99;
    assert_eq!(app.password_retry_delay_ms(), 5_000);
    app.password_failures = 100;
    assert_eq!(app.password_retry_delay_ms(), 10_000);
    app.password_failures = 10_000;
    assert_eq!(app.password_retry_delay_ms(), 10_000);
}

/// Choosing a password is the same as having typed it: the file it is written
/// into does not then ask for it.
#[test]
fn choosing_a_password_counts_as_giving_it() {
    let mut app = a_game();
    if let Some(dialog) = app.password_dialog.as_mut() {
        dialog.new = "phoebe".to_string();
        dialog.retype = "phoebe".to_string();
    }
    app.submit_password();
    assert_eq!(
        app.password_given,
        Some(stars_formats::password_salt("phoebe"))
    );
    assert!(!app.password_needed(stars_formats::password_salt("phoebe")));
    // Clearing it forgets it again.
    app.set_password("");
    assert_eq!(app.password_given, None);
}
