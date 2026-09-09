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

    /// What its **tooltip** says, which is not the same as its name.
    ///
    /// `TbWndProc`'s `WM_MOUSEMOVE` arm calls `ShowTooltip(itb + 0x16a, &rc)`,
    /// so the eighteen strings are one contiguous block, `0x16a` to `0x17b`,
    /// in exactly this order — an independent confirmation of the order the
    /// layout table implies. The combo's is the one after them,
    /// [`COMBO_TOOLTIP`].
    #[must_use]
    pub fn tooltip(self) -> &'static str {
        match self {
            Button::Normal => "Normal View",
            Button::SurfaceMinerals => "Surface Mineral View",
            Button::MineralConcentration => "Mineral Concentration View",
            Button::PlanetValue => "Planet Value View",
            Button::Population => "Population View",
            Button::NoPlayerInfo => "No Player Info View",
            Button::AddWaypoints => "Add Way Points Mode",
            Button::ScannerCoverage => "Scanner Coverage Overlay",
            Button::MineFields => "Mine Fields Overlay",
            Button::FleetPaths => "Fleet Paths Overlay",
            Button::IdleFleets => "Idle Fleets Filter",
            Button::PlanetNames => "Planet Names Overlay",
            Button::ShipDesignFilter => "Ship Design Filter",
            Button::ShipDesignMenu => "Design Filter Menu",
            Button::EnemyClassFilter => "Enemy Ship Class Filter",
            Button::EnemyClassMenu => "Enemy Class Filter Menu",
            Button::Zoom => "Zoom Menu",
            Button::ShipCount => "Ship Counts Overlay",
        }
    }

    /// The string id that tooltip comes from: `itb + 0x16a`.
    #[must_use]
    pub fn tooltip_id(self) -> u16 {
        0x16a + u16::from(self.index())
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

/// What the coverage combo's tooltip says — string `0x17c`, the one after the
/// eighteen buttons'.
pub const COMBO_TOOLTIP: &str = "Scanner Effective %";

/// How long the pointer must rest on something before its tooltip appears —
/// `SetTimer(0x39e, 700)` in `TooltipWndProc`.
pub const TOOLTIP_DELAY: f64 = 0.700;

/// Except that a tooltip shown **within this long** of the last one closing
/// appears at once: `GetTickCount() <= vtickTooltipLast + 400`. Moving along a
/// row of buttons therefore reads the whole row without waiting.
pub const TOOLTIP_REPEAT: f64 = 0.400;

/// And one that has been up this long puts itself away, checked by a 50ms
/// timer along with "has the pointer left the button".
pub const TOOLTIP_LIFETIME: f64 = 10.0;

/// The tooltip's background — `hbrTooltip`, `HbrGet(0x9fffff)`, which as a
/// COLORREF is the pale yellow Windows uses for tooltips.
pub const TOOLTIP_BACKGROUND: [u8; 3] = [0xff, 0xff, 0x9f];

/// Its one-pixel frame is `hbrWindowFrame` — `GetSysColor(6)`,
/// `COLOR_WINDOWFRAME`, black by default — and its text `crWindowText`.
pub const TOOLTIP_FRAME: [u8; 3] = [0x00, 0x00, 0x00];

/// The text sits at `(3, 3)` and the window is the text plus six each way.
pub const TOOLTIP_MARGIN: f32 = 3.0;

/// What the pointer is resting on, as far as the tooltip is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tip {
    /// One of the eighteen buttons.
    Button(Button),
    /// The coverage combo.
    Combo,
}

impl Tip {
    /// What it says.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Tip::Button(button) => button.tooltip(),
            Tip::Combo => COMBO_TOOLTIP,
        }
    }
}

/// The toolbar's tooltip, as `ShowTooltip` (`1068:17ea`) and `TooltipWndProc`
/// (`1068:19e4`) run it.
///
/// The rules are all timing, and worth having exactly because they are what
/// makes a tooltip feel like the program's rather than the toolkit's: it waits
/// [`TOOLTIP_DELAY`] before the first one, shows the next one **at once** if
/// the last closed less than [`TOOLTIP_REPEAT`] ago, and takes any tooltip
/// away once it has been up for [`TOOLTIP_LIFETIME`], when the pointer leaves
/// what it belongs to, or on any click.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tooltip {
    over: Option<Tip>,
    /// When the pointer arrived on it.
    since: f64,
    /// When the tooltip went up, if it is up.
    shown: Option<f64>,
    /// When the last one came down.
    hidden: Option<f64>,
}

impl Tooltip {
    /// Tell it where the pointer is, and how long the program has been
    /// running. Call it once a frame.
    pub fn hover(&mut self, over: Option<Tip>, now: f64) {
        if over != self.over {
            if self.shown.is_some() {
                self.hidden = Some(now);
            }
            self.over = over;
            self.since = now;
            self.shown = None;
        }
        let Some(_) = self.over else {
            return;
        };
        match self.shown {
            // Up already: it comes down after its ten seconds.
            Some(shown) if now - shown >= TOOLTIP_LIFETIME => {
                self.shown = None;
                self.hidden = Some(now);
                // The original destroys the window and does not put it back
                // until the pointer moves somewhere else.
                self.over = None;
            }
            Some(_) => {}
            // Not up yet: either the wait is over, or the last tooltip closed
            // recently enough that this one does not wait at all.
            None => {
                let waited = now - self.since >= TOOLTIP_DELAY;
                let followed_on = self
                    .hidden
                    .is_some_and(|hidden| now - hidden <= TOOLTIP_REPEAT);
                if waited || followed_on {
                    self.shown = Some(now);
                }
            }
        }
    }

    /// Take it away — a click does this, and so does anything that moves the
    /// pointer off the toolbar.
    pub fn dismiss(&mut self, now: f64) {
        if self.shown.is_some() {
            self.hidden = Some(now);
        }
        self.shown = None;
        self.over = None;
    }

    /// What to draw, if anything.
    #[must_use]
    pub fn showing(&self) -> Option<Tip> {
        self.shown.and(self.over)
    }
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
