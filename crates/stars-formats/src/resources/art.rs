//! Which picture is which, and how the sheets are tiled.
//!
//! The executable holds thirty-eight bitmaps and the code that loads them
//! names none of them in the file — a picture is a number. This is that
//! numbering, transcribed from `InitStuff`, where every one of them is loaded
//! in a single run, together with the arithmetic each is indexed by, taken
//! from the `DibBlt` that draws it.
//!
//! Most of the pictures are **sheets**: a grid of fixed-size cells indexed by
//! one number. The two things worth knowing before reading the tables below
//! are that a bitmap is stored bottom-up, so the original's row arithmetic
//! counts from the bottom, and that the sheets do not agree on which way round
//! the index runs — the planets are laid out one way and the ships another.
//! Each function here gives the cell in the **decoded** picture, where the top
//! row is row zero, so the flipping is done once, here.
//!
//! See `docs/formats/resources.md`.

use super::Name;

/// A cell of a sheet: which bitmap it is in, and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// The bitmap resource's numeric id.
    pub resource: u16,
    /// Left edge, in pixels from the left of the decoded picture.
    pub x: u32,
    /// Top edge, in pixels from the **top** of the decoded picture.
    pub y: u32,
    /// Cell width.
    pub width: u32,
    /// Cell height.
    pub height: u32,
}

impl Cell {
    /// The resource's name, for [`super::read_bitmap`].
    #[must_use]
    pub fn name(self) -> Name {
        Name::Id(self.resource)
    }
}

/// The seven sheets of component pictures (`rghdibInventory`, `InitStuff`
/// loads ids 500 to 506).
pub const COMPONENT_SHEETS: [u16; 7] = [500, 501, 502, 503, 504, 505, 506];

/// The picture for a component, from its `ibmp`.
///
/// `DibBlt(…, 0x40, 0x40, rghdibInventory[ibmp / 0x20], (ibmp & 7) << 6,
/// (3 - ((ibmp >> 3) & 3)) * 0x40, …)` — 64-pixel cells, eight to a row and
/// four rows to a sheet, so thirty-two a sheet. The row is counted from the
/// bottom there and from the top here, which for a four-row sheet of 64-pixel
/// cells comes to the same number: `(ibmp >> 3) & 3`.
///
/// The last sheet is only 256 pixels wide — four columns, not eight — so an
/// `ibmp` in its right-hand half names a cell that is not there. That is
/// caught by [`super::Image::crop`] rather than here, because nothing in the
/// binary says which of those indices are ever used.
#[must_use]
pub fn component(ibmp: u16) -> Option<Cell> {
    let sheet = usize::from(ibmp / 32);
    Some(Cell {
        resource: *COMPONENT_SHEETS.get(sheet)?,
        x: u32::from(ibmp & 7) * 64,
        y: u32::from((ibmp >> 3) & 3) * 64,
        width: 64,
        height: 64,
    })
}

/// The two sheets whose colour table the game rewrites with the button
/// face when it reads the system colours (`FGetSystemColors`, `1018:08d2`):
/// entry 253 of the toolbar (id 178), which is the magenta its background
/// is keyed with, and entry 249 of the designer's plaque (id 449). Each is
/// `(bitmap id, colour-table entry)`; see [`super::read_bitmap_recoloured`].
pub const SYSTEM_COLOURED: [(u16, usize); 2] = [(178, 253), (449, 249)];

/// The sheet of pictures an empty design slot wears (`hbmpBackBld`,
/// `FCreateStuff` loads id 119 at `1000:07b2`): 576 by 192, twenty-one
/// 64-pixel cells drawn eight to a row.
pub const EMPTY_SLOT_SHEET: u16 = 119;

/// The category masks `IEmptyBmpFromGrhst` (`10c8:6716`) knows, in the
/// order it walks them at `1120:0c52`: the position of the first that equals
/// a slot's mask is the cell to draw, and a mask it does not know gets cell
/// 0, the "Combo" picture.
///
/// Entries 16 to 19 are **byte-swapped in the binary** — `0x000a`, `0x0019`,
/// `0x0008` and `0x0010` where the pictures under them read *Orbital or
/// Elect* (`0x0a00`), *Mine Elect Mech* (`0x1900`), *Elect* (`0x0800`) and
/// *Mech* (`0x1000`) — so those four pictures are never drawn and an empty
/// Elect, Mech, Orbital-or-Elect or Mine-Elect-Mech slot wears the Combo
/// picture in the original. The table is kept as the binary has it.
pub const EMPTY_SLOT_MASKS: [u16; 21] = [
    0x19ff, 0x0001, 0x0002, 0x0004, 0x0030, 0x193e, 0x1800, 0x1802, 0x0040, 0x000c, 0x0008, 0x0080,
    0x180a, 0x0034, 0x0100, 0x0200, 0x000a, 0x0019, 0x0008, 0x0010, 0x1804,
];

/// The picture an empty slot with this category mask wears.
///
/// `DrawSlotDlg` (`10c8:2650`) blits it from the sheet at
/// `((i & 7) << 6, ((i >> 3) & 3) << 6)`, 64 pixels square.
#[must_use]
pub fn empty_slot(mask: u16) -> Cell {
    let i = EMPTY_SLOT_MASKS
        .iter()
        .position(|&m| m == mask)
        .unwrap_or(0);
    let i = u32::try_from(i).unwrap_or(0);
    Cell {
        resource: EMPTY_SLOT_SHEET,
        x: (i & 7) * 64,
        y: ((i >> 3) & 3) * 64,
        width: 64,
        height: 64,
    }
}

/// How big a race emblem is wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmblemSize {
    /// 32 pixels — `hdibRaces`, id 133.
    Large,
    /// 16 pixels — `hdibRacesT`, id 80.
    Medium,
    /// 8 pixels — `hdibRacesX`, id 79.
    Small,
}

impl EmblemSize {
    /// The bitmap it comes out of, and the cell size.
    #[must_use]
    pub fn sheet(self) -> (u16, u32) {
        match self {
            EmblemSize::Large => (133, 32),
            EmblemSize::Medium => (80, 16),
            EmblemSize::Small => (79, 8),
        }
    }
}

/// How many emblems a race can be given.
pub const EMBLEMS: u8 = 32;

/// A race's emblem — `PLAYER.logo`, `0..32`.
///
/// `DibBlt(…, 0x20, 0x20, hdibRaces, (logo & 7) << 5,
/// (3 - (logo >> 3)) * 0x20, …)`: eight to a row, four rows, counted from the
/// bottom, which again lands on `logo >> 3` from the top. All three sheets are
/// the same thirty-two emblems at three sizes, and every one of them is eight
/// cells wide and four tall.
#[must_use]
pub fn emblem(logo: u8, size: EmblemSize) -> Option<Cell> {
    if logo >= EMBLEMS {
        return None;
    }
    let (resource, cell) = size.sheet();
    Some(Cell {
        resource,
        x: u32::from(logo & 7) * cell,
        y: u32::from(logo >> 3) * cell,
        width: cell,
        height: cell,
    })
}

/// The planet pictures (`hdibPlanets`, id 112): 448 by 256, so seven 64-pixel
/// cells across and four down — twenty-eight of them.
pub const PLANET_SHEET: u16 = 112;

/// A planet's picture.
///
/// `DibBlt(…, 0x40, 0x40, hdibPlanets, iOffset % 7 << 6, iOffset / 7 << 6, …)`
/// — and note that this one, alone among the sheets, does **not** count its
/// rows from the bottom in the original's arithmetic. Since the picture is
/// stored bottom-up, that means the index runs up the decoded picture rather
/// than down it, which is why the row here is subtracted.
#[must_use]
pub fn planet(index: u16) -> Option<Cell> {
    let row = u32::from(index) / 7;
    if row > 3 {
        return None;
    }
    Some(Cell {
        resource: PLANET_SHEET,
        x: u32::from(index % 7) * 64,
        y: (3 - row) * 64,
        width: 64,
        height: 64,
    })
}

/// The five sheets of ship pictures at full size (`rghdibShips`, ids 552 to
/// 556).
pub const SHIP_SHEETS: [u16; 5] = [552, 553, 554, 555, 556];
/// The same five at half size (`rghdibShipsT`, ids 557 to 561).
pub const SHIP_SHEETS_SMALL: [u16; 5] = [557, 558, 559, 560, 561];

/// How many ship pictures there are.
///
/// `DrawFleetBitmap` reduces every index it is given modulo this before
/// looking anything up, and the number is not arbitrary: it is the thirty-two
/// ship hulls and five starbase hulls, **four pictures apiece**. The sheets
/// hold exactly that many — four of eight columns by four rows, and a fifth
/// only five columns wide, so `4 × 32 + 20 = 148`.
pub const SHIP_PICTURES: u16 = 148;

/// How many pictures each hull owns, which the ship designer lets the player
/// spin between.
pub const PICTURES_PER_HULL: u16 = 4;

/// A ship's picture, at 64 pixels or at 32.
///
/// `DibBlt(…, 0x40, 0x40, rghdibShips[i >> 5], ((i & 0x1f) >> 2) << 6,
/// (3 - (i & 3)) * 0x40, …)`, and the half-size sheets are drawn by the same
/// arithmetic with 32 in place of 64. This one is laid out **down** the
/// columns: four pictures a column, eight columns, thirty-two a sheet. The row
/// counts from the bottom, which for four rows is `i & 3` from the top.
///
/// The index is taken modulo [`SHIP_PICTURES`] first, exactly as
/// `DrawFleetBitmap` does — so every index names a picture and this cannot
/// fail. That modulo is also what keeps the narrow fifth sheet in bounds: its
/// last column is the last four pictures there are.
#[must_use]
pub fn ship(index: u16, size: ShipSize) -> Cell {
    let index = index % SHIP_PICTURES;
    let (sheets, cell) = match size {
        ShipSize::Large => (&SHIP_SHEETS, 64),
        ShipSize::Small => (&SHIP_SHEETS_SMALL, 32),
    };
    Cell {
        resource: sheets[usize::from(index >> 5)],
        x: u32::from((index & 0x1f) >> 2) * cell,
        y: u32::from(index & 3) * cell,
        width: cell,
        height: cell,
    }
}

/// Which picture a hull is drawn with, given which of its four is wanted.
///
/// A hull's own `ibmp` is the **base** of its group of four — every one of the
/// thirty-seven is a multiple of four — and a design keeps the one its owner
/// chose in the low two bits of its own copy. `BuildDlg` spins between them
/// with a pair of arrow buttons: `iCur = (iCur + 4 ± 1) & 3`, so the choice
/// wraps within the hull's own four and never wanders into another hull's.
#[must_use]
pub fn hull_picture(base: u16, variant: u8) -> u16 {
    base - base % PICTURES_PER_HULL + u16::from(variant) % PICTURES_PER_HULL
}

/// Which of the two ship sheets to take a picture from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShipSize {
    /// 64 pixels.
    Large,
    /// 32 pixels.
    Small,
}

/// How the executable names a picture, in a form a table can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// By number.
    Id(u16),
    /// By name.
    Named(&'static str),
}

impl Which {
    /// The same, as a [`Name`].
    #[must_use]
    pub fn name(self) -> Name {
        match self {
            Which::Id(id) => Name::Id(id),
            Which::Named(text) => Name::Text(text.to_string()),
        }
    }
}

/// Every bitmap the executable loads, in the order `InitStuff` loads them: the
/// variable it goes into, how it is named, and what it is.
///
/// The whole of the game's artwork is here — thirty-eight bitmaps, and they
/// are most of the executable's four megabytes. The sheets have accessors
/// above; the rest are listed so that nothing is nameless.
pub const CATALOGUE: [(&str, Which, &str); 38] = [
    (
        "hbr50Screen",
        Which::Named("Screen50Bmp"),
        "a 50% dither, used as a pattern brush",
    ),
    ("rghbrPat[0]", Which::Id(460), "a pattern brush"),
    ("rghbrPat[1]", Which::Id(461), "a pattern brush"),
    ("rghbrPat[2]", Which::Id(462), "a pattern brush"),
    ("hbrCargo", Which::Named("CargoBmp"), "the cargo hatching"),
    ("hbrDock", Which::Named("DockBmp"), "the dock hatching"),
    (
        "hbmpScanner",
        Which::Named("ScannerBmp"),
        "the scanner's own glyphs",
    ),
    (
        "hbmpScanShip",
        Which::Id(88),
        "the ship marks drawn on the map",
    ),
    (
        "hbmpUnknownPlanet",
        Which::Named("UnknownPlanetBmp"),
        "the planet nobody has visited",
    ),
    (
        "hbmpNumbers",
        Which::Id(249),
        "the small digits drawn on the map",
    ),
    (
        "hdibPlanets",
        Which::Id(112),
        "planet pictures, a sheet of 28",
    ),
    (
        "hdibThings",
        Which::Id(87),
        "the things that are neither planet nor fleet",
    ),
    ("hdibToolbar", Which::Id(178), "the toolbar"),
    (
        "rghdibShips[0]",
        Which::Id(552),
        "ship pictures at 64 pixels",
    ),
    (
        "rghdibShips[1]",
        Which::Id(553),
        "ship pictures at 64 pixels",
    ),
    (
        "rghdibShips[2]",
        Which::Id(554),
        "ship pictures at 64 pixels",
    ),
    (
        "rghdibShips[3]",
        Which::Id(555),
        "ship pictures at 64 pixels, and the game's palette",
    ),
    (
        "rghdibShips[4]",
        Which::Id(556),
        "ship pictures at 64 pixels",
    ),
    (
        "rghdibShipsT[0]",
        Which::Id(557),
        "the same ships at 32 pixels",
    ),
    (
        "rghdibShipsT[1]",
        Which::Id(558),
        "the same ships at 32 pixels",
    ),
    (
        "rghdibShipsT[2]",
        Which::Id(559),
        "the same ships at 32 pixels",
    ),
    (
        "rghdibShipsT[3]",
        Which::Id(560),
        "the same ships at 32 pixels",
    ),
    (
        "rghdibShipsT[4]",
        Which::Id(561),
        "the same ships at 32 pixels",
    ),
    (
        "rghdibInventory[0]",
        Which::Id(500),
        "component pictures, 32 to a sheet",
    ),
    (
        "rghdibInventory[1]",
        Which::Id(501),
        "component pictures, 32 to a sheet",
    ),
    (
        "rghdibInventory[2]",
        Which::Id(502),
        "component pictures, 32 to a sheet",
    ),
    (
        "rghdibInventory[3]",
        Which::Id(503),
        "component pictures, 32 to a sheet",
    ),
    (
        "rghdibInventory[4]",
        Which::Id(504),
        "component pictures, 32 to a sheet",
    ),
    (
        "rghdibInventory[5]",
        Which::Id(505),
        "component pictures, 32 to a sheet",
    ),
    (
        "rghdibInventory[6]",
        Which::Id(506),
        "component pictures, a half-width sheet",
    ),
    (
        "hdibRaces",
        Which::Id(133),
        "the 32 race emblems at 32 pixels",
    ),
    ("hdibRacesT", Which::Id(80), "the same emblems at 16 pixels"),
    ("hdibRacesX", Which::Id(79), "the same emblems at 8 pixels"),
    (
        "hbmpBackBld",
        Which::Id(119),
        "the pictures an empty design slot wears",
    ),
    ("hbmpMsg", Which::Id(134), "the message pane's own marks"),
    (
        "hbmpMono",
        Which::Id(199),
        "a strip of monochrome glyphs, blitted through a mask",
    ),
    (
        "hdibPlaque",
        Which::Id(449),
        "the picture behind the ship designer",
    ),
    (
        "unnamed",
        Which::Id(1079),
        "loaded from somewhere this project has not found",
    ),
];
