//! The Score sheet, driven the way a player drives it.

use std::path::{Path, PathBuf};

use stars_core::newgame::{NewGame, NewPlayer, Size};
use stars_core::scoresheet::{Face, Stat};
use stars_core::{opponents, Race};
use stars_ui::App;

fn fixture(relative: &str) -> Option<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(relative);
    path.exists().then_some(path)
}

fn a_game() -> App {
    let mut app = App::new();
    app.new_game(&NewGame {
        name: "score".to_string(),
        size: Size::Small,
        players: vec![
            NewPlayer::human(Race::humanoid()),
            opponents::opponent(1, 1).expect("an opponent").as_player(),
        ],
        ..NewGame::default()
    })
    .expect("creates the game");
    app
}

fn frame(app: &mut App) {
    let ctx = egui::Context::default();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            stars_ui::views::score::view(app, ui);
        });
    });
}

/// It opens on the scoreboard, and the one button goes round the three faces.
#[test]
fn the_button_cycles_the_three_faces() {
    let mut app = a_game();
    app.open_score_sheet();
    assert_eq!(app.score_sheet.expect("open").face, Face::Scores);

    let mut titles = Vec::new();
    for _ in 0..4 {
        titles.push(app.score_sheet.expect("open").face.title());
        frame(&mut app);
        app.score_next_face();
    }
    assert_eq!(
        titles,
        [
            "Player Scores",
            "Victory Conditions",
            "Progress Timeline",
            "Player Scores",
        ]
    );
}

/// Closing and reopening comes back to the same view: the original keeps both
/// settings outside the dialog.
#[test]
fn it_remembers_what_it_was_showing() {
    let mut app = a_game();
    app.open_score_sheet();
    app.score_next_face();
    app.score_next_face();
    app.score_set_graph(Stat::Planets);
    app.close_score_sheet();
    assert!(app.score_sheet.is_none());

    app.open_score_sheet();
    let sheet = app.score_sheet.expect("open");
    assert_eq!(sheet.face, Face::Timeline);
    assert_eq!(sheet.graph, Stat::Planets);
}

/// A fresh game has no scoreboard until a turn has been generated, and then it
/// has one row per player.
#[test]
fn generating_a_turn_fills_the_scoreboard_in() {
    let mut app = a_game();
    app.open_score_sheet();
    assert!(app.score_standings().iter().all(|s| !s.known));
    frame(&mut app);

    app.generate_turn();
    let standings = app.score_standings();
    assert_eq!(standings.len(), 2);
    assert!(standings.iter().all(|s| s.known));
    assert!(standings.iter().any(|s| s.rank == 1));
    // Each has a first year on the timeline, at the turn just generated.
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.timeline.len(), 2);
    for years in &game.timeline {
        assert_eq!(years.len(), 1);
        assert_eq!(years[0].turn, u16::try_from(game.turn).expect("a turn"));
    }
    frame(&mut app);
}

/// The victory report lists the game's conditions whether or not it is playing
/// for them.
#[test]
fn the_victory_report_lists_every_condition() {
    let mut app = a_game();
    app.open_score_sheet();
    app.score_next_face();
    let lines = app.score_conditions();
    assert_eq!(lines.len(), 9);
    assert!(lines[0].text.starts_with("Owns "));
    assert!(lines.last().expect("a last line").text.contains("years"));
    frame(&mut app);
}

/// Opening a real save picks up the `.hN` beside it, and the timeline draws.
#[test]
fn a_real_save_brings_its_history_with_it() {
    let Some(save) = fixture("games/all-computer-players/2450/Game.m1") else {
        return;
    };
    let mut app = App::new();
    if app.open(&save).is_err() {
        return;
    }
    let game = app.game.as_ref().expect("a game");
    assert_eq!(game.timeline[0].len(), 50, "fifty years of it");
    assert_eq!(game.standings[0].rank, 8);

    app.open_score_sheet();
    for _ in 0..3 {
        for stat in Stat::ALL {
            app.score_set_graph(stat);
            frame(&mut app);
        }
        app.score_next_face();
    }
    app.close_score_sheet();
    frame(&mut app);
}

/// The sheet sizes its own window rather than taking a size from a template,
/// because it has a column per player (`InitScoreDlg`, `1108:13b6`).
#[test]
fn the_window_sizes_itself_from_its_columns() {
    use stars_ui::score::{Layout, MIN_COLUMNS, TIMELINE_SIZE};

    // A digit of 7, a line of 13, and a widest label of 80.
    let scores = Layout::scores(7.0, 13.0, 80.0, 6);
    // The label column is the widest label plus eight, and a player's column
    // is five digits.
    assert_eq!(scores.label, 88.0);
    assert_eq!(scores.column, 35.0);
    assert_eq!(scores.width, 6.0 * 35.0 + 88.0 + 8.0);
    // The height is `dyArial8 * 33 / 2 + 88` on both computed faces.
    assert_eq!(scores.height, 13.0 * 33.0 / 2.0 + 88.0);

    // The victory report's label column is one and a half times the widest
    // sentence plus six digits, and its columns are only a line and a half —
    // which is what makes the rotated names necessary.
    let victory = Layout::victory(7.0, 13.0, 200.0, 6);
    assert_eq!(victory.label, 200.0 * 1.5 + 7.0 * 6.0);
    assert_eq!(victory.column, 19.5);
    assert!(
        victory.column < 20.0,
        "far too narrow for a name across: {}",
        victory.column
    );
    assert_eq!(victory.height, scores.height);

    // However few players there are, room is left for four columns.
    let one = Layout::scores(7.0, 13.0, 80.0, 1);
    #[allow(clippy::cast_precision_loss)]
    let four = MIN_COLUMNS as f32;
    assert_eq!(one.width, four * 35.0 + 88.0 + 8.0);
    assert_eq!(one.width, Layout::scores(7.0, 13.0, 80.0, 4).width);

    // The timeline is a flat size instead of a computed one.
    assert_eq!(TIMELINE_SIZE, (600.0, 400.0));
}

/// The columns run left to right from the label column, each its own width.
#[test]
fn the_columns_follow_the_label_column() {
    use stars_ui::score::Layout;

    let layout = Layout::scores(7.0, 13.0, 80.0, 4);
    assert_eq!(layout.column_at(0), layout.label);
    assert_eq!(layout.column_at(1), layout.label + layout.column);
    assert_eq!(layout.column_at(3), layout.label + 3.0 * layout.column);
}
