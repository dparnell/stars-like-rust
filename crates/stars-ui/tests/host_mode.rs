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

// --- Auto Generate -----------------------------------------------------
//
// There are no options behind it in 2.7j: `DrawHostOptions` draws nothing and
// nothing in the program ever writes the force-generate fields, so what the
// button does is the one thing `HostTimerProc` implements — look every ten
// seconds and generate the year when every turn is in.

#[test]
fn the_watch_looks_on_the_ten_second_beat() {
    let (dir, mut app) = a_hosted_game("Watch");
    assert!(!app.auto_generate);
    assert!(!app.auto_generate_due(0.0), "not watching, nothing is due");

    app.set_auto_generate(true, 100.0);
    assert!(app.auto_generate);
    // It has just looked, so it is not due again until the beat comes round.
    assert!(!app.auto_generate_due(100.0));
    assert!(!app.auto_generate_due(109.9));
    assert!(app.auto_generate_due(110.0));
    assert_eq!(App::HOST_TICK_SECONDS, 10.0);

    app.auto_generate_looked(110.0);
    assert!(!app.auto_generate_due(115.0));

    app.set_auto_generate(false, 120.0);
    assert!(!app.auto_generate_due(200.0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A game with nobody in it cannot be watched: there is nothing to wait for.
#[test]
fn a_game_of_computer_players_cannot_auto_generate() {
    use stars_core::newgame::{NewGame, Size};

    let mut app = App::new();
    app.new_game(&NewGame {
        name: "Ai".to_string(),
        size: Size::Small,
        players: vec![
            opponents::opponent(0, 1).expect("an opponent").as_player(),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    assert!(app.auto_generate_blocked());
    app.set_auto_generate(true, 0.0);
    assert!(!app.auto_generate, "it should refuse to watch");

    // A game with a person in it is fine.
    let (dir, mut app) = a_hosted_game("Person");
    assert!(!app.auto_generate_blocked());
    app.set_auto_generate(true, 0.0);
    assert!(app.auto_generate);
    let _ = std::fs::remove_dir_all(&dir);
}

/// What host mode says while it waits — the line the original writes into the
/// frame's title bar.
#[test]
fn the_watch_says_how_many_are_out() {
    let (dir, app) = a_hosted_game("Waiting");
    assert_eq!(app.host_title(), "Host Mode 1 Player Out");
    let year = app.game.as_ref().expect("a game").turn;
    submit(&dir, "Waiting", 0, &app, true, year);
    assert_eq!(app.host_title(), "Host Mode 0 Players Out");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The dialog is resource 115, and its player list is painted rather than
/// placed — everything from `y = 28` down the left is `DrawHostDialog2`'s.
#[test]
fn the_template_leaves_the_player_list_to_the_painter() {
    use stars_ui::dialog::{Class, HOST_LIST_TOP, HOST_MODE, HOST_NUMBER_SAMPLE};

    assert_eq!(HOST_MODE.caption, "Stars! Host Mode");
    assert_eq!(HOST_MODE.size, (260, 220));
    assert_eq!(
        HOST_MODE.controls.len(),
        15 - 1,
        "the icon placeholder aside"
    );

    // Every control the template places is at x >= 190 or in the two header
    // rows, so the left of the dialog below y = 28 is empty.
    for control in HOST_MODE.controls {
        assert!(
            control.at.0 >= 190 || control.at.1 < 28,
            "{:?} is in the list's area",
            control.text
        );
    }
    assert_eq!(HOST_LIST_TOP, 48.0, "and the list starts below that");

    // The five buttons run down the right, 65 by 14, twenty apart.
    for (index, id) in [0x407u16, 0x408, 0x7df, 0x2, 0x76].into_iter().enumerate() {
        let control = HOST_MODE.control(id).expect("a host button");
        assert_eq!(control.class, Class::Button);
        assert_eq!(control.at.0, 190);
        #[allow(clippy::cast_possible_truncation)]
        let y = 54 + 20 * index as i16;
        assert_eq!(control.at.1, y);
        assert_eq!((control.at.2, control.at.3), (65, 14));
    }
    assert_eq!(
        HOST_MODE.control(0x408).expect("auto").label(),
        "Auto Generate"
    );
    // The number column is measured from a literal, not from the widest row.
    assert_eq!(HOST_NUMBER_SAMPLE, "#16:");
}

/// A row is a **sentence**, and the number carries a hash — both of which this
/// project had wrong.
#[test]
fn a_row_reads_as_the_original_writes_it() {
    use stars_core::newgame::{NewGame, NewPlayer, Size};
    use stars_core::{opponents, Race};
    use stars_ui::App;

    let mut app = App::new();
    app.new_game(&NewGame {
        name: "host".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");

    let (number, sentence) = app.host_row(0);
    assert_eq!(number, "#1:", "`#%d:`, not a bare figure");
    assert!(
        sentence.ends_with('.'),
        "`%s are %s.` is a sentence: {sentence}"
    );
    assert!(sentence.contains(" are "), "{sentence}");

    // `fNoHostNames` drops the name and leaves the status, so a host who
    // should not know who is who still sees who is waited for.
    app.no_host_names = true;
    let (number, hidden) = app.host_row(0);
    assert_eq!(number, "#1:", "the number stays");
    assert!(!hidden.contains(" are "), "{hidden}");
    assert!(hidden.starts_with(' '), "` %s`: {hidden}");
    assert!(
        sentence.contains(hidden.trim()),
        "the same status word, without the name"
    );
}
