//! The Host Mode dialog — `HostModeDialog`, `IDD_HOST_MODE`.
//!
//! See `docs/ui/host-mode.md`.

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::{opponents, Race};
use stars_ui::{App, TurnStatus};

/// A game written to a temporary directory, opened from its host file, with
/// one human player and one computer player.
fn a_hosted_game(name: &str) -> (std::path::PathBuf, App) {
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

    let dir = std::env::temp_dir().join(format!("stars-ui-host-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    app.save_new_game(&dir.join(format!("{name}.hst")))
        .expect("writes the game");
    app.open_host_mode();
    (dir, app)
}

/// Write player 2 an order file, as that player's client would.
fn submit(dir: &std::path::Path, name: &str, player: usize, app: &App, done: bool, year: i16) {
    let seed = app.game.as_ref().expect("a game").seed;
    let number = u8::try_from(player).expect("a player number");
    let header = stars_formats::FileHeader {
        flag_done: done,
        ..stars_formats::FileHeader::new(
            seed,
            stars_formats::FileType::Orders,
            number,
            year.unsigned_abs(),
            1,
        )
    };
    let bytes = app
        .order_log(0, [0; 11])
        .to_file(&header)
        .expect("builds the order file");
    std::fs::write(dir.join(format!("{name}.x{}", player + 1)), bytes).expect("writes");
}

#[test]
fn a_computer_player_is_never_waited_for() {
    let (dir, app) = a_hosted_game("Ais");
    // Player 1 is a person and has not submitted; player 2 is a computer
    // player, which the host never waits for.
    assert_eq!(app.turn_status(0), TurnStatus::StillOut);
    assert_eq!(app.turn_status(1), TurnStatus::TurnedIn);
    assert_eq!(app.turns_outstanding(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_submitted_turn_is_turned_in() {
    let (dir, app) = a_hosted_game("In");
    let year = app.game.as_ref().expect("a game").turn;
    submit(&dir, "In", 0, &app, true, year);
    assert_eq!(app.turn_status(0), TurnStatus::TurnedIn);
    assert_eq!(app.turns_outstanding(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every way a file can be there and still not count.
#[test]
fn a_file_that_is_not_this_turn_says_which_way_it_is_wrong() {
    let (dir, app) = a_hosted_game("Wrong");
    let year = app.game.as_ref().expect("a game").turn;

    submit(&dir, "Wrong", 0, &app, false, year);
    assert_eq!(app.turn_status(0), TurnStatus::PartiallyDone);

    submit(&dir, "Wrong", 0, &app, true, year + 3);
    assert_eq!(app.turn_status(0), TurnStatus::WrongYear);

    std::fs::write(dir.join("Wrong.x1"), b"not a Stars! file at all").expect("writes");
    assert_eq!(app.turn_status(0), TurnStatus::Corrupted);

    // All of them are still outstanding.
    assert_eq!(app.turns_outstanding(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dead_player_is_not_waited_for() {
    let (dir, mut app) = a_hosted_game("Dead");
    if let Some(game) = app.game.as_mut() {
        game.players[0].dead = true;
    }
    assert_eq!(app.turn_status(0), TurnStatus::Dead);
    assert!(!TurnStatus::Dead.outstanding());
    assert_eq!(app.turns_outstanding(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The two the original draws in green are the two nobody is waiting on.
#[test]
fn only_the_green_statuses_are_not_outstanding() {
    for status in [TurnStatus::Dead, TurnStatus::TurnedIn] {
        assert!(!status.outstanding(), "{}", status.name());
    }
    for status in [
        TurnStatus::StillOut,
        TurnStatus::PartiallyDone,
        TurnStatus::Corrupted,
        TurnStatus::WrongYear,
        TurnStatus::WrongGame,
    ] {
        assert!(status.outstanding(), "{}", status.name());
    }
    assert_eq!(TurnStatus::WrongGame.name(), "not in the right game");
}

#[test]
fn the_heading_says_what_is_open_and_what_comes_next() {
    let (dir, app) = a_hosted_game("Kestrel");
    assert_eq!(app.host_file_name(), "Kestrel");
    assert_eq!(app.game_name(), "Kestrel");
    let year = app.game.as_ref().expect("a game").year();
    assert_eq!(app.next_year(), year + 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Generating advances the year, and writes everybody's files.
#[test]
fn generating_moves_the_year_on() {
    let (dir, mut app) = a_hosted_game("Turn");
    let before = app.game.as_ref().expect("a game").turn;
    app.generate_turns(1);
    assert_eq!(app.game.as_ref().expect("a game").turn, before + 1);
    assert_eq!(app.next_year(), 2400 + i32::from(before) + 2);

    app.generate_turns(3);
    assert_eq!(app.game.as_ref().expect("a game").turn, before + 4);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Shift and Ctrl turn one generation into a run of them.
#[test]
fn the_modifiers_force_a_run_of_turns() {
    assert_eq!(App::generate_passes(false, false), 1);
    assert_eq!(App::generate_passes(true, false), 9);
    assert_eq!(App::generate_passes(false, true), 99);
    assert_eq!(App::generate_passes(true, true), 999);
}

/// The four formats the original writes "time since last change" in.
#[test]
fn the_elapsed_clock_reads_as_the_original_writes_it() {
    use stars_ui::views::host::elapsed_text;
    assert_eq!(elapsed_text(0.0), "0 seconds");
    assert_eq!(elapsed_text(59.0), "59 seconds");
    assert_eq!(elapsed_text(64.0), "1:04");
    assert_eq!(elapsed_text(3_600.0), "1:00:00");
    assert_eq!(elapsed_text(90_061.0), "1 days 1:01:01");
}
