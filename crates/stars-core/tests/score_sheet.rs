//! The Score sheet's model, against the scoreboards Stars! itself wrote.
//!
//! See `docs/ui/score-sheet.md`.

use std::path::{Path, PathBuf};

use stars_core::scoresheet::{self, Face, Stat};
use stars_core::GameState;
use stars_formats::{victory, StarsFile};

fn fixture(relative: &str) -> Option<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(relative);
    path.exists().then_some(path)
}

/// One line of the victory report by the condition it is about.
///
/// The lines are **not** indexed by condition: `TECH_FIELDS` is a second value
/// for the tech condition rather than a line of its own, so the numbering
/// slips by one after it.
fn line_for(lines: &[scoresheet::Condition], which: usize) -> scoresheet::Condition {
    lines
        .iter()
        .find(|line| line.which == which)
        .expect("a line for that condition")
        .clone()
}

fn read(relative: &str) -> Option<StarsFile> {
    let bytes = std::fs::read(fixture(relative)?).ok()?;
    StarsFile::decode(&bytes).ok()
}

/// A game with public scores: the file carries a row for every player, and
/// the sheet shows all sixteen.
#[test]
fn a_public_scoreboard_names_every_player() {
    let Some(file) = read("games/no-random-events/2500/Game.m1") else {
        return;
    };
    let (state, _) = GameState::from_file(&file);
    let standings = scoresheet::standings(&state);
    assert_eq!(standings.len(), 16);
    assert!(
        standings.iter().all(|s| s.known),
        "public scores fill in every row"
    );

    // Player 3 is the runaway leader of that game.
    let leader = &standings[3];
    assert_eq!(leader.rank, 1);
    assert_eq!(leader.score.score, 1542);
    assert_eq!(leader.score.planets, 34);
    assert_eq!(leader.score.starbases, 23);
    assert_eq!(leader.score.tech_levels, 89);
    assert_eq!(scoresheet::best(&standings, Stat::Score), Some(1542));

    // And they are the only one to have met a victory condition — holding the
    // highest score for long enough.
    let lines = scoresheet::conditions(&state);
    assert!(scoresheet::met(
        leader,
        &line_for(&lines, victory::HIGH_SCORE_AT)
    ));
    for other in standings.iter().filter(|s| s.player != 3) {
        assert_eq!(other.victory, 0, "player {} met nothing", other.player);
    }
}

/// A game without public scores tells a player only about themselves; the
/// other fifteen columns are blank rather than zero.
#[test]
fn a_private_scoreboard_leaves_the_others_blank() {
    let Some(file) = read("games/all-computer-players/2450/Game.m1") else {
        return;
    };
    let (state, _) = GameState::from_file(&file);
    let standings = scoresheet::standings(&state);
    let known: Vec<usize> = standings
        .iter()
        .filter(|s| s.known)
        .map(|s| s.player)
        .collect();
    assert_eq!(known, vec![0]);
    assert_eq!(standings[0].rank, 8);
    assert_eq!(standings[0].score.score, 189);
}

/// The timeline comes out of the `.hN`, and the player file's own row extends
/// it to this year.
#[test]
fn the_timeline_runs_from_the_history_file_to_this_year() {
    let (Some(player), Some(history)) = (
        read("games/all-computer-players/2450/Game.m1"),
        read("games/all-computer-players/2450/Game.h1"),
    ) else {
        return;
    };
    let (mut state, _) = GameState::from_file(&player);
    // Only the current row so far.
    assert_eq!(state.timeline[0].len(), 1);

    state.read_scores(&history);
    state.read_scores(&player);
    let years = &state.timeline[0];
    assert_eq!(state.turn, 50, "year 2450");
    assert_eq!(
        years.len(),
        50,
        "turns 1..49 from the history, plus this one"
    );
    assert_eq!(years.first().map(|y| y.turn), Some(1));
    assert_eq!(years.last().map(|y| y.turn), Some(50));
    assert!(
        years.windows(2).all(|w| w[0].turn < w[1].turn),
        "in turn order, one row per turn"
    );
    // The last row is the player file's, not the history's.
    assert_eq!(years.last().map(|y| y.score.score), Some(189));

    // Reading either file again changes nothing.
    let before = state.timeline.clone();
    state.read_scores(&history);
    state.read_scores(&player);
    assert_eq!(state.timeline, before);

    // The score climbed over those fifty years.
    let line = scoresheet::line(&state, 0, Stat::Score);
    assert_eq!(line.len(), 50);
    assert_eq!(line.first().map(|(_, v)| *v), Some(13));
    assert_eq!(line.last().map(|(_, v)| *v), Some(189));
}

/// Every player's history is in the file when scores are public.
#[test]
fn a_public_history_carries_everybody() {
    let Some(history) = read("games/no-random-events/2500/Game.h1") else {
        return;
    };
    let Some(player) = read("games/no-random-events/2500/Game.m1") else {
        return;
    };
    let (mut state, _) = GameState::from_file(&player);
    state.read_scores(&history);
    state.read_scores(&player);
    assert_eq!(state.timeline.len(), 16);
    // The owner has the lot; the rest only from the year the host began
    // telling them apart.
    assert_eq!(state.timeline[0].len(), 100);
    for player in 1..16 {
        assert_eq!(state.timeline[player].len(), 81, "player {player}");
        assert_eq!(state.timeline[player][0].turn, 20);
    }
    // A hundred years is the most a file keeps, plus the current one.
    assert!(state
        .timeline
        .iter()
        .all(|years| years.len() <= stars_core::score::TIMELINE_YEARS));
}

/// The horizontal axis: the last hundred years, rounded so the gridlines fall
/// on whole numbers.
#[test]
fn the_year_axis_rounds_and_then_slides() {
    assert_eq!(scoresheet::span(0), (0, 0));
    assert_eq!(scoresheet::span(1), (0, 5));
    assert_eq!(scoresheet::span(5), (0, 5));
    assert_eq!(scoresheet::span(6), (0, 10));
    assert_eq!(scoresheet::span(50), (0, 50));
    // Past fifty the span rounds to tens instead of fives.
    assert_eq!(scoresheet::span(51), (0, 60));
    assert_eq!(scoresheet::span(100), (0, 100));
    // And past a hundred years the window slides rather than growing.
    assert_eq!(scoresheet::span(120), (20, 100));
    assert_eq!(scoresheet::span(300), (200, 100));

    assert_eq!(scoresheet::year_step(50), 5);
    assert_eq!(scoresheet::year_step(60), 10);
}

/// The vertical axis snaps to familiar numbers, and never squashes a small
/// graph flat.
#[test]
fn the_value_axis_climbs_a_ladder() {
    assert_eq!(scoresheet::value_scale(0), (5, 1));
    assert_eq!(scoresheet::value_scale(4), (5, 1));
    assert_eq!(scoresheet::value_scale(12), (12, 1));
    assert_eq!(scoresheet::value_scale(13), (13, 2));
    assert_eq!(scoresheet::value_scale(60), (60, 5));
    assert_eq!(scoresheet::value_scale(300), (300, 25));
    assert_eq!(scoresheet::value_scale(1200), (1200, 100));
    assert_eq!(scoresheet::value_scale(12000), (12000, 1000));
    // Beyond the ladder it aims at a dozen gridlines, in thousands of five
    // hundred: 100,000 / 12 is 8,333, which rounds down to 8,000.
    assert_eq!(scoresheet::value_scale(100_000), (100_000, 8_000));
    // Whatever the figure, a dozen or so lines is what comes out.
    for max in [7, 40, 199, 5_000, 40_000, 2_000_000] {
        let (top, step) = scoresheet::value_scale(max);
        assert!(step > 0, "{max}");
        assert!((1..=13).contains(&(top / step)), "{max}: {top}/{step}");
    }
}

/// The report lists every condition, whether or not the game is playing for
/// it, and turns the planet condition's percentage into a count.
#[test]
fn the_victory_report_lists_the_settings() {
    let mut state = GameState::new(1);
    state.galaxy_planets = 500;
    state.victory[victory::PLANET_CONTROL] = 0x80 | 4; // on, 20 + 4*5 = 40%
    state.victory[victory::SCORE] = 3; // off, but still listed
    let lines = scoresheet::conditions(&state);
    assert_eq!(lines.len(), 9);
    let planets = line_for(&lines, victory::PLANET_CONTROL);
    assert_eq!(planets.text, "Owns 200 planets.");
    assert!(planets.active);
    let score = line_for(&lines, victory::SCORE);
    assert!(!score.active, "off, but still listed");
    assert_eq!(score.text, "Exceeds a score of 4000.");
    // The tech condition takes two of the settings, so there is no line of its
    // own for the number of fields.
    assert!(lines.iter().all(|line| line.which != victory::TECH_FIELDS));
    // The last two are settings rather than something a player can meet.
    assert!(!line_for(&lines, victory::MUST_MEET).scoreable);
    assert!(!line_for(&lines, victory::LEAST_YEARS).scoreable);
    assert!(lines[..7].iter().all(|line| line.scoreable));
}

/// The button goes round the three faces rather than back and forth.
#[test]
fn the_faces_cycle() {
    let mut face = Face::Scores;
    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.push(face.title());
        face = face.next();
    }
    assert_eq!(
        seen,
        ["Player Scores", "Victory Conditions", "Progress Timeline"]
    );
    assert_eq!(face, Face::Scores, "and back to the start");
}

/// The timeline's menu is the scoreboard's rows without their colons.
#[test]
fn the_graph_menu_matches_the_scoreboard() {
    assert_eq!(Stat::ALL.len(), 8);
    assert_eq!(Stat::Planets.label(), "Planets:");
    assert_eq!(Stat::Planets.name(), "Planets");
    assert_eq!(Stat::Score.name(), "Score");
    for stat in Stat::ALL {
        assert!(stat.label().ends_with(':'), "{}", stat.label());
    }
}

/// Sixteen player colours, all different, and player 0's is the pale yellow
/// the game gives them.
#[test]
fn the_player_colours_are_the_game_s_own() {
    assert_eq!(scoresheet::player_colour(0), [0xf0, 0xf0, 0x3f]);
    assert_eq!(scoresheet::player_colour(1), [0xff, 0x00, 0x00]);
    assert_eq!(scoresheet::player_colour(3), [0x00, 0x00, 0xff]);
    let mut all = scoresheet::PLAYER_COLOURS.to_vec();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 16, "no two players share a colour");
    // Past sixteen it wraps, which only a mod that big could reach.
    assert_eq!(scoresheet::player_colour(16), scoresheet::player_colour(0));
}
