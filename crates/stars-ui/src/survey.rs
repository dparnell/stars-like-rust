//! The Selection Summary pane's own layout, wording and colours.
//!
//! `DrawMineSurvey` (`1028:065a`) lays the planet case out itself rather than
//! from a tile table: **four lines of text** across the top — value,
//! population, owner and how old the report is — and then **six equal rows**
//! filling whatever is left, three for the environment and three for the
//! minerals. See `docs/ui/mine-survey-pane.md`.

/// What the value column is sized from: `idsN999mr`, five characters wide.
pub const VALUE_SAMPLE: &str = "999mR";

/// What the value column adds to that sample, and what the label column adds
/// to its own widest label.
pub const COLUMN_PAD: f32 = 6.0;

/// The unit under the mineral scale (`DS:0x504`).
pub const KT: &str = "kT";

/// What is drawn against a mineral bar that runs past the end of the scale
/// (`DS:0x507`).
pub const OVERFLOW: &str = "+";

/// The three environment labels, wide and narrow (`DS:0x47e` and `DS:0x492`).
pub const ENV_LABELS: [(&str, &str); 3] = [
    ("Gravity", "Grav"),
    ("Temperature", "Temp"),
    ("Radiation", "Rad"),
];

/// The three mineral labels (`DS:0x4cc`). A narrow pane draws the **first
/// four characters** of each rather than a shorter word of its own.
pub const MINERAL_LABELS: [&str; 3] = ["Ironium", "Boranium", "Germanium"];

/// How many characters of a mineral label a narrow pane draws.
pub const MINERAL_LABEL_NARROW: usize = 4;

/// `idsVal` and `idsVal + 1`: the narrow form first, as the string table has
/// them.
pub const VALUE_LABEL: (&str, &str) = ("Value: ", "Val: ");
/// `idsPop` and `idsPop + 1`, two trailing spaces and all.
pub const POPULATION_LABEL: (&str, &str) = ("Population:  ", "Pop:  ");

/// `idsUninhabited`.
pub const UNINHABITED: &str = "Uninhabited";
/// `idsMsg1264`: what a planet nobody has surveyed says for its population.
pub const UNKNOWN_POPULATION: &str = "???";

/// The band each environment variable's bar is drawn in — `rghbrPlanetAttr`,
/// the **dark** shade of each pair (`FCreateStuff`, `1000:024d`).
///
/// Gravity is blue, temperature red, radiation green; the COLORREFs are
/// `0x7f0000`, `0x7f` and `0x7f00`, which is `0x00bbggrr` and so **not** the
/// order they read in.
pub const ENV_BAND: [[u8; 3]; 3] = [[0x00, 0x00, 0x7f], [0x7f, 0x00, 0x00], [0x00, 0x7f, 0x00]];

/// The diamond marking the planet's own value — the **bright** shade of the
/// same pairs (`rghbrPlanetAttr[i][1]`).
pub const ENV_MARK: [[u8; 3]; 3] = [[0x00, 0x00, 0xff], [0xff, 0x00, 0x00], [0x00, 0xff, 0x00]];

/// What a mineral's surface stock is drawn in — `rghbrMinSum[i][0]`, the
/// bright shade: blue, green and yellow.
pub const MINERAL_BAR: [[u8; 3]; 3] = [[0x00, 0x00, 0xff], [0x00, 0xff, 0x00], [0xff, 0xff, 0x00]];

/// What the same bar is extended in by this year's mining — `rghbrMinSum[i][1]`,
/// the dark shade, drawn first and left showing past the bright one.
pub const MINERAL_SUM: [[u8; 3]; 3] = [[0x00, 0x00, 0x7f], [0x00, 0x7f, 0x00], [0x7f, 0x7f, 0x00]];

/// What each mineral's **label** is written in (`rgcrMin`, `DS:0x448`).
///
/// Not the same three colours as the bars: boranium's label is the *dark*
/// green while its bar is the bright one.
pub const MINERAL_TEXT: [[u8; 3]; 3] = [[0x00, 0x00, 0xff], [0x00, 0x7f, 0x00], [0xff, 0xff, 0x00]];

/// How tall one of the six rows is.
///
/// `dySBar`-style arithmetic: four lines of text are taken off the top and
/// what is left is split six ways, rounded up and then **forced even**
/// (`((h - dyArial8 * 4 - 2) / 6 + 1) & ~1`).
#[must_use]
pub fn row_height(pane_height: f32, line: f32) -> f32 {
    let left = pane_height - line * 4.0 - 2.0;
    let row = (left / 6.0).floor() + 1.0;
    #[allow(clippy::cast_possible_truncation)]
    let even = (row.max(2.0) as i32) & !1;
    #[allow(clippy::cast_precision_loss)]
    let even = even as f32;
    even.max(2.0)
}

/// Whether the pane is drawn in its **narrow** form.
///
/// The original measures the widest label, adds six, and switches when four of
/// those would not fit across the pane (`if (labelWidth * 4 >= rc.right)`).
#[must_use]
pub fn narrow(label_width: f32, pane_width: f32) -> bool {
    label_width * 4.0 >= pane_width
}

/// The step between the mineral scale's ticks.
///
/// The original works out how many labels of the widest figure would fit —
/// each label plus half its own width again — divides the scale by that, and
/// then **rounds the step up to a round number** whose size depends on how big
/// the scale is:
///
/// | `max` | rounded up to a multiple of |
/// | --- | --- |
/// | under 500 | 10 |
/// | under 1000 | 50 |
/// | under 2500 | 100 |
/// | under 7500 | 250 |
/// | under 15000 | 500 |
/// | 15000 and over | 1000 |
///
/// `max` is `cMinGrafMax`, which ships at 5000, so the shipped ladder rounds
/// to 250.
#[must_use]
pub fn scale_step(max: i32, fits: i32) -> i32 {
    let rough = if fits > 0 { max / fits } else { max }.max(1);
    let round_to = match max {
        ..500 => 10,
        500..1000 => 50,
        1000..2500 => 100,
        2500..7500 => 250,
        7500..15000 => 500,
        _ => 1000,
    };
    ((rough + round_to - 1) / round_to) * round_to
}

/// How many ticks the scale carries: `cMinGrafMax / step`, so the last one
/// lands on or before the end.
#[must_use]
pub fn scale_ticks(max: i32, step: i32) -> i32 {
    if step <= 0 {
        0
    } else {
        max / step
    }
}

// --- The fleet half -------------------------------------------------------
//
// `DrawMineSurvey`'s `grobjFleet` arm. The picture goes at the top left with
// the owner's emblem tucked under its right, and the text runs down a column
// starting `0x56` from the pane's left edge, a line and two pixels apart.

/// Where the fleet's picture sits, from the pane's top left corner.
pub const PICTURE_AT: (f32, f32) = (6.0, 6.0);
/// How big it is: a 64-pixel bitmap in a 66-pixel black square.
pub const PICTURE_SIDE: f32 = 64.0;
/// The owner's emblem, a 32-pixel bitmap in a 34-by-36 black square, offset
/// from the picture's own corner.
pub const EMBLEM_AT: (f32, f32) = (0x13 as f32, 0x47 as f32);
/// How big the emblem is.
pub const EMBLEM_SIDE: f32 = 32.0;
/// Where the text column starts, from the pane's left edge.
pub const TEXT_LEFT: f32 = 0x56 as f32;
/// Where a gauge starts, from the same edge, once the label before it is
/// measured; it runs to four pixels short of the pane's right.
pub const GAUGE_LEFT: f32 = 0x5a as f32;
/// What one row of the fleet's text adds to a line.
pub const ROW_GAP: f32 = 2.0;

/// `rghbrMineral`: ironium, boranium, germanium, **colonists** and fuel.
///
/// The first four stack up in the cargo gauge and the last fills the fuel one.
/// Colonists are white and fuel is red; the three minerals are the same blue,
/// dark green and yellow the mineral labels use.
pub const CARGO_COLOURS: [[u8; 3]; 5] = [
    [0x00, 0x00, 0xff],
    [0x00, 0x7f, 0x00],
    [0xff, 0xff, 0x00],
    [0xff, 0xff, 0xff],
    [0xff, 0x00, 0x00],
];

/// `idsFuel2` and `idsCargo`, trailing space and all. Neither has a narrow
/// form: the pane shortens everything else instead.
pub const FUEL_LABEL: &str = "Fuel:";
/// The cargo gauge's label.
pub const CARGO_LABEL: &str = "Cargo: ";

/// `idsFleetMassLdkt` and `idsMassLdkt`.
pub const MASS_LABEL: (&str, &str) = ("Fleet Mass: ", "Mass: ");
/// `idsWaypointS` and `idsWpS`.
pub const WAYPOINT_LABEL: (&str, &str) = ("Next Waypoint: ", "WP: ");
/// `idsWaypointTaskS` and `idsTaskS`.
pub const TASK_LABEL: (&str, &str) = ("Waypoint Task: ", "Task: ");
/// `idsWarpSpeedD` and `idsWarpD`.
pub const WARP_LABEL: (&str, &str) = ("Warp Speed: ", "Warp: ");
/// `idsWarpSpeedStopped` and `idsWarpStopped`.
pub const STOPPED: (&str, &str) = ("Warp Speed: (stopped)", "Warp: (stopped)");
/// `idsUseStargate`, which has no narrow form of its own.
pub const USE_STARGATE: &str = "Use Stargate";
/// `idsNone`: what the waypoint row says for a fleet going nowhere.
pub const NO_WAYPOINT: &str = "(none)";
/// The pseudo-warp that means "take the gate" rather than fly.
pub const STARGATE_WARP: u8 = 11;

/// One of the pane's two gauges (`LDrawGauge`, `1050:4560`).
///
/// A one-pixel frame in the window-text colour, the segments stacked left to
/// right inside it, and `hbrButtonFace` for whatever is left. The label is
/// centred on the bar and drawn **only when it fits** — the original measures
/// it against the bar's width less three and simply leaves it out otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Gauge {
    /// Each segment's amount and the colour it is drawn in, in stacking order.
    pub segments: Vec<(i32, [u8; 3])>,
    /// What a full bar is worth.
    pub total: i32,
    /// The label: `"%ld of %ldmg"` for fuel (`idsLdLdmg`) and
    /// `"%ld of %ldkT"` for cargo (`idsLdLdkt`).
    pub label: String,
}

/// What the pane says about a fleet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FleetSummary {
    /// `Ship Count: %ld`.
    pub ships: String,
    /// The fuel gauge, for a fleet whose insides are known.
    pub fuel: Option<Gauge>,
    /// The cargo gauge, likewise.
    pub cargo: Option<Gauge>,
    /// `Fleet Mass: %ldkT`.
    pub mass: String,
    /// Where it is going, what it will do there, and how fast — three rows for
    /// a fleet the player commands, and for anybody else's just the speed, and
    /// only when it is known.
    pub orders: Vec<String>,
    /// `This fleet can destroy up to %ld mines per year.`, when it can
    /// (`idsFleetCanDestroyLdMinesPerYear`).
    pub sweeping: Option<String>,
}
