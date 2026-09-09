//! The pop-up summary.
//!
//! `Popup` (`10c0:0c7c`) puts up a small borderless-but-framed window of the
//! class `starspopup`, takes the mouse capture, and `PopupWndProc`
//! (`10c0:0000`) destroys it again on the next **button up** — so it is a
//! press-and-hold, not a click. `DrawPopup` (`10c0:01c0`) paints it.
//!
//! The original has fifteen kinds in `GlobalPD.grPopup`, raised from all over
//! the client. Two of them are what a press in the scanner's status bar
//! raises (`ScannerWndProc`, `1058:043a`), and they are the two here:
//!
//! * `grPopupUnknownObj` (4) — a planet's name, ID and coordinates;
//! * `grPopupFleet` (3) — what a fleet is carrying.
//!
//! Which one appears turns on `sel.scan.grobj`, and since `ChangeScanSel`
//! turns the scan into the planet whenever the point has one, the fleet list
//! is reached only by a fleet out in open space. Nothing at all is raised
//! unless `sel.scan.grobjFull` has the planet or the fleet bit.
//!
//! See `docs/ui/scanner.md`.

/// The window's fill: the class background is `GetStockObject(WHITE_BRUSH)`
/// and `DrawPopup` sets an opaque white text background over it.
pub const BACKGROUND: [u8; 3] = [0xff, 0xff, 0xff];
/// Its one-pixel frame, from `WS_BORDER`.
pub const FRAME: [u8; 3] = [0x00, 0x00, 0x00];
/// The text colour.
pub const TEXT: [u8; 3] = [0x00, 0x00, 0x00];
/// A damaged stack's row, `SetTextColor(0x0000ff)`.
pub const DAMAGED: [u8; 3] = [0xff, 0x00, 0x00];

/// The margin the contents keep from every edge: the first row is at `y = 4`
/// and the first column at `x = 4`, and the height budgets `+ 8` for the pair.
pub const MARGIN: f32 = 4.0;

/// What the planet pop-up's label column adds to the widest label **when it
/// is drawn** — the sizing pass uses [`LABEL_GAP_SIZE`] instead, so the
/// window has four pixels of slack on its right.
pub const LABEL_GAP: f32 = 4.0;
/// What that same column adds when the window is being sized.
pub const LABEL_GAP_SIZE: f32 = 8.0;

/// What the fleet pop-up leaves between the name column and the count column.
pub const COLUMN_GAP: f32 = 16.0;
/// What the damage column adds to its own sample.
pub const DAMAGE_GAP: f32 = 4.0;

/// The planet pop-up's four labels, in the order it draws them: `idsPlanet`,
/// `idsId`, `idsX` and `idsY`, trailing spaces and all.
pub const PLANET_LABELS: [&str; 4] = ["Planet: ", "ID: ", "X: ", "Y: "];

/// The narrowest the planet pop-up's value column gets: `idsN9999`, measured
/// against the planet's name.
pub const VALUE_SAMPLE: &str = "9999";

/// The fleet pop-up's headers: `idsShipName`, the literal `"#"` at `DS:0xbf4`
/// and `idsDamage2`.
pub const HEADER_NAME: &str = "Ship Name";
/// The count column's header.
pub const HEADER_COUNT: &str = "#";
/// The damage column's header.
pub const HEADER_DAMAGE: &str = "Damage";

/// What the damage column is sized from: `idsN9999999`.
pub const DAMAGE_SAMPLE: &str = "99999@99%";

/// What a fleet with nothing in it says: `idsNone2`.
pub const NONE: &str = "None";

/// The pop-up that is up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Popup {
    /// `grPopupUnknownObj`: four labelled rows about a planet.
    Planet(PlanetSummary),
    /// `grPopupFleet`: a fleet's ships, one row per design.
    Fleet(FleetSummary),
    /// `grPopupComponent`: a component's details, which is the **Technology
    /// Browser's own panel** — `DrawPopup` calls the same
    /// `DisplayComponentInfo` the browser calls, at the same width.
    ///
    /// Carried as the `(category, index)` pair the caller looked up, so the
    /// view can build the panel the same way the browser builds it.
    Component((u16, usize)),
    /// `grPopupString`: a paragraph, word-wrapped to a width the caller
    /// passes. The research dialog's tech note is one.
    Note(String),
}

/// How big the component pop-up is (`Popup`, `10c0:0c7c`).
///
/// The width is **the same formula the Technology Browser's own panel uses**,
/// and so is the height once `dyArial10` is put back — the browser adds it out
/// of a global where the pop-up names it. The two windows are the same panel.
///
/// ```text
/// width  = 0x158, and 0x28 wider again when dyArial8 > 14
/// height = dyArial8 * 12 + dyArial10 + 0x4e
/// ```
#[must_use]
pub fn component_size(line: f32, line10: f32) -> (f32, f32) {
    let wide = if line > 14.0 { 0x28 as f32 } else { 0.0 };
    (0x158 as f32 + wide, line * 12.0 + line10 + 0x4e as f32)
}

/// `grPopupUnknownObj`, whose four values go beside [`PLANET_LABELS`].
///
/// The pop-up reads `sel.scan` rather than anything passed to it, so it is
/// always about the selection: the planet's name, `idpl + 1`, and the scan's
/// own x and y.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetSummary {
    /// The planet's name, its ID, its x and its y, in the label order.
    pub values: [String; 4],
}

/// `grPopupFleet`: one row per design the fleet holds any of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetSummary {
    /// The rows, in **design-slot** order — the original walks slots 0 to 15
    /// and skips the empty ones.
    pub rows: Vec<FleetRow>,
    /// Whether the damage column may be drawn at all (`POPUPDATA.fRedDamage`).
    ///
    /// The Selection Summary's own ship tile sets this for a fleet you can see
    /// in full detail (`MineClick`, `1028:3e7b`). **The scanner's status bar
    /// sets neither this nor the hull filter beside it**, so in the original
    /// they carry whatever the last pop-up left in the same union; from a cold
    /// start that is nothing, which is what is reproduced here.
    pub show_damage: bool,
}

/// One design's row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRow {
    /// The design's name (`DecorateHullName`, `10c0:…`).
    pub name: String,
    /// How many, as `"%d"`.
    pub count: String,
    /// `"%d@%d%%"` — how many of them are damaged and how badly — when any
    /// of them are.
    pub damage: Option<String>,
}

impl FleetSummary {
    /// Whether the damage column appears: only when it is switched on **and**
    /// the sizing pass found a damaged stack to put in it, which is what
    /// `dxDamage` ends up non-zero for.
    #[must_use]
    pub fn damage_column(&self) -> bool {
        self.show_damage && self.rows.iter().any(|row| row.damage.is_some())
    }
}

/// How a damaged stack's `"%d@%d%%"` reads.
///
/// Both halves are floored at one, and both come out of the packed damage word
/// the fleet record carries — `pctSh = w & 0x7f`, `pctDp = (w >> 7) & 0x1ff`:
///
/// ```text
/// ships   = pctSh * count / 100        , at least 1
/// percent = w / 640                    , at least 1
/// ```
///
/// The second is the original's own shortcut and not a mistake: `pctDp` is in
/// 500ths, `w / 640` is `pctDp / 5` for every value either field can take, and
/// `pctDp / 5` is the percentage.
#[must_use]
pub fn damage_text(count: i32, ships_pct: i32, armor_pct: i32) -> Option<String> {
    let packed = (ships_pct & 0x7f) | ((armor_pct & 0x1ff) << 7);
    if packed == 0 {
        return None;
    }
    let ships = (ships_pct * count / 100).max(1);
    let percent = (packed / 640).max(1);
    Some(format!("{ships}@{percent}%"))
}
