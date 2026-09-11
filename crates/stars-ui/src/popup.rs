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
    /// `grPopupPlanetIndustry` (`PtDisplayFactoryMineInfo`, `10c0:31a0`):
    /// how many mines or factories a planet has, could hold, and can staff.
    Industry(IndustrySummary),
    /// `grPopupResources` (`PtDisplayResourceInfo`): what a planet makes in
    /// a year and where it goes.
    Resources(ResourceSummary),
    /// `grPopupShdef` (`10`): a ship or starbase design, drawn with the
    /// designer's own panel — `DrawSlotDlg` and `DrawBuildSelHull`, the two
    /// the dialog itself draws — with the design's name over the top.
    Design(stars_core::design::ShipDesign),
    /// `grPopupPlanet` (`PtDisplayPlanetPopInfo`): who lives on a planet,
    /// what it can hold, and what it will do to them.
    Population(PopulationSummary),
    /// `grPopupMineral` (12): one mineral's three figures for a planet,
    /// which the Minerals, Mining Rate and Min Conc columns of the planets
    /// report raise, and the mineral gauges in the planet pane.
    ///
    /// `DrawPopup` draws this one inline rather than through a `PtDisplay`
    /// helper, which is why it is a table and not a sentence.
    Mineral(MineralSummary),
}

/// What the industry pop-up says.
///
/// The original writes it as one sentence assembled from five fragments
/// (`idsHave` and the four after it) with the figures dropped in between.
/// This keeps the figures and says the same thing in its own words, which
/// is what this project does with the game's prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndustrySummary {
    /// Whose planet it is about.
    pub planet: String,
    /// `true` for the Fact column, `false` for Mine.
    pub factories: bool,
    /// How many are built.
    pub built: i64,
    /// How many the planet could hold (`CMaxMines` / `CMaxFactories`).
    pub most: i64,
    /// How many the colonists there can staff.
    pub operable: i64,
    /// An Alternate Reality race builds neither, and the panel says so
    /// instead of giving figures.
    pub innate: bool,
}

impl IndustrySummary {
    /// The heading, `idsSInfo` (`0x552`) — `Mine Info` or `Factory Info`.
    #[must_use]
    pub fn heading(&self) -> String {
        format!("{} Info", if self.factories { "Factory" } else { "Mine" })
    }

    /// The noun, singular or plural as the count needs — the original picks
    /// between `Mine`/`Mines` and `Factory`/`Factories` on `!= 1`.
    #[must_use]
    pub fn noun(&self, count: i64) -> &'static str {
        match (self.factories, count == 1) {
            (true, true) => "factory",
            (true, false) => "factories",
            (false, true) => "mine",
            (false, false) => "mines",
        }
    }

    /// What it reads.
    #[must_use]
    pub fn text(&self) -> String {
        if self.innate {
            return if self.factories {
                "This race builds no factories.".to_string()
            } else {
                "This race builds no mines. Its colonists mine on their own,                  at a tenth of the square root of the population at the                  starbase."
                    .to_string()
            };
        }
        format!(
            "{} has {} {}. There is room for {}, and the colonists there can              staff {}.",
            self.planet,
            self.built,
            self.noun(self.built),
            self.most,
            self.operable,
        )
    }
}

/// What the resources pop-up says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceSummary {
    /// The planet.
    pub planet: String,
    /// Resources a year (`CResourcesAtPlanet`).
    pub total: i64,
    /// What goes to research.
    pub research: i64,
    /// What is left for the planet. `None` when nothing goes to research:
    /// the original stops the sentence there rather than saying so twice.
    pub spare: Option<i64>,
    /// An Alternate Reality race's resources are the square root of the
    /// population, and the panel adds a line saying so.
    pub innate: bool,
}

impl ResourceSummary {
    /// The heading, `idsResourceInfo` (`0x233`).
    #[must_use]
    pub fn heading(&self) -> &'static str {
        "Resource Info"
    }

    /// What it reads.
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = format!("{} makes {} resources a year.", self.planet, self.total);
        match self.spare {
            Some(spare) => out.push_str(&format!(
                " {} of them go to research, leaving {} for the planet.",
                self.research, spare
            )),
            None => out.push_str(" None of them go to research."),
        }
        if self.innate {
            out.push_str(" This race's resources are the square root of its population.");
        }
        out
    }
}

/// Who lives on a planet, as the population pop-up puts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inhabited {
    /// Ours, with the colonists on it.
    Ours(i64),
    /// Somebody else's. The figure is a guess, and `None` when the planet
    /// has not been looked at closely enough to make one.
    Enemy(Option<i64>),
    /// Nobody's.
    Nobody,
}

/// What the population pop-up says.
///
/// The original builds three sentences out of a dozen fragments in
/// alternating faces, choosing between them on who owns the planet, whether
/// it is worth anything, and whether there is room to grow. The choices are
/// kept; the wording is this project's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopulationSummary {
    /// The planet.
    pub planet: String,
    /// Who is on it.
    pub who: Inhabited,
    /// What it is worth to this race, in **tenths** of a percent, which is
    /// how the original prints the hostile case (`%d.%d%%`). `None` when
    /// the planet has not been scanned.
    pub value: Option<i16>,
    /// The most this race could ever put there, in colonists.
    pub capacity: Option<i64>,
    /// Next year's growth and the total it reaches, in colonists. Only ours
    /// and only with room to grow.
    pub growth: Option<(i64, i64)>,
    /// Another player's defence coverage as a percentage, or `None` for a
    /// planet with none. Meaningless unless [`Inhabited::Enemy`].
    pub defenses: Option<i64>,
}

impl PopulationSummary {
    /// What it reads.
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = match &self.who {
            Inhabited::Ours(pop) => format!("You have {pop} colonists on {}.", self.planet),
            Inhabited::Nobody => format!("{} is uninhabited.", self.planet),
            Inhabited::Enemy(None) => {
                format!(
                    "Somebody else holds {}; how many, nobody knows.",
                    self.planet
                )
            }
            Inhabited::Enemy(Some(pop)) => {
                format!(
                    "Somebody else holds {}, with roughly {pop} colonists.",
                    self.planet
                )
            }
        };
        let ours = matches!(self.who, Inhabited::Ours(_));
        if let Some(value) = self.value {
            if value < 0 {
                let hostile = -i32::from(value);
                out.push_str(&format!(
                    " It kills about {}.{}% of {} every year.",
                    hostile / 10,
                    hostile % 10,
                    if ours {
                        "the colonists there"
                    } else {
                        "any colonists landed on it"
                    }
                ));
            } else if let Some(capacity) = self.capacity.filter(|c| *c > 0) {
                out.push_str(&if ours {
                    format!(" It will hold up to {capacity} of your colonists.")
                } else {
                    format!(" Colonised, it would hold up to {capacity} of your colonists.")
                });
            }
        }
        match (&self.who, self.growth) {
            (Inhabited::Ours(_), Some((0, _))) => {
                out.push_str(" It will not grow next year.");
            }
            (Inhabited::Ours(_), Some((by, to))) => {
                out.push_str(&format!(" Next year it grows by {by}, to {to}."));
            }
            (Inhabited::Enemy(_), _) => out.push_str(&match self.defenses {
                Some(pct) => format!(" Its defences cover about {pct}%."),
                None => " It appears to have no defences.".to_string(),
            }),
            _ => {}
        }
        out
    }
}

/// What the mineral pop-up says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MineralSummary {
    /// Which of the three, which titles the window.
    pub mineral: usize,
    /// kT on the surface, or `None` — the original prints `Unknown` when
    /// the figure is negative, which is a planet nobody has landed on.
    pub surface: Option<i64>,
    /// Concentration, or `None` for unknown.
    pub concentration: Option<i64>,
    /// The note the original puts after the concentration on a home world:
    /// [`HOME_FLOOR`] while it is below thirty, [`HOME_WORLD`] above.
    pub home_note: Option<&'static str>,
    /// kT a year, when the rate is known.
    pub rate: Option<i64>,
}

/// The mineral pop-up's three labels: `idsSurface` (`0x265`),
/// `idsMineralConcentration` (`0x264`) and `idsMiningRate` (`0x266`),
/// trailing spaces and all.
pub const MINERAL_LABELS: [&str; 3] = ["On Surface: ", "Mineral Concentration: ", "Mining Rate: "];

/// `idsUnknown2` (`0x254`), which stands in for a figure nobody has.
pub const UNKNOWN: &str = "Unknown";

/// `idsN30` (`0x51f`): a home world below thirty still mines as if it were
/// thirty (`MANUAL.PDF` p. 6-5).
pub const HOME_FLOOR: &str = " (30)";

/// `idsHw` (`0x51e`).
pub const HOME_WORLD: &str = " (HW)";

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
