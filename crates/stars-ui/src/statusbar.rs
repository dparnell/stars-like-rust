//! The scanner's status bar.
//!
//! `DrawScannerSBar` (`1058:62d8`) draws **two rows** along the bottom of the
//! scanner's client area, each cell sunk into the button face by
//! `DrawLockLight` (`1058:6b00`). The top row carries the object's **id**, its
//! **x**, its **y** and its **name**; the bottom row carries a **distance**.
//!
//! The bar is `dySBar` tall, and `dySBar` is `(dyArial8 + 12) * 2`
//! (`FrameWndProc`, `1020:0714`) — two rows, each a line of Arial 8 plus
//! twelve. The text is Arial 8 **bold** (`rghfontArial8[1]`).
//!
//! `MANUAL.PDF` p. 5-16 describes the same thing from the outside: "ID#,
//! coordinates and name of planet selected in the Scanner" for a planet,
//! "Coordinates and name" for a fleet or an object — and the binary agrees,
//! because only `grobjPlanet` and `grobjOther` ever put anything in the id
//! cell.
//!
//! See `docs/ui/scanner.md`.

/// The text colour, `crButtonText` — `COLOR_BTNTEXT`, black.
pub const TEXT: [u8; 3] = [0x00, 0x00, 0x00];

/// What each row adds to the height of one line of text: `dySBar` is
/// `(dyArial8 + 12) * 2`, so a row is `dyArial8 + 12`.
pub const ROW_EXTRA: f32 = 12.0;

/// How far a cell is inset from its row on every side — the original builds
/// the cell rectangle as `top + 4`, `bottom - 4`, `left + 4`, `right - 4`.
pub const CELL_INSET: f32 = 4.0;

/// The gap the original leaves between one cell and the next: it advances
/// `left` by the cell's width **plus four**.
pub const CELL_GAP: f32 = 4.0;

/// What a cell adds to the width of the text it is sized for.
pub const CELL_PAD: f32 = 6.0;

/// Where the text sits inside its cell.
pub const TEXT_LEFT: f32 = 3.0;
/// Where the text sits inside its cell.
pub const TEXT_TOP: f32 = 2.0;

/// The id cell is as wide as this sample plus [`CELL_PAD`] — the original
/// measures the literal at `DS:0x5a2` rather than the text it is about to
/// draw, so the cell does not jump about.
pub const ID_SAMPLE: &str = "ID #000";
/// The x and y cells are both as wide as this sample plus [`CELL_PAD`]
/// (`DS:0x5b8`); the y cell reuses the x cell's measurement.
pub const COORD_SAMPLE: &str = "X: 8888";

/// The three left cells are drawn only when the scanner's client area is
/// **wider than this** (`if (0x167 < rc.right)`). Below it the name cell has
/// the whole row, which is what `MANUAL.PDF` p. 5-16 means by "if the scanner
/// is too narrow to display all the status bar information".
pub const CELLS_WIDTH: f32 = 359.0;

/// The distance is worded with [`UNIT_LONG`] when the client area is **wider
/// than this** and with [`UNIT_SHORT`] otherwise (`idsLy + (0x15d < rc.right)`).
pub const WIDE_UNIT_WIDTH: f32 = 349.0;

/// `idsLy`, the narrow unit.
pub const UNIT_SHORT: &str = "ly";
/// `idsLightYears`, the wide one.
pub const UNIT_LONG: &str = "light years";
/// `idsFrom`, spaces and all.
pub const FROM: &str = " from ";

/// What the bar has to say.
///
/// Four cells and a second row, exactly as the original draws them. Every
/// field is already in the words the game would print, because the formats are
/// the game's: `"ID #%d"` and `"WP #%d"` at `DS:0x5aa` and `DS:0x5b1`,
/// `"X: %d"` and `"Y: %d"` at `DS:0x5c0` and `DS:0x5c6`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusBar {
    /// The leftmost cell: `ID #12` for a planet, `WP #3` for a waypoint, and
    /// empty for a fleet, a space object or nothing at all.
    pub id: String,
    /// The x cell. Empty when the coordinate is not **positive**, which is the
    /// original's `if (0 < x)` guard and so leaves the cell blank at the
    /// galaxy's own edge.
    pub x: String,
    /// The y cell, on the same rule.
    pub y: String,
    /// The name cell: a planet's, a fleet's, an object's, `Deep Space
    /// Waypoint`, or the tape's `Deep Space`.
    pub name: String,
    /// The bottom row, when there is a distance to report.
    pub distance: Option<Distance>,
}

/// The bottom row.
///
/// The original builds it by printing `"%ld.%ld  l.y."` and then overwriting
/// everything after the first space with the unit the window is wide enough
/// for — so the figure and the unit are two separate decisions, and the double
/// space in the format never reaches the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distance {
    /// The `%ld.%ld` figure.
    pub figure: String,
    /// What to name after `from`, when the bar names where it measured from.
    ///
    /// The measuring tape hands `DrawScannerSBar` the anchor's own `SCAN`,
    /// and that is exactly what suppresses this clause; a waypoint drag hands
    /// it nothing, so the fleet gets named.
    pub from: Option<String>,
}

impl Distance {
    /// The row as it is drawn. `wide` is whether the scanner is wider than
    /// [`WIDE_UNIT_WIDTH`], which is what picks the unit.
    #[must_use]
    pub fn text(&self, wide: bool) -> String {
        let unit = if wide { UNIT_LONG } else { UNIT_SHORT };
        match &self.from {
            Some(name) => format!("{} {unit}{FROM}{name}", self.figure),
            None => format!("{} {unit}", self.figure),
        }
    }
}

impl StatusBar {
    /// Fill in the two coordinate cells for a point.
    ///
    /// The original prints a coordinate only when it is **positive** — the
    /// guards are `if (0 < x)` and `if (0 < y)` — so a point on the galaxy's
    /// own zero line leaves its cell blank.
    pub fn place(&mut self, at: stars_core::movement::Point) {
        if at.x > 0 {
            self.x = format!("X: {}", at.x);
        }
        if at.y > 0 {
            self.y = format!("Y: {}", at.y);
        }
    }
}
