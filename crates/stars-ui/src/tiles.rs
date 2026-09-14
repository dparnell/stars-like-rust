//! The planet pane's tiles.
//!
//! The pane is not one panel but **six tiles in two columns**, laid out from
//! the table `rgtilePlanet` (`1120:07fc`) rather than from code
//! (`PlanetWndProc`, `1048:0000`). Each record is sixteen bytes; the fields
//! that matter here are a **line count**, a **pixel remainder**, a `grbit`
//! naming the tile, the routine that draws it, and a packed word whose low
//! three bits are the column and whose bit 7 says the tile is **open**.
//!
//! `InitTiles` (`1000:0eb8`) turns the first two into a height:
//!
//! ```text
//! dyFull = remainder + lines * dyArial8
//! ```
//!
//! and then walks each column writing every tile's top, which is why the tops
//! in the shipped image are stale — they are output, not input. `ReflowColumn`
//! (`1048:…`) does the same walk again whenever a tile opens or closes.
//!
//! See `docs/ui/planet-pane.md`.

/// How far apart the two columns are (`iCol * 0xc6 + 4`).
pub const COLUMN_PITCH: f32 = 0xc6 as f32;
/// Where the first column starts.
pub const COLUMN_LEFT: f32 = 4.0;
/// How wide a tile is (`left + 0xbe`).
pub const TILE_WIDTH: f32 = 0xbe as f32;
/// The gap above the first tile and between one tile and the next.
pub const TILE_GAP: f32 = 4.0;
/// How wide the pane is: two columns of tiles at their pitch, and the
/// margin before the first — `4 + 0xc6 + 0xbe`, 392 pixels. The Window
/// Layout changes the tiles' heights, never this.
pub const PANE_WIDTH: f32 = COLUMN_LEFT + COLUMN_PITCH + TILE_WIDTH;
/// What the title bar adds to a line of text.
pub const TITLE_EXTRA: f32 = 2.0;
/// What a **closed** tile stands at, title bar and frame and nothing else.
pub const CLOSED_EXTRA: f32 = 3.0;
/// What the body's top is past the tile's own (`prc->top + dyArial8 + 4`).
pub const BODY_EXTRA: f32 = 4.0;
/// How wide the button at the right end of the title bar is, and where the
/// shadow line beside it goes (`right - 0x12`).
pub const BUTTON_WIDTH: f32 = 0x11 as f32;

/// One tile of the planet pane, in the table's own order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    /// What the tile is called; the last two take their titles from the game
    /// instead, so these are empty.
    pub title: &'static str,
    /// Which column it is in: the low three bits of the packed word.
    pub column: u8,
    /// Which tile this is, whatever order the table happens to be in: bits
    /// 3 to 6 of the packed word at `+10`.
    ///
    /// That word is three fields — `column:3, id:4, fPopped:1` — and the id
    /// is what `stars.ini` names a tile by, as a letter counting from `a`.
    /// The planet pane's six are 0, 1, 4, 5, 6 and 7, so its shipped
    /// setting reads `ABE*FGH`: every tile open, three in each column.
    pub id: u8,
    /// How many lines of `dyArial8` its height is built from.
    pub lines: i16,
    /// What is added to those lines.
    pub extra: i16,
    /// The `grbit` that names it, which is what `EnsureTileSize`
    /// (`1048:58df`) matches on when the window layout changes.
    pub grbit: u16,
    /// What that match adds or takes off, as a multiple of `dyArial8` and a
    /// flat number of pixels.
    ///
    /// `EnsureTileSize` walks the two tables in **two loops with different
    /// rules**, so this cannot be derived from `grbit`: `0x40` is the planet
    /// pane's production queue, moving by `(dyArial8 + 2) * 2`, and the fleet
    /// pane's location tile, moving by a flat six; `0x01` is Minerals On Hand,
    /// which does not move at all, and Fuel & Cargo, which moves by
    /// `dyArial8 * 4 + 2`. Only `0x04` and `0x80` mean the same thing in both,
    /// and `0x04` is the one tile the two tables genuinely share.
    pub resize: (i16, i16),
}

/// `rgtilePlanet`, read out of the binary at `1120:07fc`.
///
/// The left column is the planet, its minerals and its status; the right is
/// what is in orbit, what it is building, and what it is building from.
pub const PLANET_TILES: [Tile; 6] = [
    Tile {
        title: "",
        column: 0,
        id: 0,
        lines: 1,
        extra: 85,
        grbit: 0x80,
        resize: (0, 10),
    },
    Tile {
        title: "Minerals On Hand",
        column: 0,
        id: 1,
        lines: 6,
        extra: 5,
        grbit: 0x01,
        resize: (0, 0),
    },
    Tile {
        title: "Status",
        column: 0,
        id: 4,
        lines: 8,
        extra: 6,
        grbit: 0x08,
        resize: (0, 0),
    },
    Tile {
        title: "",
        column: 1,
        id: 5,
        lines: 6,
        extra: 22,
        grbit: 0x04,
        resize: (2, 8),
    },
    Tile {
        title: "Production",
        column: 1,
        id: 6,
        lines: 10,
        extra: 20,
        grbit: 0x40,
        resize: (2, 4),
    },
    Tile {
        title: "",
        column: 1,
        id: 7,
        lines: 8,
        extra: 15,
        grbit: 0x100,
        resize: (0, 0),
    },
];

impl Tile {
    /// How tall the tile stands when it is open.
    ///
    /// `InitTiles`'s `remainder + lines * dyArial8` is the **full** size:
    /// that is what the table ships holding, and what the window starts at.
    /// `EnsureTileSize` (`1048:587e`) does not recompute it but **moves it
    /// by a delta** — taking [`Tile::resize`] off on the way into the small
    /// layout and adding it back on the way out — which is why it is
    /// guarded by a bit that says which state the tables are in and does
    /// nothing when asked for the one they are already in.
    ///
    /// So: the large size is the table's own, and only the small one is
    /// arrived at by arithmetic.
    #[must_use]
    pub fn height(&self, line: f32, small: bool) -> f32 {
        let mut height = f32::from(self.extra) + f32::from(self.lines) * line;
        if small {
            let (lines, plus) = self.resize;
            height -= f32::from(lines) * line + f32::from(plus);
        }
        height.max(line + CLOSED_EXTRA)
    }

    /// How tall it stands when it is closed: the title bar alone.
    #[must_use]
    pub fn closed_height(line: f32) -> f32 {
        line + CLOSED_EXTRA
    }
}

/// `rgtileShip`, read out of the binary at `1120:090e`.
///
/// The **same window** as the planet pane with a different table: four tiles
/// down the left — the fleet, where it is, its waypoints and the task waiting
/// at the next one — and three down the right. The last is
/// `DrawPlanetShipList` again, the same routine in the same corner, so
/// whichever is selected the pane's bottom right answers "what else is here?".
pub const SHIP_TILES: [Tile; 7] = [
    Tile {
        title: "",
        column: 0,
        id: 0,
        lines: 1,
        extra: 85,
        grbit: 0x80,
        resize: (0, 10),
    },
    Tile {
        title: "",
        column: 0,
        id: 5,
        lines: 3,
        extra: 5,
        grbit: 0x40,
        resize: (0, 6),
    },
    Tile {
        title: "Fleet Waypoints",
        column: 0,
        id: 3,
        lines: 11,
        extra: 19,
        grbit: 0x20,
        resize: (1, 9),
    },
    Tile {
        title: "Waypoint Task",
        column: 0,
        id: 4,
        lines: 6,
        extra: 12,
        grbit: 0x100,
        resize: (0, 2),
    },
    Tile {
        title: "Fuel & Cargo",
        column: 1,
        id: 1,
        lines: 7,
        extra: 14,
        grbit: 0x01,
        resize: (4, 2),
    },
    Tile {
        title: "Fleet Composition",
        column: 1,
        id: 9,
        lines: 12,
        extra: 16,
        grbit: 0x200,
        resize: (3, 8),
    },
    Tile {
        title: "",
        column: 1,
        id: 8,
        lines: 6,
        extra: 22,
        grbit: 0x04,
        resize: (2, 8),
    },
];

/// Where every tile of one column sits, top-down.
///
/// `ReflowColumn` starts four pixels down and adds each tile's height and four
/// more, so a closed tile takes the column's later tiles up with it.
#[must_use]
pub fn column_tops(
    tiles: &[Tile],
    line: f32,
    small: bool,
    open: &[bool],
    column: u8,
) -> Vec<(usize, f32, f32)> {
    let mut out = Vec::new();
    let mut y = TILE_GAP;
    for (index, tile) in tiles.iter().enumerate() {
        if tile.column != column {
            continue;
        }
        let height = if open.get(index).copied().unwrap_or(true) {
            tile.height(line, small)
        } else {
            Tile::closed_height(line)
        };
        out.push((index, y, height));
        y += height + TILE_GAP;
    }
    out
}
