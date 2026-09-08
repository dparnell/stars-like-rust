//! The scanner's toolbar.
//!
//! `DrawToolbar` (`1068:06f0`) walks a 29-entry table of signed bytes laid out
//! left to right: a non-negative entry is a button, and a negative one is a
//! gap or the coverage combo. The table is in the toolbar's own code segment,
//! which is why the decompiler shows it as a read from nowhere.
//!
//! Eighteen buttons, and the sheet behind them (`hdibToolbar`, id 178) is
//! 432 by 23 — exactly eighteen cells of 24, one row. Every button's picture
//! is used once.
//!
//! The buttons are **not** in the order they appear: the table interleaves
//! them, and once laid out through it they come out in exactly the order
//! `MANUAL.PDF` pp. 5-12..5-15 introduces them, which is what identifies the
//! ones whose pictures are ambiguous.
//!
//! See `docs/ui/toolbar.md`.

use stars_formats::resources::art::Cell;

/// The toolbar's own bitmap.
pub const SHEET: u16 = 178;
/// One button's picture: 24 across, 23 down, all in one row.
pub const ART: (u32, u32) = (24, 23);

/// The eighteen buttons, named by what the manual calls them.
///
/// The discriminant is the button's own index — which is both its cell in the
/// sheet and what [`LAYOUT`] holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Button {
    /// The default view: planets, fleets and everything else.
    Normal = 0,
    /// What is on each planet's surface.
    SurfaceMinerals = 1,
    /// What is in the ground.
    MineralConcentration = 2,
    /// How good each planet is for this race.
    PlanetValue = 3,
    /// How many people live there.
    Population = 4,
    /// The map with every trace of ownership taken off.
    NoPlayerInfo = 5,
    /// Give the fleet under command waypoints with the mouse alone.
    AddWaypoints = 6,
    /// Radar coverage, at the percentage the combo beside it holds.
    ScannerCoverage = 7,
    /// Minefields, as coloured grids.
    MineFields = 8,
    /// Where this player's fleets are going.
    FleetPaths = 9,
    /// Only the fleets with nothing to do.
    IdleFleets = 10,
    /// Planet names.
    PlanetNames = 11,
    /// Only the fleets holding a chosen design of this player's.
    ShipDesignFilter = 12,
    /// The menu that chooses which design (narrow).
    ShipDesignMenu = 13,
    /// Only the opponents' fleets holding a chosen class.
    EnemyClassFilter = 14,
    /// The menu that chooses which class (narrow).
    EnemyClassMenu = 15,
    /// Zoom the scanner.
    Zoom = 16,
    /// How many ships are at each place.
    ShipCount = 17,
}

impl Button {
    /// All eighteen, by index.
    pub const ALL: [Button; 18] = [
        Button::Normal,
        Button::SurfaceMinerals,
        Button::MineralConcentration,
        Button::PlanetValue,
        Button::Population,
        Button::NoPlayerInfo,
        Button::AddWaypoints,
        Button::ScannerCoverage,
        Button::MineFields,
        Button::FleetPaths,
        Button::IdleFleets,
        Button::PlanetNames,
        Button::ShipDesignFilter,
        Button::ShipDesignMenu,
        Button::EnemyClassFilter,
        Button::EnemyClassMenu,
        Button::Zoom,
        Button::ShipCount,
    ];

    /// Its index, which is also its cell in the sheet.
    #[must_use]
    pub fn index(self) -> u8 {
        self as u8
    }

    /// One back from an index, or `None` for a number no button has.
    #[must_use]
    pub fn from_index(index: i8) -> Option<Button> {
        usize::try_from(index)
            .ok()
            .and_then(|i| Self::ALL.get(i))
            .copied()
    }

    /// The name the manual gives it, which is what the tooltip should say.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Button::Normal => "Normal",
            Button::SurfaceMinerals => "Surface Minerals",
            Button::MineralConcentration => "Mineral Concentration",
            Button::PlanetValue => "Planet Value",
            Button::Population => "Population",
            Button::NoPlayerInfo => "No Player Information",
            Button::AddWaypoints => "Add Waypoints Mode",
            Button::ScannerCoverage => "Scanner Coverage",
            Button::MineFields => "Mine Fields",
            Button::FleetPaths => "Fleet Paths",
            Button::IdleFleets => "Idle Fleets",
            Button::PlanetNames => "Planet Names",
            Button::ShipDesignFilter => "Ship Design Filter",
            Button::ShipDesignMenu => "Choose a ship design",
            Button::EnemyClassFilter => "Enemy Ship Class Filter",
            Button::EnemyClassMenu => "Choose an enemy ship class",
            Button::Zoom => "Zoom",
            Button::ShipCount => "Ship Count",
        }
    }

    /// Whether this is one of the six views, which are a radio group rather
    /// than toggles: `grbitScan` keeps the chosen one in its low four bits.
    #[must_use]
    pub fn is_view(self) -> bool {
        (self as u8) < 6
    }

    /// The two narrow buttons — the menus hung off the two filters. Everything
    /// else is 29 wide; these are 11, and only 7 pixels of their picture is
    /// blitted.
    #[must_use]
    pub fn is_narrow(self) -> bool {
        matches!(self, Button::ShipDesignMenu | Button::EnemyClassMenu)
    }

    /// How wide the button is, border and all (`DxOfBtn`, `1068:0bb2`).
    #[must_use]
    pub fn width(self) -> u32 {
        if self.is_narrow() {
            11
        } else {
            29
        }
    }

    /// How much of its picture is drawn.
    #[must_use]
    pub fn art_width(self) -> u32 {
        if self.is_narrow() {
            7
        } else {
            ART.0
        }
    }

    /// Where its picture sits in the sheet.
    #[must_use]
    pub fn cell(self) -> Cell {
        Cell {
            resource: SHEET,
            x: u32::from(self.index()) * ART.0,
            y: 0,
            width: self.art_width(),
            height: ART.1,
        }
    }
}

/// The toolbar's layout, left to right, exactly as the table in the toolbar's
/// code segment holds it.
///
/// A non-negative entry is a button index. `-1` is a six-pixel gap, `-2` a
/// two-pixel one, and `-3` the scanner-coverage combo.
pub const LAYOUT: [i8; 29] = [
    0, 1, 2, 3, 4, 5, -1, 6, -1, 7, -2, -3, -1, 8, -1, 9, -1, 11, 17, -1, 10, -1, 12, 13, -1, 14,
    15, -1, 16,
];

/// The combo box's width. The original picks the wider of the two by the
/// height of its font; this takes the wider.
pub const COMBO_WIDTH: u32 = 70;

/// How the toolbar's own three colours come out.
///
/// `SetSysColors` (`main.c`, `1000`-segment start-up) takes them straight from
/// Windows: `GetSysColor(15)` for the face, `20` for the highlight and `16`
/// for the shadow — `COLOR_BTNFACE`, `COLOR_BTNHIGHLIGHT`, `COLOR_BTNSHADOW`.
/// A modern desktop has no such colours to give, so these are the **Windows
/// 3.1 defaults**, which is what every screenshot of the game shows.
///
/// The face, which is also the strip's own background (`WM_ERASEBKGND` fills
/// the client rectangle with it).
pub const FACE: [u8; 3] = [0xc0, 0xc0, 0xc0];
/// The lit edge: top and left of a button that is up.
pub const HILITE: [u8; 3] = [0xff, 0xff, 0xff];
/// And the shadow: bottom and right of one that is up.
pub const SHADOW: [u8; 3] = [0x80, 0x80, 0x80];

/// How tall a button is, bevel and all — `DrawBitmapButton` draws rows `y` to
/// `y + 0x1b`.
pub const BUTTON_HEIGHT: u32 = 28;
/// And how tall the strip is: `ItbFromPpt` takes a click on `4 <= y < 0x20`.
pub const ROW_HEIGHT: u32 = 32;
/// Where the row of buttons starts, in from the strip's left and top edges.
pub const MARGIN: u32 = 4;

/// How far into a button its picture is pushed — `fDown` in
/// `DrawBitmapButton`, which is a **distance and not a flag**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Press {
    /// Not pressed: the picture sits at `+2, +2` and the bevel is lit from the
    /// top left.
    #[default]
    Up = 0,
    /// Latched on — the view showing, or an overlay switched on. The bevel
    /// turns over and the picture moves a pixel down and right.
    Latched = 1,
    /// Held down under the pointer: another pixel again, and the corner the
    /// latched state leaves dark is lit back up.
    Held = 2,
}

impl Press {
    /// The offset it gives the picture, in pixels.
    #[must_use]
    pub fn offset(self) -> f32 {
        self as u8 as f32
    }

    /// Whether the bevel is turned over.
    #[must_use]
    pub fn is_down(self) -> bool {
        self != Press::Up
    }
}

/// One thing in the toolbar's row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    /// A button, and how wide it is.
    Button(Button),
    /// Blank space.
    Gap(u32),
    /// The scanner-coverage combo.
    Combo,
}

impl Item {
    /// How wide it is.
    #[must_use]
    pub fn width(self) -> u32 {
        match self {
            Item::Button(button) => button.width(),
            Item::Gap(width) => width,
            Item::Combo => COMBO_WIDTH,
        }
    }
}

/// The toolbar's row, in order.
#[must_use]
pub fn items() -> Vec<Item> {
    LAYOUT
        .iter()
        .filter_map(|entry| match *entry {
            -1 => Some(Item::Gap(6)),
            -2 => Some(Item::Gap(2)),
            -3 => Some(Item::Combo),
            index => Button::from_index(index).map(Item::Button),
        })
        .collect()
}

/// The coverage percentages the combo offers, which the original lists from a
/// hundred down in tens — `(100 - pct) / 10` is the list index it snaps to.
pub const COVERAGE_STEPS: [u8; 10] = [100, 90, 80, 70, 60, 50, 40, 30, 20, 10];

/// The lowest coverage the combo will accept.
pub const COVERAGE_MIN: u8 = 2;
/// The highest.
pub const COVERAGE_MAX: u8 = 100;

/// Read a coverage percentage the way the combo reads what was typed into it.
///
/// It takes the leading digits, insists that what follows is nothing or a
/// `%`, and clamps to `2..=100`; anything else reads as zero and is clamped up
/// to the minimum.
#[must_use]
pub fn coverage_from_text(text: &str) -> u8 {
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    let rest = &text[digits.len()..];
    let value = if rest.is_empty() || rest == "%" {
        digits.parse::<u32>().unwrap_or(0)
    } else {
        0
    };
    u8::try_from(value.clamp(u32::from(COVERAGE_MIN), u32::from(COVERAGE_MAX)))
        .unwrap_or(COVERAGE_MAX)
}
