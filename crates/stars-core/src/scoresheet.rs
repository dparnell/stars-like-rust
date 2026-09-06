//! The Score sheet — Reports (Score), or **F10**.
//!
//! `ScoreXDlg` (`1108:0f66`) is one dialog with three faces, cycled by a single
//! button: the scoreboard (`DrawScoreReport`, `1108:1e0c`), the victory
//! conditions (`DrawVCReport`, `1108:168e`) and the timeline
//! (`DrawHistoryReport`, `1108:2494`). This module is the model behind all
//! three; the figures themselves are read from the file into
//! [`crate::score::Standing`] and [`crate::score::Year`].
//!
//! `MANUAL.PDF` p. 2-3: *"The Score sheet shows your score and current
//! ranking, and a history of scores since the game began. If Public Player
//! Scores is selected in the game setup, all player's scores and rankings
//! appear in the Score sheet."*
//!
//! See `docs/ui/score-sheet.md`.

use crate::score::{Standing, Year};
use crate::GameState;
use stars_formats::victory;

/// The three faces of the sheet, in the order the button cycles them.
///
/// `ScoreXDlg` keeps the current one in two bits of `gd` and advances it with
/// `(face + 1) % 3`, so the button goes round rather than back and forth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Face {
    /// The scoreboard: every figure, a column per player.
    #[default]
    Scores,
    /// What each player must do to win, and how far along they are.
    Victory,
    /// One figure, drawn year by year.
    Timeline,
}

impl Face {
    /// The three, in the order the button cycles them.
    pub const ALL: [Face; 3] = [Face::Scores, Face::Victory, Face::Timeline];

    /// The next face the button moves to.
    #[must_use]
    pub fn next(self) -> Face {
        match self {
            Face::Scores => Face::Victory,
            Face::Victory => Face::Timeline,
            Face::Timeline => Face::Scores,
        }
    }

    /// The window's title, which changes with the face (string ids 1210–1212).
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Face::Scores => "Player Scores",
            Face::Victory => "Victory Conditions",
            Face::Timeline => "Progress Timeline",
        }
    }
}

/// The eight figures a score row carries.
///
/// The order is `LFetchScoreXVal`'s (`1108:2f94`), which is also the order the
/// scoreboard lists them in and the order of the timeline's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    /// Planets owned.
    Planets,
    /// Starbases with a dock — an Orbital Fort does not count.
    Starbases,
    /// Ships with no offensive power.
    UnarmedShips,
    /// Armed ships below [`crate::score::CAPITAL_POWER`].
    EscortShips,
    /// Armed ships at or above it.
    CapitalShips,
    /// The six tech levels added together.
    TechLevels,
    /// Resources produced in a year.
    Resources,
    /// The score itself.
    Score,
}

impl Stat {
    /// All eight, in the sheet's order.
    pub const ALL: [Stat; 8] = [
        Stat::Planets,
        Stat::Starbases,
        Stat::UnarmedShips,
        Stat::EscortShips,
        Stat::CapitalShips,
        Stat::TechLevels,
        Stat::Resources,
        Stat::Score,
    ];

    /// The scoreboard's row label, colon and all (string ids 435–442).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Stat::Planets => "Planets:",
            Stat::Starbases => "Starbases:",
            Stat::UnarmedShips => "Unarmed Ships:",
            Stat::EscortShips => "Escort Ships:",
            Stat::CapitalShips => "Capital Ships:",
            Stat::TechLevels => "Tech Levels:",
            Stat::Resources => "Resources:",
            Stat::Score => "Score:",
        }
    }

    /// The same without the colon, which is how the timeline names it: the
    /// original builds its menu from these very strings and drops the last
    /// character of each.
    #[must_use]
    pub fn name(self) -> &'static str {
        self.label().trim_end_matches(':')
    }

    /// The figure itself.
    #[must_use]
    pub fn of(self, standing: &Standing) -> i32 {
        let score = &standing.score;
        match self {
            Stat::Planets => score.planets,
            Stat::Starbases => score.starbases,
            Stat::UnarmedShips => score.unarmed_ships,
            Stat::EscortShips => score.escort_ships,
            Stat::CapitalShips => score.capital_ships,
            Stat::TechLevels => score.tech_levels,
            Stat::Resources => score.resources,
            Stat::Score => score.score,
        }
    }

    /// The figure out of one year of the timeline.
    #[must_use]
    pub fn of_year(self, year: &Year) -> i32 {
        self.of(&Standing {
            score: year.score,
            ..Standing::default()
        })
    }
}

/// A player's colour, as the game assigned it (`rgcrPlrHistory`, `1120:002e`).
///
/// `MANUAL.PDF` p. 5-16: *"Stars! assigns each player a color when a game is
/// created. Use the Reports (Score) to open the Score sheet, then switch to
/// the history graph to see which player has which color."* — so this table is
/// where a player's colour is defined, and the scanner borrows it.
///
/// Stored in the file as `COLORREF`s, which are `0x00bbggrr`; these are the
/// same sixteen values written the way they read.
pub const PLAYER_COLOURS: [[u8; 3]; 16] = [
    [0xf0, 0xf0, 0x3f],
    [0xff, 0x00, 0x00],
    [0x00, 0xff, 0x00],
    [0x00, 0x00, 0xff],
    [0xff, 0xff, 0x00],
    [0xff, 0x00, 0xff],
    [0x00, 0xff, 0xff],
    [0x7f, 0x00, 0x00],
    [0x00, 0x7f, 0x00],
    [0x00, 0x00, 0x7f],
    [0x7f, 0x7f, 0x7f],
    [0x39, 0xc8, 0x67],
    [0xff, 0x7f, 0x23],
    [0x23, 0x7f, 0xff],
    [0x7f, 0x7f, 0x00],
    [0x60, 0x60, 0x60],
];

/// One player's colour, wrapping round for a game with more than sixteen.
#[must_use]
pub fn player_colour(player: usize) -> [u8; 3] {
    PLAYER_COLOURS[player % PLAYER_COLOURS.len()]
}

/// The scoreboard, one entry per player the game has.
///
/// A player the file said nothing about still gets a row: the original draws
/// every player's name and column separator and simply leaves the figures out,
/// which is how an unknown player and a player with nothing look different.
#[must_use]
pub fn standings(state: &GameState) -> Vec<Standing> {
    (0..state.players.len())
        .map(|player| {
            state.standings.get(player).copied().unwrap_or(Standing {
                player,
                ..Standing::default()
            })
        })
        .collect()
}

/// Whether a player's figures should be drawn at all.
///
/// `DrawScoreReport` skips the numbers of a player it knows to be dead, and
/// greys their name — so a dead player is a name in grey over an empty column,
/// not a column of zeroes.
#[must_use]
pub fn shows_figures(state: &GameState, standing: &Standing) -> bool {
    standing.known && !state.players.get(standing.player).is_some_and(|p| p.dead)
}

/// The best figure in a row, which the sheet draws in blue.
///
/// The original picks the maximum over **every** row it has a figure for,
/// including players it will then decline to draw, and a tie colours both.
#[must_use]
pub fn best(standings: &[Standing], stat: Stat) -> Option<i32> {
    standings
        .iter()
        .filter(|standing| standing.known)
        .map(|standing| stat.of(standing))
        .max()
}

/// One line of the victory report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    /// Which condition this is — an index into `GAME.rgvc`, i.e. one of the
    /// constants in [`stars_formats::victory`]. The last two lines are the
    /// how-many and how-soon settings rather than conditions a player meets.
    pub which: usize,
    /// Whether the game is playing for it. A condition switched off is still
    /// listed, in grey, with its setting — that is what lets a player see what
    /// the game is *not* asking of them.
    pub active: bool,
    /// Whether a player can meet it, which the last two lines cannot.
    pub scoreable: bool,
    /// The sentence, with the settings in it.
    pub text: String,
}

/// The victory report's nine lines.
///
/// `DrawVCReport` writes one sentence per line out of a lead string, a value,
/// and a trailing string — the tech line has two of each. The wording here is
/// this project's own; the **structure and the numbers** are the original's,
/// including the two quirks worth knowing about:
///
/// * the planet condition is set as a percentage and shown as a **count** —
///   the original multiplies by the galaxy's planet total and divides by a
///   hundred before printing it;
/// * a condition the game is not playing for is still listed, with its
///   setting, drawn grey.
#[must_use]
pub fn conditions(state: &GameState) -> Vec<Condition> {
    let value = |which: usize| crate::victory::value(&state.victory, which);
    let active = |which: usize| crate::victory::active(&state.victory, which);
    let planets = i64::from(value(victory::PLANET_CONTROL)) * i64::from(state.galaxy_planets) / 100;

    let mut out = Vec::new();
    let mut line = |which: usize, scoreable: bool, text: String| {
        out.push(Condition {
            which,
            active: active(which),
            scoreable,
            text,
        });
    };
    line(
        victory::PLANET_CONTROL,
        true,
        format!("Owns {planets} planets."),
    );
    line(
        victory::TECH_LEVEL,
        true,
        format!(
            "Attains tech {} in {} fields.",
            value(victory::TECH_LEVEL),
            value(victory::TECH_FIELDS)
        ),
    );
    line(
        victory::SCORE,
        true,
        format!("Exceeds a score of {}.", value(victory::SCORE)),
    );
    line(
        victory::SCORE_EXCESS,
        true,
        format!(
            "Exceeds second place score by {}%.",
            value(victory::SCORE_EXCESS)
        ),
    );
    line(
        victory::PRODUCTION,
        true,
        format!(
            "Has a production capacity of {} thousand.",
            value(victory::PRODUCTION)
        ),
    );
    line(
        victory::CAPITAL_SHIPS,
        true,
        format!("Owns {} capital ships.", value(victory::CAPITAL_SHIPS)),
    );
    line(
        victory::HIGH_SCORE_AT,
        true,
        format!(
            "Has the highest score after {} years.",
            value(victory::HIGH_SCORE_AT)
        ),
    );
    line(
        victory::MUST_MEET,
        false,
        format!(
            "Winner must meet {} of the above selected criteria.",
            value(victory::MUST_MEET)
        ),
    );
    line(
        victory::LEAST_YEARS,
        false,
        format!(
            "At least {} years must pass before a winner is declared.",
            value(victory::LEAST_YEARS)
        ),
    );
    out
}

/// Whether a player has met one of the report's conditions.
#[must_use]
pub fn met(standing: &Standing, condition: &Condition) -> bool {
    condition.scoreable && standing.victory & crate::victory::bit(condition.which) != 0
}

/// The stretch of years the timeline draws, as `(first turn, span)`.
///
/// `DrawHistoryReport` shows the last hundred years at most, and rounds the
/// span so the gridlines land on whole numbers: up to the next multiple of
/// five, then — once past fifty — to the next multiple of ten.
#[must_use]
pub fn span(turn: i16) -> (u16, u16) {
    let turn = u16::try_from(turn).unwrap_or(0);
    let first = turn.saturating_sub(100);
    let mut span = turn.div_ceil(5) * 5;
    if span > 100 {
        span = 100;
    } else if span > 50 {
        span = span.div_ceil(10) * 10;
    }
    (first, span)
}

/// How far apart the timeline's year gridlines are: five up to a fifty-year
/// span, ten beyond.
#[must_use]
pub fn year_step(span: u16) -> u16 {
    if span <= 50 {
        5
    } else {
        10
    }
}

/// The timeline's vertical scale, as `(top of the axis, gridline interval)`.
///
/// `DrawHistoryReport` walks a ladder of thresholds rather than computing a
/// scale, which is why the axis snaps to familiar numbers. A graph with
/// nothing above four is still drawn to five.
#[must_use]
pub fn value_scale(max: i32) -> (i32, i32) {
    let max = max.max(5);
    let step = match max {
        ..=12 => 1,
        13..=25 => 2,
        26..=60 => 5,
        61..=120 => 10,
        121..=300 => 25,
        301..=600 => 50,
        601..=1200 => 100,
        1201..=6000 => 500,
        6001..=12000 => 1000,
        _ => max / 12 / 500 * 500,
    };
    (max, step)
}

/// One player's line on the timeline: the years inside the window, in order.
#[must_use]
pub fn line(state: &GameState, player: usize, stat: Stat) -> Vec<(u16, i32)> {
    let (first, span) = span(state.turn);
    state
        .timeline
        .get(player)
        .map(|years| {
            years
                .iter()
                .filter(|year| year.turn >= first && year.turn <= first + span)
                .map(|year| (year.turn, stat.of_year(year)))
                .collect()
        })
        .unwrap_or_default()
}

/// The largest value any player's line reaches inside the window, which is
/// what the vertical axis is scaled to.
///
/// The original takes the maximum over **every** row it holds, whether or not
/// that row falls inside the window it is about to draw.
#[must_use]
pub fn peak(state: &GameState, stat: Stat) -> Option<i32> {
    state
        .timeline
        .iter()
        .flatten()
        .map(|year| stat.of_year(year))
        .max()
}
