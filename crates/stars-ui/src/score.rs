//! The Score sheet's geometry.
//!
//! `InitScoreDlg` (`1108:13b6`) sizes the window itself rather than taking a
//! size from the dialog template, because the sheet has a **column per
//! player** and does not know how many there are until it opens. It measures
//! two things in Arial 8 bold — one digit, and the widest label the face uses
//! — and builds everything else out of those.
//!
//! The columns are narrow: five digits on the scoreboard, and only a line and
//! a half on the victory report. Neither is anywhere near wide enough for a
//! player's name, which is why the names are **rotated** — the original draws
//! them with a 90-degree Arial, `rghfontArial8[4]`.
//!
//! See `docs/ui/score-sheet.md`.

/// The single character every column is measured against: `"9"` at `DS:0x15e8`,
/// the widest digit.
pub const DIGIT_SAMPLE: &str = "9";

/// The widest row label on the scoreboard (`idsUnarmedShips2`), which is what
/// sets its label column.
pub const WIDEST_LABEL: &str = "Unarmed Ships:";

/// The widest sentence on the victory report
/// (`idsExceedsSecondPlaceScore`), which sets that face's label column.
pub const WIDEST_SENTENCE: &str = "Exceeds second place score by ";

/// However few players a game has, the window still leaves room for four
/// columns.
pub const MIN_COLUMNS: usize = 4;

/// How many digits wide one player's column is on the scoreboard.
pub const SCORE_COLUMN_DIGITS: f32 = 5.0;

/// What the label column adds to the widest label it holds.
pub const LABEL_PAD: f32 = 8.0;

/// What the whole window adds past its last column.
pub const RIGHT_PAD: f32 = 8.0;

/// The timeline's window is a flat 600 by 400 rather than a computed size.
pub const TIMELINE_SIZE: (f32, f32) = (600.0, 400.0);

/// How tall the two computed faces are: `dyArial8 * 33 / 2 + 88`.
#[must_use]
pub fn height(line: f32) -> f32 {
    line * 33.0 / 2.0 + 88.0
}

/// Where the two computed faces put their columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// How wide the column of labels down the left is.
    pub label: f32,
    /// How wide one player's column is.
    pub column: f32,
    /// How wide the whole window wants to be.
    pub width: f32,
    /// And how tall.
    pub height: f32,
}

impl Layout {
    /// The scoreboard: the label column is the widest label plus eight, and a
    /// player's column is five digits.
    #[must_use]
    pub fn scores(digit: f32, line: f32, label: f32, players: usize) -> Self {
        let column = digit * SCORE_COLUMN_DIGITS;
        Self::build(label + LABEL_PAD, column, line, players)
    }

    /// The victory report: the label column is **one and a half times** the
    /// widest sentence plus six digits, and a player's column is only a line
    /// and a half — which is what makes the rotated names necessary.
    #[must_use]
    pub fn victory(digit: f32, line: f32, sentence: f32, players: usize) -> Self {
        let label = sentence * 3.0 / 2.0 + digit * 6.0;
        Self::build(label, line * 3.0 / 2.0, line, players)
    }

    fn build(label: f32, column: f32, line: f32, players: usize) -> Self {
        #[allow(clippy::cast_precision_loss)]
        let columns = players.max(MIN_COLUMNS) as f32;
        Self {
            label,
            column,
            width: columns * column + label + RIGHT_PAD,
            height: height(line),
        }
    }

    /// Where one player's column starts, from the window's left edge.
    #[must_use]
    pub fn column_at(&self, player: usize) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let index = player as f32;
        self.label + index * self.column
    }
}
