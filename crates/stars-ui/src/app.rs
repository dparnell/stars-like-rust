//! The shared application state every frontend drives.
//!
//! This is deliberately free of any rendering: it holds the loaded game, what
//! the player has selected, and the derived facts the screens need, so that the
//! desktop and web shells can differ only in how they draw it.

use std::path::{Path, PathBuf};

use stars_core::design::ShipDesign;
use stars_core::newgame::{Created, NewGame};
use stars_core::{GameState, Planet};
use stars_formats::block::{Block, BlockType};
use stars_formats::production::{ProductionQueueRecord, QueueClass, QueueItem};
use stars_formats::{
    battle_records_in_with, ActionLayout, BattleRecord, PlanetRecord, StarsFile, Universe,
};

use crate::popup::{FleetRow, FleetSummary, PlanetSummary, Popup};
use crate::statusbar::{Distance, StatusBar};
use crate::vcr::Vcr;

/// The menu items `InitializeMenu` greys on a condition of their own.
///
/// The rest of the bar is alive whenever a game is open, so only these five
/// need asking about. See [`App::menu_item_enabled`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    /// Turn (`&Generate`, `0x69`).
    Generate,
    /// Turn (`&Wait for New`, `0x6a`).
    WaitForNew,
    /// Commands (`&Player Relations...`, `0x7de`).
    PlayerRelations,
    /// File (`Save &And Submit`, `0xedb`).
    SaveAndSubmit,
    /// Commands (`&Change Password...`, `0x10e`).
    ChangePassword,
}

/// Which screen the frontend is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    /// The galaxy map.
    #[default]
    Galaxy,
    /// Planet detail.
    Planets,
    /// Fleet list.
    Fleets,
    /// Everybody else's fleets — the original's third report, which this
    /// project had no screen for until the report tables were built.
    EnemyFleets,
    /// Battle playback.
    Battles,
    /// Player and race summary.
    Players,
}

impl Screen {
    /// Every screen, in the order a frontend should offer them.
    pub const ALL: [Screen; 6] = [
        Screen::Galaxy,
        Screen::Planets,
        Screen::Fleets,
        Screen::EnemyFleets,
        Screen::Battles,
        Screen::Players,
    ];

    /// The tab label.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Screen::Galaxy => "Galaxy",
            Screen::Planets => "Planets",
            Screen::Fleets => "Fleets",
            Screen::EnemyFleets => "Others' Fleets",
            Screen::Battles => "Battles",
            Screen::Players => "Players",
        }
    }
}

/// Where a planet's starbase design sits in the flattened design list.
///
/// `PLANET.isb` counts within the **starbase** designs — the original reads
/// them out of `lprgshdefSB`, a table of its own — and this project keeps
/// ships and starbases in one list, with the starbases from slot 16 on. Two
/// places here used to index that list with `isb` directly, which named a
/// ship design instead: the Starbase tile's title and the hull the scanner
/// asks about.
pub(crate) fn starbase_slot(isb: u8) -> usize {
    usize::from(isb & 0x0f) + usize::from(stars_core::startup::FIRST_STARBASE_SLOT)
}

/// A planet's production queue as the file's own items.
fn queue_items(planet: &Planet) -> Vec<QueueItem> {
    planet
        .queue
        .iter()
        .map(|entry| QueueItem {
            count: u16::try_from(entry.count).unwrap_or(0),
            item: entry.item,
            class: if entry.ship {
                QueueClass::Fleet
            } else {
                QueueClass::Planet
            },
            completion: u16::try_from(entry.completion).unwrap_or(0),
        })
        .collect()
}

/// The registration serial and machine fingerprint of an order file already on
/// disk, or zeros.
fn existing_registration(path: &Path) -> (i32, [u8; 11]) {
    let zero = (0, [0u8; 11]);
    let Ok(bytes) = std::fs::read(path) else {
        return zero;
    };
    let Ok(file) = StarsFile::decode(&bytes) else {
        return zero;
    };
    stars_formats::order_log(&file)
        .header
        .map_or(zero, |h| (h.serial_number, h.config))
}

/// A deterministic cipher salt for an order file, distinct from the ones the
/// state files use so the two do not share a keystream.
fn order_salt(game_id: u32, player: u8, turn: i16) -> u16 {
    let mixed = game_id.rotate_right(u32::from(player) % 32)
        ^ (u32::from(turn.unsigned_abs()) << 5)
        ^ 0x0051_7bd3;
    ((mixed ^ (mixed >> 13)) & 0x07FF) as u16
}

/// The object class a waypoint or transfer names for a planet (`grobj`).
const PLANET_CLASS: u8 = 1;
/// The object class for a fleet.
const FLEET_CLASS: u8 = 2;

/// What the player has picked out of the current game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    /// The selected planet's id.
    pub planet: Option<i16>,
    /// The selected fleet, as an index into [`GameState::fleets`].
    pub fleet: Option<usize>,
    /// Which of the two the scanner has in front — the original's `sel.grobj`,
    /// which is one thing rather than two.
    ///
    /// The scanner keeps both because a fleet in orbit is at a planet and the
    /// status bar wants the planet either way; this says which one clicking
    /// last landed on, and so which pane is showing.
    pub on_fleet: bool,
    /// The space object selected, when one is — the original's
    /// `sel.grobj == grobjThing`, which wins over both of the above.
    ///
    /// Only the right-click menu selects one: they take no part in the
    /// click-again cycle, which is planets and fleets only.
    pub thing: Option<ScanThing>,
    /// Which of the selected fleet's waypoints the map has in hand — the
    /// original's `sel.iwpAct`.
    ///
    /// It is what **Delete** removes and what a drag picks up, and the status
    /// bar names it `WP #n`. Waypoint 0 is where the fleet is; it can be the
    /// one in hand but it can never be deleted.
    pub waypoint: Option<usize>,
}

/// What the Find dialog found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindResult {
    /// A planet, by id.
    Planet(i16),
    /// A fleet, by index into [`GameState::fleets`].
    Fleet(usize),
}

/// The scanner's six views, named as the original's toolbar names them
/// (`idsNormalView` and the five after it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScanView {
    /// Planets by who holds them.
    #[default]
    Normal,
    /// What is on each planet's surface.
    SurfaceMineral,
    /// What is in the ground.
    MineralConcentration,
    /// How good each planet is for this race.
    PlanetValue,
    /// How many people live there.
    Population,
    /// The map with everybody's colours taken off.
    NoPlayerInfo,
}

impl SurveySubject {
    /// The fleet this is looking at, if it is looking at one.
    #[must_use]
    pub fn fleet_index(self) -> Option<usize> {
        match self {
            Self::Fleet(index) => Some(index),
            _ => None,
        }
    }
}

impl ScanView {
    /// Every view, in the order the toolbar has them.
    pub const ALL: [Self; 6] = [
        Self::Normal,
        Self::SurfaceMineral,
        Self::MineralConcentration,
        Self::PlanetValue,
        Self::Population,
        Self::NoPlayerInfo,
    ];

    /// The toolbar's own name for the view.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal View",
            Self::SurfaceMineral => "Surface Mineral View",
            Self::MineralConcentration => "Mineral Concentration View",
            Self::PlanetValue => "Planet Value View",
            Self::Population => "Population View",
            Self::NoPlayerInfo => "No Player Info View",
        }
    }
}

/// The scanner's overlays and filters, each a toolbar toggle of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanOverlays {
    /// `Planet Names Overlay`.
    pub names: bool,
    /// `Scanner Coverage Overlay`.
    pub scanner_coverage: bool,
    /// `Mine Fields Overlay`.
    pub minefields: bool,
    /// `Fleet Paths Overlay`.
    pub fleet_paths: bool,
    /// `Ship Counts Overlay`.
    pub ship_counts: bool,
    /// `Idle Fleets Filter`: show only the fleets with nothing to do.
    pub idle_fleets: bool,
    /// `Ship Design Filter`: count only the chosen designs, in this player's
    /// own fleets (`grbitScan & 0x200`).
    pub ship_design_filter: bool,
    /// `Enemy Ship Class Filter`: count only the chosen classes, in everybody
    /// else's fleets (`grbitScan & 0x800`).
    pub enemy_class_filter: bool,
    /// `Player Colors` (`grbitScan & 0x2000`).
    ///
    /// The one bit of `grbitScan` with no button on the toolbar: it is the
    /// **View menu's** own item, `0x98d`. It changes nothing on its own —
    /// toggling it only redraws the scanner when `grbitScan & 0x1400` is set,
    /// which is to say when planet names or ship counts are being drawn, since
    /// those are the only two things it colours.
    pub player_colours: bool,
}

/// What the survey pane is looking at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurveySubject {
    /// Nothing: the pane reads "Deep Space".
    DeepSpace,
    /// A planet, by id.
    Planet(i16),
    /// A fleet, by index into [`GameState::fleets`].
    Fleet(usize),
    /// A space object, which the scanner's right-click menu selects.
    Thing(ScanThing),
}

/// One of the survey pane's bars: a label, a reading, and where it sits in a
/// range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurveyBar {
    /// What is being measured.
    pub label: String,
    /// The reading, in the units the game shows.
    pub value: String,
    /// Where the marker goes.
    pub at: i32,
    /// The bottom of the band that matters — the race's habitable low, or
    /// zero for a mineral.
    pub low: i32,
    /// The top of it: the habitable high, or the mineral's concentration.
    pub high: i32,
    /// Whether the race is immune to this variable, in which case the whole
    /// bar is habitable.
    pub immune: bool,
    /// Where terraforming could move the marker to, when it could
    /// (`FCanTerraformLppl`). Environment bars only.
    pub terraform: Option<i32>,
    /// How far the bar reaches once this year's mining is counted. Mineral
    /// bars only, and never less than [`SurveyBar::at`]: the original draws
    /// this first in the dark shade and the surface stock over it in the
    /// bright one, so the difference is what mining will add.
    pub sum: i32,
}

/// The whole application, minus the drawing.
#[derive(Default)]
pub struct App {
    /// The currently loaded game, or `None` on the title screen.
    pub game: Option<GameState>,
    /// The universe the game is played in, when one is known: read from the
    /// `.xy` beside a save, or generated with a new game.
    pub universe: Option<Universe>,
    /// The New Game wizard's settings while it is open.
    pub setup: Option<NewGame>,
    /// Where it came from, for the title bar.
    pub path: Option<PathBuf>,
    /// The screen being shown.
    pub screen: Screen,
    /// What is selected.
    pub selection: Selection,
    /// The scanner's zoom, `-4..=4` (`iScanZoom`). Zero is life size.
    pub scan_zoom: i8,
    /// View (Toolbar), inverted so that the default is **shown**.
    ///
    /// `MANUAL.PDF` p. 2-9 offers it as a way to make room: "If the screen
    /// still seems too cramped try hiding the Toolbar using the menu item View
    /// (Toolbar). Most of the Toolbar functions are available" from the menus.
    pub toolbar_hidden: bool,
    /// View (Window Layout) — `iWindowLayout`, `0..=2`.
    pub window_layout: WindowLayout,
    /// Whether the Game Parameters window is open.
    pub game_parameters: bool,
    /// The race viewer, while it is open: whose race, and which of the six
    /// pages is showing.
    pub race_viewer: Option<(usize, usize)>,
    /// The Custom Race Wizard, while it is open.
    pub race_wizard: Option<RaceWizard>,
    /// Where the scanner's right-click menu was opened, in galaxy units, while
    /// it is up.
    pub scan_menu_at: Option<(i16, i16)>,
    /// Which of the planet pane's six tiles are open.
    ///
    /// The original keeps this as `fPopped` in each `rgtilePlanet` record,
    /// writes it to `stars.ini` and reflows the column whenever it changes.
    /// See [`crate::tiles`].
    pub open_tiles: [bool; 6],
    /// The same, for the seven the fleet pane shows instead (`rgtileShip`).
    pub open_ship_tiles: [bool; 7],
    /// `fNoHostNames`: whether the host dialog leaves the players' names out
    /// of its list and shows only how far each turn has got.
    pub no_host_names: bool,
    /// Which of the Game Parameters window's three pages is showing.
    ///
    /// The original is the Advanced Game wizard read-only, walked with
    /// `< Back` and `Next >`; page 3 is the winning conditions, which is where
    /// `MANUAL.PDF` p. 2-3 sends the player.
    pub parameters_page: usize,
    /// The pop-up summary and where its bottom-right corner sits, in screen
    /// pixels, while one is up.
    ///
    /// The original keeps one at a time in `GlobalPD.grPopup` and destroys it
    /// on the next button-up, so this is a press-and-hold. See
    /// [`crate::popup`].
    pub popup: Option<(crate::popup::Popup, (f32, f32))>,
    /// Which fleet the pane's last tile is showing, as owner and fleet id.
    ///
    /// Both halves are needed: a fleet id is the player's own numbering, so
    /// two players each have a fleet 1.
    pub pane_fleet_chosen: Option<(i16, u16)>,
    /// The Battle Plans dialog, while it is open.
    pub battle_plans: Option<BattlePlans>,
    /// The Change Password dialog, while it is open.
    pub password_dialog: Option<PasswordDialog>,
    /// The prompt that asks for a turn password, while a save is waiting on it.
    pub password_prompt: Option<PasswordPrompt>,
    /// Whether the Host Mode dialog is open.
    pub host_mode: bool,
    /// Whether host mode is watching for turns and generating on its own.
    pub auto_generate: bool,
    /// When the watch last looked, on the frontend's clock.
    pub auto_generate_checked: Option<f64>,
    /// The salt last accepted at that prompt (`lSaltLast`), so the same
    /// password is asked for once a session and not once a file.
    pub password_given: Option<u32>,
    /// How many passwords have been got wrong (`vcPasswordFailures`). It counts
    /// for the session and is never reset, which is what slows a guesser down.
    pub password_failures: u32,
    /// The clock reading the prompt will take another attempt at, while a wait
    /// is running.
    pub password_retry_at: Option<f64>,
    /// `stars.ini`'s `[Misc] DefaultPassword` (`vszDefPass`), if the frontend
    /// found one: a password kept there is offered before the prompt is.
    pub default_password: String,
    /// Whether opening a guarded turn should ask for its password.
    ///
    /// A frontend with somebody at the keyboard sets this; anything else
    /// leaves it clear and reads the file straight through. The original draws
    /// the same line with `ini.fValidate` — in batch mode `FCheckPassword`
    /// refuses instead of asking, because there is nobody to answer — except
    /// that this reads the file rather than refusing it. That is the honest
    /// choice here: the salt is a gate on the interface and never encrypted
    /// anything, so a tool re-encoding a save is not pretending to have got
    /// past a protection. What it must not do is pretend to be the player, and
    /// nothing here does: the prompt is what a player sees.
    pub prompt_for_password: bool,
    /// Whether the Find box is open (View (Find), Ctrl+F).
    pub find_open: bool,
    /// Which of the scanner's six views is showing.
    pub scan_view: ScanView,
    /// The scanner's overlays and filters.
    pub scan_overlays: ScanOverlays,
    /// Which of this player's sixteen design slots the Ship Design filter
    /// counts (`grbitScanShip`), a bit each.
    pub scan_design_filter: u16,
    /// Which of the eight ship classes the Enemy Ship Class filter counts
    /// (`grbitScanEShip`), a bit each.
    pub scan_class_filter: u8,
    /// Whose minefields the overlay draws (`grbitScanMines`): a bit each for
    /// yours, friends', neutrals' and enemies'. Unlike the two ship filters
    /// this starts **full** — `stars.ini` defaults it to `0xf`.
    pub scan_minefield_filter: u8,
    /// What the toolbar's coverage combo holds, in percent
    /// (`vpctRadarView`). The overlay is drawn as though every scanner were
    /// only this effective, which is how a player sees what a cloaked ship
    /// would get away with — `MANUAL.PDF` p. 5-13.
    pub scan_coverage_pct: u8,
    /// The toolbar's tooltip, and its timing — see [`crate::toolbar::Tooltip`].
    pub tooltip: crate::toolbar::Tooltip,
    /// `Add Way Points Mode`: whether clicking the map gives the selected
    /// fleet orders instead of selecting what is under the pointer.
    pub add_waypoints: bool,
    /// Which waypoint a drag is moving, while one is under way.
    pub dragging_waypoint: Option<usize>,
    /// Where that waypoint was before the drag picked it up.
    ///
    /// The original rubber-bands the leg and only writes the waypoint on
    /// release, so it still has the old point to put back when the drop turns
    /// out to be a delete (`FHandleWayPointDrag`, `1058:8176`). This engine
    /// moves the waypoint as the drag goes, so it keeps the old point here.
    pub dragging_waypoint_from: Option<stars_core::movement::Point>,
    /// A waypoint whose drag ended on one of its neighbours, waiting for the
    /// player to confirm that it should go.
    pub waypoint_delete: Option<usize>,
    /// The scale the mineral graph and the mineral views are drawn against
    /// (`cMinGrafMax`), which ships at [`MINERAL_GRAPH_MAX`].
    ///
    /// One number serves both — `MANUAL.PDF` p. 5-13: "rescaling that graph
    /// rescales the bars in this view" — and the graph's own menu is what
    /// changes it. See [`App::mineral_scale_menu`].
    pub mineral_scale: i32,
    /// Where the mineral graph's scale menu is showing, while it is.
    pub mineral_menu: Option<egui::Pos2>,
    /// A design the **pop-up** is showing, which is not the designer being
    /// open on it.
    ///
    /// `grPopupShdef` draws the designer's own panel — `DrawSlotDlg` and
    /// `DrawBuildSelHull`, the same two the dialog draws — over whatever
    /// raised it, read-only. Setting this makes every `designer_*` accessor
    /// answer about that design instead, which is how the panel gets drawn
    /// without opening the dialog.
    pub designer_peek: Option<ShipDesign>,
    /// The four report windows' state — which column each sorts on, which
    /// columns it shows, and which is open. See [`crate::report`].
    pub reports: crate::report::Reports,
    /// The column menu a header click opened: the report, the column, and
    /// where the click was.
    pub report_menu: Option<(crate::report::Report, usize, egui::Pos2)>,
    /// The four saved cargo orders the blue diamond offers (`vrgZip`).
    pub zip_orders: [ZipOrder; 4],
    /// Which slot the Customize dialog is showing, while it is open.
    pub zip_dialog: Option<usize>,
    /// The notice `Hide` puts up the first time, while it is showing.
    pub tutor_notice: Option<String>,
    /// Whether the tutor's `Panic!` dialog is up.
    pub tutor_panic: bool,
    /// The tutorial, while it is running — the original's `tutor` global.
    pub tutor: Option<crate::tutorial::Tutor>,
    /// The measuring tape, while it is stretched: where it started and where
    /// its far end is now.
    pub measuring: Option<(stars_core::movement::Point, stars_core::movement::Point)>,
    /// What the Find box holds.
    pub find_text: String,
    /// The Ship and Starbase Designer, while it is open (`hwndSlotDlg`).
    pub designer: Option<Designer>,
    /// The Production dialog, while it is open (`hwndProdDlg`).
    pub production: Option<Production>,
    /// The Research dialog, while it is open.
    pub research_dialog: Option<ResearchDialog>,
    /// The Technology Browser, while it is open (`hwndBrowser`). Modeless in
    /// the original, so it sits alongside whatever else is on screen.
    pub browser: Option<Browser>,
    /// The Score sheet, while it is open (`hwndScoreXDlg`, F10).
    pub score_sheet: Option<ScoreSheet>,
    /// The Player Relations dialog, while it is open: which player its
    /// listbox has selected.
    pub relations_dialog: Option<usize>,
    /// The game's own pictures, when a copy of the original executable has
    /// been found to read them out of. Everything that draws one falls back to
    /// drawing without it, so this being `None` costs nothing but the
    /// pictures.
    pub art: Option<crate::art::Art>,
    /// What the Score sheet was last set to.
    ///
    /// The original keeps the face and the timeline's figure in `gd`, which
    /// outlives the dialog, so closing and reopening the sheet comes back to
    /// the same view.
    score_settings: ScoreSheet,
    /// The player's production templates, slots 1..3 — the `<Customize>`
    /// dialog's, which the original keeps in `stars.ini`. Slot 0 is filled in
    /// from the player's own default queue; see [`App::production_templates`].
    templates: Vec<stars_formats::ProductionTemplate>,
    /// The `<Customize>` dialog while it is open: which slot it is on, and the
    /// templates as they were when it opened.
    ///
    /// The original copies the whole `ZIPPRODQ[4]` array before opening the
    /// dialog and puts it back if the dialog is cancelled, so **Cancel undoes
    /// an Import or a Delete** as well as a rename.
    customize: Option<(usize, Vec<stars_formats::ProductionTemplate>)>,
    /// Which message the pane is showing — the original's `iMsgCur`.
    ///
    /// `-1` means the pane is at the start of the list and showing nothing,
    /// which is a state the original has and uses: it is what the pane sits in
    /// when every message of the year is filtered.
    pub message_index: i32,
    /// `fViewFilteredMsg`: whether the messages the player has silenced are
    /// shown anyway. Kept for the session, not saved.
    pub view_filtered: bool,
    /// Battle recordings found in the loaded file.
    pub battles: Vec<BattleRecord>,
    /// The battle being played, if any.
    pub vcr: Option<Vcr>,
    /// Whether playback is running.
    pub playing: bool,
    /// The most recent error, for the frontend to show.
    pub error: Option<String>,
    /// What the last generated turn did, for the frontend to show.
    pub last_turn: Option<TurnSummary>,
    /// The file exactly as it was read, so saving can put back everything the
    /// simulation does not model.
    file: Option<StarsFile>,
    /// Whether anything has been changed since it was loaded.
    pub dirty: bool,
    /// Planets whose queue the player has edited. Only these are rewritten.
    edited: std::collections::BTreeSet<i16>,
    /// Fleets the player has renamed, as `(owner, fleet number)`. Only these
    /// have their name block rewritten.
    renamed: std::collections::BTreeSet<(i16, u16)>,
    /// Fleets whose battle plan or repeat-orders flag the player changed. Both
    /// live inside the fleet block, so only these have theirs patched.
    fleet_edits: std::collections::BTreeSet<(i16, u16)>,
    /// The warp the fleet screen last used, remembered between orders.
    pub warp: u8,
    /// The name the fleet screen's rename box holds.
    pub fleet_name: String,
    /// The order log for this turn, in the order the player made the moves.
    ///
    /// A Stars! order log is **not** a list of intentions: it records what the
    /// player's client already did, so the host can replay it and stay in step.
    /// Every entry here has already been applied to the loaded game; nothing
    /// re-applies them. [`App::order_file`] writes them out as a `.xN`.
    ///
    /// Cargo transfers and fleet orders are appended as they happen, because
    /// they are events. The production queues and the research setting are
    /// **state**, and their log records replace whatever the host has, so they
    /// are added once at the end from what was actually changed.
    pub orders: Vec<stars_formats::LogRecord>,
    /// Whether the research setting was changed this turn.
    research_edited: bool,
    /// Whether anything in the local player's own block was changed.
    player_edited: bool,
    /// Whether the host's password was changed. It lives in a block of its own
    /// rather than in a player block, so it is tracked separately.
    host_password_edited: bool,
    /// Whether the local player's battle plans were changed. They live in
    /// their own blocks, so those are rewritten rather than patched.
    battle_plans_edited: bool,
}

/// A generated turn, reduced to what a player wants to be told.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TurnSummary {
    /// The year it advanced to.
    pub year: i32,
    /// Planets that mined something.
    pub mined: usize,
    /// Planets whose population changed, and by how much in total.
    pub population: i32,
    /// Ships finished.
    pub ships: i32,
    /// Technology gained, as `(player, field count)`.
    pub breakthroughs: Vec<(usize, usize)>,
    /// Pipeline steps the engine did not perform, named.
    pub skipped: Vec<String>,
    /// Orders replayed from other players' `.xN` files, as
    /// `(player, operations)`.
    pub replayed: Vec<(usize, usize)>,
    /// What the year has to tell this session's player: the message id and
    /// the engine's own words for it. See [`stars_core::message`].
    ///
    /// Every message is kept, filtered or not — the filter is a reading
    /// choice, so it is applied when the list is shown rather than when it is
    /// built.
    pub messages: Vec<(u16, String)>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("path", &self.path)
            .field("screen", &self.screen)
            .field("selection", &self.selection)
            .field("battles", &self.battles.len())
            .field("playing", &self.playing)
            .finish_non_exhaustive()
    }
}

impl App {
    /// Create an empty application (no game loaded).
    #[must_use]
    pub fn new() -> Self {
        Self {
            warp: 7,
            // The state the original starts in when `stars.ini` says nothing:
            // `grbitScan` defaults to 0xe0 — the Normal view with scanner
            // coverage, minefields and fleet paths already on — the coverage
            // to 100%, and the minefield filter to all four.
            scan_coverage_pct: 100,
            tooltip: crate::toolbar::Tooltip::default(),
            // Every tile of the planet pane ships open: bit 7 of each
            // `rgtilePlanet` record's packed word is set.
            open_tiles: [true; 6],
            open_ship_tiles: [true; 7],
            scan_minefield_filter: 0xf,
            mineral_scale: MINERAL_GRAPH_MAX,
            mineral_menu: None,
            scan_overlays: ScanOverlays {
                scanner_coverage: true,
                minefields: true,
                fleet_paths: true,
                ..ScanOverlays::default()
            },
            ..Self::default()
        }
    }

    /// Human-readable one-line status used by the frontends' title bars.
    #[must_use]
    pub fn status_line(&self) -> String {
        match &self.game {
            Some(state) => {
                let name = self
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .or_else(|| {
                        self.universe
                            .as_ref()
                            .and_then(|u| u.game().ok())
                            .map(|g| g.name)
                    })
                    .unwrap_or_default();
                format!("Stars! — {name} — year {}", state.year())
            }
            None => "Stars! — no game loaded".to_string(),
        }
    }

    /// Load a save file, and the universe beside it if there is one.
    ///
    /// Planet coordinates live only in the `.xy`, so a game loaded without one
    /// has planets with no position — the galaxy map says so rather than
    /// drawing them at the origin.
    ///
    /// # Errors
    /// Returns a message suitable for showing to the player.
    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let loaded = self.read_save(path)?;
        // `FCheckPassword` (`1040:58d8`): a turn whose player put a password on
        // it is not opened until that password is given. The original fails the
        // load outright when it is not (`file.c`, `goto LError`), so the game
        // stays parked here until the prompt is answered.
        let salt = loaded.password;
        if self.prompt_for_password && self.password_needed(salt) {
            self.password_prompt = Some(PasswordPrompt {
                salt,
                typed: String::new(),
                error: None,
                pending: Box::new(loaded),
            });
            return Ok(());
        }
        self.install(loaded);
        Ok(())
    }

    /// Read a save and everything that goes with it, without installing it.
    fn read_save(&mut self, path: &Path) -> Result<Loaded, String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let file = StarsFile::decode(&bytes)
            .map_err(|e| format!("cannot decode {}: {e}", path.display()))?;

        let (mut state, _) = GameState::from_file(&file);
        let universe = find_universe(path);
        if let Some(universe) = &universe {
            state.apply_universe(universe);
        }
        // The Score sheet's timeline lives in the `.hN` beside the save. The
        // player file is read again afterwards so that its own row — this
        // year's, which the history file does not have yet — wins the tie.
        if let Some(history) = find_history(path) {
            state.read_scores(&history);
            state.read_scores(&file);
        }

        let header = &file.latest_segment().header;
        let layout = ActionLayout::for_version(header.version_major, header.version_minor);
        let battles = battle_records_in_with(file.segment_blocks(file.latest_segment()), layout);

        // Whose password guards this file. A player's file names its player in
        // the header and that player's salt is the one to ask for
        // (`lSaltCur = rgplr[iPlayer].lSalt`); a host file names none and is
        // guarded by the host's own salt, which it carries in a type-36 block
        // after the player blocks.
        let player = usize::from(header.player);
        let password = if player < stars_core::newgame::MAX_PLAYERS {
            state.players.get(player).map_or(0, |p| p.password)
        } else {
            state.host_password
        };

        Ok(Loaded {
            state,
            universe,
            battles,
            file,
            path: path.to_path_buf(),
            password,
        })
    }

    /// Make a loaded save the current game.
    fn install(&mut self, loaded: Loaded) {
        let Loaded {
            state,
            universe,
            battles,
            file,
            path,
            password: _,
        } = loaded;
        self.battles = battles;

        self.selection = Selection {
            planet: state.planets.first().map(|p| p.id),
            fleet: (!state.fleets.is_empty()).then_some(0),
            on_fleet: false,
            thing: None,
            waypoint: None,
        };
        self.vcr = None;
        self.playing = false;
        self.game = Some(state);
        self.universe = universe;
        self.setup = None;
        self.path = Some(path);
        self.file = Some(file);
        self.dirty = false;
        self.edited.clear();
        self.renamed.clear();
        self.fleet_edits.clear();
        self.player_edited = false;
        self.host_password_edited = false;
        self.battle_plans_edited = false;
        self.orders.clear();
        self.error = None;
        // `ReadPlayerMessages` (`msg.c`) ends by moving the pane to the first
        // message the player has not filtered, which leaves it before the
        // start when every one of them is.
        self.view_filtered = false;
        self.show_first_message();
    }

    /// Write the game back, as bytes.
    ///
    /// Saving is **not** re-encoding the simulation. The overwhelming majority
    /// of a save file is data this project models partially or not at all, and
    /// re-deriving it would lose whatever was not understood. So the file is
    /// kept exactly as it was read and only the blocks the player actually
    /// changed are replaced — today that is the production queues.
    ///
    /// A queue is a type-28 block and its planet is positional: it belongs to
    /// the planet block it follows, and in every one of the 26,938 queue blocks
    /// in this repository's fixtures it sits **immediately** after. A planet
    /// that gained a queue therefore gets one inserted in that position, and a
    /// planet whose queue was emptied has its block dropped.
    ///
    /// Two boundaries matter and are easy to get wrong. Only queues the player
    /// **actually edited** are rewritten — a queue re-encoded from the
    /// simulation's own model would come back subtly different wherever that
    /// model is a simplification, and there is no reason to touch it. And only
    /// the **latest segment** is rewritten: a file can hold several turns, the
    /// game state is read from the last of them, and writing the current queues
    /// over an earlier turn's would corrupt the history the file is keeping.
    ///
    /// # Errors
    /// Returns a message suitable for showing to the player.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let file = self.file.as_ref().ok_or("no file is open")?;
        let game = self.game.as_ref().ok_or("no game is loaded")?;

        let source = &file.blocks;
        let latest = file.latest_segment();
        let (first, last) = (latest.start, latest.end);
        let mut blocks: Vec<Block> = Vec::with_capacity(source.len());
        let mut pending: Option<u16> = None;
        // The name block of a renamed fleet, waiting for the end of that
        // fleet's run of waypoints. An empty vector means the name was
        // cleared, so the block should go rather than be replaced.
        let mut pending_name: Option<Vec<u8>> = None;
        // The player's battle plans are written as a run of blocks. Editing
        // one can change how many there are, so the whole run is replaced at
        // the first of them rather than patched block by block.
        let mut wrote_battle_plans = false;
        // A host password only belongs in a host file, and it is only written
        // when it was changed here: an untouched file keeps whatever it had.
        let host_file = latest.header.file_type == stars_formats::FileType::Host;
        let rewrite_host_password = host_file && self.host_password_edited;
        let had_salt_block = source[first..last].iter().any(|b| b.type_id == 36);
        let insert_host_password =
            rewrite_host_password && !had_salt_block && game.host_password != 0;

        for (index, block) in source.iter().enumerate() {
            if index < first || index >= last {
                // An earlier turn the file is keeping: leave it alone.
                blocks.push(block.clone());
                continue;
            }
            // A fleet's name follows its waypoints. When the run ends without
            // one, this is where a new name is inserted.
            let in_fleet_run = matches!(block.type_id, 19 | 20 | stars_formats::FLEET_NAME_BLOCK);
            if !in_fleet_run {
                if let Some(name) = pending_name.take() {
                    if !name.is_empty() {
                        blocks.push(
                            Block::new(stars_formats::FLEET_NAME_BLOCK, name)
                                .map_err(|e| format!("cannot write a fleet name: {e}"))?,
                        );
                    }
                }
            }
            if block.type_id == stars_formats::FLEET_NAME_BLOCK {
                match pending_name.take() {
                    // A renamed fleet's block is replaced, or dropped when the
                    // name was cleared.
                    Some(name) => {
                        if !name.is_empty() {
                            blocks.push(
                                Block::new(stars_formats::FLEET_NAME_BLOCK, name)
                                    .map_err(|e| format!("cannot write a fleet name: {e}"))?,
                            );
                        }
                    }
                    None => blocks.push(block.clone()),
                }
                continue;
            }
            if block.type_id == 30 && self.battle_plans_edited {
                let me = u8::try_from(self.local_player()).unwrap_or(0) & 0x0F;
                let mine = stars_formats::BattlePlanRecord::from_payload(&block.data)
                    .is_ok_and(|p| p.race_id == me);
                if mine {
                    if !wrote_battle_plans {
                        wrote_battle_plans = true;
                        for (slot, plan) in game
                            .players
                            .get(self.local_player())
                            .map(|p| p.battle_plans.as_slice())
                            .unwrap_or_default()
                            .iter()
                            .enumerate()
                        {
                            let mut plan = plan.clone();
                            plan.race_id = me;
                            plan.plan_id = u8::try_from(slot).unwrap_or(0) & 0x0F;
                            let data = plan
                                .encode()
                                .map_err(|e| format!("cannot write a battle plan: {e}"))?;
                            blocks.push(
                                Block::new(30, data)
                                    .map_err(|e| format!("cannot write a battle plan: {e}"))?,
                            );
                        }
                    }
                    continue;
                }
            }
            // The host's password is a type-36 block straight after the last
            // player block, and only a host file has one. Changing it replaces
            // that block, clearing it drops the block, and setting one on a
            // file that never had a password puts it where the game would.
            if block.type_id == 36 && rewrite_host_password {
                if game.host_password != 0 {
                    blocks.push(
                        Block::new(36, game.host_password.to_le_bytes().to_vec())
                            .map_err(|e| format!("cannot write the host password: {e}"))?,
                    );
                }
                continue;
            }
            if block.type_id == 6 {
                match self
                    .player_edited
                    .then(|| self.patched_player(game, &block.data))
                    .flatten()
                {
                    Some(patched) => blocks.push(
                        Block::new(6, patched)
                            .map_err(|e| format!("cannot write a player: {e}"))?,
                    ),
                    None => blocks.push(block.clone()),
                }
                let last_player = !source.get(index + 1).is_some_and(|b| b.type_id == 6);
                if last_player && insert_host_password {
                    blocks.push(
                        Block::new(36, game.host_password.to_le_bytes().to_vec())
                            .map_err(|e| format!("cannot write the host password: {e}"))?,
                    );
                }
                continue;
            }
            if matches!(block.type_id, 16..=18) {
                pending_name = self.fleet_name_block(game, &block.data, block.type_id);
                if let Some(patched) = self.patched_fleet(game, &block.data, block.type_id) {
                    blocks.push(
                        Block::new(block.type_id, patched)
                            .map_err(|e| format!("cannot write a fleet: {e}"))?,
                    );
                    continue;
                }
            }
            match block.block_type() {
                BlockType::Planet => {
                    blocks.push(block.clone());
                    pending = PlanetRecord::decode(&block.data, block.type_id).map(|p| p.id);
                    // A planet that has gained a queue needs a block making for
                    // it, in the position the game puts one.
                    let next_is_queue = source
                        .get(index + 1)
                        .is_some_and(|b| b.block_type() == BlockType::ProductionQueue);
                    if !next_is_queue {
                        if let Some(encoded) = pending
                            .filter(|id| self.was_edited(*id))
                            .and_then(|id| self.encode_queue(game, id))
                        {
                            if !encoded.is_empty() {
                                blocks.push(
                                    Block::new(BlockType::ProductionQueue.id(), encoded)
                                        .map_err(|e| format!("cannot write a queue: {e}"))?,
                                );
                            }
                        }
                        pending = None;
                    }
                }
                BlockType::ProductionQueue => {
                    let encoded = pending
                        .take()
                        .filter(|id| self.was_edited(*id))
                        .and_then(|id| self.encode_queue(game, id))
                        .unwrap_or_else(|| block.data.clone());
                    // An emptied queue loses its block rather than keeping an
                    // empty one, which is what a file with no queue looks like.
                    if !encoded.is_empty() {
                        blocks.push(
                            Block::new(BlockType::ProductionQueue.id(), encoded)
                                .map_err(|e| format!("cannot write a queue: {e}"))?,
                        );
                    }
                }
                BlockType::PartialPlanet | BlockType::MinimalPlanet => {
                    pending = None;
                    blocks.push(block.clone());
                }
                _ => blocks.push(block.clone()),
            }
        }

        // A renamed fleet that was the last thing in the file.
        if let Some(name) = pending_name.take() {
            if !name.is_empty() {
                blocks.push(
                    Block::new(stars_formats::FLEET_NAME_BLOCK, name)
                        .map_err(|e| format!("cannot write a fleet name: {e}"))?,
                );
            }
        }

        let mut out = file.clone();
        out.blocks = blocks;
        out.encode()
            .map_err(|e| format!("cannot write the file: {e}"))
    }

    /// The local player's block with the settings they can change written back
    /// into it, or `None` for anyone else's block.
    ///
    /// Decoded and re-encoded rather than rebuilt, so the whole fixed region —
    /// the home planet, the player rank, the four bytes nothing has identified
    /// — survives untouched.
    fn patched_player(&self, game: &GameState, data: &[u8]) -> Option<Vec<u8>> {
        let mut record = stars_formats::PlayerRecord::from_payload(data).ok()?;
        let index = usize::from(record.player_number);
        if index != self.local_player() {
            return None;
        }
        let player = game.players.get(index)?;
        record.default_queue = Some(player.default_queue.clone());
        if let Some(research) = record.research.as_mut() {
            research.budget_pct = player.research_pct;
        }
        record.player_relations = player.relations.clone();
        record.password = Some(player.password);
        record.encode().ok()
    }

    /// A fleet block with the two settings the player can change written back
    /// into it, or `None` if this fleet was not changed.
    ///
    /// The block is **decoded and re-encoded** rather than rebuilt from the
    /// simulation's model, so everything the model does not carry — the damage
    /// table, the flag bits nothing has identified — survives untouched. Only
    /// the battle plan and the repeat-orders flag are overwritten.
    fn patched_fleet(&self, game: &GameState, data: &[u8], type_id: u8) -> Option<Vec<u8>> {
        let mut record = stars_formats::FleetRecord::decode(data, type_id)?;
        let owner = i16::from(record.owner);
        if !self.fleet_edits.contains(&(owner, record.id)) {
            return None;
        }
        let fleet = game
            .fleets
            .iter()
            .find(|f| f.owner == owner && f.id == record.id)?;
        record.battle_plan = Some(fleet.battle_plan);
        record.repeat_orders = fleet.repeat_orders;
        Some(record.encode(type_id))
    }

    /// The name block a fleet block's fleet should carry, if the player renamed
    /// it this turn.
    ///
    /// `Some(empty)` means the name was cleared and the block should go.
    fn fleet_name_block(&self, game: &GameState, data: &[u8], type_id: u8) -> Option<Vec<u8>> {
        let record = stars_formats::FleetRecord::decode(data, type_id)?;
        let owner = i16::from(record.owner);
        if !self.renamed.contains(&(owner, record.id)) {
            return None;
        }
        let fleet = game
            .fleets
            .iter()
            .find(|f| f.owner == owner && f.id == record.id)?;
        Some(match fleet.name.as_ref().filter(|n| !n.is_empty()) {
            Some(name) => stars_formats::encode_user_string(name),
            None => Vec::new(),
        })
    }

    /// Create a brand-new game and make it the loaded one.
    ///
    /// The universe, the homeworlds and the starting fleets all come from
    /// [`stars_core::newgame`]; nothing is read from disk. The generator is
    /// seeded from the game id, so the same settings and id give the same
    /// universe from this engine — though not the one the original would have
    /// produced, for the reason that module's docs give.
    ///
    /// The result has no file behind it, so [`App::save`] refuses until it is
    /// given one; [`App::save_universe`] can write the `.xy`.
    ///
    /// # Errors
    /// Returns a message suitable for showing to the player.
    pub fn new_game(&mut self, config: &NewGame) -> Result<(), String> {
        let seed = config.id;
        self.new_game_seeded(config, seed)
    }

    /// The same, with the generator seeded from something other than the
    /// game's id.
    ///
    /// Almost every game seeds from its own `lid`, but the tutorial does
    /// not: `CreateTutorWorld` (`1078:5e5e`) calls `Randomize` with a fixed
    /// `0x499602d2` and gives the game an unrelated id.
    ///
    /// # Errors
    ///
    /// A message suitable for showing to the player.
    pub fn new_game_seeded(&mut self, config: &NewGame, seed: u32) -> Result<(), String> {
        let mut rng = stars_core::rng::Rng::randomize(seed);
        let Created { state, universe } =
            stars_core::newgame::generate(config, &mut rng).map_err(|e| e.to_string())?;

        self.selection = Selection {
            planet: state
                .planets
                .iter()
                .find(|p| p.owner == Some(0))
                .or_else(|| state.planets.first())
                .map(|p| p.id),
            fleet: (!state.fleets.is_empty()).then_some(0),
            on_fleet: false,
            thing: None,
            waypoint: None,
        };
        self.game = Some(state);
        self.universe = Some(universe);
        self.setup = None;
        self.battles.clear();
        self.vcr = None;
        self.playing = false;
        self.file = None;
        self.path = None;
        self.dirty = false;
        self.edited.clear();
        self.renamed.clear();
        self.fleet_edits.clear();
        self.player_edited = false;
        self.host_password_edited = false;
        self.battle_plans_edited = false;
        self.orders.clear();
        self.error = None;
        self.last_turn = None;
        self.screen = Screen::Galaxy;
        Ok(())
    }

    /// Write a generated game to disk as a complete set of Stars! files.
    ///
    /// `path` names the host file; the universe and one turn file per player
    /// are written beside it under the same stem, so choosing `Kestrel.hst`
    /// produces `Kestrel.xy`, `Kestrel.hst` and `Kestrel.m1`, `.m2`, …
    ///
    /// The game is then **re-opened from the host file**, so what is on screen
    /// afterwards is what is on disk, and further saves go through the ordinary
    /// edit-preserving path rather than rewriting everything.
    ///
    /// # Errors
    /// Returns a message suitable for showing to the player.
    pub fn save_new_game(&mut self, path: &Path) -> Result<Vec<PathBuf>, String> {
        let game = self.game.as_ref().ok_or("no game is loaded")?;
        let universe = self
            .universe
            .as_ref()
            .ok_or("this game has no universe to write")?;

        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .ok_or("the file name has no stem to build the other files from")?;

        let mut written = Vec::new();
        let write = |name: String, bytes: Vec<u8>| -> Result<PathBuf, String> {
            let target = directory.join(name);
            std::fs::write(&target, bytes)
                .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
            Ok(target)
        };

        let bytes = universe
            .encode()
            .map_err(|e| format!("cannot encode the universe: {e}"))?;
        written.push(write(format!("{stem}.xy"), bytes)?);

        let bytes = stars_core::save::host_file(game)
            .map_err(|e| format!("cannot build the host file: {e}"))?;
        let host = write(format!("{stem}.hst"), bytes)?;
        written.push(host.clone());

        for player in 0..game.players.len() {
            let bytes = stars_core::save::player_file(game, player)
                .map_err(|e| format!("cannot build player {player}'s file: {e}"))?;
            written.push(write(format!("{stem}.m{}", player + 1), bytes)?);
        }

        if !self.orders.is_empty() || self.research_edited || !self.edited.is_empty() {
            written.push(self.save_orders(&host)?);
        }

        self.open(&host)?;
        Ok(written)
    }

    /// Whether the loaded game can be written back to a save file.
    ///
    /// A game opened from disk can: saving replaces the blocks the player
    /// edited and leaves the rest of the file alone. A **generated** game has
    /// no file to edit, and is written instead by [`App::save_new_game`],
    /// which produces the whole set of files from scratch.
    #[must_use]
    pub fn can_save_game(&self) -> bool {
        self.file.is_some()
    }

    /// Write the universe of the loaded game as a `.xy` file.
    ///
    /// # Errors
    /// Returns a message suitable for showing to the player.
    pub fn save_universe(&self, path: &Path) -> Result<(), String> {
        let universe = self
            .universe
            .as_ref()
            .ok_or("this game has no universe file")?;
        let bytes = universe
            .encode()
            .map_err(|e| format!("cannot encode the universe: {e}"))?;
        std::fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    /// The order files other players have submitted, beside the open game.
    ///
    /// Our own player's log is deliberately skipped: this session applied every
    /// order as it was made, which is exactly what the log it wrote records, so
    /// replaying it would apply each cargo transfer twice.
    #[must_use]
    pub fn submitted_orders(&self) -> Vec<(usize, stars_formats::OrderLog)> {
        let Some(path) = self.path.as_ref() else {
            return Vec::new();
        };
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            return Vec::new();
        };
        let local = self.local_player();

        (0..game.players.len())
            .filter(|player| *player != local)
            .filter_map(|player| {
                let file = directory.join(format!("{stem}.x{}", player + 1));
                let bytes = std::fs::read(file).ok()?;
                let decoded = StarsFile::decode(&bytes).ok()?;
                // Only a file for this game and this year is a submission for
                // the turn about to be generated.
                let header = &decoded.latest_segment().header;
                if header.game_id != game.seed || i16::try_from(header.turn).ok() != Some(game.turn)
                {
                    return None;
                }
                Some((player, stars_formats::order_log(&decoded)))
            })
            .collect()
    }

    /// Which player's orders this session is recording.
    ///
    /// A turn file names its player in the header, and that is whose orders a
    /// `.xN` carries. A host file names none, so the first human player is
    /// taken — which is the local player of a game this project generated.
    #[must_use]
    pub fn local_player(&self) -> usize {
        if let Some(file) = &self.file {
            let player = usize::from(file.latest_segment().header.player);
            if player < stars_core::newgame::MAX_PLAYERS {
                return player;
            }
        }
        self.game
            .as_ref()
            .and_then(|game| {
                game.players
                    .iter()
                    .position(|p| matches!(p.control, stars_core::ai::Control::Human))
            })
            .unwrap_or(0)
    }

    /// The order log for this turn, ready to write.
    ///
    /// The events the player caused — cargo transfers and fleet orders — are
    /// already in [`Self::orders`] in the order they happened. Two things are
    /// **state** rather than events, and their log records replace whatever the
    /// host holds, so they go on the end: one production-queue record per
    /// planet whose queue was edited, and one research record if the setting
    /// was changed.
    ///
    /// `serial` and `config` identify the copy of Stars! that wrote the file.
    /// This project has no registration, so it writes zeros unless a caller
    /// passes values it read from a file the real client produced — see
    /// [`stars_formats::OrderLog::new`].
    #[must_use]
    pub fn order_log(&self, serial: i32, config: [u8; 11]) -> stars_formats::OrderLog {
        use stars_formats::{LogRecord, ProductionQueueRecord, ResearchOrder};

        let mut log = stars_formats::OrderLog::new(serial, config);
        log.records.clone_from(&self.orders);
        let Some(game) = self.game.as_ref() else {
            return log;
        };

        for id in &self.edited {
            let Some(planet) = game.planets.iter().find(|p| p.id == *id) else {
                continue;
            };
            let record = ProductionQueueRecord {
                planet_id: u16::try_from(*id).ok(),
                items: queue_items(planet),
            };
            log.records.push(LogRecord::production_queue(&record));
        }

        if self.research_edited {
            if let Some(player) = game.players.get(self.local_player()) {
                log.records
                    .push(LogRecord::research_settings(ResearchOrder {
                        pct_resources: player.research_pct,
                        current_field: u8::try_from(player.research.current_field).unwrap_or(0),
                        next_field: match player.research.next_field {
                            stars_core::research::NextField::Field(f) => {
                                u8::try_from(f).unwrap_or(0)
                            }
                            stars_core::research::NextField::Same => 6,
                            stars_core::research::NextField::Lowest => 7,
                        },
                    }));
            }
        }
        log
    }

    /// Write this turn's orders as a `.xN` file.
    ///
    /// If a `.xN` for the same player already sits beside `path`, its
    /// registration serial and machine fingerprint are carried over, so a file
    /// this project writes stays consistent with the ones the real client wrote
    /// for that player. Otherwise both are zero, which is what an unregistered
    /// copy carries.
    ///
    /// # Errors
    /// Returns a message suitable for showing to the player.
    pub fn save_orders(&self, path: &Path) -> Result<PathBuf, String> {
        let game = self.game.as_ref().ok_or("no game is loaded")?;
        let player = self.local_player();
        let number = u8::try_from(player).map_err(|_| "player index out of range".to_string())?;

        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .ok_or("the file name has no stem to build the order file from")?;
        let target = directory.join(format!("{stem}.x{}", player + 1));

        let (serial, config) = existing_registration(&target);
        let log = self.order_log(serial, config);
        let header = stars_formats::FileHeader {
            flag_done: true,
            ..stars_formats::FileHeader::new(
                game.seed,
                stars_formats::FileType::Orders,
                number,
                game.turn.unsigned_abs(),
                order_salt(game.seed, number, game.turn),
            )
        };
        let bytes = log
            .to_file(&header)
            .map_err(|e| format!("cannot build the order file: {e}"))?;
        std::fs::write(&target, bytes)
            .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
        Ok(target)
    }

    /// Write the game back to a file.
    ///
    /// # Errors
    /// Returns a message suitable for showing to the player.
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        let bytes = self.to_bytes()?;
        std::fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        // The orders go beside the state file: a host replays them, and a
        // player file alone does not tell it what was done.
        self.save_orders(path)?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    /// Whether the player changed this planet's queue.
    fn was_edited(&self, id: u16) -> bool {
        i16::try_from(id).is_ok_and(|id| self.edited.contains(&id))
    }

    /// The packed queue for one planet, or `None` if the game has no such
    /// planet to speak for.
    fn encode_queue(&self, game: &GameState, id: u16) -> Option<Vec<u8>> {
        let wanted = i16::try_from(id).ok()?;
        let planet = game.planets.iter().find(|p| p.id == wanted)?;
        let items = planet
            .queue
            .iter()
            .map(|entry| QueueItem {
                count: u16::try_from(entry.count).unwrap_or(0),
                item: entry.item,
                class: if entry.ship {
                    QueueClass::Fleet
                } else {
                    QueueClass::Planet
                },
                completion: u16::try_from(entry.completion).unwrap_or(0),
            })
            .collect();
        Some(
            ProductionQueueRecord {
                planet_id: None,
                items,
            }
            .encode(),
        )
    }

    /// Advance the game by one year.
    ///
    /// The generator is seeded from the game id, which is reproducible but
    /// **not** the sequence the original produced: its generator is seeded from
    /// the clock at process start and its state is in no save file. See
    /// `docs/rng/prng.md`. Anything the turn decides by a roll — the mining
    /// remainder above all — will therefore differ from what the original
    /// engine would have done with the same save.
    pub fn generate_turn(&mut self) {
        // Other players' submitted orders, replayed before the year runs. Our
        // own are not: this session already applied them as they were made,
        // which is exactly what the log records.
        let logs = self.submitted_orders();
        let me = self.local_player();
        let Some(state) = self.game.as_mut() else {
            return;
        };
        let (orders, replays) = stars_core::replay::replay_logs(state, &logs);
        let replayed = replays
            .iter()
            .map(|r| (r.player, r.applied()))
            .filter(|(_, count)| *count > 0)
            .collect();

        let mut rng = stars_core::rng::Rng::randomize(state.seed);
        let report = stars_core::generate_turn_with_orders(state, &orders, &mut rng);
        // The year's news, for the player whose session this is.
        let messages: Vec<(u16, String)> = state
            .messages
            .iter()
            .filter(|m| m.player == me)
            .map(|m| (m.id, m.summary()))
            .collect();
        // The log covers one turn; the year has moved on.
        self.orders.clear();
        self.research_edited = false;
        self.player_edited = false;
        self.host_password_edited = false;
        self.battle_plans_edited = false;
        // A new year's messages: the pane goes back to the first of them.
        self.show_first_message();
        self.last_turn = Some(TurnSummary {
            year: report.year,
            mined: report.mined.len(),
            population: report.population.iter().map(|(_, d)| *d).sum(),
            ships: report.ships_built.iter().map(|(_, _, n)| *n).sum(),
            breakthroughs: report
                .breakthroughs
                .iter()
                .enumerate()
                .filter(|(_, gained)| !gained.is_empty())
                .map(|(player, gained)| (player, gained.len()))
                .collect(),
            skipped: report.skipped.iter().map(|s| format!("{s:?}")).collect(),
            replayed,
            messages,
        });
    }

    /// Move cargo between a fleet and the planet it orbits.
    ///
    /// `amount` is what the **fleet** gains, matching the order log's own sign
    /// convention: positive loads from the planet, negative unloads onto it.
    /// The move is applied at once, because that is what the client does, and
    /// recorded in [`Self::orders`].
    ///
    /// Returns how much actually moved — a hold has a capacity and a planet has
    /// only what it has.
    pub fn transfer_cargo(&mut self, fleet: usize, kind: usize, amount: i32) -> i32 {
        use stars_core::orders::{apply_cargo_transfer, CARGO_KINDS};
        use stars_formats::{CargoTransfer, CargoTransferRecord, GrobjClass, LogRecord};

        if amount == 0 || kind >= CARGO_KINDS {
            return 0;
        }
        let Some(game) = self.game.as_mut() else {
            return 0;
        };
        let Some(fleet_record) = game.fleets.get(fleet) else {
            return 0;
        };
        let Some(planet) = fleet_record.orbiting else {
            return 0;
        };
        // The order names a fleet by the word the file packs: the number in the
        // low nine bits and the owner above it.
        let owner = u16::try_from(fleet_record.owner.max(0)).unwrap_or(0);
        let source = (owner << 9) | (fleet_record.id & 0x1ff);

        let mut quantities = [0i32; CARGO_KINDS];
        quantities[kind] = amount;
        let record = CargoTransferRecord {
            source,
            destination: planet,
            source_class: Some(GrobjClass::Fleet),
            destination_class: Some(GrobjClass::Planet),
            mode: 0x12,
            selector: 1 << kind,
            quantities,
        };
        let moved = apply_cargo_transfer(game, &record)[kind];
        if moved != 0 {
            // The log's own transfer record: the two objects, their classes,
            // a bitmask of the cargo kinds moved and one quantity per kind.
            self.orders.push(LogRecord::cargo(&CargoTransfer {
                id1: source,
                id2: planet,
                grobj1: FLEET_CLASS,
                grobj2: PLANET_CLASS,
                items_mask: 1 << kind,
                quantities: vec![moved],
                quantity_bytes: Vec::new(),
            }));
            self.dirty = true;
        }
        moved
    }

    /// Record the order that puts a fleet's current waypoint on the log.
    ///
    /// `insert` writes the operation the client uses for a waypoint that was
    /// not there before; an update overwrites one that was. The pair is the
    /// idiom the real logs use: an insert to add the leg, then an update each
    /// time its task changes.
    fn log_waypoint(&mut self, fleet: usize, index: usize, insert: bool) {
        use stars_formats::{LogRecord, WaypointOrder};

        let Some(game) = self.game.as_ref() else {
            return;
        };
        let Some(fleet_record) = game.fleets.get(fleet) else {
            return;
        };
        let Some(waypoint) = fleet_record.waypoints.get(index) else {
            return;
        };
        let owner = u16::try_from(fleet_record.owner.max(0)).unwrap_or(0);
        let order = WaypointOrder {
            fleet_id: (owner << 9) | (fleet_record.id & 0x1ff),
            waypoint_index: u16::try_from(index).unwrap_or(0),
            x: waypoint.position.x,
            y: waypoint.position.y,
            target_id: waypoint
                .target
                .and_then(|t| i16::try_from(t).ok())
                .unwrap_or(0),
            task: waypoint.task,
            warp: waypoint.warp,
            // The waypoint's own class, not a guess from whether it has a
            // target: since a waypoint can land on a fleet or a `THING` as
            // well as a planet, "has a target" no longer means "planet".
            grobj: waypoint.target_class,
            valid_task: waypoint.task != 0,
            flags_high: 0,
            task_data: waypoint
                .transport
                .as_ref()
                .map(stars_formats::TransportTask::encode)
                .unwrap_or_default(),
        };
        let record = LogRecord::waypoint(&order, insert);
        // A drag moves a waypoint every frame it is held, and the original
        // writes the move **once**, on release. Rather than defer the write,
        // an update that follows an update to the same waypoint replaces it —
        // the same rewind `LogChangeRelations` does, and it leaves the log
        // holding one record with the final position in it.
        if !insert {
            if let Some(last) = self.orders.last() {
                if last.record_type == stars_formats::LogRecordType::FleetOrderUpdate
                    && last.data.get(..4) == record.data.get(..4)
                {
                    self.orders.pop();
                }
            }
        }
        self.orders.push(record);
    }

    /// Set what share of a player's resources goes to research.
    pub fn set_research(&mut self, player: usize, percent: u8) {
        if let Some(p) = self.game.as_mut().and_then(|g| g.players.get_mut(player)) {
            p.research_pct = percent.min(100);
            self.research_edited = true;
            self.player_edited = true;
            self.dirty = true;
        }
    }

    /// Set what a fleet does when it arrives.
    ///
    /// The task goes on the waypoint the fleet is heading to, which is where
    /// the game keeps it — it is performed on arrival and consumed then, which
    /// is why a reached waypoint always reads `none`.
    pub fn set_task(&mut self, fleet: usize, task: u8) {
        let index = fleet;
        let Some(fleet) = self.game.as_mut().and_then(|g| g.fleets.get_mut(fleet)) else {
            return;
        };
        // The destination if it has one, otherwise where it stands.
        let at = if fleet.waypoints.len() > 1 {
            fleet.waypoints.len() - 1
        } else {
            0
        };
        if let Some(waypoint) = fleet.waypoints.get_mut(at) {
            waypoint.task = task;
            if task != stars_formats::task::TRANSPORT {
                waypoint.transport = None;
            }
        }
        self.log_waypoint(index, at, false);
        self.dirty = true;
    }

    /// Set one cargo kind's instruction on a fleet's Transport task.
    pub fn set_transport(&mut self, fleet: usize, kind: usize, action: stars_formats::XferAction) {
        use stars_formats::{ItemAction, TransportTask};

        let index = fleet;
        let Some(fleet) = self.game.as_mut().and_then(|g| g.fleets.get_mut(fleet)) else {
            return;
        };
        let at = if fleet.waypoints.len() > 1 {
            fleet.waypoints.len() - 1
        } else {
            0
        };
        let Some(waypoint) = fleet.waypoints.get_mut(at) else {
            return;
        };
        if kind >= 5 {
            return;
        }
        waypoint.task = stars_formats::task::TRANSPORT;
        let mut orders = waypoint.transport.unwrap_or(TransportTask {
            items: [ItemAction {
                quantity: 0,
                action: stars_formats::XferAction::None,
            }; 5],
        });
        orders.items[kind].action = action;
        waypoint.transport = Some(orders);
        self.log_waypoint(index, at, false);
        self.dirty = true;
    }

    /// Send a fleet to a planet, at a given warp.
    ///
    /// This replaces whatever the fleet was doing: its waypoint list becomes
    /// where it is now, then where it is going.
    pub fn set_destination(&mut self, fleet: usize, planet: i16, warp: u8) {
        let index = fleet;
        let Some(game) = self.game.as_mut() else {
            return;
        };
        let Some(target) = game
            .planets
            .iter()
            .chain(game.known_planets.iter())
            .find(|p| p.id == planet)
            .and_then(|p| p.position)
        else {
            return;
        };
        let Some(fleet) = game.fleets.get_mut(fleet) else {
            return;
        };
        // Replacing an existing leg is a delete followed by an insert, which
        // is what the client's own log does.
        let replacing = fleet.waypoints.len() > 1;
        let fleet_word = (u16::try_from(fleet.owner.max(0)).unwrap_or(0) << 9) | (fleet.id & 0x1ff);
        let here = fleet.position;
        fleet.waypoints = vec![
            stars_core::fleet::Waypoint {
                position: here,
                target: None,
                target_class: 1,
                warp: 0,
                task: 0,
                transport: None,
                task_data: Vec::new(),
            },
            stars_core::fleet::Waypoint {
                position: target,
                target: u16::try_from(planet).ok(),
                target_class: 1,
                warp,
                task: 0,
                transport: None,
                task_data: Vec::new(),
            },
        ];
        fleet.warp = Some(warp);
        if replacing {
            self.orders.push(stars_formats::LogRecord::delete_waypoint(
                stars_formats::FleetOrderDelete {
                    fleet_id: fleet_word,
                    order_index: 1,
                    delete_extra: false,
                },
            ));
        }
        self.log_waypoint(index, 1, true);
        self.dirty = true;
    }

    /// Apply a run of orders to the loaded game and record them.
    ///
    /// The orders are applied by **replaying them**, through exactly the code a
    /// host runs on the submitted log. That is the point: what this session
    /// does to its own copy and what the host does to its copy cannot drift,
    /// because it is the same function.
    ///
    /// Returns whether anything was applied; an order naming something the
    /// player does not own changes nothing and is not logged.
    fn apply_and_log(&mut self, records: Vec<stars_formats::LogRecord>) -> bool {
        let player = self.local_player();
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let mut cargo = stars_core::TurnOrders::default();
        let log = stars_formats::OrderLog {
            header: None,
            records: records.clone(),
        };
        let report = stars_core::replay::replay(game, player, &log, &mut cargo);
        if report.applied() == 0 {
            return false;
        }
        self.orders.extend(records);
        self.dirty = true;
        true
    }

    /// Split ships out of a fleet into a new one.
    ///
    /// `count` ships of design slot `design` leave. The new fleet starts where
    /// the old one is, orbiting whatever it orbits, under the same battle plan.
    /// Splitting off everything leaves no original behind, which is what the
    /// game does too.
    ///
    /// Returns whether the split happened.
    pub fn split_fleet(&mut self, fleet: usize, design: u8, count: i32) -> bool {
        use stars_formats::{CargoTransfer, FleetSplit, LogRecord};

        if count <= 0 {
            return false;
        }
        let Some(game) = self.game.as_ref() else {
            return false;
        };
        let Some(source) = game.fleets.get(fleet) else {
            return false;
        };
        let owner = u16::try_from(source.owner.max(0)).unwrap_or(0);
        let from = (owner << 9) | (source.id & 0x1ff);
        let new_id = stars_core::turn::next_fleet_id(game, source.owner);
        let to = (owner << 9) | (new_id & 0x1ff);

        self.apply_and_log(vec![
            LogRecord::split_fleet(FleetSplit { fleet_id: from }),
            LogRecord::ships(&CargoTransfer {
                id1: from,
                id2: to,
                grobj1: FLEET_CLASS,
                grobj2: FLEET_CLASS,
                items_mask: 1 << design,
                // Negative: the ships leave the fleet named first.
                quantities: vec![-count],
                quantity_bytes: Vec::new(),
            }),
        ])
    }

    /// Merge fleets into the first of them, which takes their ships and cargo.
    ///
    /// Returns whether the merge happened.
    pub fn merge_fleets(&mut self, survivor: usize, absorbed: &[usize]) -> bool {
        use stars_formats::{FleetMerge, LogRecord};

        let Some(game) = self.game.as_ref() else {
            return false;
        };
        let word = |index: usize| -> Option<u16> {
            let fleet = game.fleets.get(index)?;
            let owner = u16::try_from(fleet.owner.max(0)).ok()?;
            Some((owner << 9) | (fleet.id & 0x1ff))
        };
        let Some(first) = word(survivor) else {
            return false;
        };
        let mut fleets = vec![first];
        fleets.extend(absorbed.iter().filter_map(|index| word(*index)));
        if fleets.len() < 2 {
            return false;
        }
        self.apply_and_log(vec![LogRecord::merge_fleets(&FleetMerge { fleets })])
    }

    /// Rename a fleet. An empty name clears it.
    ///
    /// Returns whether the rename happened.
    pub fn rename_fleet(&mut self, fleet: usize, name: &str) -> bool {
        use stars_formats::{FleetName, LogRecord};

        let Some(game) = self.game.as_ref() else {
            return false;
        };
        let Some(record) = game.fleets.get(fleet) else {
            return false;
        };
        let owner = u16::try_from(record.owner.max(0)).unwrap_or(0);
        let key = (record.owner, record.id);
        let order = LogRecord::fleet_name(&FleetName {
            id: (owner << 9) | (record.id & 0x1ff),
            grobj: u16::from(FLEET_CLASS),
            name: name.to_string(),
        });
        if self.apply_and_log(vec![order]) {
            self.renamed.insert(key);
            true
        } else {
            false
        }
    }

    /// Set which battle plan a fleet fights under.
    pub fn set_battle_plan(&mut self, fleet: usize, plan: u8) -> bool {
        use stars_formats::{FleetPlan, LogRecord};

        let Some(id) = self.fleet_word(fleet) else {
            return false;
        };
        let key = self.fleet_key(fleet);
        if self.apply_and_log(vec![LogRecord::fleet_plan(FleetPlan {
            fleet_id: id,
            plan,
        })]) {
            self.fleet_edits.extend(key);
            true
        } else {
            false
        }
    }

    /// Set whether a fleet's waypoint orders repeat once it reaches the last.
    pub fn set_repeat_orders(&mut self, fleet: usize, repeat: bool) -> bool {
        use stars_formats::{FleetRepeatOrders, LogRecord};

        let Some(id) = self.fleet_word(fleet) else {
            return false;
        };
        let key = self.fleet_key(fleet);
        if self.apply_and_log(vec![LogRecord::repeat_orders(FleetRepeatOrders {
            fleet_id: id,
            repeat,
        })]) {
            self.fleet_edits.extend(key);
            true
        } else {
            false
        }
    }

    /// Set how the local player regards one other: `0` neutral, `1` friend,
    /// `2` enemy.
    ///
    /// The order carries the **whole** table, so a second change replaces the
    /// record rather than adding one — which is what the game's own client
    /// does (`LogChangeRelations` rewinds the log when the previous record is
    /// already a relations record).
    pub fn set_relations(&mut self, toward: usize, value: u8) -> bool {
        use stars_formats::{LogRecord, LogRecordType, Relations};

        let me = self.local_player();
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let players = game.players.len();
        if toward >= players || toward == me {
            return false;
        }
        let Some(player) = game.players.get_mut(me) else {
            return false;
        };
        player.relations.resize(players, 0);
        player.relations[toward] = value;
        let table = player.relations.clone();
        self.player_edited = true;

        if self
            .orders
            .last()
            .is_some_and(|r| r.record_type == LogRecordType::Relations)
        {
            self.orders.pop();
        }
        self.orders
            .push(LogRecord::relations(&Relations { toward: table }));
        self.dirty = true;
        true
    }

    /// A fleet's `(owner, number)`, which is how the save path finds its block.
    fn fleet_key(&self, fleet: usize) -> Option<(i16, u16)> {
        let record = self.game.as_ref()?.fleets.get(fleet)?;
        Some((record.owner, record.id))
    }

    /// Set the production queue the local player's new colonies start with.
    ///
    /// The order carries the whole queue, so a second change replaces the
    /// record rather than adding one — the client's own writer likewise skips
    /// it when the previous record is already one of these.
    pub fn set_default_queue(&mut self, queue: stars_formats::DefaultQueue) -> bool {
        use stars_formats::{LogRecord, LogRecordType};

        let me = self.local_player();
        let Some(player) = self.game.as_mut().and_then(|g| g.players.get_mut(me)) else {
            return false;
        };
        // `LogChangeZpq1` compares the working copy with the player's before
        // writing anything, so setting it back to what it already was is not
        // an order.
        if player.default_queue == queue {
            return true;
        }
        player.default_queue = queue.clone();

        if self
            .orders
            .last()
            .is_some_and(|r| r.record_type == LogRecordType::PlayerZpq1)
        {
            self.orders.pop();
        }
        self.orders
            .push(LogRecord::raw(LogRecordType::PlayerZpq1, queue.encode()));
        self.player_edited = true;
        self.dirty = true;
        true
    }

    /// Set or clear the local player's turn password.
    ///
    /// An empty string clears it. What is stored — here, in the file and in the
    /// order record — is the salt the game derives from the text, never the
    /// text: see [`stars_formats::password`]. The salt is a checksum a 1995
    /// game used to keep the other players in a hot-seat or play-by-mail game
    /// out of each other's turns; it is not worth anything as protection now,
    /// and nothing here treats it as though it were.
    ///
    /// Returns whether it changed.
    pub fn set_password(&mut self, text: &str) -> bool {
        use stars_formats::{LogRecord, LogRecordType, PasswordChange};

        let salt = stars_formats::password_salt(text);
        let me = self.local_player();
        let Some(player) = self.game.as_mut().and_then(|g| g.players.get_mut(me)) else {
            return false;
        };
        if player.password == salt {
            return false;
        }
        player.password = salt;
        // `NewPasswordDlg` sets `lSaltLast` as it stores the new salt, so a
        // password just chosen is not asked for again this session.
        self.password_given = (salt != 0).then_some(salt);

        // Like the relations table, this is state rather than an event: the
        // last record wins, so an earlier one is dropped instead of stacking.
        if self
            .orders
            .last()
            .is_some_and(|r| r.record_type == LogRecordType::ChangePassword)
        {
            self.orders.pop();
        }
        self.orders
            .push(LogRecord::change_password(PasswordChange { salt }));
        self.player_edited = true;
        self.dirty = true;
        true
    }

    // --- The planet pane ---------------------------------------------------
    //
    // `PlanetWndProc` (`1048:0000`) draws the selected planet as six tiles in
    // two columns; see `docs/ui/planet-pane.md`. Each tile's rows are built
    // here, as label/value pairs, so what the pane says can be tested without
    // drawing it.

    /// The planet the pane is showing, if one is selected.
    #[must_use]
    pub fn pane_planet(&self) -> Option<&stars_core::planet::Planet> {
        let game = self.game.as_ref()?;
        let id = self.selection.planet?;
        game.planets
            .iter()
            .chain(game.known_planets.iter())
            .find(|p| p.id == id)
    }

    /// The pane's title bar: the planet's name (`SetPlanetTitleBar`,
    /// `1048:3dec`), or "Planet View" when nothing is selected.
    #[must_use]
    pub fn planet_pane_title(&self) -> String {
        let Some(planet) = self.pane_planet() else {
            return "Planet View".to_string();
        };
        self.universe
            .as_ref()
            .and_then(|u| {
                u.planets_resolved()
                    .into_iter()
                    .find(|p| i16::try_from(p.id).is_ok_and(|id| id == planet.id))
                    .and_then(|p| p.name)
            })
            .map_or_else(|| format!("Planet #{}", planet.id), ToString::to_string)
    }

    /// The race whose eyes the pane is looking through.
    fn pane_race(&self) -> Option<&stars_core::Race> {
        let game = self.game.as_ref()?;
        game.players.get(self.local_player()).map(|p| &p.race)
    }

    /// The **Minerals On Hand** tile: what is on the surface, and what is
    /// dug and built.
    #[must_use]
    pub fn planet_minerals_tile(&self) -> Vec<(String, String)> {
        let Some(planet) = self.pane_planet() else {
            return Vec::new();
        };
        // The original prints each mineral's name in its own colour, then the
        // amount as `%ldkT`.
        let mut rows: Vec<(String, String)> = ["Ironium", "Boranium", "Germanium"]
            .iter()
            .zip(planet.surface_min.iter())
            .map(|(name, amount)| ((*name).to_string(), format!("{amount}kT")))
            .collect();

        let Some(race) = self.pane_race() else {
            return rows;
        };
        // Alternate Reality has no factories and its mines are not capped by
        // population, so the original prints a bare count with a star.
        if race.is_ar() {
            rows.push((
                "Mines".to_string(),
                format!("{}*", stars_core::mining::mines_operating(planet, race)),
            ));
            rows.push(("Factories".to_string(), "n/a".to_string()));
            return rows;
        }
        rows.push((
            "Mines".to_string(),
            format!(
                "{} of {}",
                planet.mines,
                stars_core::max_operable_mines(planet, race, false)
            ),
        ));
        rows.push((
            "Factories".to_string(),
            format!(
                "{} of {}",
                planet.factories,
                stars_core::max_operable_factories(planet, race, false)
            ),
        ));
        rows
    }

    /// The **Status** tile: population, resources, scanning and defence.
    #[must_use]
    pub fn planet_status_tile(&self) -> Vec<(String, String)> {
        use stars_core::components::best_planetary_scanner;

        let (Some(planet), Some(race)) = (self.pane_planet(), self.pane_race()) else {
            return Vec::new();
        };
        let levels = self
            .game
            .as_ref()
            .and_then(|g| g.players.get(self.local_player()))
            .map_or([0u8; 6], |p| p.research.levels);
        let ar = race.is_ar();
        let none = if ar { "n/a" } else { "none" };

        let mut rows = vec![(
            "Population".to_string(),
            comma_format(i64::from(planet.pop) * 100),
        )];
        // `%d of %d`: what production may spend, of what the planet makes.
        // The first figure is the second less the research skim, which is the
        // share the player has set aside (`1048:1716`).
        let total = i32::from(
            stars_core::resources::resources_at_planet(planet, race, i16::from(levels[0]))
                .unwrap_or(0),
        );
        let research_pct = self
            .game
            .as_ref()
            .and_then(|g| g.players.get(self.local_player()))
            .map_or(0, |p| i32::from(p.research_pct));
        let available = total - total * research_pct / 100;
        rows.push((
            "Resources/Year".to_string(),
            format!("{available} of {total}"),
        ));

        // Scanning. A planet scans only if it has been given a scanner, which
        // for everybody but Alternate Reality means one has been built.
        let scanner = best_planetary_scanner(&levels);
        let range = stars_core::scanning::planet_scanner_range_for_tech(
            planet,
            race,
            &levels,
            scanner.is_some(),
        );
        rows.push((
            "Scanner Type".to_string(),
            if ar {
                "Organic".to_string()
            } else {
                scanner.map_or_else(|| none.to_string(), |s| s.name.to_string())
            },
        ));
        rows.push((
            "Scanner Range".to_string(),
            if range.normal <= 0 {
                none.to_string()
            } else if range.penetrating > 0 {
                format!("{}/{} l.y.", range.penetrating, range.normal)
            } else if range.normal < 100 {
                // Under a hundred the original spells it out.
                format!("{} light years", range.normal)
            } else {
                format!("{} l.y.", range.normal)
            },
        ));

        // Defence.
        if ar {
            rows.push(("Defenses".to_string(), "n/a".to_string()));
            rows.push(("Defense Type".to_string(), "n/a".to_string()));
            rows.push(("Def Coverage".to_string(), "n/a".to_string()));
            return rows;
        }
        rows.push((
            "Defenses".to_string(),
            format!(
                "{} of {}",
                planet.defenses,
                stars_core::resources::max_operable_defenses(planet, race)
            ),
        ));
        let part = stars_core::bombing::best_defence_part(levels);
        rows.push((
            "Defense Type".to_string(),
            if planet.defenses == 0 {
                none.to_string()
            } else {
                part.map_or_else(|| none.to_string(), |p| p.name.to_string())
            },
        ));
        rows.push((
            "Def Coverage".to_string(),
            if planet.defenses == 0 || part.is_none() {
                none.to_string()
            } else {
                let (against_all, against_smart) =
                    stars_core::bombing::pct_survive(planet, race, levels);
                format!(
                    "{:.1}% ({:.1}%)",
                    (1.0 - against_all) * 100.0,
                    (1.0 - against_smart) * 100.0
                )
            },
        ));
        rows
    }

    /// The **Starbase** tile, or nothing when the planet has none.
    #[must_use]
    pub fn planet_starbase_tile(&self) -> (String, Vec<(String, String)>) {
        let Some(planet) = self.pane_planet() else {
            return ("< no starbase >".to_string(), Vec::new());
        };
        if !planet.starbase {
            return ("< no starbase >".to_string(), Vec::new());
        }
        let design = self
            .game
            .as_ref()
            .and_then(|g| g.designs.get(self.local_player()))
            .and_then(|d| d.get(starbase_slot(planet.starbase_design?)))
            .filter(|d| d.hull_id >= 0);
        let title = design.map_or_else(
            || "Starbase".to_string(),
            |d| {
                if d.name.is_empty() {
                    "Starbase".to_string()
                } else {
                    d.name.clone()
                }
            },
        );
        let rows = vec![
            (
                "Dock Capacity".to_string(),
                design.map_or_else(|| "none".to_string(), |_| "Unlimited".to_string()),
            ),
            (
                "Armor".to_string(),
                design.map_or_else(
                    || "none".to_string(),
                    |d| {
                        d.armor(false)
                            .map_or_else(|| "none".to_string(), |a| format!("{a}dp"))
                    },
                ),
            ),
            (
                "Shields".to_string(),
                design.map_or_else(|| "none".to_string(), |d| format!("{}dp", d.shields(false))),
            ),
            ("Damage".to_string(), "none".to_string()),
        ];
        (title, rows)
    }

    /// The **Production** tile: the queue, in build order.
    #[must_use]
    pub fn planet_production_tile(&self) -> Vec<String> {
        self.planet_production_rows()
            .into_iter()
            .map(|(line, _)| line)
            .collect()
    }

    /// The same rows, each with the mark the original draws it by.
    ///
    /// `FillPlanetProdLB` fills the tile and the dialog's queue list from the
    /// same routine, so the tile is coloured by the same estimate: an item that
    /// will practically never be built is red here too.
    #[must_use]
    pub fn planet_production_rows(&self) -> Vec<(String, stars_core::production::EtaMark)> {
        use stars_core::production::EtaMark;
        let Some(planet) = self.pane_planet() else {
            return Vec::new();
        };
        if planet.queue.is_empty() {
            return vec![("--- Queue is Empty ---".to_string(), EtaMark::Ordinary)];
        }
        let me = self.local_player();
        let who = self
            .game
            .as_ref()
            .and_then(|g| g.players.get(me))
            .map(stars_core::parts::Builder::player);
        let designs: &[stars_core::design::ShipDesign] = self
            .game
            .as_ref()
            .and_then(|g| g.designs.get(me))
            .map_or(&[], Vec::as_slice);
        let research_pct = self
            .game
            .as_ref()
            .and_then(|g| g.players.get(me))
            .map_or(0, |p| p.research_pct);

        planet
            .queue
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let name = if entry.ship {
                    self.game
                        .as_ref()
                        .and_then(|g| g.designs.get(me))
                        .and_then(|d| d.get(usize::from(entry.item)))
                        .filter(|d| d.hull_id >= 0 && !d.name.is_empty())
                        .map_or_else(|| format!("Design #{}", entry.item), |d| d.name.clone())
                } else {
                    crate::views::item_name(entry.item)
                };
                let mark = who.as_ref().map_or(EtaMark::Ordinary, |who| {
                    stars_core::production::eta(planet, who, research_pct, designs, index)
                        .mark(entry.item, entry.ship)
                });
                (format!("{} {}", entry.count, name), mark)
            })
            .collect()
    }

    /// The title of the pane's last tile.
    ///
    /// `DrawPlanetShipList` (`1048:377e`) draws the same tile for both panes
    /// and titles it by which one it is in: `Fleets in Orbit` for a planet
    /// (string `0x0338`) and `Other Fleets Here` for a fleet (`0x0339`), the
    /// "other" being the selected fleet, which the tile leaves out.
    #[must_use]
    pub fn pane_fleets_title(&self) -> &'static str {
        if self.selection.on_fleet && self.pane_fleet().is_some() {
            "Other Fleets Here"
        } else {
            "Fleets in Orbit"
        }
    }

    /// What the tile's dropdown holds: the fleets at the pane's location.
    ///
    /// The selected fleet is left out when the pane is showing one — the
    /// original passes it as `idSkip` — so the tile always answers "what
    /// **else** is here?".
    #[must_use]
    pub fn pane_fleet_list(&self) -> Vec<PaneFleet> {
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        // A fleet's own position when one is selected, the planet's otherwise.
        let at = match (self.selection.on_fleet, self.pane_fleet()) {
            (true, Some(fleet)) => Some(fleet.position),
            _ => self.pane_planet().and_then(|planet| planet.position),
        };
        let Some(at) = at else {
            return Vec::new();
        };
        let skip = self
            .selection
            .on_fleet
            .then(|| self.pane_fleet().map(|fleet| (fleet.owner, fleet.id)))
            .flatten();
        let me = self.local_player();

        game.fleets
            .iter()
            .enumerate()
            .filter(|(_, fleet)| fleet.position == at && !fleet.stacks.is_empty())
            .filter(|(_, fleet)| Some((fleet.owner, fleet.id)) != skip)
            .map(|(index, fleet)| PaneFleet {
                index,
                key: (fleet.owner, fleet.id),
                name: self.fleet_display_name(index),
                ships: fleet.stacks.iter().map(|stack| stack.count).sum(),
                mine: usize::try_from(fleet.owner).is_ok_and(|owner| owner == me),
            })
            .collect()
    }

    /// The fleet the tile's dropdown is showing, as an index into the game's
    /// fleets.
    ///
    /// The first one when nothing has been chosen, and nothing at all when the
    /// list is empty — which is the state the original disables the tile's
    /// buttons and draws no gauges in.
    #[must_use]
    pub fn pane_fleet_choice(&self) -> Option<usize> {
        let list = self.pane_fleet_list();
        self.pane_fleet_chosen
            .and_then(|key| list.iter().find(|entry| entry.key == key))
            .or_else(|| list.first())
            .map(|entry| entry.index)
    }

    /// Choose one from the dropdown, by the key [`PaneFleet::key`] carries.
    pub fn choose_pane_fleet(&mut self, key: (i16, u16)) {
        self.pane_fleet_chosen = Some(key);
    }

    /// The **fuel and cargo gauges** the tile draws under its dropdown.
    ///
    /// `None` when there is nothing to draw them for, which the original
    /// decides two ways: nothing is selected, or what is selected is not known
    /// in full (`det != 7`). Full detail is only ever had of one's own fleets,
    /// so somebody else's shows the dropdown and no gauges.
    #[must_use]
    pub fn pane_fleet_gauges(&self) -> Option<FleetGauges> {
        let index = self.pane_fleet_choice()?;
        let game = self.game.as_ref()?;
        let fleet = game.fleets.get(index)?;
        let owner = usize::try_from(fleet.owner).ok()?;
        if owner != self.local_player() {
            return None;
        }
        let designs = game.designs.get(owner)?;
        Some(FleetGauges {
            fuel: fleet.cargo.fuel,
            fuel_capacity: fleet.fuel_capacity(designs),
            minerals: fleet.cargo.minerals,
            colonists: fleet.cargo.colonists,
            cargo_capacity: fleet.cargo_capacity(designs),
        })
    }

    /// A fleet's name, as the game writes it (`PszGetFleetName`, `util.c`).
    ///
    /// A fleet the player has renamed shows that name. Otherwise it is named
    /// for its **primary design** — the one it has most of — with a `+`
    /// appended when it carries more than one design, and the number the player
    /// sees, which is the stored id **plus one**. A fleet with no design at all
    /// falls back to the word `Fleet`. Somebody else's fleet is prefixed with
    /// their race name.
    #[must_use]
    pub fn fleet_display_name(&self, index: usize) -> String {
        let Some(game) = self.game.as_ref() else {
            return String::new();
        };
        let Some(fleet) = game.fleets.get(index) else {
            return String::new();
        };
        let owner = usize::try_from(fleet.owner).ok();
        let prefix = match owner {
            Some(owner) if owner != self.local_player() => game.players.get(owner).map_or_else(
                || format!("player {} ", owner + 1),
                |p| format!("{} ", p.name),
            ),
            _ => String::new(),
        };
        if let Some(name) = &fleet.name {
            return format!("{prefix}{name}");
        }
        // The design it has most of, and whether it is carrying more than one.
        let primary = fleet
            .stacks
            .iter()
            .filter(|s| s.count > 0)
            .max_by_key(|s| s.count)
            .map(|s| usize::from(s.design));
        let mixed = fleet.stacks.iter().filter(|s| s.count > 0).count() > 1;
        let class = primary
            .and_then(|slot| {
                owner
                    .and_then(|o| game.designs.get(o))
                    .and_then(|d| d.get(slot))
            })
            .filter(|d| d.hull_id >= 0 && !d.name.is_empty())
            .map_or_else(|| "Fleet".to_string(), |d| d.name.clone());
        let plus = if mixed { "+" } else { "" };
        format!("{prefix}{class}{plus} #{}", fleet.id + 1)
    }

    // --- The Find dialog ---------------------------------------------------
    //
    // `FSelectSz` (`1058:945a`): type a name, get taken to it. See
    // `docs/ui/scanner.md`.

    /// Find a planet or a fleet by name and select it.
    ///
    /// The original's search order, which is not the obvious one:
    ///
    /// 1. an **exact** planet name wins outright;
    /// 2. failing that, a **fleet** — by name, or by number, so `Fleet #7`,
    ///    `#7` and `7` all find your own fleet 7;
    /// 3. failing that, the first planet whose name **starts with** what was
    ///    typed.
    ///
    /// So an exact fleet name beats a partial planet name, and every
    /// comparison ignores case.
    ///
    /// Returns what it selected.
    pub fn find(&mut self, text: &str) -> Option<FindResult> {
        let typed = text.trim();
        if typed.is_empty() {
            return None;
        }
        let lower = typed.to_lowercase();

        // Planets, in the order the game holds their names.
        let mut prefix: Option<i16> = None;
        let mut exact: Option<i16> = None;
        if let Some(universe) = self.universe.as_ref() {
            for planet in universe.planets_resolved() {
                let Some(name) = planet.name else { continue };
                let Ok(id) = i16::try_from(planet.id) else {
                    continue;
                };
                let name = name.to_lowercase();
                if name == lower {
                    exact = Some(id);
                    break;
                }
                if prefix.is_none() && name.starts_with(&lower) {
                    prefix = Some(id);
                }
            }
        }
        if let Some(id) = exact {
            self.selection.planet = Some(id);
            self.screen = Screen::Planets;
            return Some(FindResult::Planet(id));
        }

        // Fleets. "Fleet" at the front is skipped — and the original skips six
        // characters for a five-letter word, taking the space with it, so
        // `Fleet7` loses its digit too.
        let mut rest = typed;
        if lower.starts_with("fleet") {
            rest = typed.get(6..).unwrap_or("");
        }
        let rest = rest.trim_start();
        let rest = rest.strip_prefix('#').unwrap_or(rest).trim_start();
        // A leading zero does not start a number: the original tests `> '0'`.
        let numbered = rest
            .starts_with(|c: char| ('1'..='9').contains(&c))
            .then(|| {
                rest.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse::<u16>()
                    .ok()
            })
            .flatten()
            .filter(|n| *n <= 512);

        let me = self.local_player();
        let found = self.game.as_ref().and_then(|game| {
            if let Some(number) = numbered {
                // The number the player types is the one they are shown,
                // which is the stored id **plus one** — `PszGetFleetName`
                // prints `(id & 0x1ff) + 1` — so the original subtracts one
                // before looking the fleet up.
                let id = number.wrapping_sub(1);
                if let Some(index) = game
                    .fleets
                    .iter()
                    .position(|f| f.id == id && usize::try_from(f.owner).is_ok_and(|o| o == me))
                {
                    return Some(index);
                }
            }
            game.fleets.iter().position(|f| {
                f.name
                    .as_ref()
                    .is_some_and(|name| name.to_lowercase() == lower)
            })
        });
        if let Some(index) = found {
            self.selection.fleet = Some(index);
            self.screen = Screen::Fleets;
            return Some(FindResult::Fleet(index));
        }

        // And last, the planet whose name merely starts the same way.
        if let Some(id) = prefix {
            self.selection.planet = Some(id);
            self.screen = Screen::Planets;
            return Some(FindResult::Planet(id));
        }
        None
    }

    // --- The measuring tape ------------------------------------------------
    //
    // `FHandleMeasuringTape` (`1058:9974`): a right-drag across the map, with
    // the scanner's status bar reporting what is under the far end and how far
    // away it is. See `docs/ui/scanner.md`.

    /// Start measuring from a point, as pressing the right button does.
    pub fn measure_from(&mut self, x: i16, y: i16) {
        self.measuring = Some((
            stars_core::movement::Point::new(x, y),
            stars_core::movement::Point::new(x, y),
        ));
    }

    /// Drag the far end of the tape.
    ///
    /// The end **snaps** to the nearest object — the original re-runs
    /// `FFindNearestObject` on every mouse move and moves the end onto whatever
    /// it finds. `wide` is the Shift key, which widens what counts.
    pub fn measure_to(&mut self, x: i16, y: i16, wide: bool) {
        let Some((from, _)) = self.measuring else {
            return;
        };
        let at = stars_core::movement::Point::new(x, y);
        let snapped = self.nearest_object(at, if wide { 24.0 } else { 8.0 });
        self.measuring = Some((from, snapped.map_or(at, |(_, p)| p)));
    }

    /// Let go of the tape.
    pub fn measure_end(&mut self) {
        self.measuring = None;
    }

    /// The nearest thing to a point, within `reach`, and where it is.
    ///
    /// `FFindNearestObject` (`1038:…`) searches planets, fleets and space
    /// objects together; this searches the same three, nearest first. Planets
    /// come first, so a fleet in orbit loses the tie to the planet it is at —
    /// which is the rule the status bar depends on.
    #[must_use]
    pub fn nearest_scan(
        &self,
        at: stars_core::movement::Point,
        reach: f64,
    ) -> Option<(ScanObject, stars_core::movement::Point)> {
        let game = self.game.as_ref()?;
        let mut best: Option<(f64, ScanObject, stars_core::movement::Point)> = None;
        let mut consider = |object: ScanObject, p: stars_core::movement::Point| {
            let d = stars_core::movement::distance(p, at);
            if d <= reach && best.as_ref().is_none_or(|(b, _, _)| d < *b) {
                best = Some((d, object, p));
            }
        };
        for planet in game.planets.iter().chain(game.known_planets.iter()) {
            if let Some(p) = planet.position {
                consider(ScanObject::Planet(planet.id), p);
            }
        }
        for (index, fleet) in game.fleets.iter().enumerate() {
            if fleet.stacks.is_empty() {
                continue;
            }
            consider(ScanObject::Fleet(index), fleet.position);
        }
        for (index, field) in game.minefields.iter().enumerate() {
            consider(
                ScanObject::Thing(ScanThing::Minefield(index)),
                field.position,
            );
        }
        for (index, packet) in game.packets.iter().enumerate() {
            consider(ScanObject::Thing(ScanThing::Packet(index)), packet.position);
        }
        for (index, hole) in game.wormholes.iter().enumerate() {
            consider(ScanObject::Thing(ScanThing::Wormhole(index)), hole.position);
        }
        for (index, trader) in game.traders.iter().enumerate() {
            consider(ScanObject::Thing(ScanThing::Trader(index)), trader.position);
        }
        best.map(|(_, object, p)| (object, p))
    }

    /// The nearest thing to a point, by the name the game would print for it.
    #[must_use]
    pub fn nearest_object(
        &self,
        at: stars_core::movement::Point,
        reach: f64,
    ) -> Option<(String, stars_core::movement::Point)> {
        self.nearest_scan(at, reach)
            .map(|(object, p)| (self.object_name(object), p))
    }

    /// What the game calls one thing the scanner found.
    ///
    /// `PszGetLocName` (`1038:3b08`) picks between `PszGetPlanetName`,
    /// `PszGetFleetName` and `PszGetThingName` on the object's class.
    #[must_use]
    pub fn object_name(&self, object: ScanObject) -> String {
        match object {
            ScanObject::Planet(id) => self.planet_name(id),
            ScanObject::Fleet(index) => self.fleet_display_name(index),
            ScanObject::Thing(thing) => self.thing_name(thing),
        }
    }

    /// A planet's name, or a stand-in when the universe file is not to hand.
    #[must_use]
    pub fn planet_name(&self, id: i16) -> String {
        self.universe
            .as_ref()
            .and_then(|u| {
                u.planets_resolved()
                    .into_iter()
                    .find(|p| i16::try_from(p.id).is_ok_and(|p| p == id))
                    .and_then(|p| p.name)
            })
            .map_or_else(|| format!("Planet #{id}"), ToString::to_string)
    }

    /// What the scanner's status bar says (`DrawScannerSBar`, `1058:62d8`).
    ///
    /// The bar reports one point — the original's `sel.scan` — and, on its
    /// second row, how far that is from another. Three things feed it:
    ///
    /// * the **measuring tape**, while it is stretched: the point is its far
    ///   end and the distance runs back to the anchor. `FHandleMeasuringTape`
    ///   (`1058:9b8b`) fills in an `SBAR` whose `pscan` is the anchor's own
    ///   scan, and that is what leaves the `from <name>` clause off;
    /// * a **waypoint being dragged**: the point is the leg's far end and the
    ///   distance runs back to the selection, which does get named because
    ///   `FHandleWayPointDrag` (`1058:8551`) leaves `pscan` null;
    /// * otherwise the **selection**, whose two points are the same, so the
    ///   second row stays empty.
    ///
    /// Only a planet or a waypoint puts anything in the id cell, which is why
    /// `MANUAL.PDF` p. 5-16 promises an ID# for a planet and only coordinates
    /// and a name for a fleet or an object.
    #[must_use]
    pub fn status_bar(&self) -> StatusBar {
        // The tape. Nothing is reported until the far end has moved more than
        // two units from the anchor, which is the guard that stops a stray
        // right-click redrawing the bar.
        if let Some((from, at)) = self.measuring {
            if (at.x - from.x).abs() > 2 || (at.y - from.y).abs() > 2 {
                let mut bar = match self.nearest_scan(at, 0.5) {
                    Some((object, _)) => self.bar_for(object),
                    // Nothing there: the original hands the bar `idsDeepSpace`
                    // as a ready-made string and an id of -1.
                    None => StatusBar {
                        name: "Deep Space".to_string(),
                        ..StatusBar::default()
                    },
                };
                bar.place(at);
                bar.distance = Some(Distance {
                    figure: distance_figure(from, at),
                    from: None,
                });
                return bar;
            }
        }

        // A waypoint under the mouse. The leg's far end is a waypoint unless
        // it has landed on a planet, in which case the planet wins.
        //
        // The waypoint the map merely has *in hand* counts too: `ChangeScanSel`
        // copies the whole scan into `sel.scan`, `iwp` and all, so the bar goes
        // on naming it after the drag is let go — and it is the only thing that
        // says what **Delete** will remove.
        if let Some(index) = self.dragging_waypoint.or(self.selection.waypoint) {
            if let Some(at) = self.waypoint_point(index) {
                let mut bar = match self.nearest_scan(at, 0.5) {
                    Some((object @ ScanObject::Planet(_), _)) => self.bar_for(object),
                    _ => StatusBar {
                        id: format!("WP #{index}"),
                        name: "Deep Space Waypoint".to_string(),
                        ..StatusBar::default()
                    },
                };
                bar.place(at);
                bar.distance = self
                    .selected_point()
                    .filter(|from| *from != at)
                    .map(|from| Distance {
                        figure: distance_figure(from, at),
                        from: Some(self.selected_object().map_or_else(
                            || "Deep Space".to_string(),
                            |object| self.object_name(object),
                        )),
                    });
                return bar;
            }
        }

        // The selection. `ChangeScanSel` (`1058:8e5a`) turns the scan into the
        // planet whenever the point it landed on has one, so a fleet in orbit
        // shows the planet's ID and name and not the fleet's.
        let Some(object) = self.selected_object() else {
            return StatusBar::default();
        };
        let Some(at) = self.object_position(object) else {
            return StatusBar::default();
        };
        let object = match object {
            ScanObject::Fleet(_) => {
                self.nearest_scan(at, 0.0)
                    .map_or(object, |(found, _)| match found {
                        ScanObject::Planet(id) => ScanObject::Planet(id),
                        _ => object,
                    })
            }
            other => other,
        };
        let mut bar = self.bar_for(object);
        bar.place(at);
        bar
    }

    /// The cells one object fills in.
    fn bar_for(&self, object: ScanObject) -> StatusBar {
        StatusBar {
            // `"ID #%d"` with `idpl + 1`: the number the player sees is the
            // stored index plus one.
            id: match object {
                ScanObject::Planet(id) => format!("ID #{}", id + 1),
                _ => String::new(),
            },
            name: self.object_name(object),
            ..StatusBar::default()
        }
    }

    /// Where one of the selected fleet's waypoints is.
    #[must_use]
    pub fn waypoint_point(&self, index: usize) -> Option<stars_core::movement::Point> {
        let fleet = self.selection.fleet?;
        let waypoint = self
            .game
            .as_ref()?
            .fleets
            .get(fleet)?
            .waypoints
            .get(index)?;
        Some(waypoint.position)
    }

    // --- The pop-up summary -------------------------------------------------
    //
    // `ScannerWndProc` (`1058:043a`) → `Popup` (`10c0:0c7c`). See
    // `crate::popup` and `docs/ui/scanner.md`.

    /// The pop-up a press in the status bar's **upper** row raises.
    ///
    /// Nothing at all unless the scan has a planet or a fleet
    /// (`sel.scan.grobjFull & (grobjPlanet | grobjFleet)`); the fleet's ship
    /// list when `sel.scan.grobj` is a fleet — which, since `ChangeScanSel`
    /// turns the scan into the planet whenever the point has one, means a
    /// fleet out in open space — and the planet's summary otherwise.
    #[must_use]
    pub fn status_bar_popup(&self) -> Option<Popup> {
        let object = self.selected_object()?;
        let at = self.object_position(object)?;
        match object {
            // A fleet at a planet is the planet, as the bar itself shows it.
            ScanObject::Fleet(index) => match self.planet_at(at) {
                Some(id) => Some(self.planet_popup(id, at)),
                None => Some(self.fleet_popup(index)),
            },
            ScanObject::Planet(id) => Some(self.planet_popup(id, at)),
            // A space object is neither, so the press does nothing.
            ScanObject::Thing(_) => None,
        }
    }

    /// The planet at a point, if one is there.
    fn planet_at(&self, at: stars_core::movement::Point) -> Option<i16> {
        self.game
            .as_ref()?
            .planets
            .iter()
            .chain(self.game.as_ref()?.known_planets.iter())
            .find(|planet| planet.position == Some(at))
            .map(|planet| planet.id)
    }

    /// `grPopupUnknownObj`: the planet's name, its ID and the scan's own
    /// coordinates, one per row.
    fn planet_popup(&self, id: i16, at: stars_core::movement::Point) -> Popup {
        Popup::Planet(PlanetSummary {
            values: [
                self.planet_name(id),
                (id + 1).to_string(),
                at.x.to_string(),
                at.y.to_string(),
            ],
        })
    }

    /// `grPopupFleet`: one row per design the fleet holds any of, in **design
    /// slot** order, since the original walks slots 0 to 15 and skips the
    /// empty ones.
    pub(crate) fn fleet_popup(&self, index: usize) -> Popup {
        let mut rows = Vec::new();
        if let Some(game) = self.game.as_ref() {
            if let Some(fleet) = game.fleets.get(index) {
                let designs = usize::try_from(fleet.owner)
                    .ok()
                    .and_then(|owner| game.designs.get(owner));
                let mut stacks: Vec<_> = fleet
                    .stacks
                    .iter()
                    .filter(|stack| stack.count > 0)
                    .collect();
                stacks.sort_by_key(|stack| stack.design);
                for stack in stacks {
                    let name = designs
                        .and_then(|designs| designs.get(usize::from(stack.design)))
                        .filter(|design| design.hull_id >= 0 && !design.name.is_empty())
                        .map_or_else(String::new, |design| design.name.clone());
                    rows.push(FleetRow {
                        name,
                        count: stack.count.to_string(),
                        damage: crate::popup::damage_text(
                            stack.count,
                            stack.damaged_pct,
                            stack.damage_pct,
                        ),
                    });
                }
            }
        }
        Popup::Fleet(FleetSummary {
            rows,
            // The scanner leaves `fRedDamage` alone; see `FleetSummary`.
            show_damage: false,
        })
    }

    // --- Waypoint dragging -------------------------------------------------
    //
    // `FAddWayPoint` (`1058:7504`) and `FHandleWayPointDrag` (`1058:8176`):
    // giving a fleet its orders by dragging on the map. See
    // `docs/ui/scanner.md`.

    /// The warp the client suggests for a leg of this length.
    ///
    /// The fleet's cruising speed ([`stars_core::movement::ideal_warp`]),
    /// slowed as far as it can go without arriving any later
    /// ([`stars_core::movement::settle_warp`]).
    #[must_use]
    pub fn suggested_warp(&self, fleet: usize, distance: i32) -> u8 {
        let Some(game) = self.game.as_ref() else {
            return 5;
        };
        let Some(record) = game.fleets.get(fleet) else {
            return 5;
        };
        let designs = usize::try_from(record.owner)
            .ok()
            .and_then(|owner| game.designs.get(owner))
            .map_or(&[][..], Vec::as_slice);
        let ideal = stars_core::movement::ideal_warp(&record.stacks, designs, false);
        if ideal <= 0 {
            return 0;
        }
        u8::try_from(stars_core::movement::settle_warp(ideal, distance).clamp(0, 15)).unwrap_or(5)
    }

    /// The most waypoints a fleet may hold, the origin among them.
    ///
    /// `FAddWayPoint` (`1058:7504`) refuses outright once the fleet already
    /// has this many, with a beep and an alert. The alert's own text says 86,
    /// one less, because it counts the legs rather than the entries.
    pub const WAYPOINT_MAX: usize = 0x57;

    /// Add a waypoint to the selected fleet, at a point on the map.
    ///
    /// This is what shift-clicking the map does, and what clicking it does in
    /// Add Way Points mode: the leg is appended to whatever orders the fleet
    /// already has, at the warp the client suggests, and the order log gets
    /// the insert the real client writes.
    ///
    /// `snap` is how far, **in galaxy units**, the waypoint will reach for
    /// something to land on. `FAddWayPoint` measures the click against the
    /// nearest object and keeps the object's own position when it is within
    /// `ScanToPt(20)` — twenty screen pixels, converted by the zoom — so the
    /// caller passes twenty pixels' worth of galaxy units and gets the
    /// original's behaviour. Pass `0.0` to land exactly where asked.
    ///
    /// Snapping is not only cosmetic: a waypoint that lands *on* an object
    /// records that object, and a waypoint that records an object is the only
    /// kind a task can be given at. A leg that stops half a light year short
    /// of a planet cannot be told to unload there.
    ///
    /// Returns whether a waypoint was added.
    pub fn add_waypoint(&mut self, x: i16, y: i16, snap: f64) -> bool {
        let Some(index) = self.selection.fleet else {
            return false;
        };
        let me = self.local_player();
        let asked = stars_core::movement::Point::new(x, y);
        let Some(game) = self.game.as_ref() else {
            return false;
        };
        let Some(fleet) = game.fleets.get(index) else {
            return false;
        };
        // Only your own fleets take orders.
        if usize::try_from(fleet.owner).is_ok_and(|owner| owner != me) {
            return false;
        }
        if fleet.waypoints.len() >= Self::WAYPOINT_MAX {
            return false;
        }

        // Where it actually lands, and what it lands on. The nearest object
        // within reach wins; nothing in reach leaves it in deep space at the
        // point asked for.
        let (at, target, target_class) = match self.nearest_scan(asked, snap) {
            Some((object, position)) => {
                let (target, class) = self.waypoint_target(object);
                (position, target, class)
            }
            None => (asked, None, stars_core::fleet::grobj::POSITION),
        };

        let Some(game) = self.game.as_ref() else {
            return false;
        };
        let Some(fleet) = game.fleets.get(index) else {
            return false;
        };
        let from = fleet
            .waypoints
            .last()
            .map_or(fleet.position, |w| w.position);
        #[allow(clippy::cast_possible_truncation)]
        let distance = stars_core::movement::distance(from, at) as i32;
        // A leg that goes nowhere is refused, which is what stops a snap onto
        // the waypoint the fleet is already sitting at from adding anything.
        if distance <= 0 {
            return false;
        }
        let warp = self.suggested_warp(index, distance);

        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let Some(fleet) = game.fleets.get_mut(index) else {
            return false;
        };
        fleet.waypoints.push(stars_core::fleet::Waypoint {
            position: at,
            target,
            target_class,
            warp,
            task: stars_formats::task::NONE,
            transport: None,
            task_data: Vec::new(),
        });
        let last = fleet.waypoints.len() - 1;
        if fleet.waypoints.len() == 2 {
            fleet.warp = Some(warp);
        }
        self.log_waypoint(index, last, true);
        // `FAddWayPoint` leaves the new waypoint as the one in hand
        // (`pscan->iwp = sel.iwpAct + 1`, then `ChangeScanSel`), so a second
        // shift-click carries on from it and Delete takes it back off.
        self.selection.waypoint = Some(last);
        self.dirty = true;
        true
    }

    /// What a waypoint records when it lands on something.
    ///
    /// `FAddWayPoint` stores the object's own id and its `grobj` class in the
    /// waypoint's high nibble (`1058:7504`), and the class matters on its own:
    /// a bare id cannot say whether it means planet 7 or fleet 7.
    #[must_use]
    pub fn waypoint_target(&self, object: ScanObject) -> (Option<u16>, u8) {
        let game = self.game.as_ref();
        match object {
            ScanObject::Planet(id) => (u16::try_from(id).ok(), stars_core::fleet::grobj::PLANET),
            ScanObject::Fleet(index) => (
                game.and_then(|game| game.fleets.get(index)).map(|f| f.id),
                stars_core::fleet::grobj::FLEET,
            ),
            ScanObject::Thing(thing) => {
                let id = game.and_then(|game| match thing {
                    ScanThing::Minefield(i) => game.minefields.get(i).map(|f| f.id),
                    ScanThing::Packet(i) => game.packets.get(i).map(|p| p.id),
                    ScanThing::Wormhole(i) => game.wormholes.get(i).map(|w| w.id),
                    ScanThing::Trader(i) => game.traders.get(i).map(|t| t.id),
                });
                (id, stars_core::fleet::grobj::THING)
            }
        }
    }

    /// Move one of the selected fleet's waypoints, as dragging it does.
    ///
    /// Waypoint 0 is where the fleet is and cannot be dragged. Returns whether
    /// anything moved.
    pub fn move_waypoint(&mut self, waypoint: usize, x: i16, y: i16, snap: f64) -> bool {
        let Some(index) = self.selection.fleet else {
            return false;
        };
        if waypoint == 0 {
            return false;
        }
        let me = self.local_player();
        let asked = stars_core::movement::Point::new(x, y);
        let Some(game) = self.game.as_ref() else {
            return false;
        };
        let Some(fleet) = game.fleets.get(index) else {
            return false;
        };
        if usize::try_from(fleet.owner).is_ok_and(|owner| owner != me)
            || waypoint >= fleet.waypoints.len()
        {
            return false;
        }

        // The drop lands on whatever is in reach, exactly as a new waypoint
        // does — `FHandleWayPointDrag` calls the same `FFindNearestObject`
        // with the same twenty-pixel radius.
        let (at, target, target_class) = match self.nearest_scan(asked, snap) {
            Some((object, position)) => {
                let (target, class) = self.waypoint_target(object);
                (position, target, class)
            }
            None => (asked, None, stars_core::fleet::grobj::POSITION),
        };

        let Some(game) = self.game.as_ref() else {
            return false;
        };
        let Some(fleet) = game.fleets.get(index) else {
            return false;
        };
        let from = fleet.waypoints[waypoint - 1].position;
        #[allow(clippy::cast_possible_truncation)]
        let distance = stars_core::movement::distance(from, at) as i32;
        let warp = self.suggested_warp(index, distance.max(1));

        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let Some(fleet) = game.fleets.get_mut(index) else {
            return false;
        };
        let leg = &mut fleet.waypoints[waypoint];
        leg.position = at;
        leg.target = target;
        leg.target_class = target_class;
        leg.warp = warp;
        if waypoint == 1 {
            fleet.warp = Some(warp);
        }
        self.log_waypoint(index, waypoint, false);
        self.dirty = true;
        true
    }

    /// Whether a waypoint now sits exactly on the one before or after it.
    ///
    /// `FHandleWayPointDrag` tests this on release and, when it holds, puts
    /// the waypoint back where it was and asks whether to delete it instead —
    /// dragging a waypoint onto its neighbour is how the original lets you
    /// throw one away from the map.
    #[must_use]
    pub fn waypoint_meets_neighbour(&self, waypoint: usize) -> bool {
        let Some(index) = self.selection.fleet else {
            return false;
        };
        let Some(game) = self.game.as_ref() else {
            return false;
        };
        let Some(fleet) = game.fleets.get(index) else {
            return false;
        };
        let Some(leg) = fleet.waypoints.get(waypoint) else {
            return false;
        };
        if waypoint == 0 {
            return false;
        }
        let before = fleet.waypoints[waypoint - 1].position == leg.position;
        let after = fleet
            .waypoints
            .get(waypoint + 1)
            .is_some_and(|next| next.position == leg.position);
        before || after
    }

    /// Put a waypoint back where a drag picked it up, without logging a move.
    ///
    /// The original never wrote the new point in the first place; this engine
    /// moves as it drags, so it has to undo.
    pub fn revert_waypoint(&mut self, waypoint: usize, to: stars_core::movement::Point) -> bool {
        self.move_waypoint(waypoint, to.x, to.y, 0.0)
    }

    /// Drop one of the selected fleet's waypoints.
    ///
    /// Returns whether one went.
    pub fn delete_waypoint(&mut self, waypoint: usize) -> bool {
        let Some(index) = self.selection.fleet else {
            return false;
        };
        if waypoint == 0 {
            return false;
        }
        let me = self.local_player();
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let Some(fleet) = game.fleets.get_mut(index) else {
            return false;
        };
        if usize::try_from(fleet.owner).is_ok_and(|owner| owner != me)
            || waypoint >= fleet.waypoints.len()
        {
            return false;
        }
        let fleet_word = (u16::try_from(fleet.owner.max(0)).unwrap_or(0) << 9) | (fleet.id & 0x1ff);
        fleet.waypoints.remove(waypoint);
        // Removing one can leave its neighbours on the same point, and the
        // original collapses that pair rather than leaving a leg of zero
        // length behind (`DeleteCurWayPoint`, `1050:9b08`).
        let doubled = fleet
            .waypoints
            .get(waypoint)
            .is_some_and(|next| next.position == fleet.waypoints[waypoint - 1].position);
        if doubled {
            fleet.waypoints.remove(waypoint);
        }
        if fleet.waypoints.len() < 2 {
            fleet.warp = None;
        }
        let mut record = |index: usize| {
            self.orders.push(stars_formats::LogRecord::delete_waypoint(
                stars_formats::FleetOrderDelete {
                    fleet_id: fleet_word,
                    order_index: u16::try_from(index).unwrap_or(1),
                    delete_extra: false,
                },
            ));
        };
        record(waypoint);
        if doubled {
            record(waypoint);
        }
        self.dirty = true;
        true
    }

    /// Which waypoint the Waypoint Task tile is about.
    ///
    /// The original's tile is always about `sel.iwpAct`, the waypoint the
    /// scanner has in hand. With nothing in hand it falls back to the first
    /// leg, which is what the fleet is actually doing.
    #[must_use]
    pub fn task_waypoint(&self) -> Option<usize> {
        let fleet = self.pane_fleet()?;
        let held = self.selection.waypoint.unwrap_or(1);
        (held < fleet.waypoints.len()).then_some(held)
    }

    /// The waypoint the Waypoint Task tile is about, if there is one.
    #[must_use]
    pub fn task_leg(&self) -> Option<&stars_core::fleet::Waypoint> {
        self.pane_fleet()?.waypoints.get(self.task_waypoint()?)
    }

    /// Change the task of the waypoint in hand.
    ///
    /// Changing it clears the old task's payload: the ten bytes mean something
    /// different under every task, and carrying a Transport's cargo table into
    /// a Patrol would set a nonsense range.
    ///
    /// Returns whether anything changed.
    pub fn set_waypoint_task(&mut self, task: u8) -> bool {
        let Some(waypoint) = self.task_waypoint() else {
            return false;
        };
        let Some(index) = self.pane_fleet_index() else {
            return false;
        };
        if !self.own_fleet(index) {
            return false;
        }
        let Some(leg) = self
            .game
            .as_mut()
            .and_then(|game| game.fleets.get_mut(index))
            .and_then(|fleet| fleet.waypoints.get_mut(waypoint))
        else {
            return false;
        };
        if leg.task == task {
            return false;
        }
        leg.task = task;
        leg.task_data = vec![0; 10];
        leg.transport = (task == stars_formats::task::TRANSPORT)
            .then(|| stars_formats::TransportTask::decode(&leg.task_data))
            .flatten();
        self.log_waypoint(index, waypoint, false);
        self.dirty = true;
        true
    }

    /// Write one word of the task payload of the waypoint in hand.
    ///
    /// The ten bytes are a union: Lay Mine Field and Transfer Fleet keep their
    /// one setting in word 0, Patrol keeps its range in word 1 (the warp it
    /// patrols at is word 0), and Transport fills all five with a packed
    /// quantity and action per cargo.
    pub fn set_waypoint_task_word(&mut self, word: usize, value: u16) -> bool {
        let Some(waypoint) = self.task_waypoint() else {
            return false;
        };
        let Some(index) = self.pane_fleet_index() else {
            return false;
        };
        if !self.own_fleet(index) || word >= 5 {
            return false;
        }
        let Some(leg) = self
            .game
            .as_mut()
            .and_then(|game| game.fleets.get_mut(index))
            .and_then(|fleet| fleet.waypoints.get_mut(waypoint))
        else {
            return false;
        };
        if leg.task_data.len() < 10 {
            leg.task_data.resize(10, 0);
        }
        let at = word * 2;
        let bytes = value.to_le_bytes();
        if leg.task_data[at..at + 2] == bytes {
            return false;
        }
        leg.task_data[at..at + 2].copy_from_slice(&bytes);
        self.log_waypoint(index, waypoint, false);
        self.dirty = true;
        true
    }

    /// Read one word of the task payload of the waypoint in hand.
    #[must_use]
    pub fn waypoint_task_word(&self, word: usize) -> u16 {
        self.task_leg()
            .and_then(|leg| leg.task_data.get(word * 2..word * 2 + 2))
            .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]))
    }

    /// Set one cargo's instruction on a Transport task.
    ///
    /// Each of the five words is packed `quantity:12, action:4`, so a quantity
    /// cannot exceed 4095 whatever is typed.
    pub fn set_waypoint_transport(
        &mut self,
        slot: usize,
        action: stars_formats::XferAction,
        quantity: u16,
    ) -> bool {
        if slot >= 5 {
            return false;
        }
        let packed = (quantity.min(0x0fff)) | (u16::from(action.to_raw()) << 12);
        if !self.set_waypoint_task_word(slot, packed) {
            return false;
        }
        // Keep the decoded view beside the raw bytes, since that is what the
        // simulation reads.
        let (Some(waypoint), Some(index)) = (self.task_waypoint(), self.pane_fleet_index()) else {
            return true;
        };
        if let Some(leg) = self
            .game
            .as_mut()
            .and_then(|game| game.fleets.get_mut(index))
            .and_then(|fleet| fleet.waypoints.get_mut(waypoint))
        {
            leg.transport = stars_formats::TransportTask::decode(&leg.task_data);
        }
        true
    }

    /// One cargo's instruction on a Transport task.
    #[must_use]
    pub fn waypoint_transport(&self, slot: usize) -> (stars_formats::XferAction, u16) {
        let word = self.waypoint_task_word(slot);
        (
            stars_formats::XferAction::from_raw(u8::try_from(word >> 12).unwrap_or(0)),
            word & 0x0fff,
        )
    }

    /// The fleet the pane is about, as an index.
    #[must_use]
    fn pane_fleet_index(&self) -> Option<usize> {
        self.survey_subject().fleet_index()
    }

    /// Whether a fleet takes orders from the player at the keyboard.
    fn own_fleet(&self, index: usize) -> bool {
        let me = self.local_player();
        self.game
            .as_ref()
            .and_then(|game| game.fleets.get(index))
            .is_some_and(|fleet| usize::try_from(fleet.owner).map_or(true, |owner| owner == me))
    }

    /// The note the Waypoint Task tile writes under its dropdowns.
    ///
    /// `DrawShipWayPtOrders` (`1050:0912`) picks one per task and paints the
    /// warnings in red. The wording here is this project's own — the
    /// original's notices are its authored prose — but which note appears
    /// when is the original's.
    #[must_use]
    pub fn waypoint_task_note(&self) -> Option<(String, bool)> {
        let leg = self.task_leg()?;
        let fleet = self.pane_fleet()?;
        match leg.task {
            stars_formats::task::SCRAP => Some((
                "The whole fleet is broken up here. Some of its minerals come back.".to_string(),
                false,
            )),
            stars_formats::task::MERGE => (leg.target_class != stars_core::fleet::grobj::FLEET)
                .then(|| {
                    (
                        "This waypoint is not on a fleet, so there is nothing to merge into."
                            .to_string(),
                        true,
                    )
                }),
            stars_formats::task::COLONIZE => Some(if fleet.cargo.colonists <= 0 {
                (
                    "The fleet is carrying no colonists. Load some before it gets there."
                        .to_string(),
                    true,
                )
            } else {
                (
                    "The ships are broken up on arrival to supply the new colony.".to_string(),
                    false,
                )
            }),
            stars_formats::task::LAY_MINES => {
                let designs = self
                    .game
                    .as_ref()
                    .and_then(|game| {
                        usize::try_from(fleet.owner)
                            .ok()
                            .and_then(|owner| game.designs.get(owner))
                    })
                    .map_or(&[][..], Vec::as_slice);
                let rate: i32 = (0..3)
                    .map(|kind| stars_core::minefield::mines_laid(fleet, designs, kind))
                    .sum();
                Some(if rate > 0 {
                    (format!("This fleet lays {rate} mines a year."), false)
                } else {
                    (
                        "No ship in this fleet carries a mine layer, so nothing will be laid."
                            .to_string(),
                        true,
                    )
                })
            }
            _ => None,
        }
    }

    /// Delete the waypoint the map has in hand.
    ///
    /// `FHandleKey` (`1018:165a`) sends **Backspace** (`VK_BACK`) and
    /// **Delete** (`VK_DELETE`) straight here whenever the selection is a
    /// fleet — no mode, no modifier, and, unlike the drag, **no question
    /// first**. Only a waypoint dropped onto its neighbour gets asked about;
    /// the key just does it.
    ///
    /// `DeleteCurWayPoint` (`1050:9b08`) beeps and does nothing when the fleet
    /// holds only waypoint 0, or when the one in hand *is* waypoint 0 — that
    /// is where the fleet is, and it is not an order.
    ///
    /// The key passes `fBackup = 8`, which is what makes the selection fall
    /// back to the waypoint **before** the one that went rather than stepping
    /// on to the one after.
    ///
    /// Returns whether one went.
    pub fn delete_current_waypoint(&mut self) -> bool {
        let Some(waypoint) = self.selection.waypoint else {
            return false;
        };
        if waypoint == 0 || !self.delete_waypoint(waypoint) {
            return false;
        }
        let held = self
            .selection
            .fleet
            .and_then(|index| self.game.as_ref()?.fleets.get(index))
            .map_or(0, |fleet| fleet.waypoints.len());
        self.selection.waypoint = (held > 0).then(|| (waypoint - 1).min(held - 1));
        true
    }

    /// Which of the selected fleet's waypoints is at a point, if any
    /// (`FNearAWayPoint`, `1058:8074`).
    ///
    /// `tolerance` is in galaxy units; the original works in screen pixels and
    /// converts, which comes to the same thing.
    #[must_use]
    pub fn waypoint_at(&self, x: i16, y: i16, tolerance: f64) -> Option<usize> {
        let fleet = self.selection.fleet?;
        let game = self.game.as_ref()?;
        let record = game.fleets.get(fleet)?;
        let at = stars_core::movement::Point::new(x, y);
        // The **nearest** one, not the first in reach: the original picks the
        // waypoint out with `FFindNearestObject`, which answers with whatever
        // is closest.
        record
            .waypoints
            .iter()
            .enumerate()
            .skip(1)
            .map(|(index, w)| (index, stars_core::movement::distance(w.position, at)))
            .filter(|(_, d)| *d <= tolerance)
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(index, _)| index)
    }

    // --- The scanner -------------------------------------------------------
    //
    // `ScannerWndProc` (`1058:0032`) and `DrawScanner` (`1058:108a`). The map
    // itself; see `docs/ui/scanner.md`.

    /// The zoom percentages, `vrgpctZoom` (`1068:0da4`) verbatim: nine levels
    /// from a quarter size to four times.
    pub const ZOOM_PERCENT: [i16; 9] = [25, 38, 50, 75, 100, 125, 150, 200, 400];

    /// What the current zoom shows, in per cent.
    #[must_use]
    pub fn scan_zoom_percent(&self) -> i16 {
        let index = usize::try_from(i32::from(self.scan_zoom) + 4).unwrap_or(4);
        Self::ZOOM_PERCENT[index.min(8)]
    }

    /// Scale a distance from galaxy units to screen units (`PtToScan`,
    /// `1058:0efc`).
    ///
    /// The original works in integers and shifts rather than multiplying by the
    /// percentage, so the two disagree by a fraction at some zooms — 38% is
    /// really three eighths. This is the shift arithmetic, not the table.
    #[must_use]
    pub fn scan_scale(&self, d: i32) -> i32 {
        match self.scan_zoom {
            1 => (d * 5) >> 2,
            2 => (d * 3) >> 1,
            3 => d << 1,
            4 => d << 2,
            -1 => (d * 3) >> 2,
            -2 => d >> 1,
            -3 => (d * 3) >> 3,
            -4 => d >> 2,
            _ => d,
        }
    }

    /// Zoom in or out one step, stopping at the ends.
    pub fn scan_zoom_by(&mut self, steps: i8) {
        self.scan_zoom = (self.scan_zoom + steps).clamp(-4, 4);
    }

    /// Turn a galaxy position into a scanner one (`LogicalToScan`,
    /// `1058:744e`).
    ///
    /// Note the **y flip**: the scanner mirrors the galaxy's y about the
    /// universe's height, so a planet stored at the top of the file is drawn at
    /// the bottom of the map.
    #[must_use]
    pub fn logical_to_scan(&self, x: i32, y: i32, height: i32) -> (i32, i32) {
        (self.scan_scale(x), self.scan_scale(height - y))
    }

    // --- The fleet pane ----------------------------------------------------
    //
    // The same window as the planet pane, with a different tile table
    // (`rgtileShip`, `1120:090e`) when a fleet is selected. See
    // `docs/ui/fleet-pane.md`.

    /// The fleet the pane is showing, if one is selected.
    #[must_use]
    pub fn pane_fleet(&self) -> Option<&stars_core::fleet::Fleet> {
        let SurveySubject::Fleet(index) = self.survey_subject() else {
            return None;
        };
        self.game.as_ref().and_then(|g| g.fleets.get(index))
    }

    /// The designs of whoever owns the fleet on show.
    fn pane_fleet_designs(&self) -> &[stars_core::design::ShipDesign] {
        let (Some(game), Some(fleet)) = (self.game.as_ref(), self.pane_fleet()) else {
            return &[];
        };
        usize::try_from(fleet.owner)
            .ok()
            .and_then(|owner| game.designs.get(owner))
            .map_or(&[][..], Vec::as_slice)
    }

    /// Where the fleet is: the planet's name, or `In Deep Space`
    /// (`DrawShipPlanet`, `1050:17b6`).
    #[must_use]
    pub fn fleet_location_title(&self) -> String {
        let Some(fleet) = self.pane_fleet() else {
            return "In Deep Space".to_string();
        };
        let Some(id) = fleet.orbiting else {
            return "In Deep Space".to_string();
        };
        let id = i16::try_from(id).unwrap_or(-1);
        self.universe
            .as_ref()
            .and_then(|u| {
                u.planets_resolved()
                    .into_iter()
                    .find(|p| i16::try_from(p.id).is_ok_and(|p| p == id))
                    .and_then(|p| p.name)
            })
            .map_or_else(|| format!("Planet #{id}"), ToString::to_string)
    }

    /// The **Fleet Waypoints** tile: where it has come from, where it is going,
    /// and what the leg costs (`DrawShipOrders`, `1050:0000`).
    #[must_use]
    pub fn fleet_waypoints_tile(&self) -> Vec<(String, String)> {
        let Some(fleet) = self.pane_fleet() else {
            return Vec::new();
        };
        let here = fleet.waypoints.first();
        let next = fleet.waypoints.get(1);
        let mut rows = vec![(
            "Coming From".to_string(),
            here.map_or_else(
                || format!("({}, {})", fleet.position.x, fleet.position.y),
                |w| format!("({}, {})", w.position.x, w.position.y),
            ),
        )];
        let Some(next) = next else {
            rows.push(("Next Way Pt".to_string(), "(none)".to_string()));
            return rows;
        };
        rows.push((
            "Next Way Pt".to_string(),
            format!("({}, {})", next.position.x, next.position.y),
        ));
        let warp = i32::from(next.warp);
        rows.push((
            "Warp Factor".to_string(),
            if warp == 0 {
                "(stopped)".to_string()
            } else {
                warp.to_string()
            },
        ));
        let distance = stars_core::movement::distance(fleet.position, next.position);
        rows.push(("Distance".to_string(), format!("{distance:.0} l.y.")));
        // A year covers the square of the warp factor.
        let per_year = warp * warp;
        rows.push((
            "Travel Time".to_string(),
            if per_year <= 0 {
                "never".to_string()
            } else {
                let years = distance / f64::from(per_year);
                format!("{years:.1} years")
            },
        ));
        let designs = self.pane_fleet_designs();
        if !designs.is_empty() && warp > 0 {
            let ife = self
                .game
                .as_ref()
                .and_then(|g| {
                    usize::try_from(fleet.owner)
                        .ok()
                        .and_then(|o| g.players.get(o))
                })
                .is_some_and(|p| p.race.has_lrt(stars_core::race::lrt::IFE));
            #[allow(clippy::cast_possible_truncation)]
            let fuel = fleet.fuel_use(designs, next.warp, distance as i32, ife);
            rows.push(("Est Fuel Usage".to_string(), format!("{fuel}kT")));
        }
        rows
    }

    /// The **Waypoint Task** tile (`DrawShipWayPtOrders`, `1050:0912`).
    #[must_use]
    pub fn fleet_task_tile(&self) -> String {
        self.pane_fleet()
            .and_then(|f| f.waypoints.get(1))
            .map_or_else(
                || "(no task here)".to_string(),
                |w| task_name(w.task).to_string(),
            )
    }

    /// The **Fuel & Cargo** tile (`DrawShipCargo`, `1050:1a54`).
    #[must_use]
    pub fn fleet_cargo_tile(&self) -> Vec<(String, String)> {
        let Some(fleet) = self.pane_fleet() else {
            return Vec::new();
        };
        let designs = self.pane_fleet_designs();
        let mut rows = vec![(
            "Fuel".to_string(),
            if designs.is_empty() {
                format!("{}mg", fleet.cargo.fuel)
            } else {
                format!("{} of {}mg", fleet.cargo.fuel, fleet.fuel_capacity(designs))
            },
        )];
        for (name, amount) in ["Ironium", "Boranium", "Germanium"]
            .iter()
            .zip(fleet.cargo.minerals.iter())
        {
            rows.push(((*name).to_string(), format!("{amount}kT")));
        }
        rows.push((
            "Colonists".to_string(),
            format!("{}kT", fleet.cargo.colonists),
        ));
        if !designs.is_empty() {
            rows.push((
                "Cargo".to_string(),
                format!(
                    "{} of {}kT",
                    fleet.cargo.minerals.iter().sum::<i32>() + fleet.cargo.colonists,
                    fleet.cargo_capacity(designs)
                ),
            ));
        }
        rows
    }

    /// The **Fleet Composition** tile: which designs, and how many of each
    /// (`DrawFleetComp`, `1050:1e72`).
    #[must_use]
    pub fn fleet_composition_tile(&self) -> Vec<(String, String)> {
        let Some(fleet) = self.pane_fleet() else {
            return Vec::new();
        };
        let designs = self.pane_fleet_designs();
        fleet
            .stacks
            .iter()
            .filter(|stack| stack.count > 0)
            .map(|stack| {
                let name = designs
                    .get(usize::from(stack.design))
                    .filter(|d| d.hull_id >= 0 && !d.name.is_empty())
                    .map_or_else(|| format!("Design #{}", stack.design), |d| d.name.clone());
                (name, stack.count.to_string())
            })
            .collect()
    }

    // --- The mine survey pane ---------------------------------------------
    //
    // `DrawMineSurvey` (`1028:065a`): whatever is selected, summarised — a
    // planet's environment and minerals, a fleet's cargo and orders, or one of
    // the space objects. See `docs/ui/mine-survey-pane.md`.

    /// What the survey pane is summarising.
    #[must_use]
    pub fn survey_subject(&self) -> SurveySubject {
        // The Fleets screen is always about a fleet; the scanner is about
        // whichever of the two was last clicked, so that cycling through the
        // things at one spot swaps the pane as it goes.
        // A space object wins, as `sel.grobj == grobjThing` does.
        if self.screen == Screen::Galaxy {
            if let Some(thing) = self.selection.thing {
                return SurveySubject::Thing(thing);
            }
        }
        if self.screen == Screen::Fleets
            || (self.screen == Screen::Galaxy && self.selection.on_fleet)
        {
            if let Some(index) = self.selection.fleet {
                if self.game.as_ref().is_some_and(|g| index < g.fleets.len()) {
                    return SurveySubject::Fleet(index);
                }
            }
        }
        match self.pane_planet() {
            Some(planet) => SurveySubject::Planet(planet.id),
            None => SurveySubject::DeepSpace,
        }
    }

    /// The pane's title: `"<name> Summary"`, or `"Deep Space"` when nothing is
    /// selected (`SetMineralTitleBar`, `1028:47dc`).
    #[must_use]
    pub fn survey_title(&self) -> String {
        match self.survey_subject() {
            SurveySubject::DeepSpace => "Deep Space".to_string(),
            SurveySubject::Planet(_) => format!("{} Summary", self.planet_pane_title()),
            SurveySubject::Fleet(index) => format!("{} Summary", self.fleet_display_name(index)),
            SurveySubject::Thing(thing) => format!("{} Summary", self.thing_name(thing)),
        }
    }

    /// What the pane says about the space object selected.
    ///
    /// `DrawMineSurvey` (`1028:065a`) switches on the object's `ith` and gives
    /// each kind a shape of its own. All four get the picture plinth the fleet
    /// gets — a 64-pixel bitmap out of `hdibThings` in a black square — but a
    /// wormhole and the Mystery Trader belong to nobody, so the second square
    /// that holds an owner's emblem is not drawn for them at all.
    #[must_use]
    pub fn survey_thing(&self) -> crate::survey::ThingSummary {
        let mut out = crate::survey::ThingSummary::default();
        let SurveySubject::Thing(thing) = self.survey_subject() else {
            return out;
        };
        let Some(game) = self.game.as_ref() else {
            return out;
        };
        match thing {
            ScanThing::Minefield(index) => {
                let Some(field) = game.minefields.get(index) else {
                    return out;
                };
                // The three minefield kinds are the first three pictures.
                out.picture = field.kind.min(2);
                out.emblem = true;
                let kind = MINEFIELD_KINDS
                    .get(usize::from(field.kind))
                    .copied()
                    .unwrap_or("Standard");
                #[allow(clippy::cast_possible_truncation)]
                let radius = field.radius() as i32;
                // The rate the pane prints is what the field would lose this
                // year, which counts the planets inside it.
                let inside = game
                    .planets
                    .iter()
                    .chain(game.known_planets.iter())
                    .filter_map(|planet| planet.position)
                    .filter(|at| field.contains(*at))
                    .count();
                let demolition = usize::try_from(field.owner)
                    .ok()
                    .and_then(|owner| game.players.get(owner))
                    .is_some_and(|player| player.race.prt() == Some(stars_core::race::Prt::Sd));
                let decay = stars_core::minefield::decay_amount(
                    field,
                    i32::try_from(inside).unwrap_or(i32::MAX),
                    demolition,
                );
                out.rows = vec![
                    format!("Location:  ({}, {})", field.position.x, field.position.y),
                    format!("Field Type:  {kind}"),
                    format!("Field Radius:  {radius} l.y. ({} mines)", field.mines),
                    format!("Decay rate:  {decay} / year"),
                ];
                // One's own fields are counted: this one of that many.
                if usize::try_from(field.owner).is_ok_and(|owner| owner == self.local_player()) {
                    let mine = game
                        .minefields
                        .iter()
                        .filter(|other| other.owner == field.owner)
                        .collect::<Vec<_>>();
                    let which = mine
                        .iter()
                        .position(|other| other.id == field.id)
                        .map_or(1, |at| at + 1);
                    out.rows.push(format!("Field: {which} of {}", mine.len()));
                }
            }
            ScanThing::Packet(index) => {
                let Some(packet) = game.packets.get(index) else {
                    return out;
                };
                // A packet aimed at no planet is salvage, and salvage has its
                // own picture, no speed and nowhere to be going: the original
                // skips both rows and draws the minerals alone.
                let salvage = packet.target == 0;
                out.picture = if salvage { 3 } else { 4 };
                out.emblem = true;
                if !salvage {
                    out.rows.push(format!(
                        "Traveling at Warp {}",
                        packet.warp + stars_core::packet::WARP_BIAS
                    ));
                    let target = game
                        .planets
                        .iter()
                        .chain(game.known_planets.iter())
                        .find(|planet| planet.id == i16::try_from(packet.target).unwrap_or(-1));
                    out.rows.push(format!(
                        "Destination: {}",
                        target.map_or_else(
                            || crate::survey::UNKNOWN.to_string(),
                            |planet| self.planet_name(planet.id)
                        )
                    ));
                }
                // What it is carrying, as a right-aligned `"%s: "` against the
                // amount.
                for (name, amount) in crate::survey::MINERAL_LABELS
                    .iter()
                    .zip(packet.minerals.iter())
                {
                    out.table.push((format!("{name}: "), format!("{amount}kT")));
                }
            }
            ScanThing::Trader(index) => {
                let Some(trader) = game.traders.get(index) else {
                    return out;
                };
                out.picture = 6;
                out.emblem = false;
                // The notice comes first and only until this player has traded
                // — `1 << idPlayer & grbitPlr`, the same mask that stops them
                // trading twice. The wording is this project's own, as the
                // game's message text always is; `docs/formulas/wanderers.md`
                // has what it is asking for.
                let me = u32::try_from(self.local_player()).unwrap_or(0);
                if trader.detected_by & (1u16 << (me & 0x0F)) == 0 {
                    out.notice = Some(
                        "Send it a fleet carrying at least 5,000kT of minerals and it will \
                         take the fleet, ships and all, in exchange for technology."
                            .to_string(),
                    );
                }
                out.rows
                    .push(format!("Trader is traveling at Warp {}.", trader.warp));
            }
            ScanThing::Wormhole(index) => {
                let Some(hole) = game.wormholes.get(index) else {
                    return out;
                };
                out.picture = 5;
                out.emblem = false;
                // The far end is another wormhole, named by the low nine bits
                // of its `idFull` — the same mask the mover uses.
                let far = hole
                    .dest_known
                    .then(|| {
                        game.wormholes
                            .iter()
                            .find(|other| other.id == hole.partner & 0x01FF)
                            .map(|other| format!("({}, {})", other.position.x, other.position.y))
                    })
                    .flatten();
                let values = [
                    format!("({}, {})", hole.position.x, hole.position.y),
                    far.unwrap_or_else(|| crate::survey::UNKNOWN.to_string()),
                    wormhole_stability(hole).to_string(),
                ];
                out.table = crate::survey::WORMHOLE_LABELS
                    .iter()
                    .zip(values)
                    .map(|(label, value)| ((*label).to_string(), value))
                    .collect();
            }
        }
        out
    }

    /// The planet's four headline rows: how good it is, who lives there, and
    /// how old the report is.
    ///
    /// `narrow` is the pane's own narrow form — the original switches every
    /// label when four of the widest would not fit across it, and the pairs
    /// are the string table's own (`idsVal`/`idsVal + 1` and so on).
    #[must_use]
    pub fn survey_planet_rows(&self, narrow: bool) -> Vec<(String, String)> {
        let pick = |(wide, short): (&'static str, &'static str)| {
            if narrow {
                short
            } else {
                wide
            }
        };
        let (Some(planet), Some(race)) = (self.pane_planet(), self.pane_race()) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        if planet.detail != stars_core::planet::Detail::Minimal {
            let value = stars_core::hab::pct_planet_desirability(planet, race);
            // A wide pane also shows what the race could terraform it up to,
            // and only when that is an improvement and positive
            // (`PctPlanetOptValue`).
            let tech = self
                .game
                .as_ref()
                .and_then(|game| game.players.get(self.local_player()))
                .map_or([0u8; 6], |player| player.research.levels);
            let reach = stars_core::terraform::optimal_env(planet, race, tech);
            let optimum = stars_core::ai::colonise::pct_planet_opt_value(planet, race, reach);
            let text = if !narrow && optimum > value && optimum > 0 {
                format!("{value}% ({optimum}%)")
            } else {
                format!("{value}%")
            };
            rows.push((pick(crate::survey::VALUE_LABEL).to_string(), text));
        }
        rows.push((
            pick(crate::survey::POPULATION_LABEL).to_string(),
            match (planet.owner, planet.detail) {
                (None, _) => crate::survey::UNINHABITED.to_string(),
                (Some(_), stars_core::planet::Detail::Full) => {
                    comma_format(i64::from(planet.pop) * 100)
                }
                // Somebody else's planet is only ever an estimate, and the
                // original prints it as `"%c%ld00"` with `%c` a **±**
                // (`0xb1`) — so no comma grouping, and the hundreds put back
                // by the format rather than by the arithmetic.
                (Some(_), stars_core::planet::Detail::Scanned) => {
                    format!("\u{b1}{}00", planet.pop)
                }
                (Some(_), stars_core::planet::Detail::Minimal) => {
                    crate::survey::UNKNOWN_POPULATION.to_string()
                }
            },
        ));
        if let Some(owner) = planet.owner {
            let name = self
                .game
                .as_ref()
                .and_then(|g| usize::try_from(owner).ok().and_then(|o| g.players.get(o)))
                .map_or_else(
                    || format!("player {}", owner + 1),
                    |p| p.plural_name.clone(),
                );
            rows.push((String::new(), name));
        }
        // How old the report is. The original prints the years since the
        // planet was last seen, from `PLANET.turn`; this engine does not keep
        // that stamp, so it can only say so much — a planet the player owns is
        // always current, and anything else is left unsaid rather than guessed
        // at.
        if planet.detail == stars_core::planet::Detail::Full {
            rows.push((
                String::new(),
                if narrow {
                    "Current"
                } else {
                    "Report is current"
                }
                .to_string(),
            ));
        }
        rows
    }

    /// The three environment bars: gravity, temperature and radiation, with
    /// the planet's value and the race's habitable band.
    #[must_use]
    pub fn survey_environment(&self) -> Vec<SurveyBar> {
        let (Some(planet), Some(race)) = (self.pane_planet(), self.pane_race()) else {
            return Vec::new();
        };
        if planet.detail == stars_core::planet::Detail::Minimal {
            return Vec::new();
        }
        // A **Claim Adjuster** looking at somebody else's planet is shown
        // *their* habitable band rather than its own, because it is the band
        // that race's planet will be terraformed towards. The original copies
        // the owner's nine habitability bytes over its own for the length of
        // the drawing and puts them back afterwards (`GetRaceStat(rsMajorAdv)
        // == 3`, and only when the owner's race is known).
        let band = self.survey_band_race().unwrap_or(race);
        let tech = self
            .game
            .as_ref()
            .and_then(|game| game.players.get(self.local_player()))
            .map_or([0u8; 6], |player| player.research.levels);
        let reach = stars_core::terraform::optimal_env(planet, band, tech);
        crate::survey::ENV_LABELS
            .iter()
            .enumerate()
            .map(|(index, (label, _))| SurveyBar {
                label: (*label).to_string(),
                value: env_text(index, planet.env[index]),
                at: i32::from(planet.env[index]),
                low: i32::from(band.env_min[index]),
                high: i32::from(band.env_max[index]),
                immune: band.is_immune(index),
                terraform: (reach[index] != planet.env[index]).then(|| i32::from(reach[index])),
                sum: 0,
            })
            .collect()
    }

    /// Whose habitable band the environment bars are drawn against.
    ///
    /// This player's, except that a **Claim Adjuster** looking at a planet
    /// held by a race it knows is shown that race's instead.
    fn survey_band_race(&self) -> Option<&stars_core::Race> {
        let planet = self.pane_planet()?;
        let me = self.local_player();
        let game = self.game.as_ref()?;
        let mine = game.players.get(me)?;
        if mine.race.prt() != Some(stars_core::race::Prt::Ca) {
            return None;
        }
        let owner = usize::try_from(planet.owner?).ok()?;
        if owner == me {
            return None;
        }
        Some(&game.players.get(owner)?.race)
    }

    /// The three mineral bars: what is on the surface, and how rich the ground
    /// underneath is.
    #[must_use]
    pub fn survey_minerals(&self) -> Vec<SurveyBar> {
        let Some(planet) = self.pane_planet() else {
            return Vec::new();
        };
        if planet.detail == stars_core::planet::Detail::Minimal {
            return Vec::new();
        }
        // What this year's mining will add: the planet's own mines for a
        // planet the player holds, and whatever remote miners of theirs are in
        // orbit of one nobody holds (`EstMineralsMined`). Somebody else's
        // planet gets nothing, because none of it is the player's to mine.
        let mined = self.survey_mining_estimate();
        crate::survey::MINERAL_LABELS
            .iter()
            .enumerate()
            .map(|(index, label)| SurveyBar {
                label: (*label).to_string(),
                value: format!("{}kT", planet.surface_min[index]),
                at: planet.surface_min[index],
                low: 0,
                high: i32::from(planet.min_conc[index]),
                immune: false,
                terraform: None,
                sum: planet.surface_min[index] + mined[index],
            })
            .collect()
    }

    /// What this year's mining will add to each mineral, for the bar's dark
    /// extension.
    fn survey_mining_estimate(&self) -> [i32; 3] {
        let none = [0; 3];
        let (Some(planet), Some(game)) = (self.pane_planet(), self.game.as_ref()) else {
            return none;
        };
        let me = self.local_player();
        match planet.owner {
            // The player's own planet mines with its own mines.
            Some(owner) if usize::try_from(owner).is_ok_and(|o| o == me) => {
                match game.players.get(me) {
                    Some(player) => {
                        stars_core::mining::minerals_mined(planet, &player.race, None, None)
                    }
                    None => none,
                }
            }
            // Nobody's planet: whatever remote miners the player has in orbit.
            None => {
                let Some(player) = game.players.get(me) else {
                    return none;
                };
                let mines: i32 = game
                    .fleets
                    .iter()
                    .filter(|fleet| {
                        usize::try_from(fleet.owner).is_ok_and(|o| o == me)
                            && fleet.orbiting == u16::try_from(planet.id).ok()
                    })
                    .map(|fleet| self.fleet_remote_mines(fleet))
                    .sum();
                if mines <= 0 {
                    return none;
                }
                stars_core::mining::minerals_mined(planet, &player.race, Some(mines), None)
            }
            // Somebody else's: the player mines none of it.
            Some(_) => none,
        }
    }

    /// How many robot mines a fleet carries.
    fn fleet_remote_mines(&self, fleet: &stars_core::fleet::Fleet) -> i32 {
        let Some(game) = self.game.as_ref() else {
            return 0;
        };
        let Some(designs) = usize::try_from(fleet.owner)
            .ok()
            .and_then(|owner| game.designs.get(owner))
        else {
            return 0;
        };
        stars_core::mining::remote_mines(designs, &fleet.stacks)
    }

    /// What the pane says about a fleet.
    ///
    /// `DrawMineSurvey`'s fleet arm. How much is shown turns on how well the
    /// fleet is known: the **ship count** and the **mass** are always there,
    /// the two gauges need a fleet whose insides are visible, and the three
    /// order rows and the mine-sweeping line need one the player commands.
    /// Somebody else's fleet gets its speed alone, and only when that is known.
    ///
    /// `narrow` is the pane's narrow form, which switches every label at once.
    #[must_use]
    pub fn survey_fleet(&self, narrow: bool) -> crate::survey::FleetSummary {
        let pick = |(wide, short): (&'static str, &'static str)| {
            if narrow {
                short
            } else {
                wide
            }
        };
        let mut out = crate::survey::FleetSummary::default();
        let SurveySubject::Fleet(index) = self.survey_subject() else {
            return out;
        };
        let (Some(game), Some(fleet)) = (
            self.game.as_ref(),
            self.game.as_ref().and_then(|g| g.fleets.get(index)),
        ) else {
            return out;
        };
        let designs = game
            .designs
            .get(usize::try_from(fleet.owner).unwrap_or(usize::MAX))
            .map_or(&[][..], Vec::as_slice);
        // The original's `det == 7`: a fleet the player commands, and so the
        // only one whose cargo, orders and hulls are all on file.
        let ours = usize::try_from(fleet.owner).is_ok_and(|owner| owner == self.local_player());

        let ships: i32 = fleet.stacks.iter().map(|s| s.count).sum();
        out.ships = format!("Ship Count: {ships}");
        out.mass = format!(
            "{}{}kT",
            pick(crate::survey::MASS_LABEL),
            fleet.mass(designs)
        );

        if ours && !designs.is_empty() {
            let fuel = fleet.fuel_capacity(designs);
            out.fuel = Some(crate::survey::Gauge {
                segments: vec![(fleet.cargo.fuel, crate::survey::CARGO_COLOURS[4])],
                total: fuel,
                label: format!("{} of {}mg", fleet.cargo.fuel, fuel),
            });
            // The cargo gauge stacks all four holds — the three minerals and
            // then the colonists — against the fleet's capacity.
            let hold = fleet.cargo_capacity(designs);
            let carried: i32 = fleet.cargo.minerals.iter().sum::<i32>() + fleet.cargo.colonists;
            let mut segments: Vec<(i32, [u8; 3])> = (0..3)
                .map(|i| (fleet.cargo.minerals[i], crate::survey::CARGO_COLOURS[i]))
                .collect();
            segments.push((fleet.cargo.colonists, crate::survey::CARGO_COLOURS[3]));
            out.cargo = Some(crate::survey::Gauge {
                segments,
                total: hold,
                label: format!("{carried} of {hold}kT"),
            });
        }

        if ours {
            // Where it is going. The name is `PszGetLocName`, so a leg that
            // lands on nothing reads `Space (x, y)` rather than a bare pair.
            let next = fleet.waypoints.get(1);
            out.orders.push(format!(
                "{}{}",
                pick(crate::survey::WAYPOINT_LABEL),
                match next {
                    None => crate::survey::NO_WAYPOINT.to_string(),
                    Some(w) => self.location_name(w.target_class, w.target, w.position),
                }
            ));
            if let Some(next) = next {
                out.orders.push(format!(
                    "{}{}",
                    pick(crate::survey::TASK_LABEL),
                    task_name(next.task)
                ));
            }
            out.orders.push(match next.map(|w| w.warp) {
                None | Some(0) => pick(crate::survey::STOPPED).to_string(),
                Some(crate::survey::STARGATE_WARP) => crate::survey::USE_STARGATE.to_string(),
                Some(warp) => format!("{}{warp}", pick(crate::survey::WARP_LABEL)),
            });
            let sweep = stars_core::minefield::fleet_sweep(fleet, designs);
            if sweep > 0 {
                out.sweeping = Some(format!(
                    "This fleet can destroy up to {sweep} mines per year."
                ));
            }
        } else if let Some(warp) = fleet.warp {
            // Somebody else's: the speed, and only because it was scanned.
            out.orders.push(if warp == 0 {
                pick(crate::survey::STOPPED).to_string()
            } else {
                format!("{}{warp}", pick(crate::survey::WARP_LABEL))
            });
        }
        out
    }

    /// What the game calls a point a waypoint lands on (`PszGetLocName`,
    /// `1038:3b08`).
    ///
    /// A planet, a fleet or a space object by name; `Deep Space` for the
    /// nowhere point `(-1, -1)`; and `Space (%d, %d)` for anywhere else.
    #[must_use]
    pub fn location_name(
        &self,
        class: u8,
        id: Option<u16>,
        at: stars_core::movement::Point,
    ) -> String {
        if let Some(id) = id {
            match class {
                PLANET_CLASS => return self.planet_name(i16::try_from(id).unwrap_or(-1)),
                FLEET_CLASS => {
                    if let Some(index) = self
                        .game
                        .as_ref()
                        .and_then(|game| game.fleets.iter().position(|f| f.id == id))
                    {
                        return self.fleet_display_name(index);
                    }
                }
                _ => {}
            }
        }
        if at.x == -1 && at.y == -1 {
            "Deep Space".to_string()
        } else {
            format!("Space ({}, {})", at.x, at.y)
        }
    }

    // --- The message pane -------------------------------------------------
    //
    // `MessageWndProc` (`1030:5c92`) and `SetMsgTitle` (`1030:7218`): the pane
    // shows the year's messages one at a time, with Prev and Next stepping over
    // the ones the player has filtered.

    /// The local player's messages for the year, in the order they were sent.
    #[must_use]
    pub fn messages(&self) -> Vec<&stars_core::message::Message> {
        let me = self.local_player();
        self.game
            .as_ref()
            .map(|game| game.messages.iter().filter(|m| m.player == me).collect())
            .unwrap_or_default()
    }

    /// How many messages the year holds (`cMsg`).
    #[must_use]
    pub fn message_count(&self) -> usize {
        self.messages().len()
    }

    /// The message the pane is showing, if it is showing one.
    #[must_use]
    pub fn current_message(&self) -> Option<stars_core::message::Message> {
        let index = usize::try_from(self.message_index).ok()?;
        self.messages().get(index).map(|m| (*m).clone())
    }

    /// The next message to show, stepping over what the filter hides
    /// (`IMsgNext`, `1030:7808`).
    ///
    /// `filtered_only` inverts the test, which is how the pane walks the
    /// *hidden* messages when the player asks to see them.
    #[must_use]
    pub fn message_next(&self, filtered_only: bool) -> Option<usize> {
        let messages = self.messages();
        let filter = self.message_filter();
        let mut i = self.message_index;
        loop {
            i += 1;
            let index = usize::try_from(i).ok()?;
            let message = messages.get(index)?;
            if filter.hidden(message.id) == filtered_only || (self.view_filtered && !filtered_only)
            {
                return Some(index);
            }
        }
    }

    /// The previous message to show (`IMsgPrev`, `1030:78d8`).
    #[must_use]
    pub fn message_previous(&self, filtered_only: bool) -> Option<usize> {
        let messages = self.messages();
        let filter = self.message_filter();
        let mut i = self.message_index;
        loop {
            i -= 1;
            let index = usize::try_from(i).ok()?;
            let message = messages.get(index)?;
            if filter.hidden(message.id) == filtered_only || (self.view_filtered && !filtered_only)
            {
                return Some(index);
            }
        }
    }

    /// Show the next message; returns whether there was one.
    pub fn show_next_message(&mut self) -> bool {
        match self.message_next(false) {
            Some(index) => {
                self.message_index = i32::try_from(index).unwrap_or(-1);
                true
            }
            None => false,
        }
    }

    /// Show the previous message; returns whether there was one.
    pub fn show_previous_message(&mut self) -> bool {
        match self.message_previous(false) {
            Some(index) => {
                self.message_index = i32::try_from(index).unwrap_or(-1);
                true
            }
            None => false,
        }
    }

    /// Show the first message, as Home does.
    pub fn show_first_message(&mut self) {
        self.message_index = -1;
        self.show_next_message();
    }

    /// Show the last message, as End does.
    pub fn show_last_message(&mut self) {
        self.message_index = i32::try_from(self.message_count()).unwrap_or(0);
        self.show_previous_message();
    }

    /// Silence the kind of message being shown, or stop silencing it.
    ///
    /// This is the `+` key and the button at the left of the title bar. The
    /// whole family of wordings goes with it; see [`Self::filter_message`].
    /// Returns whether anything changed.
    pub fn toggle_message_filter(&mut self) -> bool {
        let Some(message) = self.current_message() else {
            return false;
        };
        let hidden = self.message_filter().hidden(message.id);
        self.filter_message(message.id, !hidden)
    }

    /// Whether anything the player has been sent this year is filtered.
    ///
    /// The original keeps a second bitfield of the ids it has *sent*
    /// (`bitfMsgSent`) and shows the view-filtered button only where the two
    /// masks overlap: there is no point offering to reveal messages that do not
    /// exist. Here the year's own messages serve as that mask.
    #[must_use]
    pub fn has_filtered_messages(&self) -> bool {
        let filter = self.message_filter();
        self.messages().iter().any(|m| filter.hidden(m.id))
    }

    /// Show the filtered messages too, or stop showing them — the `-` key and
    /// the button at the right of the title bar.
    ///
    /// The original refuses when nothing is filtered, and moves off the current
    /// message if the change would leave the pane showing something it should
    /// not.
    pub fn toggle_view_filtered(&mut self) -> bool {
        if !self.has_filtered_messages() {
            self.view_filtered = false;
            return false;
        }
        self.view_filtered = !self.view_filtered;
        let showing_filtered = self
            .current_message()
            .is_some_and(|m| self.message_filter().hidden(m.id));
        if showing_filtered != self.view_filtered {
            let index = self
                .message_next(self.view_filtered)
                .or_else(|| self.message_previous(self.view_filtered));
            self.message_index = index.and_then(|i| i32::try_from(i).ok()).unwrap_or(-1);
        }
        true
    }

    /// What the pane's title bar says.
    ///
    /// `"Year: 2401  Messages: 3 of 12"`, or `"Year: 2401  Messages: (none)"`
    /// — `idsYearDCMessagesDD` and `idsYearDCMessagesNone`.
    #[must_use]
    pub fn message_title(&self) -> String {
        let year = self.game.as_ref().map_or(2400, stars_core::GameState::year);
        let count = self.message_count();
        if count == 0 {
            return format!("Year: {year}  Messages: (none)");
        }
        let at = self.message_index + 1;
        format!("Year: {year}  Messages: {at} of {count}")
    }

    /// What the middle button says: `Goto`, or `View` for a battle.
    #[must_use]
    pub fn message_goto_label(&self) -> &'static str {
        match self.message_goto() {
            stars_core::message::Goto::Position(_, _) => "View",
            _ => "Goto",
        }
    }

    /// What the message being shown points at.
    #[must_use]
    pub fn message_goto(&self) -> stars_core::message::Goto {
        use stars_core::message::Goto;

        let Some(message) = self.current_message() else {
            return Goto::None;
        };
        // A filtered message's button is dead, even when it is on screen
        // because the player asked to see the filtered ones.
        if !self.view_filtered && self.message_filter().hidden(message.id) {
            return Goto::None;
        }
        let fleets: Vec<u16> = self
            .game
            .as_ref()
            .map(|g| g.fleets.iter().map(|f| f.id).collect())
            .unwrap_or_default();
        message.goto(&fleets)
    }

    /// Follow the message to what it is about: the Goto button, and Enter.
    ///
    /// Returns whether it went anywhere.
    pub fn message_goto_follow(&mut self) -> bool {
        use stars_core::message::Goto;

        match self.message_goto() {
            Goto::Planet(id) => {
                self.selection.planet = Some(id);
                self.screen = Screen::Planets;
                true
            }
            Goto::Fleet(id) => {
                let Some(index) = self
                    .game
                    .as_ref()
                    .and_then(|g| g.fleets.iter().position(|f| f.id == id))
                else {
                    return false;
                };
                self.selection.fleet = Some(index);
                self.screen = Screen::Fleets;
                true
            }
            Goto::Position(x, y) => {
                // The original opens the battle at that place; this engine
                // shows the map there instead, which is as far as it goes.
                self.screen = Screen::Galaxy;
                let _ = (x, y);
                true
            }
            Goto::Thing(_) | Goto::Elsewhere | Goto::None => false,
        }
    }

    /// The body of the pane: what the message says, or why it is not saying
    /// anything.
    #[must_use]
    pub fn message_body(&self) -> String {
        let count = self.message_count();
        let Some(message) = self.current_message() else {
            if count > 0 {
                // `idsMessagesHaveSentYearFilteredIfWant`.
                return "All the messages you have been sent this year are filtered out. \
                        If you want to view these messages, press the button at the right \
                        of the title bar."
                    .to_string();
            }
            return String::new();
        };
        if !self.view_filtered && self.message_filter().hidden(message.id) {
            // `idsMessageTypeHasFilteredWillShownDefault`.
            return "This message type has been filtered out and will not be shown by \
                    default anymore."
                .to_string();
        }
        message.summary()
    }

    /// Silence a kind of message for the local player, or stop silencing it.
    ///
    /// The whole family of wordings goes with it — see
    /// [`stars_core::message::set_filtered`] — because the game's several
    /// sentences for one happening are one thing to a reader.
    ///
    /// This changes nothing about the game: a filtered message is still sent
    /// and still written to the file, and all that changes is whether the list
    /// steps over it.
    ///
    /// Returns whether it changed.
    pub fn filter_message(&mut self, id: u16, hidden: bool) -> bool {
        use stars_formats::{LogRecord, LogRecordType};

        let me = self.local_player();
        let Some(player) = self.game.as_mut().and_then(|g| g.players.get_mut(me)) else {
            return false;
        };
        let before = player.message_filter;
        stars_core::message::set_filtered(&mut player.message_filter, id, hidden);
        let filter = player.message_filter;
        if filter == before {
            return false;
        }

        // The record carries the whole bitfield, so the last one wins and an
        // earlier one is replaced rather than stacked.
        if self
            .orders
            .last()
            .is_some_and(|r| r.record_type == LogRecordType::MessageFilter)
        {
            self.orders.pop();
        }
        self.orders.push(LogRecord::message_filter(&filter));
        self.dirty = true;
        true
    }

    /// The local player's message filter.
    #[must_use]
    pub fn message_filter(&self) -> stars_formats::MessageFilter {
        let me = self.local_player();
        self.game
            .as_ref()
            .and_then(|g| g.players.get(me))
            .map_or_else(stars_formats::MessageFilter::new, |p| p.message_filter)
    }

    /// Define or retune one of the local player's battle plans.
    ///
    /// `slot` is the plan's position in the list; passing the position one past
    /// the end appends a plan, which is how a new one is made. The owner and
    /// the slot are stamped into the record rather than taken from `plan`: the
    /// original routes a type-30 record by the nibbles in its first byte, and
    /// the two default plans that ship sharing a plan id show those nibbles
    /// cannot be trusted to say where a plan lives.
    ///
    /// Returns whether the change was accepted — the host applies the same
    /// bounds to the record this logs.
    pub fn set_battle_plan_definition(
        &mut self,
        slot: usize,
        plan: &stars_formats::BattlePlanRecord,
    ) -> bool {
        use stars_formats::{BattlePlanChange, LogRecord};

        let me = self.local_player();
        let mut plan = plan.clone();
        plan.race_id = u8::try_from(me).unwrap_or(0) & 0x0F;
        plan.plan_id = u8::try_from(slot).unwrap_or(0) & 0x0F;
        let change = BattlePlanChange {
            plan,
            delete: false,
        };
        let Ok(record) = LogRecord::battle_plan(&change) else {
            return false;
        };
        // The editor logs a record per change, where the game's dialog logs one
        // when it is dismissed. A run of edits to the same plan is collapsed to
        // the last of them: the host applies whichever survives, and there is
        // no sense sending a record per keystroke.
        if self.orders.last().is_some_and(|r| {
            r.as_battle_plan()
                .is_some_and(|c| !c.delete && usize::from(c.plan.plan_id) == slot)
        }) {
            self.orders.pop();
        }
        if !self.apply_and_log(vec![record]) {
            return false;
        }
        self.battle_plans_edited = true;
        true
    }

    /// Delete one of the local player's battle plans.
    ///
    /// The plans after it move up a slot and every one of the player's fleets
    /// pointing at or past it has its plan index decremented, which is what
    /// `DeleteBattlePlan` does; the fleets that changed are marked so their
    /// blocks are rewritten too.
    pub fn delete_battle_plan(&mut self, slot: usize) -> bool {
        use stars_formats::{BattlePlanChange, BattlePlanRecord, LogRecord};

        let me = self.local_player();
        let owner = i16::try_from(me).unwrap_or(-1);
        let Some(game) = self.game.as_ref() else {
            return false;
        };
        // Every fleet at or past the slot has its index moved, so each one
        // needs its block rewritten.
        let touched: Vec<(i16, u16)> = game
            .fleets
            .iter()
            .filter(|f| f.owner == owner && usize::from(f.battle_plan) >= slot)
            .map(|f| (f.owner, f.id))
            .collect();

        let change = BattlePlanChange {
            plan: BattlePlanRecord {
                race_id: u8::try_from(me).unwrap_or(0) & 0x0F,
                plan_id: u8::try_from(slot).unwrap_or(0) & 0x0F,
                tactic: stars_formats::PLAN_DELETED,
                primary_target: 0,
                secondary_target: 0,
                attack_who: 0,
                name: String::new(),
                trailing: Vec::new(),
            },
            delete: true,
        };
        let Ok(record) = LogRecord::battle_plan(&change) else {
            return false;
        };
        if !self.apply_and_log(vec![record]) {
            return false;
        }
        self.fleet_edits.extend(touched);
        self.battle_plans_edited = true;
        true
    }

    /// The object id word a log uses for one of the loaded game's fleets.
    fn fleet_word(&self, fleet: usize) -> Option<u16> {
        let record = self.game.as_ref()?.fleets.get(fleet)?;
        let owner = u16::try_from(record.owner.max(0)).ok()?;
        Some((owner << 9) | (record.id & 0x1ff))
    }

    /// Start playing a battle.
    pub fn open_battle(&mut self, index: usize) {
        self.vcr = self.battles.get(index).map(Vcr::new);
        self.playing = false;
    }

    /// The planetary items the selected planet's owner may build.
    ///
    /// The filter is the game's own: an Alternate Reality race builds no
    /// planetary installation at all, and a Claim Adjuster is never offered
    /// terraforming — its planets are already at their optimum every turn. See
    /// `ground::template_allows`.
    #[must_use]
    pub fn buildable_items(&self) -> Vec<(u16, &'static str)> {
        use stars_core::production::item;
        const ITEMS: [(u16, &str); 5] = [
            (item::AUTO_MINE, "mines"),
            (item::AUTO_FACTORY, "factories"),
            (item::AUTO_DEFENSE, "defences"),
            (item::AUTO_ALCHEMY, "mineral alchemy"),
            (item::AUTO_MAX_TERRAFORM, "terraforming"),
        ];
        let prt = self
            .selected_planet()
            .and_then(|p| p.owner)
            .and_then(|o| usize::try_from(o).ok())
            .and_then(|o| self.game.as_ref().and_then(|g| g.players.get(o)))
            .and_then(|p| p.race.prt());
        ITEMS
            .into_iter()
            .filter(|(id, _)| stars_core::ground::template_allows(prt, *id))
            .collect()
    }

    /// Add an item to the selected planet's queue.
    ///
    /// Adding to an entry that is already there increases its count rather than
    /// making a second one, which is what the original's `AddItemToQueue` does
    /// and what a player expects.
    pub fn queue_add(&mut self, item: u16, count: i32) {
        self.note_edit();
        let Some(planet) = self.selected_planet_mut() else {
            return;
        };
        match planet.queue.iter_mut().find(|e| !e.ship && e.item == item) {
            Some(entry) => entry.count += count,
            None => planet.queue.push(stars_core::production::QueueItem {
                count,
                item,
                ship: false,
                completion: 0,
            }),
        }
    }

    /// Remove one entry from the selected planet's queue.
    pub fn queue_remove(&mut self, index: usize) {
        self.note_edit();
        if let Some(planet) = self.selected_planet_mut() {
            if index < planet.queue.len() {
                planet.queue.remove(index);
            }
        }
    }

    /// Move an entry up or down the selected planet's queue.
    ///
    /// Order matters: production works the queue from the front, so an item
    /// ahead of another takes its resources first.
    pub fn queue_move(&mut self, index: usize, delta: isize) {
        self.note_edit();
        let Some(planet) = self.selected_planet_mut() else {
            return;
        };
        let Some(target) = index.checked_add_signed(delta) else {
            return;
        };
        if index < planet.queue.len() && target < planet.queue.len() {
            planet.queue.swap(index, target);
        }
    }

    /// Mark the selected planet's queue as changed, so saving rewrites it.
    fn note_edit(&mut self) {
        self.dirty = true;
        if let Some(id) = self.selection.planet {
            self.edited.insert(id);
        }
    }

    /// The selected planet, mutably — only ever one the player owns in full.
    fn selected_planet_mut(&mut self) -> Option<&mut Planet> {
        let id = self.selection.planet?;
        let game = self.game.as_mut()?;
        game.planets.iter_mut().find(|p| p.id == id)
    }

    /// The selected planet, if it is still in the game.
    #[must_use]
    pub fn selected_planet(&self) -> Option<&Planet> {
        let id = self.selection.planet?;
        let game = self.game.as_ref()?;
        game.planets
            .iter()
            .chain(game.known_planets.iter())
            .find(|p| p.id == id)
    }

    /// Every planet the player knows of, owned first, each with whether it is
    /// theirs to command.
    ///
    /// The two lists are kept apart by the loader precisely because they mean
    /// different things — `planets` can be simulated, `known_planets` has only
    /// been seen. That distinction *is* the fog of war and the map should show
    /// it.
    #[must_use]
    pub fn visible_planets(&self) -> Vec<(&Planet, bool)> {
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let mut out: Vec<(&Planet, bool)> = game.planets.iter().map(|p| (p, true)).collect();
        out.extend(game.known_planets.iter().map(|p| (p, false)));
        out
    }

    /// The bounding box of everything with a known position, as
    /// `(min_x, min_y, max_x, max_y)`.
    ///
    /// Returns `None` when no planet has a position — a game loaded without its
    /// `.xy`.
    #[must_use]
    pub fn extent(&self) -> Option<(f32, f32, f32, f32)> {
        let mut bounds: Option<(f32, f32, f32, f32)> = None;
        for (planet, _) in self.visible_planets() {
            let Some(p) = planet.position else { continue };
            let (x, y) = (f32::from(p.x), f32::from(p.y));
            bounds = Some(match bounds {
                None => (x, y, x, y),
                Some((lx, ly, hx, hy)) => (lx.min(x), ly.min(y), hx.max(x), hy.max(y)),
            });
        }
        bounds
    }
}

/// Find the `.xy` that belongs with a save file.
///
/// Stars! keeps one universe file per game, named after it. Where it sits
/// depends on how the saves were filed: a game played in one directory has it
/// alongside, but a set of turns archived a directory per year has it once at
/// the top. Both layouts appear in this repository's fixtures, so look beside
/// the save first and then one level up.
fn find_universe(path: &Path) -> Option<Universe> {
    let dir = path.parent()?;
    for candidate in [Some(dir), dir.parent()].into_iter().flatten() {
        let Ok(entries) = std::fs::read_dir(candidate) else {
            continue;
        };
        let found = entries.filter_map(Result::ok).find(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("xy"))
        });
        if let Some(entry) = found {
            if let Some(universe) = std::fs::read(entry.path())
                .ok()
                .and_then(|b| Universe::decode(&b).ok())
            {
                return Some(universe);
            }
        }
    }
    None
}

/// Find the `.hN` history file that belongs with a player file.
///
/// Stars! keeps a player's score year by year in a history file named for the
/// game and numbered for the player, beside their `.mN`. Nothing else in this
/// engine needs it — it is the Score sheet's timeline and nothing more — so a
/// missing one is not an error.
fn find_history(path: &Path) -> Option<StarsFile> {
    let extension = path.extension()?.to_str()?;
    let number = extension
        .strip_prefix('m')
        .or(extension.strip_prefix('M'))?;
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let bytes = std::fs::read(path.with_extension(format!("h{number}"))).ok()?;
    StarsFile::decode(&bytes).ok()
}

/// Group a number with commas, as `CommaFormatLong` does for the planet pane's
/// population figure.
fn comma_format(value: i64) -> String {
    let digits = value.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if value < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// An environment reading in the units the game shows (`PszCalcEnvVar`).
///
/// Gravity runs from 0.12g to 8g on a curve, temperature from -200°C to 200°C
/// and radiation from 0 to 100mR, all from a click in `0..=100`.
pub(crate) fn env_text(variable: usize, clicks: i8) -> String {
    let clicks = i32::from(clicks);
    match variable {
        // `PszCalcGravity` (`planet.c`): the curve is two straight pieces
        // measured from the middle, and the bottom half is the reciprocal of
        // the top — which is what makes gravity read 0.12g at one end, 1.00 in
        // the middle and 8.00 at the other. The original prints no unit; the
        // row's own label carries it.
        0 => {
            let d = (clicks - 50).abs();
            let mut value = if d < 26 {
                d * 4 + 100
            } else {
                (d - 25) * 24 + 200
            };
            if clicks < 50 {
                value = 10000 / value.max(1);
            }
            format!("{}.{:02}", value / 100, (value % 100).abs())
        }
        1 => format!("{}\u{b0}C", clicks * 4 - 200),
        _ => format!("{clicks}mR"),
    }
}

/// The name of a waypoint task, as the survey pane spells it.
pub(crate) fn task_name(task: u8) -> &'static str {
    match task {
        stars_formats::task::TRANSPORT => "Transport",
        stars_formats::task::COLONIZE => "Colonize",
        stars_formats::task::REMOTE_MINING => "Remote Mining",
        stars_formats::task::MERGE => "Merge With Fleet",
        stars_formats::task::SCRAP => "Scrap Fleet",
        stars_formats::task::LAY_MINES => "Lay Mine Field",
        stars_formats::task::PATROL => "Patrol",
        stars_formats::task::ROUTE => "Route",
        stars_formats::task::TRANSFER => "Transfer Fleet",
        _ => "(no task here)",
    }
}

/// The figure a distance is printed as (`PszGetDistance`, `1038:3f00`).
///
/// The original works in **hundredths of a light year, rounded to nearest** —
/// `(long)(distance * 100 + 0.5)` — and then prints the whole and the remainder
/// with `%ld.%ld`.
///
/// That format has a quirk worth keeping, because it is the game's: the
/// remainder carries **no leading zero**, so three and five hundredths of a
/// light year reads `3.5`, not `3.05`.
#[must_use]
pub fn distance_figure(
    from: stars_core::movement::Point,
    to: stars_core::movement::Point,
) -> String {
    #[allow(clippy::cast_possible_truncation)]
    let hundredths = (stars_core::movement::distance(from, to) * 100.0 + 0.5) as i64;
    format!("{}.{}", hundredths / 100, hundredths % 100)
}

/// A distance in the words the player actually sees.
///
/// `PszGetDistance` appends `"  l.y."` or `"  Light Years"` — the choice is on
/// the font's height, `dyArial8 < 15` — but its only caller is the status bar,
/// which throws that away: `DrawScannerSBar` finds the first space in the
/// result and overwrites everything after it with `idsLy` (`ly`) or
/// `idsLightYears` (`light years`), chosen on the **window's** width. So the
/// double space in the format never reaches the screen and neither does the
/// abbreviation with the full stops.
///
/// `wide` is whether the scanner is wider than
/// [`crate::statusbar::WIDE_UNIT_WIDTH`].
#[must_use]
pub fn distance_text(
    from: stars_core::movement::Point,
    to: stars_core::movement::Point,
    wide: bool,
) -> String {
    Distance {
        figure: distance_figure(from, to),
        from: None,
    }
    .text(wide)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_line_reflects_loaded_state() {
        let mut app = App::new();
        assert!(app.status_line().contains("no game"));
        app.game = Some(GameState::new(1));
        assert!(app.status_line().contains("year 2400"));
    }

    #[test]
    fn every_screen_has_a_title() {
        for screen in Screen::ALL {
            assert!(!screen.title().is_empty());
        }
        assert_eq!(Screen::default(), Screen::Galaxy);
    }

    /// Editing a queue: adding merges, order is preserved, removal works.
    #[test]
    fn the_queue_can_be_edited() {
        use stars_core::production::item;

        let mut app = App::new();
        let mut game = GameState::new(1);
        let mut planet = Planet::unowned(7);
        planet.owner = Some(0);
        game.planets.push(planet);
        game.players
            .push(stars_core::Player::new(stars_core::Race::humanoid()));
        app.game = Some(game);
        app.selection.planet = Some(7);

        app.queue_add(item::MINE, 5);
        app.queue_add(item::FACTORY, 2);
        // Adding the same item again raises the count rather than making a
        // second entry, as `AddItemToQueue` does.
        app.queue_add(item::MINE, 3);
        let queue = &app.selected_planet().unwrap().queue;
        assert_eq!(queue.len(), 2);
        assert_eq!(queue[0].count, 8);
        assert_eq!(queue[0].item, item::MINE);

        // Order matters, because production works the queue from the front.
        app.queue_move(1, -1);
        assert_eq!(app.selected_planet().unwrap().queue[0].item, item::FACTORY);
        // Moving off either end does nothing rather than panicking.
        app.queue_move(0, -1);
        app.queue_move(1, 1);
        assert_eq!(app.selected_planet().unwrap().queue.len(), 2);

        app.queue_remove(0);
        assert_eq!(app.selected_planet().unwrap().queue.len(), 1);
        app.queue_remove(99);
        assert_eq!(app.selected_planet().unwrap().queue.len(), 1);
    }

    /// The build list is the game's own race filter, not a fixed menu.
    #[test]
    fn the_build_list_follows_the_race() {
        use stars_core::production::item;
        use stars_core::race::Prt;

        let build_list_for = |prt: Prt| {
            let mut app = App::new();
            let mut game = GameState::new(1);
            let mut planet = Planet::unowned(0);
            planet.owner = Some(0);
            game.planets.push(planet);
            let mut race = stars_core::Race::humanoid();
            race.attrs[stars_core::race::RaceStat::MajorAdv as usize] = prt as i16;
            game.players.push(stars_core::Player::new(race));
            app.game = Some(game);
            app.selection.planet = Some(0);
            app.buildable_items()
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>()
        };

        // A Claim Adjuster is never offered terraforming: AutoTerraform leaves
        // its planets at their optimum every turn.
        assert!(!build_list_for(Prt::Ca).contains(&item::AUTO_MAX_TERRAFORM));
        assert!(build_list_for(Prt::Ca).contains(&item::AUTO_MINE));
        // An Alternate Reality race builds no planetary installation at all.
        let ar = build_list_for(Prt::Ar);
        assert!(!ar.contains(&item::AUTO_MINE));
        assert!(!ar.contains(&item::AUTO_FACTORY));
        assert!(!ar.contains(&item::AUTO_DEFENSE));
        // Everyone else gets the lot.
        assert_eq!(build_list_for(Prt::Joat).len(), 5);
    }

    /// Generating a turn advances the year and reports what it did.
    #[test]
    fn a_turn_can_be_generated() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let path = root.join("fixtures/games/exodus/2414/exodus.m6");
        if !path.is_file() {
            eprintln!("skipping: no Exodus fixtures");
            return;
        }
        let mut app = App::new();
        app.open(&path).expect("the fixture loads");
        let before = app.game.as_ref().unwrap().year();

        app.generate_turn();
        let turn = app.last_turn.as_ref().expect("a report");
        assert_eq!(turn.year, before + 1);
        assert_eq!(app.game.as_ref().unwrap().year(), before + 1);
        assert!(turn.mined > 0, "planets should have mined something");
        // The report names what it did not do, so a partial turn cannot be
        // mistaken for a complete one.
        assert!(!turn.skipped.is_empty());
    }

    /// Saving an untouched game gives back the bytes it was read from.
    ///
    /// This is the property that makes saving safe at all. Most of a save file
    /// is data this project models partially or not at all, so a save that
    /// re-derived the file would quietly lose whatever was not understood.
    /// Saving here replaces only the blocks the player changed, and when they
    /// have changed nothing the file must come back byte-for-byte.
    #[test]
    fn saving_an_untouched_game_changes_nothing() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let games = root.join("fixtures/games");
        if !games.is_dir() {
            eprintln!("skipping: no fixtures");
            return;
        }

        let mut checked = 0usize;
        let mut stack = vec![games];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let is_save = path.extension().is_some_and(|x| {
                    let x = x.to_string_lossy().to_lowercase();
                    x == "hst" || (x.starts_with('m') && x.len() == 2)
                });
                if !is_save {
                    continue;
                }
                let Ok(original) = std::fs::read(&path) else {
                    continue;
                };
                let mut app = App::new();
                if app.open(&path).is_err() {
                    continue;
                }
                let written = app.to_bytes().expect("an open game saves");
                assert_eq!(
                    written,
                    original,
                    "saving changed {} without being asked to",
                    path.display()
                );
                checked += 1;
            }
        }
        assert!(checked > 100, "expected many save files, checked {checked}");
        eprintln!("saved unchanged, byte for byte: {checked} files");
    }

    /// An edited queue survives a save and a reload, and nothing else moves.
    #[test]
    fn an_edited_queue_survives_a_round_trip() {
        use stars_core::production::item;

        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let path = root.join("fixtures/games/exodus/2414/exodus.m6");
        if !path.is_file() {
            eprintln!("skipping: no Exodus fixtures");
            return;
        }

        let mut app = App::new();
        app.open(&path).expect("the fixture loads");
        // A planet the player owns in full, so its queue is really theirs.
        let target = app
            .game
            .as_ref()
            .unwrap()
            .planets
            .iter()
            .find(|p| p.detail.is_full())
            .map(|p| p.id)
            .expect("an owned planet");
        app.selection.planet = Some(target);
        let before = app.selected_planet().unwrap().queue.len();

        app.queue_add(item::FACTORY, 12);
        assert!(app.dirty);
        let written = app.to_bytes().expect("saves");

        // Read the written bytes back as a fresh game.
        let temp = std::env::temp_dir().join(format!(
            "stars-ui-queue-roundtrip-{}.m6",
            std::process::id()
        ));
        std::fs::write(&temp, &written).expect("writable temp dir");
        let mut reloaded = App::new();
        reloaded.open(&temp).expect("the written file loads");
        let _ = std::fs::remove_file(&temp);

        let planet = reloaded
            .game
            .as_ref()
            .unwrap()
            .planets
            .iter()
            .find(|p| p.id == target)
            .expect("the planet is still there");
        let _ = before;
        let added = planet
            .queue
            .iter()
            .find(|e| !e.ship && e.item == item::FACTORY)
            .expect("the factories are queued");
        assert!(
            added.count >= 12,
            "the twelve factories should have survived, saw {}",
            added.count
        );

        // And the rest of the game came back intact.
        let old = app.game.as_ref().unwrap();
        let new = reloaded.game.as_ref().unwrap();
        assert_eq!(old.planets.len(), new.planets.len());
        assert_eq!(old.fleets.len(), new.fleets.len());
        assert_eq!(old.year(), new.year());
    }

    /// Cargo moves at once and is recorded, and never more than there is.
    #[test]
    fn cargo_moves_and_is_recorded() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let path = root.join("fixtures/games/exodus/2414/exodus.m6");
        if !path.is_file() {
            eprintln!("skipping: no Exodus fixtures");
            return;
        }
        let mut app = App::new();
        app.open(&path).expect("the fixture loads");

        // A fleet in orbit of a planet its owner holds, with room to carry.
        let game = app.game.as_ref().unwrap();
        let Some(index) = game.fleets.iter().position(|f| {
            f.orbiting.is_some_and(|id| {
                game.planets
                    .iter()
                    .any(|p| p.id == i16::try_from(id).unwrap_or(-1) && p.surface_min[0] > 0)
            })
        }) else {
            eprintln!("skipping: no loaded fleet in orbit of a stocked planet");
            return;
        };

        let before = game.fleets[index].cargo.minerals[0];
        let moved = app.transfer_cargo(index, 0, 10);
        let after = app.game.as_ref().unwrap().fleets[index].cargo.minerals[0];
        assert_eq!(after - before, moved, "the hold gained exactly what moved");
        if moved != 0 {
            assert_eq!(app.orders.len(), 1, "and the order was recorded");
            let logged = app.orders[0]
                .as_cargo_transfer()
                .expect("recorded as a cargo transfer");
            assert_eq!(logged.quantities, vec![moved], "at what actually moved");
            assert!(app.dirty);
        }

        // Asking for nothing does nothing, and a bad cargo kind is refused.
        assert_eq!(app.transfer_cargo(index, 0, 0), 0);
        assert_eq!(app.transfer_cargo(index, 99, 5), 0);
    }

    /// Research is clamped, and a destination gives the fleet a course.
    #[test]
    fn research_and_destinations_are_set() {
        let mut app = App::new();
        let mut game = GameState::new(1);
        game.players
            .push(stars_core::Player::new(stars_core::Race::humanoid()));
        let mut planet = Planet::unowned(3);
        planet.position = Some(stars_core::movement::Point::new(100, 200));
        game.planets.push(planet);
        game.fleets.push(stars_core::fleet::Fleet {
            name: None,
            repeat_orders: false,
            direction: None,
            id: 1,
            owner: 0,
            position: stars_core::movement::Point::new(0, 0),
            orbiting: None,
            stacks: Vec::new(),
            cargo: stars_core::fleet::Cargo::default(),
            battle_plan: 0,
            warp: None,
            waypoints: Vec::new(),
        });
        app.game = Some(game);

        app.set_research(0, 250);
        assert_eq!(app.game.as_ref().unwrap().players[0].research_pct, 100);
        app.set_research(0, 40);
        assert_eq!(app.game.as_ref().unwrap().players[0].research_pct, 40);

        app.set_destination(0, 3, 7);
        let fleet = &app.game.as_ref().unwrap().fleets[0];
        assert_eq!(fleet.waypoints.len(), 2);
        assert_eq!(fleet.waypoints[1].position.x, 100);
        assert_eq!(fleet.waypoints[1].warp, 7);
        assert_eq!(fleet.warp, Some(7));

        // A planet that is not there leaves the fleet alone.
        app.set_destination(0, 999, 5);
        assert_eq!(app.game.as_ref().unwrap().fleets[0].waypoints.len(), 2);
    }

    /// Opening a real save loads its planets, its universe and its battles.
    #[test]
    fn opening_a_save_brings_in_the_universe_beside_it() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let path = root.join("fixtures/games/exodus/2414/exodus.m6");
        if !path.is_file() {
            eprintln!("skipping: no Exodus fixtures");
            return;
        }
        let mut app = App::new();
        app.open(&path).expect("the fixture loads");

        let game = app.game.as_ref().expect("a game");
        assert!(!game.planets.is_empty());
        // The .xy sits beside it, so planets have coordinates and the map has
        // an extent to draw into.
        assert!(
            app.extent().is_some(),
            "planet coordinates should have come from the universe file"
        );
        assert!(
            game.planets.iter().any(|p| p.name.is_some()),
            "and their names with them"
        );
        assert!(!app.battles.is_empty(), "this year has battle recordings");

        app.open_battle(0);
        assert!(app.vcr.is_some());
    }
}

// --- The Ship and Starbase Designer --------------------------------------

/// Which of the four **View** radio buttons the designer is on
/// (`mdBuild`, set from `wParam - 0x812` in `SlotDlg`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesignView {
    /// `mdBuildShdef`: the player's own designs. The only view from which a
    /// design can be edited or deleted.
    #[default]
    Existing,
    /// `mdBuildHuldef`: the bare hulls the player has researched.
    Hulls,
    /// `mdBuildEnemyShdef`: designs the player has seen other players fly.
    Enemy,
    /// `mdBuildComp`: no design at all — the dropdown becomes a category
    /// filter and the parts list fills the window.
    Components,
}

impl DesignView {
    /// All four, in the order the radio group lists them.
    pub const ALL: [DesignView; 4] = [
        DesignView::Existing,
        DesignView::Hulls,
        DesignView::Enemy,
        DesignView::Components,
    ];

    /// The label beside the radio button.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            DesignView::Existing => "Existing Designs",
            DesignView::Hulls => "Available Hull Types",
            DesignView::Enemy => "Enemy Hulls",
            DesignView::Components => "Components",
        }
    }
}

/// The design being edited, and where it will be written back to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editing {
    /// The design slot it occupies: `0..16` for a ship, `16..26` for a
    /// starbase — the original's `ishdefBuild`.
    pub slot: usize,
    /// Whether **Copy** created it, so **Cancel** must throw it away again
    /// (`fHullCopy`).
    pub fresh: bool,
    /// The working copy (`shdefBuild`). Nothing outside the dialog sees it
    /// until OK.
    pub design: ShipDesign,
}

/// What the Ship and Starbase Designer is showing.
///
/// One dialog with two faces: a browser over designs and hulls, and — once
/// **Copy** or **Edit** is pressed — an editor over one design. `mdBuild`
/// carries both in the original, which is why [`Designer::editing`] and
/// [`Designer::view`] are separate here.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Designer {
    /// `fStarbaseMode`: whether the **Design** radio is on Starbase.
    pub starbase: bool,
    /// Which **View** radio is on.
    pub view: DesignView,
    /// The dropdown's selection, as an index into [`App::designer_list`].
    pub selected: usize,
    /// The design being edited, when the dialog is in edit mode.
    pub editing: Option<Editing>,
    /// Which entry of the parts-list filter dropdown is chosen.
    pub filter: usize,
    /// Which part in the list is picked out, if any.
    pub selected_part: Option<usize>,
    /// Which slot of the schematic is picked out (`iselSlot`).
    pub selected_slot: Option<usize>,
    /// A question waiting to be answered before a design is deleted or edited.
    pub confirm: Option<String>,
    /// What the last refused action was, for the dialog to explain.
    pub complaint: Option<String>,
}

/// One row of the designer's parts list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartRow {
    /// Which component table it came from.
    pub category: u16,
    /// Its index in that table.
    pub item: usize,
    /// The name the game shows.
    pub name: String,
    /// Mass of one, in kT.
    pub mass: i32,
    /// Which of the game's component pictures it is drawn with (`ibmp`).
    pub picture: u16,
}

/// One slot of the schematic, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchematicSlot {
    /// Where it sits, as `(column, row)` in half-cells of the designer's grid.
    pub cell: (i32, i32),
    /// What the hull slot accepts.
    pub allowed: u16,
    /// How many fit.
    pub capacity: u8,
    /// What is fitted, if anything: its name and how many.
    pub fitted: Option<(String, u8)>,
    /// The line under the picture: `up to 3`, `needs 1`, or `2 of 4`.
    pub label: String,
}

impl App {
    /// Open the Ship and Starbase Designer (`ShipBuilder`, F4).
    pub fn open_designer(&mut self) {
        if self.game.is_none() {
            return;
        }
        // The dialog opens on the player's first design, as `ShipBuilder` does
        // with `NthValidShdef(0)`.
        self.designer = Some(Designer::default());
    }

    /// Close it, throwing away an unsaved copy the way **Cancel** does.
    pub fn close_designer(&mut self) {
        if let Some(designer) = &self.designer {
            if designer.editing.is_some() {
                self.designer_cancel();
            }
        }
        self.designer = None;
    }

    /// Who is building, for the parts and cost model.
    #[must_use]
    fn designer_builder(&self) -> Option<stars_core::parts::Builder<'_>> {
        let game = self.game.as_ref()?;
        let player = game.players.get(self.local_player())?;
        let starbase = self.designer_peek.as_ref().map_or_else(
            || self.designer.as_ref().is_some_and(|d| d.starbase),
            |design| design.hull_id >= 32,
        );
        Some(stars_core::parts::Builder::player(player).designing_starbase(starbase))
    }

    /// The range of design slots the current mode uses: ships or starbases.
    #[must_use]
    fn designer_slots(&self) -> std::ops::Range<usize> {
        let first = usize::from(stars_core::startup::FIRST_STARBASE_SLOT);
        if self.designer.as_ref().is_some_and(|d| d.starbase) {
            first..first + stars_core::design::MAX_STARBASE_DESIGNS
        } else {
            0..stars_core::design::MAX_SHIP_DESIGNS
        }
    }

    /// A design slot holds a real design when it names a hull; `-1` is the
    /// game's `fFree`.
    fn designer_design(&self, owner: usize, slot: usize) -> Option<&ShipDesign> {
        self.game
            .as_ref()?
            .designs
            .get(owner)?
            .get(slot)
            .filter(|d| d.hull_id >= 0)
    }

    /// The slots of the current mode that hold a design, in order.
    #[must_use]
    pub fn designer_used_slots(&self) -> Vec<usize> {
        let me = self.local_player();
        self.designer_slots()
            .filter(|slot| self.designer_design(me, *slot).is_some())
            .collect()
    }

    /// What the dropdown lists, in the order it lists it.
    ///
    /// `FillBuildDD` fills it four different ways: the player's designs, the
    /// hulls they can build, every design they have seen an opponent fly, or —
    /// in Components view and while editing — the parts-list filter names.
    #[must_use]
    pub fn designer_list(&self) -> Vec<String> {
        let Some(designer) = self.designer.as_ref() else {
            return Vec::new();
        };
        if designer.editing.is_some() || designer.view == DesignView::Components {
            return self
                .designer_filters()
                .iter()
                .map(|(_, name)| (*name).to_string())
                .collect();
        }
        let me = self.local_player();
        match designer.view {
            DesignView::Existing | DesignView::Components => self
                .designer_used_slots()
                .into_iter()
                .filter_map(|slot| self.designer_design(me, slot))
                .map(|d| d.name.clone())
                .collect(),
            DesignView::Hulls => self
                .designer_hulls()
                .into_iter()
                .map(|h| h.name.to_string())
                .collect(),
            DesignView::Enemy => self
                .designer_enemy_designs()
                .into_iter()
                .map(|(owner, slot)| {
                    let name = self
                        .designer_design(owner, slot)
                        .map_or(String::new(), |d| d.name.clone());
                    format!("{} {name}", self.player_name(owner))
                })
                .collect(),
        }
    }

    /// The parts-list filter names for the current mode (`rgidsParts` /
    /// `rgidsPartsSB`).
    #[must_use]
    pub fn designer_filters(&self) -> &'static [(u16, &'static str)] {
        if self.designer.as_ref().is_some_and(|d| d.starbase) {
            &stars_core::parts::STARBASE_FILTERS
        } else {
            &stars_core::parts::SHIP_FILTERS
        }
    }

    /// The hulls the player may build, in table order — what **Available Hull
    /// Types** lists.
    #[must_use]
    pub fn designer_hulls(&self) -> Vec<&'static stars_core::components::Hull> {
        use stars_core::components::{slot, HULLS, STARBASE_HULLS};
        let Some(who) = self.designer_builder() else {
            return Vec::new();
        };
        let starbase = self.designer.as_ref().is_some_and(|d| d.starbase);
        let (category, table): (u16, &'static [stars_core::components::Hull]) = if starbase {
            (slot::SB_HULL, &STARBASE_HULLS)
        } else {
            (slot::HULL, &HULLS)
        };
        table
            .iter()
            .enumerate()
            .filter(|(item, _)| {
                stars_core::parts::availability(&who, category, *item).is_available()
            })
            .map(|(_, hull)| hull)
            .collect()
    }

    /// Every other player's designs, as `(owner, slot)` — what **Enemy Hulls**
    /// lists, in `NthValidEnemyShdef`'s order: player by player, and within a
    /// player, slot by slot.
    #[must_use]
    pub fn designer_enemy_designs(&self) -> Vec<(usize, usize)> {
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let me = self.local_player();
        let mut out = Vec::new();
        for owner in 0..game.players.len() {
            if owner == me {
                continue;
            }
            for slot in self.designer_slots() {
                if self.designer_design(owner, slot).is_some() {
                    out.push((owner, slot));
                }
            }
        }
        out
    }

    /// The design the dialog is showing: the one being edited, or whichever the
    /// dropdown has selected.
    #[must_use]
    pub fn designer_subject(&self) -> Option<ShipDesign> {
        // A pop-up showing a design wins: it is drawn with the designer's
        // own panel while the dialog itself is shut.
        if let Some(peek) = &self.designer_peek {
            return Some(peek.clone());
        }
        let designer = self.designer.as_ref()?;
        if let Some(editing) = &designer.editing {
            return Some(editing.design.clone());
        }
        let me = self.local_player();
        match designer.view {
            DesignView::Components => None,
            DesignView::Existing => {
                let slot = *self.designer_used_slots().get(designer.selected)?;
                self.designer_design(me, slot).cloned()
            }
            DesignView::Hulls => {
                let hull = self.designer_hulls().get(designer.selected).copied()?;
                Some(Self::designer_empty_design(hull))
            }
            DesignView::Enemy => {
                let (owner, slot) = *self.designer_enemy_designs().get(designer.selected)?;
                self.designer_design(owner, slot).cloned()
            }
        }
    }

    /// A bare hull with nothing on it, which is what **Available Hull Types**
    /// shows and what **Copy** turns into a new design.
    fn designer_empty_design(hull: &stars_core::components::Hull) -> ShipDesign {
        ShipDesign {
            hull_id: hull.id,
            slots: hull
                .real_slots()
                .iter()
                .map(|s| stars_core::design::DesignSlot {
                    category: s.allowed,
                    item: 0,
                    count: 0,
                })
                .collect(),
            name: hull.name.to_string(),
            // A hull's own `ibmp` is the base of its group of four ship
            // pictures, so a fresh design starts on the first of them. Zero
            // here would have given every design the Small Freighter's
            // picture, whatever it was built on.
            picture: u8::try_from(hull.picture).unwrap_or(0),
            stored_armor: if hull.id >= 32 { 1000 } else { 0 },
        }
    }

    /// The design's hull, whichever table it is in.
    #[must_use]
    pub fn designer_hull(design: &ShipDesign) -> Option<&'static stars_core::components::Hull> {
        stars_core::components::hull(design.hull_id)
    }
}

impl App {
    /// The name a player is known by, for a foreign design's label.
    #[must_use]
    pub fn player_name(&self, owner: usize) -> String {
        self.game
            .as_ref()
            .and_then(|game| game.players.get(owner))
            .map_or_else(|| format!("player {}", owner + 1), |p| p.name.clone())
    }

    /// The name the game itself puts a player under, as `PszPlayerName`
    /// (`1038:11f2`) builds it with every flag off — `(iPlayer, 0, 0, 0, 0,
    /// NULL)`, which is the call the Player Relations listbox makes.
    ///
    /// That is the race's **singular** name and nothing else: no "the", no
    /// plural, no player number. A player with no name at all falls back to
    /// string 1374, `"Player %d"` — and the original then appends `"'s"`
    /// (`1038:13df`), which the named branch never does. That looks like a
    /// possessive form leaking out of the wrong branch, but it is what the
    /// binary shows, so it is what this shows.
    #[must_use]
    pub fn psz_player_name(&self, player: usize) -> String {
        let named = self
            .game
            .as_ref()
            .and_then(|game| game.players.get(player))
            .map(|p| p.name.clone())
            .filter(|name| !name.is_empty());
        named.unwrap_or_else(|| format!("Player {}'s", player + 1))
    }

    /// How many ships of the shown design still exist, and how many were ever
    /// built — the two numbers on the plaque under the schematic
    /// (`"%ld of %ld"`, `MANUAL.PDF` p. 9-6).
    ///
    /// The engine does not keep the design's own counters, so the first is
    /// counted off the fleets in play. The second would need `SHDEF.cBuilt`,
    /// which nothing in this project reads yet, so the plaque shows the same
    /// figure twice rather than inventing one.
    #[must_use]
    pub fn designer_plaque(&self) -> Option<(i64, i64)> {
        let designer = self.designer.as_ref()?;
        if designer.editing.is_some() || designer.view != DesignView::Existing {
            return None;
        }
        let me = self.local_player();
        let slot = *self.designer_used_slots().get(designer.selected)?;
        let alive = self.designer_ships_built(me, slot);
        Some((alive, alive))
    }

    /// How many ships a player has flying that were built to one design.
    #[must_use]
    fn designer_ships_built(&self, owner: usize, slot: usize) -> i64 {
        let Some(game) = self.game.as_ref() else {
            return 0;
        };
        let slot = u8::try_from(slot).unwrap_or(u8::MAX);
        game.fleets
            .iter()
            .filter(|f| usize::try_from(f.owner).is_ok_and(|o| o == owner))
            .flat_map(|f| f.stacks.iter())
            .filter(|s| s.design == slot)
            .map(|s| i64::from(s.count))
            .sum()
    }

    /// Whether **Copy Selected Design** may be pressed: only if there is a free
    /// design slot to copy into (`FillBuildDD`).
    #[must_use]
    pub fn designer_can_copy(&self) -> bool {
        let Some(designer) = self.designer.as_ref() else {
            return false;
        };
        if designer.editing.is_some() || designer.view == DesignView::Components {
            return false;
        }
        if self.designer_free_slot().is_none() {
            return false;
        }
        let Some(design) = self.designer_subject() else {
            return false;
        };
        // A foreign design can only be copied if its hull is one the player
        // can build: "You can't copy this ship design because you can't build
        // the hull it's based on."
        if designer.view == DesignView::Enemy {
            return self.designer_can_build_hull(design.hull_id);
        }
        true
    }

    /// Whether the player may build a hull at all.
    #[must_use]
    fn designer_can_build_hull(&self, hull_id: i16) -> bool {
        use stars_core::components::slot;
        let Some(who) = self.designer_builder() else {
            return false;
        };
        let (category, item) = if hull_id >= 32 {
            (slot::SB_HULL, hull_id - 32)
        } else {
            (slot::HULL, hull_id)
        };
        let Ok(item) = usize::try_from(item) else {
            return false;
        };
        stars_core::parts::availability(&who, category, item).is_available()
    }

    /// The first free design slot of the current mode, or `None` when the
    /// player has reached the limit — sixteen ships, ten starbases.
    #[must_use]
    fn designer_free_slot(&self) -> Option<usize> {
        let me = self.local_player();
        self.designer_slots()
            .find(|slot| self.designer_design(me, *slot).is_none())
    }

    /// Whether **Edit Selected Design** may be pressed.
    ///
    /// Only a design with no ships built to it and none in a production queue
    /// can be edited — otherwise the ships in play would silently change shape.
    /// `MANUAL.PDF` p. 9-4 says the same.
    #[must_use]
    pub fn designer_can_edit(&self) -> bool {
        let Some(designer) = self.designer.as_ref() else {
            return false;
        };
        if designer.editing.is_some() || designer.view != DesignView::Existing {
            return false;
        }
        let me = self.local_player();
        let Some(&slot) = self.designer_used_slots().get(designer.selected) else {
            return false;
        };
        self.designer_ships_built(me, slot) == 0 && self.designer_queued(me, slot) == 0
    }

    /// How many of a design a player has in production queues (`CshQueued`).
    #[must_use]
    fn designer_queued(&self, owner: usize, slot: usize) -> i64 {
        let Some(game) = self.game.as_ref() else {
            return 0;
        };
        let item = u16::try_from(slot).unwrap_or(u16::MAX);
        game.planets
            .iter()
            .filter(|p| p.owner == i16::try_from(owner).ok())
            .flat_map(|p| p.queue.iter())
            .filter(|entry| entry.ship && entry.item == item)
            .map(|entry| i64::from(entry.count))
            .sum()
    }

    /// Whether **Delete Design** may be pressed.
    #[must_use]
    pub fn designer_can_delete(&self) -> bool {
        let Some(designer) = self.designer.as_ref() else {
            return false;
        };
        designer.editing.is_none()
            && designer.view == DesignView::Existing
            && !self.designer_used_slots().is_empty()
    }

    /// **Copy Selected Design**: put the shown design into the first free slot
    /// and open it for editing.
    ///
    /// A copy of one of the player's own designs is renamed by
    /// [`stars_core::design::copied_name`]; a bare hull and a foreign design
    /// keep the name they came with. A foreign design also has any component
    /// the player cannot build **stripped out of it**, which is the "the
    /// results may not be perfect" the manual warns about on p. 9-2.
    pub fn designer_copy(&mut self) {
        if !self.designer_can_copy() {
            return;
        }
        let Some(mut design) = self.designer_subject() else {
            return;
        };
        let Some(slot) = self.designer_free_slot() else {
            return;
        };
        let view = self.designer.as_ref().map(|d| d.view);

        if view == Some(DesignView::Existing) {
            design.name = stars_core::design::copied_name(&design.name);
        } else if view == Some(DesignView::Enemy) {
            if let Some(who) = self.designer_builder() {
                for s in &mut design.slots {
                    if s.count == 0 {
                        continue;
                    }
                    let buildable = stars_core::design::slot_part(s).is_some_and(|p| {
                        stars_core::parts::availability(&who, p.category, p.item).is_available()
                    });
                    if !buildable {
                        s.count = 0;
                    }
                }
            }
        }

        if let Some(designer) = self.designer.as_mut() {
            designer.editing = Some(Editing {
                slot,
                fresh: true,
                design,
            });
            designer.selected_slot = None;
            designer.selected_part = None;
            designer.filter = 0;
            designer.complaint = None;
        }
    }

    /// **Edit Selected Design**: open the selected design for editing in place.
    pub fn designer_edit(&mut self) {
        if !self.designer_can_edit() {
            return;
        }
        let Some(designer) = self.designer.as_ref() else {
            return;
        };
        let Some(&slot) = self.designer_used_slots().get(designer.selected) else {
            return;
        };
        let Some(design) = self.designer_design(self.local_player(), slot).cloned() else {
            return;
        };
        if let Some(designer) = self.designer.as_mut() {
            designer.editing = Some(Editing {
                slot,
                fresh: false,
                design,
            });
            designer.selected_slot = None;
            designer.selected_part = None;
            designer.filter = 0;
            designer.complaint = None;
        }
    }

    /// **Delete Design**: free the slot the dropdown is on.
    ///
    /// Every ship built to it is destroyed and nothing is recovered — the
    /// manual is emphatic about that (p. 9-5) — so the caller is expected to
    /// have asked first. [`App::designer_delete_warning`] is the question.
    pub fn designer_delete(&mut self) {
        if !self.designer_can_delete() {
            return;
        }
        let me = self.local_player();
        let Some(designer) = self.designer.as_ref() else {
            return;
        };
        let Some(&slot) = self.designer_used_slots().get(designer.selected) else {
            return;
        };
        let design_slot = u8::try_from(slot).unwrap_or(u8::MAX);
        let queue_item = u16::try_from(slot).unwrap_or(u16::MAX);
        let mine = i16::try_from(me).ok();

        if let Some(game) = self.game.as_mut() {
            // The design itself: a slot with no hull is a free slot.
            if let Some(design) = game.designs.get_mut(me).and_then(|d| d.get_mut(slot)) {
                design.hull_id = -1;
                design.slots.clear();
                design.name.clear();
            }
            // Every ship built to it, and every copy in a queue.
            for fleet in &mut game.fleets {
                if usize::try_from(fleet.owner).is_ok_and(|o| o == me) {
                    fleet.stacks.retain(|s| s.design != design_slot);
                }
            }
            game.fleets.retain(|f| {
                !usize::try_from(f.owner).is_ok_and(|o| o == me) || !f.stacks.is_empty()
            });
            for planet in &mut game.planets {
                if planet.owner == mine {
                    planet
                        .queue
                        .retain(|entry| !(entry.ship && entry.item == queue_item));
                }
            }
        }
        self.dirty = true;
        self.log_design_change(slot);
        if let Some(designer) = self.designer.as_mut() {
            designer.selected = designer.selected.saturating_sub(1);
            designer.confirm = None;
        }
    }

    /// Record a design change in the order log (`LogChangeShDef`, `log.c`).
    ///
    /// This is how a design reaches the host: a state file is not rewritten
    /// with the new design, an `rtLogShDef` order is written beside it and the
    /// host replays it. The header word carries the **full** design slot —
    /// `SHDEF.det`'s five-bit `ishdef`, which is 16..=25 for a starbase — and
    /// the low nibble is the mode: `1` when a design follows, `0` for a bare
    /// delete.
    fn log_design_change(&mut self, slot: usize) {
        use stars_formats::{LogRecord, ShipDesignChange};

        let me = self.local_player();
        let Ok(player) = u8::try_from(me) else {
            return;
        };
        let Ok(index) = u8::try_from(slot) else {
            return;
        };
        let starbase = slot >= usize::from(stars_core::startup::FIRST_STARBASE_SLOT);
        let design = self.designer_design(me, slot).map(|design| {
            stars_core::save::design_record(
                design,
                index & 0x0f,
                starbase,
                u32::try_from(self.designer_ships_built(me, slot)).unwrap_or(0),
            )
        });
        let change = ShipDesignChange {
            mode: u8::from(design.is_some()),
            player: player & 0x0f,
            design_index: index & 0x1f,
            header_high: 0,
            design,
        };
        if let Ok(record) = LogRecord::ship_design(&change) {
            self.orders.push(record);
        }
    }

    /// What deleting the selected design would cost, in the original's own
    /// terms: how many ships would be destroyed and how many removed from
    /// queues. `None` when there is nothing to warn about.
    #[must_use]
    pub fn designer_delete_warning(&self) -> Option<String> {
        let me = self.local_player();
        let designer = self.designer.as_ref()?;
        let &slot = self.designer_used_slots().get(designer.selected)?;
        let name = self.designer_design(me, slot)?.name.clone();
        let alive = self.designer_ships_built(me, slot);
        let queued = self.designer_queued(me, slot);
        match (alive, queued) {
            (0, 0) => None,
            (0, q) => Some(format!(
                "You currently have {q} {name} in production queues. If you delete this \
                 design, these ships will be removed from the queues. Are you sure?"
            )),
            (a, 0) => Some(format!(
                "You currently have {a} {name}. If you delete this design, these ships \
                 will be destroyed. Are you sure?"
            )),
            (a, q) => Some(format!(
                "You currently have {a} {name} and {q} in production. If you delete this \
                 design, these ships will be destroyed and/or removed from the queues. \
                 Are you sure?"
            )),
        }
    }

    /// **OK**: write the working copy back into its slot.
    ///
    /// Refused, as the original refuses it, if a ship design has no engine —
    /// `SlotDlg` checks slot 0 and puts up "This ship design does not have any
    /// engines." The complaint is left in [`Designer::complaint`].
    pub fn designer_ok(&mut self) {
        let Some(designer) = self.designer.as_ref() else {
            return;
        };
        let Some(editing) = designer.editing.clone() else {
            return;
        };
        if !designer.starbase && !editing.design.slots.first().is_some_and(|s| s.count > 0) {
            if let Some(designer) = self.designer.as_mut() {
                designer.complaint = Some(
                    "This ship design does not have any engines. You must add engines \
                     before the design will be accepted."
                        .into(),
                );
            }
            return;
        }

        let me = self.local_player();
        if let Some(game) = self.game.as_mut() {
            if let Some(designs) = game.designs.get_mut(me) {
                if designs.len() <= editing.slot {
                    designs.resize_with(editing.slot + 1, || ShipDesign {
                        name: String::new(),
                        picture: 0,
                        stored_armor: 0,
                        hull_id: -1,
                        slots: Vec::new(),
                    });
                }
                designs[editing.slot] = editing.design;
            }
        }
        self.dirty = true;
        self.log_design_change(editing.slot);
        if let Some(designer) = self.designer.as_mut() {
            designer.editing = None;
            designer.complaint = None;
            designer.view = DesignView::Existing;
        }
        // Land on the design that was just saved.
        let used = self.designer_used_slots();
        if let Some(designer) = self.designer.as_mut() {
            designer.selected = used.iter().position(|s| *s == editing.slot).unwrap_or(0);
        }
    }

    /// **Cancel**: throw the working copy away. A design that **Copy** had just
    /// created never existed.
    pub fn designer_cancel(&mut self) {
        if let Some(designer) = self.designer.as_mut() {
            designer.editing = None;
            designer.complaint = None;
            designer.selected_slot = None;
        }
    }

    /// The parts list, filtered as the dropdown says.
    #[must_use]
    pub fn designer_parts(&self) -> Vec<PartRow> {
        let Some(designer) = self.designer.as_ref() else {
            return Vec::new();
        };
        let Some(who) = self.designer_builder() else {
            return Vec::new();
        };
        let filters = self.designer_filters();
        let mask = filters.get(designer.filter).map_or(0, |(mask, _)| *mask);
        stars_core::parts::filtered(&who, mask)
            .into_iter()
            .map(|p| PartRow {
                category: p.category,
                item: p.item,
                name: p.name.to_string(),
                mass: p.mass,
                picture: p.picture,
            })
            .collect()
    }

    /// The schematic: one entry per real hull slot, with what is in it.
    #[must_use]
    pub fn designer_schematic(&self) -> Vec<SchematicSlot> {
        use stars_core::components::slot as cat;
        let Some(design) = self.designer_subject() else {
            return Vec::new();
        };
        let Some(hull) = Self::designer_hull(&design) else {
            return Vec::new();
        };
        hull.real_slots()
            .iter()
            .enumerate()
            .filter_map(|(i, hull_slot)| {
                let cell = hull.slot_cell(i)?;
                let fitted = design
                    .slots
                    .get(i)
                    .filter(|s| s.count > 0)
                    .and_then(|s| stars_core::design::slot_part(s).map(|p| (p, s.count)));
                let label = match &fitted {
                    Some((_, count)) => format!("{count} of {}", hull_slot.capacity),
                    // An engine slot must be filled; everything else may be
                    // left empty, so it reads "up to" rather than "needs".
                    None if hull_slot.allowed & cat::ENGINE != 0 => {
                        format!("needs {}", hull_slot.capacity)
                    }
                    None => format!("up to {}", hull_slot.capacity),
                };
                Some(SchematicSlot {
                    cell,
                    allowed: hull_slot.allowed,
                    capacity: hull_slot.capacity,
                    fitted: fitted.map(|(p, count)| (p.name.to_string(), count)),
                    label,
                })
            })
            .collect()
    }

    /// The words on a slot's empty picture: the categories it accepts, in the
    /// order the game's bits run.
    #[must_use]
    pub fn designer_slot_kinds(allowed: u16) -> String {
        use stars_core::components::slot as cat;
        const NAMES: [(u16, &str); 13] = [
            (cat::ENGINE, "Engine"),
            (cat::SCANNER, "Scanner"),
            (cat::SHIELD, "Shield"),
            (cat::ARMOR, "Armor"),
            (cat::BEAM, "Beam"),
            (cat::TORPEDO, "Torpedo"),
            (cat::BOMB, "Bomb"),
            (cat::MINING, "Mining"),
            (cat::MINES, "Mine Layer"),
            (cat::SPECIAL_SB, "Orbital"),
            (cat::SPECIAL_E, "Elect"),
            (cat::SPECIAL_M, "Mech"),
            (cat::TERRA, "Terra"),
        ];
        let mut parts = Vec::new();
        for (bit, name) in NAMES {
            if allowed & bit != 0 {
                parts.push(name);
            }
        }
        parts.join(" / ")
    }
}

/// A component being dragged, either from the parts list or off a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignerDrag {
    /// Which component table it came from.
    pub category: u16,
    /// Its index in that table.
    pub item: usize,
    /// How many are being carried.
    pub count: u8,
    /// The schematic slot it was picked up from, or `None` from the list.
    pub from_slot: Option<usize>,
}

impl App {
    /// How many components one drag picks up (`IDropPart`, `10c8:5476`).
    ///
    /// * **Ctrl** from the list carries a hundred, which is more than any slot
    ///   holds, so the slot fills.
    /// * **Shift** carries four, or the whole stack if it is smaller.
    /// * Otherwise a drag off a slot moves **one**, and a drag from the list
    ///   carries the one the caller offered.
    ///
    /// `MANUAL.PDF` p. 9-3 describes the same two shortcuts.
    #[must_use]
    pub fn designer_drag_count(from_slot: Option<usize>, held: u8, ctrl: bool, shift: bool) -> u8 {
        if ctrl {
            if from_slot.is_none() {
                return 100;
            }
            return held;
        }
        if shift {
            if from_slot.is_none() || held > 4 {
                return 4;
            }
            return held;
        }
        if from_slot.is_some() {
            return 1;
        }
        held
    }

    /// Drop a dragged component on one of the schematic's slots.
    ///
    /// Returns whether the design changed. A refused drop is where the original
    /// beeps: the slot is full, or it already holds something else, or it does
    /// not take that kind of component at all. Only identical components stack.
    pub fn designer_drop_on_slot(&mut self, mut drag: DesignerDrag, target: usize) -> bool {
        use stars_core::components::slot as cat;
        let Some(designer) = self.designer.as_ref() else {
            return false;
        };
        let Some(editing) = designer.editing.as_ref() else {
            return false;
        };
        let Some(hull) = Self::designer_hull(&editing.design) else {
            return false;
        };
        let Some(hull_slot) = hull.real_slots().get(target).copied() else {
            return false;
        };
        if drag.from_slot == Some(target) {
            return false;
        }
        // An engine slot is all or nothing: whatever the drag was carrying, it
        // fills.
        if hull_slot.allowed & cat::ENGINE != 0 {
            drag.count = 100;
        }

        let current = editing
            .design
            .slots
            .get(target)
            .map_or((0u16, 0usize, 0u8), |s| {
                (s.category, usize::from(s.item), s.count)
            });
        let (have_category, have_item, have_count) = current;

        if have_count >= hull_slot.capacity {
            return false;
        }
        let stackable =
            have_count == 0 || (have_category == drag.category && have_item == drag.item);
        let accepted = have_count != 0 || drag.category & hull_slot.allowed != 0;
        if !stackable || !accepted {
            return false;
        }

        let want = u16::from(have_count) + u16::from(drag.count);
        let new = want.min(u16::from(hull_slot.capacity)) as u8;
        let moved = new - have_count;

        let Some(designer) = self.designer.as_mut() else {
            return false;
        };
        let Some(editing) = designer.editing.as_mut() else {
            return false;
        };
        if let Some(from) = drag.from_slot {
            if let Some(source) = editing.design.slots.get_mut(from) {
                source.count = source.count.saturating_sub(moved);
            }
        }
        if let Some(s) = editing.design.slots.get_mut(target) {
            s.category = drag.category;
            s.item = u8::try_from(drag.item).unwrap_or(0);
            s.count = new;
        }
        designer.selected_slot = Some(target);
        true
    }

    /// Drop a dragged component back on the parts list, which takes it off the
    /// design. Dropping an **engine** back removes the whole stack, whatever
    /// the drag was carrying.
    pub fn designer_drop_on_list(&mut self, drag: DesignerDrag) -> bool {
        use stars_core::components::slot as cat;
        let Some(from) = drag.from_slot else {
            return false;
        };
        let Some(designer) = self.designer.as_mut() else {
            return false;
        };
        let Some(editing) = designer.editing.as_mut() else {
            return false;
        };
        let Some(s) = editing.design.slots.get_mut(from) else {
            return false;
        };
        let take = if s.category & cat::ENGINE != 0 {
            s.count
        } else {
            drag.count
        };
        if take == 0 {
            return false;
        }
        s.count = s.count.saturating_sub(take);
        true
    }

    /// Step the design's picture along, which is what the two arrows under it
    /// do.
    ///
    /// Every hull owns **four** pictures and the arrows walk those four and no
    /// others: `BuildDlg` splits the index into a base and a variant, steps
    /// the variant with `(iCur + 4 ± 1) & 3` so it wraps, and puts the base
    /// back untouched. The choice can therefore never wander onto another
    /// hull's ship.
    pub fn designer_next_picture(&mut self, forward: bool) {
        use stars_formats::resources::art::PICTURES_PER_HULL;
        let step = if forward { 1 } else { PICTURES_PER_HULL - 1 };
        if let Some(editing) = self.designer.as_mut().and_then(|d| d.editing.as_mut()) {
            let picture = u16::from(editing.design.picture);
            let base = picture - picture % PICTURES_PER_HULL;
            let variant = (picture % PICTURES_PER_HULL + step) % PICTURES_PER_HULL;
            editing.design.picture = u8::try_from(base + variant).unwrap_or(0);
        }
    }

    /// Rename the design being edited, clipped to what the name field holds.
    pub fn designer_rename(&mut self, name: &str) {
        if let Some(editing) = self.designer.as_mut().and_then(|d| d.editing.as_mut()) {
            let clipped: String = name.chars().take(stars_core::design::MAX_NAME).collect();
            editing.design.name = clipped;
        }
    }

    /// The cost panel's left column: the three minerals and the resources, then
    /// the mass — `Cost of one <name>`, as `DrawBuildSelHull` lays it out.
    #[must_use]
    pub fn designer_cost_rows(&self) -> Vec<(String, String)> {
        let Some(design) = self.designer_subject() else {
            return Vec::new();
        };
        let Some(who) = self.designer_builder() else {
            return Vec::new();
        };
        let Some(cost) = design.true_cost(&who) else {
            return Vec::new();
        };
        let mut rows = vec![
            ("Ironium".into(), format!("{}kT", cost.minerals[0])),
            ("Boranium".into(), format!("{}kT", cost.minerals[1])),
            ("Germanium".into(), format!("{}kT", cost.minerals[2])),
            ("Resources".into(), cost.resources.to_string()),
        ];
        // The original prints the mass for a ship and leaves it off a starbase,
        // which never moves.
        if !design.is_starbase() {
            if let Some(mass) = design.mass() {
                rows.push(("Mass".into(), format!("{mass}kT")));
            }
        }
        rows
    }

    /// The cost panel's right column: what the design *does*.
    ///
    /// `Max Fuel`, `Armor`, `Shields` and `Rating` always; `Cloak/Jam`,
    /// `Initiative` and `Scanner Range` only at 800x600 and above, which is the
    /// `mdScreenSize` test in `DrawBuildSelHull`. There is no such thing here,
    /// so they are always shown.
    #[must_use]
    pub fn designer_stat_rows(&self) -> Vec<(String, String)> {
        let Some(design) = self.designer_subject() else {
            return Vec::new();
        };
        let regenerating = self
            .game
            .as_ref()
            .and_then(|g| g.players.get(self.local_player()))
            .is_some_and(|p| p.race.has_lrt(stars_core::race::lrt::REGENERATING_SHIELDS));

        let mut rows = Vec::new();
        if !design.is_starbase() {
            if let Some(fuel) = design.fuel_capacity() {
                rows.push(("Max Fuel:".to_string(), format!("{fuel}mg")));
            }
        }
        if let Some(armor) = design.armor(regenerating) {
            rows.push(("Armor:".to_string(), format!("{armor}dp")));
        }
        let shields = design.shields(regenerating);
        rows.push((
            "Shields:".to_string(),
            if shields == 0 {
                "none".to_string()
            } else {
                format!("{shields}dp")
            },
        ));

        if !design.is_starbase() {
            let range = design.scanner_range();
            let text = match (range.normal, range.penetrating) {
                (0, _) => None,
                (n, 0) => Some(format!("{n}")),
                (n, p) => Some(format!("{n}/{p}")),
            };
            if let Some(text) = text {
                rows.push(("Scanner Range:".to_string(), text));
            }
        }
        rows
    }
}

// --- The Production dialog -----------------------------------------------

/// What the Production dialog is editing.
///
/// The queue is a **working copy**: the original edits `lpplProdGlob` and
/// writes it back with `FinishProduction`, so Cancel throws the changes away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Production {
    /// The planet whose queue this is.
    pub planet: i16,
    /// The queue being edited.
    pub queue: Vec<stars_core::production::QueueItem>,
    /// `fNoResearch`: whether this planet gives research only what production
    /// leaves over.
    pub no_research: bool,
    /// Which inventory row is picked out.
    pub inventory_index: usize,
    /// Which queue row is picked out. `None` is the list's first line,
    /// `— Top of the Queue —`, which is where a new item goes to the front.
    pub queue_index: Option<usize>,
}

impl App {
    /// Open the Production dialog on the selected planet.
    pub fn open_production(&mut self) {
        let Some(planet) = self.selected_planet() else {
            return;
        };
        if planet.owner != i16::try_from(self.local_player()).ok() {
            return;
        }
        self.production = Some(Production {
            planet: planet.id,
            queue: planet.queue.clone(),
            no_research: planet.no_research,
            inventory_index: 0,
            queue_index: None,
        });
    }

    /// Close it without keeping the changes.
    pub fn production_cancel(&mut self) {
        self.production = None;
    }

    /// Write the working copy back to the planet and close.
    pub fn production_ok(&mut self) {
        let Some(dialog) = self.production.take() else {
            return;
        };
        self.production_write(&dialog);
    }

    /// Write one dialog's working copy back to its planet.
    ///
    /// `FinishProduction(1)` does this whenever the dialog leaves a planet, not
    /// only on OK, which is why stepping to the next planet keeps the edits.
    fn production_write(&mut self, dialog: &Production) {
        let Some(game) = self.game.as_mut() else {
            return;
        };
        let Some(planet) = game.planets.iter_mut().find(|p| p.id == dialog.planet) else {
            return;
        };
        if planet.queue == dialog.queue && planet.no_research == dialog.no_research {
            return;
        }
        planet.queue.clone_from(&dialog.queue);
        planet.no_research = dialog.no_research;
        self.dirty = true;
        self.edited.insert(dialog.planet);
    }

    /// The planet the dialog is on.
    #[must_use]
    fn production_planet(&self) -> Option<&Planet> {
        let id = self.production.as_ref()?.planet;
        self.game.as_ref()?.planets.iter().find(|p| p.id == id)
    }

    /// The inventory, with the working queue already subtracted.
    #[must_use]
    pub fn production_inventory(&self) -> Vec<stars_core::production::Available> {
        let Some(dialog) = self.production.as_ref() else {
            return Vec::new();
        };
        let Some(planet) = self.production_planet() else {
            return Vec::new();
        };
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let Some(player) = game.players.get(me) else {
            return Vec::new();
        };
        let who = stars_core::parts::Builder::player(player);
        let designs: &[stars_core::design::ShipDesign] =
            game.designs.get(me).map_or(&[], Vec::as_slice);
        stars_core::production::inventory(planet, &who, designs, &dialog.queue)
    }

    /// The queue, as `(count, name)` — what the two columns of the list show.
    #[must_use]
    pub fn production_queue_rows(&self) -> Vec<(i32, String)> {
        let Some(dialog) = self.production.as_ref() else {
            return Vec::new();
        };
        dialog
            .queue
            .iter()
            .map(|entry| {
                (
                    entry.count,
                    self.production_item_name(entry.item, entry.ship),
                )
            })
            .collect()
    }

    /// When each queue row will be finished, and how it should be drawn.
    ///
    /// One estimate per row, each a fresh simulation of the whole queue — which
    /// is what the original does too, once per row, in `FillPlanetProdLB`.
    #[must_use]
    pub fn production_schedule(&self) -> Vec<(String, stars_core::production::EtaMark)> {
        let Some(dialog) = self.production.as_ref() else {
            return Vec::new();
        };
        let Some(planet) = self.production_planet() else {
            return Vec::new();
        };
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let Some(player) = game.players.get(me) else {
            return Vec::new();
        };
        let who = stars_core::parts::Builder::player(player);
        let designs: &[stars_core::design::ShipDesign] =
            game.designs.get(me).map_or(&[], Vec::as_slice);

        // The estimate reads the queue off the planet, so it runs against the
        // dialog's working copy rather than what is saved.
        let mut working = planet.clone();
        working.queue.clone_from(&dialog.queue);
        working.no_research = dialog.no_research;

        (0..dialog.queue.len())
            .map(|index| {
                let entry = dialog.queue[index];
                let eta = stars_core::production::eta(
                    &working,
                    &who,
                    player.research_pct,
                    designs,
                    index,
                );
                (
                    eta.text(entry.item, entry.ship),
                    eta.mark(entry.item, entry.ship),
                )
            })
            .collect()
    }

    /// What the game calls one queue entry.
    #[must_use]
    fn production_item_name(&self, item: u16, ship: bool) -> String {
        if !ship {
            return stars_core::production::item_name(item).to_string();
        }
        let me = self.local_player();
        self.game
            .as_ref()
            .and_then(|g| g.designs.get(me))
            .and_then(|d| d.get(usize::from(item)))
            .filter(|d| d.hull_id >= 0)
            .map_or_else(String::new, |d| d.name.clone())
    }

    /// How many one click of **Add** or **Remove** is worth.
    ///
    /// Nothing is one, Shift is ten, Ctrl a hundred, and both together
    /// **1020** — three short of what the ten-bit count field holds, which is
    /// the original's "as many as possible" (`MANUAL.PDF` p. 7-3).
    #[must_use]
    pub fn production_step(ctrl: bool, shift: bool) -> i32 {
        match (ctrl, shift) {
            (false, false) => 1,
            (false, true) => 10,
            (true, false) => 100,
            (true, true) => 1020,
        }
    }

    /// **Add**: put the selected inventory item into the queue.
    ///
    /// It goes **under** the selected queue row, so with the list's first line
    /// selected it goes to the front. Adding the same item the neighbouring
    /// row already holds merges into it rather than making a second row, and
    /// nothing more is added than the inventory still offers.
    pub fn production_add(&mut self, count: i32) {
        let inventory = self.production_inventory();
        let Some(dialog) = self.production.as_mut() else {
            return;
        };
        let Some(row) = inventory.get(dialog.inventory_index) else {
            return;
        };
        let count = count.min(row.count).max(0);
        if count == 0 {
            return;
        }

        // The row it goes after: `None` is the top of the queue.
        let at = match dialog.queue_index {
            Some(index) => index + 1,
            None => 0,
        }
        .min(dialog.queue.len());

        let same = |entry: &stars_core::production::QueueItem| {
            entry.ship == row.ship && entry.item == row.item
        };
        // Merge with whichever neighbour matches, as `AddItemToQueue` does.
        if at > 0 && dialog.queue.get(at - 1).is_some_and(same) {
            dialog.queue[at - 1].count += count;
            dialog.queue_index = Some(at - 1);
            return;
        }
        if dialog.queue.get(at).is_some_and(same) {
            dialog.queue[at].count += count;
            dialog.queue_index = Some(at);
            return;
        }
        dialog.queue.insert(
            at,
            stars_core::production::QueueItem {
                count,
                item: row.item,
                ship: row.ship,
                completion: 0,
            },
        );
        dialog.queue_index = Some(at);
    }

    /// **Remove**: take some of the selected queue row back off.
    ///
    /// Auto alchemy comes off whole: its count is a placeholder — the entry
    /// means "as needed" — so the original removes 1020 of it whatever was
    /// asked for.
    pub fn production_remove(&mut self, count: i32) {
        let Some(dialog) = self.production.as_mut() else {
            return;
        };
        let Some(index) = dialog.queue_index else {
            return;
        };
        let Some(entry) = dialog.queue.get_mut(index) else {
            return;
        };
        let count = if !entry.ship && entry.item == stars_core::production::item::AUTO_ALCHEMY {
            1020
        } else {
            count
        };
        entry.count -= count.min(entry.count);
        if entry.count <= 0 {
            dialog.queue.remove(index);
            dialog.queue_index = index.checked_sub(1);
        }
    }

    /// **Item Up** / **Item Down**: swap a row with its neighbour.
    pub fn production_move(&mut self, up: bool) {
        let Some(dialog) = self.production.as_mut() else {
            return;
        };
        let Some(index) = dialog.queue_index else {
            return;
        };
        let target = if up {
            match index.checked_sub(1) {
                Some(target) => target,
                None => return,
            }
        } else {
            index + 1
        };
        if target >= dialog.queue.len() {
            return;
        }
        dialog.queue.swap(index, target);
        dialog.queue_index = Some(target);
    }

    /// **Clear**: empty the queue.
    pub fn production_clear(&mut self) {
        if let Some(dialog) = self.production.as_mut() {
            dialog.queue.clear();
            dialog.queue_index = None;
        }
    }

    /// **Prev** / **Next**: move to another of the player's planets without
    /// closing, writing this one's queue out first.
    ///
    /// With `starbase_only` — the original's Shift — it skips to the next
    /// planet that has a starbase.
    pub fn production_step_planet(&mut self, forward: bool, starbase_only: bool) {
        let Some(dialog) = self.production.clone() else {
            return;
        };
        self.production_write(&dialog);

        let me = i16::try_from(self.local_player()).ok();
        let Some(game) = self.game.as_ref() else {
            return;
        };
        let mine: Vec<i16> = game
            .planets
            .iter()
            .filter(|p| p.owner == me)
            .filter(|p| !starbase_only || p.starbase)
            .map(|p| p.id)
            .collect();
        if mine.is_empty() {
            return;
        }
        // The current planet may not be in the filtered list, so fall back to
        // wherever it would sort.
        let here = mine
            .iter()
            .position(|id| *id == dialog.planet)
            .unwrap_or_else(|| mine.partition_point(|id| *id < dialog.planet));
        let next = if forward {
            (here + 1) % mine.len()
        } else {
            (here + mine.len() - 1) % mine.len()
        };
        let planet = mine[next];

        self.selection.planet = Some(planet);
        let queue = game
            .planets
            .iter()
            .find(|p| p.id == planet)
            .map(|p| (p.queue.clone(), p.no_research));
        if let (Some(dialog), Some((queue, no_research))) = (self.production.as_mut(), queue) {
            dialog.planet = planet;
            dialog.queue = queue;
            dialog.no_research = no_research;
            dialog.inventory_index = 0;
            dialog.queue_index = None;
        }
    }

    /// What the selected item costs, and what the planet has: the four rows
    /// under the inventory.
    ///
    /// The quantity costed is the selected **queue** row's, if one is
    /// selected, and otherwise one of the selected inventory item — which is
    /// `GetProductionCosts`' `fOnlyOne`.
    /// What the selected item costs, as the four raw figures the dialog's own
    /// panel prints: ironium, boranium, germanium and resources.
    ///
    /// `DrawProductionDlg` (`10d0:35dc`) draws this twice, once under each
    /// list, and `queue` says which — the queue's row when it is true and the
    /// inventory's when it is false. The original prints each with `"%ld"` and
    /// puts `kT` after the first three only.
    #[must_use]
    pub fn production_costs(&self, queue: bool) -> Option<[i32; 4]> {
        let dialog = self.production.as_ref()?;
        let me = self.local_player();
        let game = self.game.as_ref()?;
        let player = game.players.get(me)?;
        let who = stars_core::parts::Builder::player(player);
        let designs: &[stars_core::design::ShipDesign] =
            game.designs.get(me).map_or(&[], Vec::as_slice);

        let (item, ship, count) = if queue {
            let entry = dialog.queue_index.and_then(|i| dialog.queue.get(i))?;
            (entry.item, entry.ship, entry.count)
        } else {
            let inventory = self.production_inventory();
            let row = inventory.get(dialog.inventory_index)?;
            (row.item, row.ship, 1)
        };
        let cost = if ship {
            designs
                .get(usize::from(item))
                .filter(|d| d.hull_id >= 0)
                .and_then(|d| d.true_cost(&who))
                .map(|c| stars_core::production::ItemCost {
                    minerals: c.minerals,
                    resources: c.resources,
                })
        } else {
            stars_core::production::item_cost(item, &who, false)
        }?;
        Some([
            cost.minerals[0] * count,
            cost.minerals[1] * count,
            cost.minerals[2] * count,
            cost.resources * count,
        ])
    }

    /// The line under the queue's cost panel: how far the first unit is paid
    /// for, and when the row will be done.
    ///
    /// `"%d%% Done,   Completion "` (`idsDDoneCompletion`) with
    /// `PszProductionETA`'s wording after it.
    #[must_use]
    pub fn production_completion(&self) -> Option<(i32, String)> {
        let dialog = self.production.as_ref()?;
        let index = dialog.queue_index?;
        let entry = dialog.queue.get(index)?;
        let eta = self
            .production_schedule()
            .get(index)
            .map(|(text, _)| text.clone())?;
        Some((entry.completion, eta))
    }

    /// The same four figures, worded against what the planet has on the
    /// surface. Kept for the tests, which check the arithmetic rather than the
    /// panel.
    #[must_use]
    pub fn production_cost_rows(&self) -> Vec<(String, String)> {
        let Some(dialog) = self.production.as_ref() else {
            return Vec::new();
        };
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let Some(player) = game.players.get(me) else {
            return Vec::new();
        };
        let who = stars_core::parts::Builder::player(player);
        let designs: &[stars_core::design::ShipDesign] =
            game.designs.get(me).map_or(&[], Vec::as_slice);

        // Whichever list the player last touched: a queue row if one is
        // picked, otherwise the inventory row.
        let (item, ship, count) = match dialog.queue_index.and_then(|i| dialog.queue.get(i)) {
            Some(entry) => (entry.item, entry.ship, entry.count),
            None => {
                let inventory = self.production_inventory();
                let Some(row) = inventory.get(dialog.inventory_index) else {
                    return Vec::new();
                };
                (row.item, row.ship, 1)
            }
        };

        let cost = if ship {
            designs
                .get(usize::from(item))
                .filter(|d| d.hull_id >= 0)
                .and_then(|d| d.true_cost(&who))
                .map(|c| stars_core::production::ItemCost {
                    minerals: c.minerals,
                    resources: c.resources,
                })
        } else {
            stars_core::production::item_cost(item, &who, false)
        };
        let Some(cost) = cost else {
            return Vec::new();
        };

        let planet = self.production_planet();
        let have = planet.map(|p| p.surface_min);
        let row = |name: &str, need: i32, have: Option<i32>| {
            (
                name.to_string(),
                match have {
                    Some(have) => format!("{need} of {have}kT"),
                    None => format!("{need}"),
                },
            )
        };
        vec![
            row("Ironium", cost.minerals[0] * count, have.map(|h| h[0])),
            row("Boranium", cost.minerals[1] * count, have.map(|h| h[1])),
            row("Germanium", cost.minerals[2] * count, have.map(|h| h[2])),
            (
                "Resources".to_string(),
                (cost.resources * count).to_string(),
            ),
        ]
    }
}

// --- Production templates ------------------------------------------------

impl App {
    /// The four production templates, filled in from the player's default
    /// queue and whatever the frontend has loaded.
    ///
    /// Slot 0 is the **default**: it is the player's own `PLAYER.zpq1`, always
    /// present, always called `<Default>`, and the only one that reaches the
    /// host. The other three are the player's own, and in the original live in
    /// `stars.ini` rather than in any save — see
    /// [`stars_formats::ProductionTemplate`].
    #[must_use]
    pub fn production_templates(&self) -> Vec<stars_formats::ProductionTemplate> {
        let mut out = self.templates.clone();
        out.resize_with(stars_formats::TEMPLATE_SLOTS, Default::default);
        out[0] = stars_formats::ProductionTemplate {
            name: "<Default>".to_string(),
            queue: self
                .game
                .as_ref()
                .and_then(|g| g.players.get(self.local_player()))
                .map(|p| p.default_queue.clone()),
        };
        out
    }

    /// What a template slot is called in the `<Customize>` dialog: its name,
    /// or `<Unused 2>` for an empty one.
    #[must_use]
    pub fn production_template_name(&self, slot: usize) -> String {
        let templates = self.production_templates();
        match templates.get(slot).filter(|t| t.queue.is_some()) {
            Some(template) if !template.name.is_empty() => template.name.clone(),
            _ => format!("<Unused {}>", slot + 1),
        }
    }

    /// Apply a template to the queue the Production dialog is editing.
    ///
    /// Every auto-build item in the queue is replaced by the template's, and
    /// the planet's "contribute only leftover resources" flag is taken from it
    /// too — which is why the manual tells you to set that checkbox before
    /// importing (p. 7-5).
    pub fn production_apply_template(&mut self, slot: usize) {
        let templates = self.production_templates();
        let Some(queue) = templates.get(slot).and_then(|t| t.queue.clone()) else {
            return;
        };
        let prt = self
            .game
            .as_ref()
            .and_then(|g| g.players.get(self.local_player()))
            .and_then(|p| p.race.prt());
        let Some(dialog) = self.production.as_mut() else {
            return;
        };
        dialog.queue = stars_core::production::apply_template(&dialog.queue, &queue, prt);
        dialog.no_research = queue.no_research;
        dialog.queue_index = None;
    }

    /// **Import**: take the queue's auto-build items into a template slot.
    ///
    /// Importing into slot 0 changes the player's own default queue, which is
    /// a real order and is logged as one.
    pub fn production_import_template(&mut self, slot: usize, name: &str) {
        let Some(dialog) = self.production.as_ref() else {
            return;
        };
        let queue = stars_core::production::import_template(&dialog.queue, dialog.no_research);
        if slot == 0 {
            self.set_default_queue(queue);
            return;
        }
        self.templates
            .resize_with(stars_formats::TEMPLATE_SLOTS, Default::default);
        let clipped: String = name
            .chars()
            .take(stars_formats::TEMPLATE_NAME_MAX)
            .collect();
        self.templates[slot] = stars_formats::ProductionTemplate {
            name: if clipped.is_empty() {
                format!("Custom #{slot}")
            } else {
                clipped
            },
            queue: Some(queue),
        };
        self.dirty = true;
    }

    /// **Rename**: change a template's name without touching its contents.
    ///
    /// The default cannot be renamed, which is why `EnableZipProdBtns` greys
    /// both buttons for slot 0.
    pub fn production_rename_template(&mut self, slot: usize, name: &str) {
        if slot == 0 || slot >= stars_formats::TEMPLATE_SLOTS {
            return;
        }
        self.templates
            .resize_with(stars_formats::TEMPLATE_SLOTS, Default::default);
        if self.templates[slot].queue.is_none() {
            return;
        }
        self.templates[slot].name = name
            .chars()
            .take(stars_formats::TEMPLATE_NAME_MAX)
            .collect();
        self.dirty = true;
    }

    /// **Delete**: empty a template slot. The default cannot be deleted; the
    /// only way to clear it is to import an empty queue over it.
    pub fn production_delete_template(&mut self, slot: usize) {
        if slot == 0 || slot >= stars_formats::TEMPLATE_SLOTS {
            return;
        }
        self.templates
            .resize_with(stars_formats::TEMPLATE_SLOTS, Default::default);
        self.templates[slot] = stars_formats::ProductionTemplate::default();
        self.dirty = true;
    }

    /// Whether a slot may be renamed or deleted (`EnableZipProdBtns`).
    #[must_use]
    pub fn production_template_editable(&self, slot: usize) -> bool {
        (1..stars_formats::TEMPLATE_SLOTS).contains(&slot)
            && self
                .production_templates()
                .get(slot)
                .is_some_and(|t| t.queue.is_some())
    }

    /// Open the `<Customize>` dialog, remembering what to put back if it is
    /// cancelled.
    pub fn production_customize_open(&mut self, slot: usize) {
        self.customize = Some((slot, self.production_templates()));
    }

    /// Which slot `<Customize>` is on, if it is open.
    #[must_use]
    pub fn production_customize_slot(&self) -> Option<usize> {
        self.customize.as_ref().map(|(slot, _)| *slot)
    }

    /// Move `<Customize>` to another slot.
    pub fn production_customize_select(&mut self, slot: usize) {
        if let Some((current, _)) = self.customize.as_mut() {
            *current = slot;
        }
    }

    /// Close `<Customize>`. `keep` is OK; anything else puts back the
    /// templates as they were when it opened, the default queue included.
    pub fn production_customize_close(&mut self, keep: bool) {
        let Some((_, snapshot)) = self.customize.take() else {
            return;
        };
        if keep {
            return;
        }
        if let Some(default) = snapshot.first().and_then(|t| t.queue.clone()) {
            self.set_default_queue(default);
        }
        self.templates = snapshot;
    }

    /// The templates as `stars.ini` would hold them: `(key, value)` pairs for
    /// the `ZipOrders` section.
    ///
    /// The original keeps them in that file rather than in a save, so a
    /// frontend that wants them to outlive the session writes these.
    #[must_use]
    pub fn production_templates_ini(&self) -> Vec<(String, String)> {
        let templates = self.production_templates();
        (1..stars_formats::TEMPLATE_SLOTS)
            .map(|slot| {
                (
                    stars_formats::ProductionTemplate::ini_key(slot),
                    templates
                        .get(slot)
                        .map(stars_formats::ProductionTemplate::encode_ini)
                        .unwrap_or_default(),
                )
            })
            .collect()
    }

    /// Load the templates a frontend read out of `stars.ini`.
    ///
    /// Slot 0 is ignored: the default lives in the player block, not the file.
    pub fn load_production_templates(&mut self, values: &[(String, String)]) {
        self.templates
            .resize_with(stars_formats::TEMPLATE_SLOTS, Default::default);
        for slot in 1..stars_formats::TEMPLATE_SLOTS {
            let key = stars_formats::ProductionTemplate::ini_key(slot);
            let Some((_, value)) = values.iter().find(|(k, _)| *k == key) else {
                continue;
            };
            self.templates[slot] =
                stars_formats::ProductionTemplate::decode_ini(value).unwrap_or_default();
        }
    }
}

// --- The Research dialog -------------------------------------------------

/// What the Research dialog is showing.
///
/// The original edits three globals — `pctResGlob`, `iResTechNow` and the
/// dropdown — and commits all three at once on OK, so **Cancel changes
/// nothing**. This is those three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchDialog {
    /// Which field's radio button is selected (`iResTechNow`).
    pub field: usize,
    /// What to research next (`iTechCur >> 4`).
    pub next: stars_core::research::NextField,
    /// The share of resources going to research (`pctResGlob`).
    pub percent: u8,
}

impl App {
    /// Open the Research dialog (`ResearchDlg`, F5).
    pub fn open_research(&mut self) {
        let me = self.local_player();
        let Some(player) = self.game.as_ref().and_then(|g| g.players.get(me)) else {
            return;
        };
        self.research_dialog = Some(ResearchDialog {
            field: player.research.current_field,
            next: player.research.next_field,
            percent: player.research_pct,
        });
    }

    /// Close it without keeping anything.
    pub fn research_cancel(&mut self) {
        self.research_dialog = None;
    }

    /// **OK**: commit the field, the next field and the percentage together.
    ///
    /// `ResearchDlg` writes all three only if any of them changed, and logs a
    /// single two-byte `rtLogResearch` order carrying `iTechCur` and the
    /// percentage.
    pub fn research_ok(&mut self) {
        let Some(dialog) = self.research_dialog.take() else {
            return;
        };
        let me = self.local_player();
        let Some(player) = self.game.as_mut().and_then(|g| g.players.get_mut(me)) else {
            return;
        };
        if player.research.current_field == dialog.field
            && player.research.next_field == dialog.next
            && player.research_pct == dialog.percent
        {
            return;
        }
        player.research.current_field = dialog.field;
        player.research.next_field = dialog.next;
        player.research_pct = dialog.percent;
        self.research_edited = true;
        self.player_edited = true;
        self.dirty = true;
    }

    /// The six rows of the **Technology Status** box: each field's name and
    /// the level held.
    #[must_use]
    pub fn research_levels(&self) -> Vec<(&'static str, u8)> {
        let me = self.local_player();
        let Some(player) = self.game.as_ref().and_then(|g| g.players.get(me)) else {
            return Vec::new();
        };
        stars_core::research::TechField::ALL
            .iter()
            .enumerate()
            .map(|(index, field)| (field.name(), player.research.levels[index]))
            .collect()
    }

    /// The **Currently Researching** box: the level being worked on, what it
    /// still costs, and how long it will take.
    ///
    /// Returns the three lines in the original's wording. `Maxed Out` stands
    /// in for the cost at level 26, and `Never` for the time when nothing is
    /// budgeted.
    #[must_use]
    pub fn research_status(&self) -> Vec<(String, String)> {
        use stars_core::research::{remaining_cost, years_to_next};
        let Some(dialog) = self.research_dialog else {
            return Vec::new();
        };
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let Some(player) = game.players.get(me) else {
            return Vec::new();
        };

        let name = stars_core::research::TechField::ALL[dialog.field.min(5)].name();
        let level = player.research.levels[dialog.field.min(5)];
        let mut rows = vec![(
            String::new(),
            // `%s, Tech Level %d` — the level being researched, one above what
            // is held.
            format!("{name}, Tech Level {}", i16::from(level) + 1),
        )];

        let remaining =
            remaining_cost(dialog.field, &player.research, &player.race, game.slow_tech);
        rows.push((
            "Resources needed to complete:".to_string(),
            match remaining {
                Some(cost) => cost.to_string(),
                None => "Maxed Out".to_string(),
            },
        ));

        let budget = self.research_projected(dialog.percent);
        let generalized = player
            .race
            .has_lrt(stars_core::race::lrt::GENERALIZED_RESEARCH);
        rows.push((
            "Estimated time to completion:".to_string(),
            match remaining {
                None => "Maxed Out".to_string(),
                Some(cost) => match years_to_next(cost, budget, generalized) {
                    None => "Never".to_string(),
                    Some(1) => "1 year".to_string(),
                    Some(years) => format!("{years} years"),
                },
            },
        ));
        rows
    }

    /// The **Resource Allocation** box.
    #[must_use]
    pub fn research_allocation(&self) -> Vec<(String, String)> {
        let Some(dialog) = self.research_dialog else {
            return Vec::new();
        };
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let Some(player) = game.players.get(me) else {
            return Vec::new();
        };
        let owner = i16::try_from(me).unwrap_or(-1);
        let energy = i16::from(player.research.levels[0]);
        let annual: i32 = game
            .planets
            .iter()
            .filter(|p| p.owner == Some(owner))
            .filter_map(|p| stars_core::resources::resources_at_planet(p, &player.race, energy))
            .map(i32::from)
            .sum();

        vec![
            (
                "Annual resources from all planets:".to_string(),
                annual.to_string(),
            ),
            (
                "Total resources spent on research last year:".to_string(),
                player.research_last_year.to_string(),
            ),
            (
                "Resources budgeted for research:".to_string(),
                format!("{}%", dialog.percent),
            ),
            (
                "Next year's projected research budget:".to_string(),
                self.research_projected(dialog.percent).to_string(),
            ),
        ]
    }

    /// What research would get next year at a given allocation.
    #[must_use]
    pub fn research_projected(&self, percent: u8) -> i32 {
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return 0;
        };
        let Some(player) = game.players.get(me) else {
            return 0;
        };
        let who = stars_core::parts::Builder::player(player);
        let designs: &[stars_core::design::ShipDesign] =
            game.designs.get(me).map_or(&[], Vec::as_slice);
        stars_core::production::projected_research_spending(
            &game.planets,
            i16::try_from(me).unwrap_or(-1),
            &who,
            percent,
            designs,
        )
    }

    /// The **Expected Research Benefits** list, nearest first.
    #[must_use]
    pub fn research_benefits(&self) -> Vec<stars_core::research::Benefit> {
        let me = self.local_player();
        let Some(player) = self.game.as_ref().and_then(|g| g.players.get(me)) else {
            return Vec::new();
        };
        stars_core::research::expected_benefits(&stars_core::parts::Builder::player(player))
    }

    /// The note under the allocation box, for a race whose traits change how
    /// research works.
    #[must_use]
    pub fn research_notes(&self) -> Vec<String> {
        use stars_core::race::lrt;
        let me = self.local_player();
        let Some(player) = self.game.as_ref().and_then(|g| g.players.get(me)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if player.race.has_lrt(lrt::GENERALIZED_RESEARCH) {
            out.push("Your race has Generalized Research".to_string());
        }
        if player.race.has_lrt(lrt::BLEEDING_EDGE_TECH) {
            out.push("Your race has Bleeding Edge Technology".to_string());
        }
        out
    }

    /// What the note under the allocation box says when it is pressed
    /// (`FTrackResearchDlg`, `10d8:1b5f`, raising `grPopupString`).
    ///
    /// The note is three lines tall and the original picks between two
    /// sentences by a rule worth stating carefully: without **Generalized
    /// Research** it is always the Bleeding Edge one; with it, the Bleeding
    /// Edge one appears only in the **lower half** of the three lines and only
    /// when the race has Bleeding Edge as well; otherwise the Generalized
    /// Research one. So a race with both traits carries both notes stacked,
    /// and which comes up depends on which half is pressed.
    ///
    /// `lower` is whether the press was past the halfway line. The wording is
    /// this project's own, as the game's authored prose always is.
    #[must_use]
    pub fn research_note_text(&self, lower: bool) -> Option<String> {
        use stars_core::race::lrt;
        let me = self.local_player();
        let player = self.game.as_ref()?.players.get(me)?;
        let generalized = player.race.has_lrt(lrt::GENERALIZED_RESEARCH);
        let bleeding = player.race.has_lrt(lrt::BLEEDING_EDGE_TECH);
        let bleeding_note = "A new technology costs twice as much to build until every one \
             of its requirements is beaten by a level, after which it drops back to normal. \
             Miniaturisation then runs at five per cent a level and stops at eighty."
            .to_string();
        let generalized_note = "Only half of what this race spends on research reaches the \
             field it is studying; fifteen per cent of the whole reaches every other field \
             instead."
            .to_string();
        if !generalized {
            return bleeding.then_some(bleeding_note);
        }
        if lower && bleeding {
            return Some(bleeding_note);
        }
        Some(generalized_note)
    }

    /// What the **Next field to research** dropdown offers, in the original's
    /// order: `<Same field>`, the six fields, then `<Lowest field>`.
    #[must_use]
    pub fn research_next_choices() -> Vec<(stars_core::research::NextField, String)> {
        use stars_core::research::{NextField, TechField};
        let mut out = vec![(NextField::Same, "<Same field>".to_string())];
        for (index, field) in TechField::ALL.iter().enumerate() {
            out.push((NextField::Field(index), field.name().to_string()));
        }
        out.push((NextField::Lowest, "<Lowest field>".to_string()));
        out
    }
}

// --- The Technology Browser ----------------------------------------------

/// What the Technology Browser is showing.
///
/// The original keeps this in globals — `vpartBrowser`, the dropdown and the
/// checkbox — and the window is **modeless**, so it stays open while the
/// player does other things.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Browser {
    /// The category the dropdown is on: an index into
    /// [`stars_core::browser::CATEGORIES`], where `0` is **All**.
    pub category: usize,
    /// The component being shown, as `(category flag, index)`.
    pub showing: (u16, usize),
    /// Whether to walk only what the player can build now.
    pub buildable_only: bool,
}

impl App {
    /// Open the Technology Browser (`BrowserDlg`, F2).
    ///
    /// It opens on the first armour, which is where `BrowserDlg` starts when
    /// it has nothing remembered (`vpartBrowser.grhst = hstArmor`).
    pub fn open_browser(&mut self) {
        if self.browser.is_some() {
            return;
        }
        self.browser = Some(Browser {
            category: 0,
            showing: (stars_core::components::slot::ARMOR, 0),
            buildable_only: false,
        });
    }

    /// Close it.
    pub fn close_browser(&mut self) {
        self.browser = None;
    }

    /// The design a planet's starbase is, for the pop-up that draws it.
    #[must_use]
    pub fn starbase_popup(&self, planet: i16) -> Option<Popup> {
        let game = self.game.as_ref()?;
        let found = game.planets.iter().find(|p| p.id == planet)?;
        let design = game
            .designs
            .get(self.local_player())?
            .get(starbase_slot(found.starbase_design?))?;
        (design.hull_id >= 0).then(|| Popup::Design(design.clone()))
    }

    /// What the population pop-up says about a planet.
    ///
    /// `PtDisplayPlanetPopInfo` chooses between three openings on who owns
    /// it, then between three middles on whether the planet is hostile, ours
    /// with room, or worth colonising, and closes either with next year's
    /// growth or with what defences another player has there.
    #[must_use]
    pub fn population_popup(&self, planet: i16) -> Option<Popup> {
        use crate::popup::{Inhabited, PopulationSummary};

        let game = self.game.as_ref()?;
        let found = game
            .planets
            .iter()
            .chain(game.known_planets.iter())
            .find(|p| p.id == planet)?;
        let me = i16::try_from(self.local_player()).ok();
        let race = game.players.get(self.local_player()).map(|p| &p.race);
        let scanned = found.detail != stars_core::planet::Detail::Minimal;

        let who = match found.owner {
            None => Inhabited::Nobody,
            owner if owner == me => Inhabited::Ours(i64::from(found.pop) * 100),
            _ => Inhabited::Enemy(found.detail.is_full().then(|| i64::from(found.pop) * 100)),
        };
        let capacity = race
            .and_then(|race| stars_core::hab::calc_planet_max_pop(found, race))
            .map(|max| i64::from(max) * 100);
        let value = scanned
            .then(|| race.map(|race| stars_core::hab::pct_planet_desirability(found, race)))
            .flatten();
        // `ChgPopFromPlanet`, and only when the planet is ours, is worth
        // something, and has somewhere to grow into.
        let growth = match (&who, value, capacity, race) {
            (Inhabited::Ours(pop), Some(value), Some(capacity), Some(race))
                if value >= 0 && *pop < capacity =>
            {
                stars_core::population::chg_pop_from_planet(found, race).map(|change| {
                    (
                        i64::from(change.delta) * 100,
                        pop + i64::from(change.delta) * 100,
                    )
                })
            }
            _ => None,
        };
        let defenses = match (&who, race) {
            (Inhabited::Enemy(_), Some(race)) if found.defenses > 0 => {
                let levels = game
                    .players
                    .get(self.local_player())
                    .map_or([0; 6], |p| p.research.levels);
                let (survive, _) = stars_core::bombing::pct_survive(found, race, levels);
                #[expect(clippy::cast_possible_truncation, reason = "a percentage")]
                let pct = ((1.0 - survive) * 100.0).round() as i64;
                Some(pct)
            }
            _ => None,
        };

        Some(Popup::Population(PopulationSummary {
            planet: found.name.unwrap_or("").to_string(),
            who,
            value,
            capacity,
            growth,
            defenses,
        }))
    }

    /// What the industry pop-up says about a planet's mines or factories.
    ///
    /// `ExecuteReportClick` fills it from `CMaxMines`/`CMaxFactories` and
    /// their operable counterparts, which is the same pair the Mine and Fact
    /// cells colour themselves against.
    #[must_use]
    pub fn industry_popup(&self, planet: i16, factories: bool) -> Option<Popup> {
        let game = self.game.as_ref()?;
        let found = game.planets.iter().find(|p| p.id == planet)?;
        let race = &game.players.get(self.local_player())?.race;
        let (built, most, operable) = if factories {
            (
                found.factories,
                stars_core::resources::max_factories(found, race),
                stars_core::resources::max_operable_factories(found, race, false),
            )
        } else {
            (
                found.mines,
                stars_core::resources::max_mines(found, race),
                stars_core::resources::max_operable_mines(found, race, false),
            )
        };
        Some(Popup::Industry(crate::popup::IndustrySummary {
            planet: found.name.unwrap_or("").to_string(),
            factories,
            built: i64::from(built),
            most: i64::from(most),
            operable: i64::from(operable),
            innate: race.is_ar(),
        }))
    }

    /// What the resources pop-up says about a planet.
    #[must_use]
    pub fn resources_popup(&self, planet: i16) -> Option<Popup> {
        let game = self.game.as_ref()?;
        let found = game.planets.iter().find(|p| p.id == planet)?;
        let player = game.players.get(self.local_player())?;
        let total = i64::from(
            stars_core::resources::resources_at_planet(
                found,
                &player.race,
                i16::from(player.research.levels[0]),
            )
            .unwrap_or(0),
        );
        let research = total * i64::from(player.research_pct) / 100;
        Some(Popup::Resources(crate::popup::ResourceSummary {
            planet: found.name.unwrap_or("").to_string(),
            total,
            research,
            // The original stops the sentence rather than saying "None …
            // leaves all of it".
            spare: (research > 0).then_some(total - research),
            innate: player.race.is_ar(),
        }))
    }

    /// What the mineral pop-up says about one of a planet's three.
    ///
    /// `DrawPopup`'s `grPopupMineral` arm: the mineral's name over three
    /// rows, each of which reads `Unknown` rather than a figure when the
    /// figure is not known — a planet nobody has landed on has no surface
    /// total, and one nobody has scanned has no concentration. The rate is
    /// left out altogether unless it can be worked out.
    #[must_use]
    pub fn mineral_popup(&self, planet: i16, mineral: usize) -> Option<Popup> {
        /// A home world mines as if its concentration were at least this.
        const HOME_FLOOR: i64 = 30;
        let game = self.game.as_ref()?;
        let found = game
            .planets
            .iter()
            .chain(game.known_planets.iter())
            .find(|p| p.id == planet)?;
        let race = game.players.get(self.local_player()).map(|p| &p.race);
        let concentration = i64::from(*found.min_conc.get(mineral)?);
        Some(Popup::Mineral(crate::popup::MineralSummary {
            mineral,
            surface: found
                .detail
                .is_full()
                .then(|| found.surface_min.get(mineral).map(|m| i64::from(*m)))
                .flatten(),
            concentration: (concentration > 0).then_some(concentration),
            home_note: found.homeworld.then_some(if concentration < HOME_FLOOR {
                crate::popup::HOME_FLOOR
            } else {
                crate::popup::HOME_WORLD
            }),
            rate: match (found.detail.is_full(), race) {
                (true, Some(race)) => stars_core::mining::minerals_mined(found, race, None, None)
                    .get(mineral)
                    .map(|m| i64::from(*m)),
                _ => None,
            },
        }))
    }

    /// The nine captions the mineral graph's scale offers, and which of
    /// them is ticked.
    ///
    /// `MineClick` (`1028:4020`) builds the menu with the current scale
    /// pre-checked and `fRightBtn` set, so it comes up on either button.
    /// Returns `(captions, checked)`, where `checked` is `None` when the
    /// scale in hand is not one of the nine — which the file can arrange,
    /// since it accepts anything from 100 to 30000.
    #[must_use]
    pub fn mineral_scale_menu(&self) -> (Vec<String>, Option<usize>) {
        (
            MINERAL_SCALES.iter().map(|kt| format!("{kt}kT")).collect(),
            MINERAL_SCALES
                .iter()
                .position(|kt| *kt == self.mineral_scale),
        )
    }

    /// Choose one of them. Anything not on the ladder is refused, as the
    /// menu can only offer what is on it.
    ///
    /// Returns whether the scale changed, which is what decides whether the
    /// pane and — when the surface-mineral view is showing — the scanner
    /// need redrawing.
    pub fn set_mineral_scale(&mut self, index: usize) -> bool {
        let Some(&scale) = MINERAL_SCALES.get(index) else {
            return false;
        };
        if scale == self.mineral_scale {
            return false;
        }
        self.mineral_scale = scale;
        true
    }

    /// The best planetary defence this race can build.
    ///
    /// `FGetBestDefensePart` walks the planetary table from item 9 — SDI,
    /// then Missile Battery, Laser Battery, Planetary Shield and Neutron
    /// Shield — while each is available, and keeps the last one that was.
    /// `None` when not even an SDI can be built.
    #[must_use]
    pub fn best_defense_part(&self) -> Option<(u16, usize)> {
        /// The first of the five defences in the planetary table.
        const FIRST_DEFENSE: usize = 9;
        /// How many there are.
        const DEFENSES: usize = 5;
        let who = self.browser_builder()?;
        let category = stars_core::components::slot::PLANETARY;
        let mut best = None;
        for item in FIRST_DEFENSE..FIRST_DEFENSE + DEFENSES {
            if !stars_core::parts::availability(&who, category, item).is_available() {
                break;
            }
            best = Some((category, item));
        }
        best
    }

    /// Who is browsing, for the costs and the requirements.
    fn browser_builder(&self) -> Option<stars_core::parts::Builder<'_>> {
        let game = self.game.as_ref()?;
        let player = game.players.get(self.local_player())?;
        Some(stars_core::parts::Builder::player(player))
    }

    /// The category the dropdown is limiting the walk to, or `None` for
    /// **All**.
    #[must_use]
    fn browser_within(&self) -> Option<u16> {
        let browser = self.browser?;
        stars_core::browser::CATEGORIES
            .get(browser.category)
            .map(|(flag, _)| *flag)
            .filter(|flag| *flag != 0)
    }

    /// What the panel shows about the component in view.
    #[must_use]
    pub fn browser_detail(&self) -> Option<stars_core::browser::Detail> {
        let browser = self.browser?;
        let who = self.browser_builder()?;
        stars_core::browser::detail(&who, browser.showing.0, browser.showing.1)
    }

    /// **Prev** and **Next**: walk the catalogue.
    pub fn browser_step(&mut self, forward: bool) {
        let Some(browser) = self.browser else {
            return;
        };
        let within = self.browser_within();
        let Some(who) = self.browser_builder() else {
            return;
        };
        let next = stars_core::browser::step(
            &who,
            within,
            browser.showing,
            forward,
            browser.buildable_only,
        );
        if let (Some(next), Some(browser)) = (next, self.browser.as_mut()) {
            browser.showing = next;
        }
    }

    /// Choose a category. The panel moves to the first component in it that
    /// the current filter will stop at.
    pub fn browser_set_category(&mut self, category: usize) {
        let Some(browser) = self.browser.as_mut() else {
            return;
        };
        browser.category = category.min(stars_core::browser::CATEGORIES.len() - 1);
        let buildable_only = browser.buildable_only;
        let within = self.browser_within();
        let Some(who) = self.browser_builder() else {
            return;
        };
        let first = stars_core::browser::first(&who, within, buildable_only);
        if let (Some(first), Some(browser)) = (first, self.browser.as_mut()) {
            browser.showing = first;
        }
    }

    /// Turn the "only what I can build" filter on or off, moving off a
    /// component the filter no longer allows.
    pub fn browser_set_buildable_only(&mut self, only: bool) {
        let Some(browser) = self.browser.as_mut() else {
            return;
        };
        browser.buildable_only = only;
        if !only {
            return;
        }
        let showing = browser.showing;
        let within = self.browser_within();
        let Some(who) = self.browser_builder() else {
            return;
        };
        if stars_core::parts::availability(&who, showing.0, showing.1).is_available() {
            return;
        }
        let next = stars_core::browser::step(&who, within, showing, true, true);
        if let (Some(next), Some(browser)) = (next, self.browser.as_mut()) {
            browser.showing = next;
        }
    }
}

// --- The Score sheet ------------------------------------------------------

/// What the Score sheet is showing.
///
/// `ScoreXDlg` (`1108:0f66`) is **modeless** and keeps both of these in `gd`,
/// so they survive the window being closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreSheet {
    /// Which of the three faces the button has cycled to.
    pub face: stars_core::scoresheet::Face,
    /// Which figure the timeline draws (`gd.iCurGraph`).
    pub graph: stars_core::scoresheet::Stat,
}

impl Default for ScoreSheet {
    fn default() -> Self {
        Self {
            face: stars_core::scoresheet::Face::Scores,
            graph: stars_core::scoresheet::Stat::Score,
        }
    }
}

impl App {
    /// Open the Score sheet (Reports (Score), F10).
    ///
    /// It comes back to whatever it was showing when it was last closed.
    pub fn open_score_sheet(&mut self) {
        if self.score_sheet.is_some() {
            return;
        }
        self.score_sheet = Some(self.score_settings);
    }

    /// Close it.
    pub fn close_score_sheet(&mut self) {
        self.score_sheet = None;
    }

    /// Turn to the next face, which is what the sheet's one button does.
    pub fn score_next_face(&mut self) {
        let Some(sheet) = self.score_sheet.as_mut() else {
            return;
        };
        sheet.face = sheet.face.next();
        self.score_settings = *sheet;
    }

    /// Choose which figure the timeline draws.
    ///
    /// The original offers this as a popup menu on the graph's title, which is
    /// why the title takes a hand cursor.
    pub fn score_set_graph(&mut self, graph: stars_core::scoresheet::Stat) {
        let Some(sheet) = self.score_sheet.as_mut() else {
            return;
        };
        sheet.graph = graph;
        self.score_settings = *sheet;
    }

    /// The scoreboard, a row per player.
    #[must_use]
    pub fn score_standings(&self) -> Vec<stars_core::score::Standing> {
        self.game
            .as_ref()
            .map(stars_core::scoresheet::standings)
            .unwrap_or_default()
    }

    /// The victory report's lines.
    #[must_use]
    pub fn score_conditions(&self) -> Vec<stars_core::scoresheet::Condition> {
        self.game
            .as_ref()
            .map(stars_core::scoresheet::conditions)
            .unwrap_or_default()
    }
}

// --- Player Relations -----------------------------------------------------

impl App {
    /// Open the Player Relations dialog (`RelationsDlg`, Commands (Player
    /// Relations), F7).
    ///
    /// It opens on the **first other player**, which is player 0 unless that
    /// is you, and is refused outright in a single-player game — see
    /// [`stars_core::relations::can_be_set`]. Returns whether it opened.
    pub fn open_relations(&mut self) -> bool {
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return false;
        };
        if !stars_core::relations::can_be_set(game) {
            return false;
        }
        let Some(first) = stars_core::relations::others(game, me).first().copied() else {
            return false;
        };
        self.relations_dialog = Some(first);
        true
    }

    /// Close it. The order has already been written — the original logs on
    /// the way out, but a change and a close leave the same single record
    /// either way, because the record carries the whole table.
    pub fn close_relations(&mut self) {
        self.relations_dialog = None;
    }

    /// Choose which player the dialog is talking about.
    pub fn relations_select(&mut self, player: usize) {
        let me = self.local_player();
        let is_other = self
            .game
            .as_ref()
            .is_some_and(|game| player < game.players.len() && player != me);
        if is_other && self.relations_dialog.is_some() {
            self.relations_dialog = Some(player);
        }
    }

    /// The players the dialog lists: everybody but the local player.
    #[must_use]
    pub fn relations_others(&self) -> Vec<usize> {
        let me = self.local_player();
        self.game
            .as_ref()
            .map(|game| stars_core::relations::others(game, me))
            .unwrap_or_default()
    }

    /// How the local player regards one other.
    #[must_use]
    pub fn regard(&self, toward: usize) -> stars_core::relations::Relation {
        let me = self.local_player();
        self.game
            .as_ref()
            .map_or(stars_core::relations::Relation::Neutral, |game| {
                stars_core::relations::regard(game, me, toward)
            })
    }

    /// Set how the local player regards one other, as the dialog's radio
    /// buttons do.
    pub fn set_regard(&mut self, toward: usize, relation: stars_core::relations::Relation) -> bool {
        self.set_relations(toward, relation.value())
    }
}

// --- The game's own pictures ----------------------------------------------

impl App {
    /// Read the pictures out of a copy of the original executable.
    ///
    /// # Errors
    ///
    /// A message suitable for showing to the player.
    pub fn load_art(&mut self, executable: Vec<u8>, source: &str) -> Result<(), String> {
        self.art = Some(crate::art::Art::open(executable, source)?);
        Ok(())
    }

    /// Whether the game's own pictures are available.
    #[must_use]
    pub fn has_art(&self) -> bool {
        self.art.is_some()
    }

    /// The cell a race's emblem sits in, for [`crate::art::draw`].
    #[must_use]
    pub fn emblem_of(
        &self,
        player: usize,
        size: stars_formats::resources::art::EmblemSize,
    ) -> Option<stars_formats::resources::art::Cell> {
        let logo = self.game.as_ref()?.players.get(player)?.logo;
        stars_formats::resources::art::emblem(logo, size)
    }

    /// The cell a planet's picture sits in.
    ///
    /// `PaintPlanetPane` picks it from the planet's own id —
    /// `(id + 8) % 28` — so every planet keeps the same face all game and
    /// neighbouring planets do not share one.
    #[must_use]
    pub fn planet_picture(&self, planet: i16) -> Option<stars_formats::resources::art::Cell> {
        let index = u16::try_from(i32::from(planet) + 8).ok()? % 28;
        stars_formats::resources::art::planet(index)
    }
}

// --- Ship pictures --------------------------------------------------------

impl App {
    /// The picture the fleet in the pane is drawn with, and how many different
    /// designs it holds.
    ///
    /// `DrawFleetBitmap` takes the fleet's **primary** design — see
    /// [`stars_core::fleet::primary_design`] — and blits that design's own
    /// picture, so a fleet looks like whatever most of it is.
    #[must_use]
    pub fn fleet_picture(&self) -> Option<(stars_formats::resources::art::Cell, usize)> {
        use stars_formats::resources::art;
        let game = self.game.as_ref()?;
        let fleet = self.pane_fleet()?;
        let owner = usize::try_from(fleet.owner).ok()?;
        let designs = game.designs.get(owner)?;
        let primary = stars_core::fleet::primary_design(fleet, designs)?;
        let picture = u16::from(designs.get(primary.design)?.picture);
        Some((art::ship(picture, art::ShipSize::Large), primary.distinct))
    }

    /// The race emblem of whoever owns the fleet in the pane.
    #[must_use]
    pub fn fleet_emblem(
        &self,
        size: stars_formats::resources::art::EmblemSize,
    ) -> Option<stars_formats::resources::art::Cell> {
        let owner = usize::try_from(self.pane_fleet()?.owner).ok()?;
        self.emblem_of(owner, size)
    }
}

// --- The scanner's toolbar ------------------------------------------------

impl App {
    /// Whether a toolbar button is showing as pressed.
    ///
    /// `FIsButtonDown` (`1068:0c3a`) reads `grbitScan`: the six views are a
    /// radio group in its low four bits and the rest are single bits. The two
    /// filter menus and Zoom are momentary and never show pressed.
    #[must_use]
    pub fn toolbar_down(&self, button: crate::toolbar::Button) -> bool {
        use crate::toolbar::Button;
        match button {
            Button::Normal => self.scan_view == ScanView::Normal,
            Button::SurfaceMinerals => self.scan_view == ScanView::SurfaceMineral,
            Button::MineralConcentration => self.scan_view == ScanView::MineralConcentration,
            Button::PlanetValue => self.scan_view == ScanView::PlanetValue,
            Button::Population => self.scan_view == ScanView::Population,
            Button::NoPlayerInfo => self.scan_view == ScanView::NoPlayerInfo,
            Button::AddWaypoints => self.add_waypoints,
            Button::ScannerCoverage => self.scan_overlays.scanner_coverage,
            // Pressed only when **all four** are being shown: a partial
            // choice leaves the button up, which is how the original says the
            // overlay is narrowed without opening the menu.
            Button::MineFields => {
                self.scan_overlays.minefields && self.scan_minefield_filter == 0xf
            }
            Button::FleetPaths => self.scan_overlays.fleet_paths,
            Button::IdleFleets => self.scan_overlays.idle_fleets,
            Button::PlanetNames => self.scan_overlays.names,
            Button::ShipCount => self.scan_overlays.ship_counts,
            Button::ShipDesignFilter => self.scan_overlays.ship_design_filter,
            Button::EnemyClassFilter => self.scan_overlays.enemy_class_filter,
            // The menus and Zoom are momentary in the original too.
            Button::ShipDesignMenu | Button::EnemyClassMenu | Button::Zoom => false,
        }
    }

    /// Whether pressing a toolbar button does anything.
    ///
    /// Everything on the row is wired now; this is kept because the view asks
    /// it, and because a button that cannot act should say so rather than look
    /// broken.
    #[must_use]
    pub fn toolbar_enabled(&self, _button: crate::toolbar::Button) -> bool {
        true
    }

    /// Press a toolbar button.
    ///
    /// A view replaces whichever view was on, as the original replaces the low
    /// four bits of `grbitScan`; everything else toggles. Zoom steps the
    /// scanner in, wrapping round at the far end, which is what clicking the
    /// magnifying glass does.
    pub fn toolbar_click(&mut self, button: crate::toolbar::Button) {
        use crate::toolbar::Button;
        match button {
            Button::Normal => self.scan_view = ScanView::Normal,
            Button::SurfaceMinerals => self.scan_view = ScanView::SurfaceMineral,
            Button::MineralConcentration => self.scan_view = ScanView::MineralConcentration,
            Button::PlanetValue => self.scan_view = ScanView::PlanetValue,
            Button::Population => self.scan_view = ScanView::Population,
            Button::NoPlayerInfo => self.scan_view = ScanView::NoPlayerInfo,
            Button::AddWaypoints => self.add_waypoints = !self.add_waypoints,
            Button::ScannerCoverage => {
                self.scan_overlays.scanner_coverage = !self.scan_overlays.scanner_coverage;
            }
            // Mine Fields opens a menu rather than toggling; the menu's
            // result is what moves the overlay.
            Button::MineFields => {}
            Button::FleetPaths => self.scan_overlays.fleet_paths = !self.scan_overlays.fleet_paths,
            Button::IdleFleets => self.scan_overlays.idle_fleets = !self.scan_overlays.idle_fleets,
            Button::PlanetNames => self.scan_overlays.names = !self.scan_overlays.names,
            Button::ShipCount => self.scan_overlays.ship_counts = !self.scan_overlays.ship_counts,
            Button::Zoom => {
                self.scan_zoom = if self.scan_zoom >= 4 {
                    -4
                } else {
                    self.scan_zoom + 1
                };
            }
            Button::ShipDesignFilter => {
                self.scan_overlays.ship_design_filter = !self.scan_overlays.ship_design_filter;
            }
            Button::EnemyClassFilter => {
                self.scan_overlays.enemy_class_filter = !self.scan_overlays.enemy_class_filter;
            }
            // The two menus do their work through the calls below.
            Button::ShipDesignMenu | Button::EnemyClassMenu => {}
        }
    }

    /// Set the coverage the combo holds, reading it the way the original reads
    /// what was typed into it.
    pub fn set_scan_coverage(&mut self, text: &str) {
        self.scan_coverage_pct = crate::toolbar::coverage_from_text(text);
    }
}

// --- The scanner's two ship filters ---------------------------------------

/// What one of the two filter menus offers, besides its three commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterEntry {
    /// Which bit it is.
    pub bit: u8,
    /// What to write beside the tick.
    pub name: String,
    /// Whether it is ticked.
    pub on: bool,
}

impl App {
    /// The designs the Ship Design filter's menu lists.
    ///
    /// `ExecuteButton` walks all sixteen slots and skips any whose `fFree` bit
    /// is set — an empty slot — so the menu is the designs that exist, and the
    /// bit each toggles is its **slot**, not its position in the menu.
    #[must_use]
    pub fn design_filter_entries(&self) -> Vec<FilterEntry> {
        let me = self.local_player();
        self.game
            .as_ref()
            .and_then(|game| game.designs.get(me))
            .map(|designs| {
                designs
                    .iter()
                    .enumerate()
                    .take(16)
                    .filter(|(_, design)| design.hull_id >= 0)
                    .map(|(slot, design)| FilterEntry {
                        bit: u8::try_from(slot).unwrap_or(0),
                        name: design.name.clone(),
                        on: self.scan_design_filter & (1 << slot) != 0,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The eight classes the Enemy Ship Class filter's menu lists.
    #[must_use]
    pub fn class_filter_entries(&self) -> Vec<FilterEntry> {
        stars_core::design::ShipClass::ALL
            .iter()
            .map(|class| FilterEntry {
                bit: class.index(),
                name: class.name().to_string(),
                on: self.scan_class_filter & (1 << class.index()) != 0,
            })
            .collect()
    }

    /// Turn one design on or off in the filter.
    ///
    /// Ticking something while the overlay is **off** turns the overlay on,
    /// which is what the original does — there is no point choosing a design
    /// and seeing nothing change. Unticking does not turn it off again.
    pub fn toggle_design_filter(&mut self, slot: u8) {
        self.scan_design_filter ^= 1 << slot;
        if self.scan_design_filter & (1 << slot) != 0 {
            self.scan_overlays.ship_design_filter = true;
        }
    }

    /// The menu's three commands: all, invert, none.
    pub fn design_filter_command(&mut self, command: FilterCommand) {
        self.scan_design_filter = command.apply(self.scan_design_filter, u16::MAX);
        if self.scan_design_filter != 0 {
            self.scan_overlays.ship_design_filter = true;
        }
    }

    /// Turn one class on or off in the enemy filter.
    pub fn toggle_class_filter(&mut self, class: u8) {
        self.scan_class_filter ^= 1 << class;
        if self.scan_class_filter & (1 << class) != 0 {
            self.scan_overlays.enemy_class_filter = true;
        }
    }

    /// The enemy menu's three commands.
    pub fn class_filter_command(&mut self, command: FilterCommand) {
        self.scan_class_filter = command.apply(self.scan_class_filter, u8::MAX);
        if self.scan_class_filter != 0 {
            self.scan_overlays.enemy_class_filter = true;
        }
    }

    /// How many ships of a fleet the scanner counts, with the filters applied.
    ///
    /// `CShipsScanVis` (`1058:4bf4`) puts the two filters on **different fleets**,
    /// which is the thing to get right: the design filter looks only at this
    /// player's own fleets and picks by design slot, and the class filter looks
    /// only at everybody else's and picks by the hull's class. A fleet that
    /// neither filter applies to is counted whole.
    #[must_use]
    pub fn filtered_ship_count(&self, fleet: &stars_core::fleet::Fleet) -> i32 {
        let me = self.local_player();
        let mine = usize::try_from(fleet.owner).is_ok_and(|owner| owner == me);
        let whole = || fleet.stacks.iter().map(|stack| stack.count).sum();

        if self.scan_overlays.ship_design_filter && mine {
            return fleet
                .stacks
                .iter()
                .filter(|stack| self.scan_design_filter & (1 << u16::from(stack.design)) != 0)
                .map(|stack| stack.count)
                .sum();
        }
        if self.scan_overlays.enemy_class_filter && !mine {
            let Some(designs) = usize::try_from(fleet.owner)
                .ok()
                .and_then(|owner| self.game.as_ref()?.designs.get(owner))
            else {
                return 0;
            };
            return fleet
                .stacks
                .iter()
                .filter(|stack| stack.count > 0)
                .filter(|stack| {
                    designs
                        .get(usize::from(stack.design))
                        .and_then(stars_core::design::ShipDesign::ship_class)
                        .is_some_and(|class| self.scan_class_filter & (1 << class.index()) != 0)
                })
                .map(|stack| stack.count)
                .sum();
        }
        whole()
    }
}

/// The three commands both filter menus begin with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterCommand {
    /// Tick everything.
    All,
    /// Tick what is not ticked and untick what is.
    Invert,
    /// Untick everything.
    None,
}

impl FilterCommand {
    /// The three, in the menu's order, with the words the game uses. The
    /// design menu spells the first and last "All Designs" and "No Designs";
    /// the enemy menu uses the same three strings.
    pub const ALL: [(FilterCommand, &'static str); 3] = [
        (FilterCommand::All, "All Designs"),
        (FilterCommand::Invert, "Invert Filter"),
        (FilterCommand::None, "No Designs"),
    ];

    /// Apply it to a mask, given which bits that mask uses.
    fn apply<T>(self, mask: T, full: T) -> T
    where
        T: std::ops::BitXor<Output = T> + Default,
    {
        match self {
            FilterCommand::All => full,
            FilterCommand::Invert => mask ^ full,
            FilterCommand::None => T::default(),
        }
    }
}

// --- The scanner's minefield filter ---------------------------------------

impl App {
    /// The four groups the minefield menu offers, and which are ticked.
    #[must_use]
    pub fn minefield_filter_entries(&self) -> Vec<FilterEntry> {
        stars_core::relations::Party::ALL
            .iter()
            .map(|party| FilterEntry {
                bit: party.index(),
                name: format!("Mine Fields of {}", party.name()),
                on: self.scan_minefield_filter & (1 << party.index()) != 0,
            })
            .collect()
    }

    /// Opening the menu with the overlay off **empties the filter first**.
    ///
    /// `ExecuteButton` clears `grbitScanMines` before it builds the menu when
    /// `grbitScan & 0x40` is clear, so a player who turned the overlay off and
    /// comes back finds nothing ticked rather than whatever was ticked before.
    pub fn open_minefield_menu(&mut self) {
        if !self.scan_overlays.minefields {
            self.scan_minefield_filter = 0;
        }
    }

    /// Tick or untick one group.
    pub fn toggle_minefield_filter(&mut self, party: u8) {
        self.scan_minefield_filter ^= 1 << party;
        self.sync_minefield_overlay();
    }

    /// The menu's two commands: all four, or none.
    ///
    /// There is no invert here — the minefield menu has two commands where the
    /// ship filters' menus have three.
    pub fn minefield_filter_command(&mut self, all: bool) {
        self.scan_minefield_filter = if all { 0xf } else { 0 };
        self.sync_minefield_overlay();
    }

    /// The overlay follows the filter exactly: empty turns it **off**, and
    /// anything ticked turns it on.
    ///
    /// This is the opposite of the two ship filters, which only ever switch
    /// themselves on — the minefield overlay is switched off again by
    /// unticking the last group.
    fn sync_minefield_overlay(&mut self) {
        self.scan_overlays.minefields = self.scan_minefield_filter != 0;
    }

    /// Whether a minefield should be drawn, given whose it is.
    #[must_use]
    pub fn shows_minefield(&self, owner: i16) -> bool {
        if !self.scan_overlays.minefields {
            return false;
        }
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return false;
        };
        let Ok(owner) = usize::try_from(owner) else {
            return false;
        };
        let party = stars_core::relations::party(game, me, owner);
        self.scan_minefield_filter & (1 << party.index()) != 0
    }
}

// --- Orbit rings ----------------------------------------------------------

/// Whose fleets are in orbit round a planet.
///
/// `DrawScanner` keeps a byte per planet and adds **one** for a fleet of this
/// player's and **two** for anybody else's, refusing to add the same kind
/// twice and stopping at three — so the three values are exactly "mine",
/// "theirs" and "both", and the ring's colour says which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OrbitRing {
    /// Only this player's fleets: a white ring.
    Yours = 1,
    /// Only other players': red.
    Theirs = 2,
    /// Some of each: magenta.
    Both = 3,
}

impl OrbitRing {
    /// Which of the three rows of rings in the scanner's sheet it is.
    #[must_use]
    pub fn row(self) -> u32 {
        u32::from(self as u8) - 1
    }

    /// The colour to draw it in without the game's own sprite, taken from the
    /// sprite itself.
    #[must_use]
    pub fn colour(self) -> [u8; 3] {
        match self {
            OrbitRing::Yours => [0xc0, 0xc0, 0xc0],
            OrbitRing::Theirs => [0xff, 0x00, 0x00],
            OrbitRing::Both => [0xff, 0x00, 0xff],
        }
    }
}

impl App {
    /// Which planets have a ring round them, and whose.
    ///
    /// A fleet earns its planet a ring only if the scanner would **count** it
    /// — the two ship filters narrow this exactly as they narrow the ship
    /// counts, which is what the manual means by "only those planets orbited
    /// by the selected ships will have orbit rings" (p. 5-15).
    #[must_use]
    pub fn orbit_rings(&self) -> std::collections::BTreeMap<u16, OrbitRing> {
        let me = self.local_player();
        let mut out: std::collections::BTreeMap<u16, u8> = std::collections::BTreeMap::new();
        let Some(game) = self.game.as_ref() else {
            return std::collections::BTreeMap::new();
        };
        for fleet in &game.fleets {
            let Some(planet) = fleet.orbiting else {
                continue;
            };
            if self.filtered_ship_count(fleet) <= 0 {
                continue;
            }
            let mine = usize::try_from(fleet.owner).is_ok_and(|owner| owner == me);
            *out.entry(planet).or_insert(0) |= if mine { 1 } else { 2 };
        }
        out.into_iter()
            .filter_map(|(planet, bits)| {
                let ring = match bits {
                    1 => OrbitRing::Yours,
                    2 => OrbitRing::Theirs,
                    3 => OrbitRing::Both,
                    _ => return None,
                };
                Some((planet, ring))
            })
            .collect()
    }
}

// --- Clicking the same spot again -----------------------------------------

/// The scale the mineral views' bars are drawn against (`cMinGrafMax`,
/// `1120:04f8`), which ships set to **5000**.
///
/// `MANUAL.PDF` p. 5-13: the mineral colours and scale "matches the display in
/// the Summary pane's mineral content graph. Rescaling that graph rescales the
/// bars in this view" — one number serves both, and the player can change it.
pub const MINERAL_GRAPH_MAX: i32 = 5000;

/// The nine scales the graph's own menu offers (`MineClick`, `1028:443c`),
/// captioned `%dkT`.
///
/// The two ends are also the range `ReadIniSettings` will accept for
/// `MineralScale`: anything outside 100 to 30000 goes back to 5000.
pub const MINERAL_SCALES: [i32; 9] = [100, 500, 1000, 2500, 5000, 7500, 10000, 20000, 30000];

/// The population each step of the Population view's circle stands for, in the
/// hundreds of colonists this engine counts in.
///
/// Nineteen thresholds at **`1058:0000`** — the very start of the scanner's own
/// code segment, which is why the reconstruction shows the lookup with no base
/// at all. A planet's circle is the first step its population does **not**
/// reach, plus two, so 2,500 colonists is radius 2 and 2,500,000 is radius 20.
pub const POPULATION_STEPS: [i32; 19] = [
    25, 50, 100, 200, 400, 800, 1000, 1500, 2250, 3000, 4000, 5000, 6000, 7500, 9000, 11000, 14000,
    18000, 25000,
];

/// One of the three bars the mineral views draw beside a planet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MineralBar {
    /// Which mineral, for its colour.
    pub mineral: usize,
    /// How tall the bar is, in pixels.
    pub height: i32,
}

/// Where the mineral views put their bars, by zoom (`vrgScanPO`, `1120:...`).
///
/// Five numbers each, and the second row is used when the map is zoomed out
/// below life size: x offset, y offset, axis length, bar width, bar spacing.
pub const MINERAL_BAR_LAYOUT: [[i32; 5]; 2] = [[7, 12, 19, 4, 6], [3, 10, 11, 2, 3]];

/// How a planet is drawn on the map: which cell of `ScannerBmp`, and how big.
///
/// `DrawScanner` draws every planet **position** as a small dot and then puts
/// something over the ones the player knows about, so an unexplored planet is
/// still on the map — everybody knows where the planets are, only not what is
/// on them.
///
/// The cells are three columns of the sheet: 3x3 dots and 5x5 dots at `x = 11`,
/// and 11x11 blobs at `x = 0` with one mask for all of them at `y = 0x45`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetMark {
    /// The cell's corner in `ScannerBmp`.
    pub cell: (u32, u32),
    /// How big the cell is, square.
    pub side: u32,
    /// The mask's corner, for the blobs that have one.
    pub mask: Option<(u32, u32)>,
    /// What to draw when there is no copy of the original to draw from.
    pub colour: [u8; 3],
}

/// The 3x3 dot every planet position gets, explored or not (`(0xb, 0xf)`).
pub const PLANET_UNEXPLORED: PlanetMark = PlanetMark {
    cell: (0xb, 0xf),
    side: 3,
    mask: None,
    colour: [0x80, 0x80, 0x80],
};

/// Where the wormhole glyph sits in `ScannerBmp`: a nine-pixel cell at `(0,
/// 0x5c)` with its mask beside it at `(9, 0x5c)`.
///
/// `DrawScanner` blits the mask and then the image, centred on the wormhole
/// (`pt - 4`).
pub const WORMHOLE_CELL: (u32, u32) = (0, 0x5c);
/// Its mask.
pub const WORMHOLE_MASK: (u32, u32) = (9, 0x5c);
/// How big both are.
pub const WORMHOLE_SIDE: u32 = 9;

/// The course line through a scanned object — see [`App::scan_scale_line`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScaleLine {
    /// Where the object is.
    pub at: stars_core::movement::Point,
    /// Which way it is going, as a vector of no particular length.
    pub heading: (i32, i32),
    /// The warp it is going at.
    pub warp: i32,
}

impl ScaleLine {
    /// How far it travels in a year: the square of the warp.
    #[must_use]
    pub fn year(&self) -> i32 {
        self.warp * self.warp
    }

    /// How far the line reaches each way — five years of that.
    #[must_use]
    pub fn reach(&self) -> i32 {
        self.year() * SCALE_YEARS
    }
}

/// How big the hole a waypoint leaves in the path is, in pixels each way.
///
/// `DrawShipScanPath` excludes `pt.x - 5 .. pt.x + 6` by `pt.y - 5 .. pt.y + 6`
/// from the clip at every waypoint before it draws the line — an **11x11 box
/// centred on the point**. `DrawScanXorLines` (`1058:8af6`) excludes the same
/// box at each corner of the drag's rubber band.
///
/// Nothing is ever drawn **into** that box: there is no waypoint glyph in the
/// game at all. The hole is the marker — the line stops short of each
/// waypoint, which both points it out and leaves whatever is at that place, a
/// planet or a fleet, unobscured.
pub const WAYPOINT_HOLE: f32 = 5.0;

/// Clip one leg of a path against the [`WAYPOINT_HOLE`] boxes at its ends.
///
/// Returns the piece of the segment that is actually drawn, or `None` when the
/// two boxes swallow it whole — which is what the original's clipping region
/// does to a leg shorter than the holes at either end of it.
///
/// The box is square, so the distance that matters is the larger of the two
/// axes (a Chebyshev radius), not the length along the leg: a diagonal leg
/// loses more of itself than a straight one, exactly as clipping against a
/// square does.
#[must_use]
pub fn leg_outside_waypoints(
    from: (f32, f32),
    to: (f32, f32),
    half: f32,
) -> Option<((f32, f32), (f32, f32))> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let reach = dx.abs().max(dy.abs());
    if reach <= 0.0 {
        return None;
    }
    let cut = half / reach;
    if cut * 2.0 >= 1.0 {
        return None;
    }
    Some((
        (from.0 + dx * cut, from.1 + dy * cut),
        (to.0 - dx * cut, to.1 - dy * cut),
    ))
}

/// One leg of the selected fleet's path — see [`App::selected_fleet_path`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathLeg {
    /// The waypoint it leaves.
    pub from: stars_core::movement::Point,
    /// The waypoint it reaches.
    pub to: stars_core::movement::Point,
    /// Whether the fleet travels this leg **twice**, which draws it yellow.
    pub doubled: bool,
}

/// One disc of the **scanner coverage** overlay — see
/// [`App::scanner_coverage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageDisc {
    /// Where its centre is, in galaxy units.
    pub position: stars_core::movement::Point,
    /// How far it reaches, after the toolbar's percentage.
    pub radius: i32,
    /// Whether it belongs to the second, penetrating pass.
    pub penetrating: bool,
}

/// The colour the **Ship Paths** overlay draws in — `hpenStarbase`, a solid
/// one-pixel pen, the same **blue** whoever owns the fleet.
///
/// The constant is `0x00ff0000`, and a COLORREF is `0x00bbggrr`: the low byte
/// is red and the high one blue, so that value is pure blue. The community
/// reconstruction's `init.c` writes it as `RGB(0xff, 0, 0)`, which is red —
/// its `RGB()` arguments are reversed throughout, and the raw constants in its
/// own disassembly (and in ours, at `1000:01af` onwards) are what to trust.
/// The pen's **name** is the other check: a starbase's square on the map is
/// blue, and so is this.
pub const PATH_COLOUR: [u8; 3] = [0x00, 0x00, 0xff];

/// The colour the **selected** fleet's own path is drawn in — `hpenShip`,
/// `0x0000ff00`, green.
pub const SHIP_PATH_COLOUR: [u8; 3] = [0x00, 0xff, 0x00];

/// And the colour a leg the fleet travels **twice** takes — `hpenYellow`,
/// `0x0000ffff`. With the Ship Paths overlay off it is the stock white pen
/// instead, which is [`DOUBLED_LEG_PLAIN`].
pub const DOUBLED_LEG_COLOUR: [u8; 3] = [0xff, 0xff, 0x00];

/// The doubled leg's colour when Ship Paths is **off**: `GetStockObject(6)`,
/// `WHITE_PEN`.
pub const DOUBLED_LEG_PLAIN: [u8; 3] = [0xff, 0xff, 0xff];

/// The line from a planet to the planet its starbase's **mass driver** is
/// aimed at — `hpenDkPurple`, `0x007f007f`.
pub const DRIVER_COLOUR: [u8; 3] = [0x7f, 0x00, 0x7f];

/// The line from a planet to the planet it **routes** to — `hpenDkGreen`,
/// `0x00007f00`.
pub const ROUTE_COLOUR: [u8; 3] = [0x00, 0x7f, 0x00];

/// How many years either way the scale line through a scanned object covers.
pub const SCALE_YEARS: i32 = 5;

/// The colour a normal coverage disc is filled with — `hbrRadar`, the constant
/// `0x0000007f`.
///
/// A COLORREF is `0x00bbggrr`, so that is **dark red**, not the dark blue this
/// project first had: the community reconstruction's `init.c` writes the
/// creation as `RGB(0x00, 0x00, 0x7f)` but its `RGB()` arguments are reversed
/// throughout. Our own binary passes `0x7f` at `1000:019f`, and the same
/// file's `hbrTooltip` — `0x9fffff`, which has to be the pale yellow Windows
/// tooltips use — settles which byte is which.
pub const COVERAGE_NORMAL: [u8; 3] = [0x7f, 0x00, 0x00];
/// And a penetrating one — `hbrRadarNear`, `0x00006060` on any screen deeper
/// than eight colours and `0x00007f7f` on one that is not: dark yellow either
/// way.
pub const COVERAGE_PENETRATING: [u8; 3] = [0x60, 0x60, 0x00];

/// The three colours the scanner tells sides apart with — `rgcrScanMine`
/// (`1058:0026`, file offset `0x58526`), which the minefields and the fleet
/// arrows both index.
///
/// The table holds pure blue, yellow and red as COLORREFs
/// (`0x00ff0000`, `0x0000ffff`, `0x000000ff`); this project lightens all three
/// by the same amount so they read against the map's black, and uses them
/// wherever the original picks a colour out of that table.
///
/// This player's.
pub const SCAN_YOURS: [u8; 3] = [0x40, 0x80, 0xff];
/// A friend's.
pub const SCAN_FRIEND: [u8; 3] = [0xff, 0xd0, 0x40];
/// Anybody else's — neutrals and enemies share it here, though the menus that
/// set relations keep the two apart.
pub const SCAN_OTHER: [u8; 3] = [0xff, 0x40, 0x40];

/// What the three kinds of minefield are called: the table of literals at
/// `DS:0x4d8`, reached through the pointers at `DS:0x4f2`.
///
/// Two places index it — `DrawMineSurvey` (`1028:1c8a`) for the pane's
/// `Field Type:  %s`, and `PszGetThingName` (`1038:279e`) for the object's own
/// name, which is `"%s%s Mine Field"` — so the kind is the adjective and the
/// words `Mine Field` are not part of it.
pub const MINEFIELD_KINDS: [&str; 3] = ["Standard", "Heavy", "Speed Bump"];

/// One line of the scanner's right-click menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanMenuItem {
    /// What choosing it selects.
    pub object: ScanObject,
    /// What it reads.
    pub label: String,
    /// Whether it is the current selection, which the original ticks.
    pub checked: bool,
    /// Whether a separator goes above it, as the original puts one between the
    /// planet and the fleets.
    pub first_of_group: bool,
}

/// One thing the scanner can have selected at a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanObject {
    /// A planet, by id.
    Planet(i16),
    /// A fleet, by index into `GameState::fleets`.
    Fleet(usize),
    /// A space object — the original's `grobjThing`.
    Thing(ScanThing),
}

/// One space object, by the list that holds it.
///
/// The original keeps all four kinds in one `lpThings` array and tells them
/// apart by `ith`; this engine keeps a list per kind, so the kind and the index
/// travel together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanThing {
    /// A minefield (`ithMinefield`).
    Minefield(usize),
    /// A mineral packet in flight (`ithMineralPacket`).
    Packet(usize),
    /// A wormhole (`ithWormhole`).
    Wormhole(usize),
    /// The Mystery Trader (`ithMysteryTrader`).
    Trader(usize),
}

impl App {
    /// Everything of the player's **own** at a point, in the order the scanner
    /// cycles them: the planet first, then their fleets in fleet order.
    ///
    /// `FGetNextObjHere` (`1058:909c`) is called with `fOnlyOurs` set, so
    /// another player's fleets are not in the cycle; and it only returns to the
    /// planet when the planet is this player's own, so somebody else's planet
    /// is not either. A spot can therefore hold plenty and still cycle through
    /// nothing.
    #[must_use]
    pub fn objects_at(&self, x: i16, y: i16) -> Vec<ScanObject> {
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if let Some(planet) = game
            .planets
            .iter()
            .chain(game.known_planets.iter())
            .find(|planet| planet.position == Some(stars_core::movement::Point::new(x, y)))
        {
            if planet.owner == i16::try_from(me).ok() {
                out.push(ScanObject::Planet(planet.id));
            }
        }
        for (index, fleet) in game.fleets.iter().enumerate() {
            if fleet.position.x == x
                && fleet.position.y == y
                && usize::try_from(fleet.owner).is_ok_and(|owner| owner == me)
            {
                out.push(ScanObject::Fleet(index));
            }
        }
        out
    }

    /// What the scanner has selected, as one object.
    #[must_use]
    pub fn selected_object(&self) -> Option<ScanObject> {
        if let Some(thing) = self.selection.thing {
            return Some(ScanObject::Thing(thing));
        }
        if self.selection.on_fleet {
            if let Some(index) = self.selection.fleet {
                return Some(ScanObject::Fleet(index));
            }
        }
        self.selection.planet.map(ScanObject::Planet)
    }

    /// Where an object is, in galaxy units.
    #[must_use]
    pub fn object_position(&self, object: ScanObject) -> Option<stars_core::movement::Point> {
        let game = self.game.as_ref()?;
        match object {
            ScanObject::Planet(id) => {
                game.planets
                    .iter()
                    .chain(game.known_planets.iter())
                    .find(|planet| planet.id == id)?
                    .position
            }
            ScanObject::Fleet(index) => Some(game.fleets.get(index)?.position),
            ScanObject::Thing(thing) => match thing {
                ScanThing::Minefield(index) => Some(game.minefields.get(index)?.position),
                ScanThing::Packet(index) => Some(game.packets.get(index)?.position),
                ScanThing::Wormhole(index) => Some(game.wormholes.get(index)?.position),
                ScanThing::Trader(index) => Some(game.traders.get(index)?.position),
            },
        }
    }

    /// Where the selection is, in galaxy units — the original's `ptSelMain`,
    /// which several of the scanner's marks compare against.
    #[must_use]
    pub fn selected_point(&self) -> Option<stars_core::movement::Point> {
        self.selected_object()
            .and_then(|object| self.object_position(object))
    }

    /// Select one thing the scanner found.
    pub fn select_object(&mut self, object: ScanObject) {
        // A planet or a fleet takes the selection off whatever space object had
        // it: `sel.grobj` names one thing at a time.
        if !matches!(object, ScanObject::Thing(_)) {
            self.selection.thing = None;
        }
        match object {
            ScanObject::Thing(thing) => {
                self.selection.thing = Some(thing);
            }
            ScanObject::Planet(id) => {
                self.selection.planet = Some(id);
                self.selection.on_fleet = false;
            }
            ScanObject::Fleet(index) => {
                if self.selection.fleet != Some(index) {
                    self.selection.waypoint = None;
                }
                self.selection.fleet = Some(index);
                self.selection.on_fleet = true;
                // The scanner keeps the planet under a fleet in orbit, because
                // the status bar names it whichever is in front.
                if let Some(planet) = self
                    .game
                    .as_ref()
                    .and_then(|game| game.fleets.get(index))
                    .and_then(|fleet| fleet.orbiting)
                {
                    if let Ok(id) = i16::try_from(planet) {
                        self.selection.planet = Some(id);
                    }
                }
            }
        }
    }

    /// The scanner's left click, on the thing the pointer landed on.
    ///
    /// This is the order `ScannerWndProc`'s `WM_LBUTTONDOWN` arm works in, and
    /// getting it round the right way matters:
    ///
    /// 1. `FFindNearestObject` finds what was clicked;
    /// 2. `ChangeScanSel(&scan, 1)` **selects it**;
    /// 3. only if the click was on the spot already selected does
    ///    `FGetNextObjHere` step to the next thing there.
    ///
    /// So a click selects what is under the pointer — the planet when the
    /// pointer is on the planet, whatever else orbits it notwithstanding — and
    /// it is the *second* click on the same spot that cycles.
    ///
    /// Returns whether the selection moved.
    pub fn scan_click_on(&mut self, hit: ScanObject) -> bool {
        // A space object never cycles: `FGetNextObjHere` is reached only when
        // what was clicked is a fleet or a planet.
        if matches!(hit, ScanObject::Thing(_)) {
            if Some(hit) == self.selected_object() {
                return false;
            }
            self.select_object(hit);
            return true;
        }
        let same_spot = self
            .object_position(hit)
            .and_then(|at| {
                let current = self.selected_object()?;
                Some(self.object_position(current)? == at)
            })
            .unwrap_or(false);
        if !same_spot {
            if Some(hit) == self.selected_object() {
                return false;
            }
            self.select_object(hit);
            return true;
        }
        // The same spot again: step round what is here. A spot whose things do
        // not cycle — somebody else's — leaves the selection where it is.
        match self.object_position(hit) {
            Some(at) => self.scan_click(at.x, at.y),
            None => false,
        }
    }

    /// What the scanner's **right click** offers, at a galaxy point.
    ///
    /// `ScannerWndProc`'s `WM_RBUTTONDOWN` arm builds a list and puts it up as
    /// a popup with the current selection ticked (`PopupMenu(..., iChecked,
    /// 1)`). The list is the planet at that point, then **every fleet there,
    /// whoever owns it** — this is not the ours-only cycle — with a separator
    /// between the two groups that is dropped when there are no fleets.
    ///
    /// The original's list also carries the `THING`s at that point: minefields,
    /// wormholes and packets. They are left out here because this project's
    /// selection has nowhere to put one — see `docs/ui/scanner.md`.
    #[must_use]
    pub fn scan_menu(&self, x: i16, y: i16) -> Vec<ScanMenuItem> {
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let at = stars_core::movement::Point::new(x, y);
        let selected = self.selected_object();
        let mut out: Vec<ScanMenuItem> = Vec::new();

        if let Some(planet) = game
            .planets
            .iter()
            .chain(game.known_planets.iter())
            .find(|planet| planet.position == Some(at))
        {
            let object = ScanObject::Planet(planet.id);
            out.push(ScanMenuItem {
                object,
                label: self.planet_name(planet.id),
                checked: Some(object) == selected,
                first_of_group: false,
            });
        }
        let planets = out.len();
        for (index, fleet) in game.fleets.iter().enumerate() {
            if fleet.position != at || fleet.stacks.is_empty() {
                continue;
            }
            let object = ScanObject::Fleet(index);
            out.push(ScanMenuItem {
                object,
                label: self.fleet_display_name(index),
                checked: Some(object) == selected,
                // The rule the separator is dropped by: it only appears when
                // there is a planet above and a fleet below.
                first_of_group: planets > 0 && out.len() == planets,
            });
        }

        // Then the space objects, behind a separator of their own — the
        // original writes one `-1` before the first of them and only if
        // something came before.
        let before = out.len();
        for thing in self.things_at(x, y) {
            let object = ScanObject::Thing(thing);
            out.push(ScanMenuItem {
                object,
                label: self.thing_name(thing),
                checked: Some(object) == selected,
                first_of_group: before > 0 && out.len() == before,
            });
        }
        out
    }

    /// How a planet the player knows about is drawn.
    ///
    /// `SCAN_LNormalScannerMode`, the arm the Normal view and the fallbacks use:
    ///
    /// * **nobody owns it** — a white 3x3 at `(0xb, 0x12)`, unless it is the
    ///   selected point, which the base loop has already drawn as the grey
    ///   11x11 starburst at `(0, 0x21)`;
    /// * **somebody owns it** — a 5x5 at `(0xb, y)`, `y` being `0` for this
    ///   player, `10` for a friend and `5` for anybody else;
    /// * **and it is selected** — the 11x11 blob at `(0, y)` with the same
    ///   three-way choice, `0`, `0x16` and `0xb`, over the mask at `(0, 0x45)`.
    ///
    /// The three colours are the sheet's own: green for this player, yellow for
    /// a friend, red for the rest.
    #[must_use]
    pub fn planet_mark(&self, planet: &Planet, selected: bool) -> PlanetMark {
        let me = self.local_player();
        let Some(owner) = planet.owner else {
            return if selected {
                PlanetMark {
                    cell: (0, 0x21),
                    side: 11,
                    mask: Some((0, 0x45)),
                    colour: [0xc0, 0xc0, 0xc0],
                }
            } else {
                PlanetMark {
                    cell: (0xb, 0x12),
                    side: 3,
                    mask: None,
                    colour: [0xff, 0xff, 0xff],
                }
            };
        };
        // Which of the three the owner earns. The original reads the relations
        // table straight, so only a **friend** is set apart; a neutral is drawn
        // like an enemy.
        let friend = usize::try_from(owner).is_ok_and(|owner| {
            self.game.as_ref().is_some_and(|game| {
                stars_core::relations::regard(game, me, owner)
                    == stars_core::relations::Relation::Friend
            })
        });
        let mine = usize::try_from(owner).is_ok_and(|owner| owner == me);
        let (small, big, colour) = if mine {
            (0, 0, [0x00, 0xc0, 0x00])
        } else if friend {
            (10, 0x16, [0xff, 0xff, 0x00])
        } else {
            (5, 0xb, [0xff, 0x00, 0x00])
        };
        if selected {
            PlanetMark {
                cell: (0, big),
                side: 11,
                mask: Some((0, 0x45)),
                colour,
            }
        } else {
            PlanetMark {
                cell: (0xb, small),
                side: 5,
                mask: None,
                colour,
            }
        }
    }

    /// What colour a fleet's arrow is drawn in.
    ///
    /// `DrawScanner` sets the text colour before blitting the arrow, and the
    /// three values it picks from are `rgcrScanMine`'s: **blue** for this
    /// player, **yellow** for a friend, **red** for anybody else. It is not a
    /// colour per player — two enemies' fleets are the same red.
    #[must_use]
    pub fn fleet_arrow_colour(&self, fleet: &stars_core::fleet::Fleet) -> [u8; 3] {
        let me = self.local_player();
        let Ok(owner) = usize::try_from(fleet.owner) else {
            return SCAN_OTHER;
        };
        if owner == me {
            return SCAN_YOURS;
        }
        let Some(game) = self.game.as_ref() else {
            return SCAN_OTHER;
        };
        if stars_core::relations::regard(game, me, owner) == stars_core::relations::Relation::Friend
        {
            SCAN_FRIEND
        } else {
            SCAN_OTHER
        }
    }

    /// Whether a fleet is drawn on the map at all.
    ///
    /// `DrawScanner` skips a fleet whose `CShipsScanVis` count is zero, so the
    /// two ship filters hide arrows exactly as they hide orbit rings and ship
    /// counts — **unless it is the selected fleet**, which is always drawn so
    /// that filtering cannot lose what the pane is showing. (The original
    /// compares fleet ids alone there, and ids are per player, so another
    /// player's fleet of the same id escapes the filter too; this compares the
    /// fleet itself.)
    #[must_use]
    pub fn fleet_scan_visible(&self, index: usize, fleet: &stars_core::fleet::Fleet) -> bool {
        self.selection.fleet == Some(index) || self.filtered_ship_count(fleet) > 0
    }

    /// The polylines the **Ship Paths** overlay draws, one per fleet, in
    /// galaxy units.
    ///
    /// `grbitScan & 0x80`, a pass of its own that runs **before** the planets
    /// are drawn, so the lines lie under the planet dots and the fleet marks.
    /// Each is drawn with `hpenStarbase` — a solid one-pixel **red** pen,
    /// `RGB(0xff, 0, 0)` — whoever the fleet belongs to, and it starts at
    /// **waypoint 0**, the fleet's own position, rather than at wherever the
    /// fleet has since been drawn.
    ///
    /// A fleet is skipped unless all of these hold:
    ///
    /// * the view is not **No Player Information**, which draws no paths at
    ///   all;
    /// * the fleet is not dead (`fDead`);
    /// * its record's **detail is more than 6** — that is, 7, a full record.
    ///   Only a fleet you own is described in that much detail, so in practice
    ///   nobody else's path is ever drawn; this engine keeps every fleet in
    ///   one list and no detail level with it, so ownership stands in for the
    ///   test (see `docs/formats/fleet.md`);
    /// * it has **more than one** waypoint — a path needs two ends;
    /// * and `CShipsScanVis` counts something of it, or it is the selected
    ///   fleet: the two ship filters narrow the paths exactly as they narrow
    ///   the arrows.
    #[must_use]
    pub fn fleet_paths(&self) -> Vec<Vec<stars_core::movement::Point>> {
        if !self.scan_overlays.fleet_paths || self.scan_view == ScanView::NoPlayerInfo {
            return Vec::new();
        }
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let me = self.local_player();
        game.fleets
            .iter()
            .enumerate()
            .filter(|(index, fleet)| {
                usize::try_from(fleet.owner).is_ok_and(|owner| owner == me)
                    && fleet.waypoints.len() > 1
                    && self.fleet_scan_visible(*index, fleet)
            })
            .map(|(_, fleet)| fleet.waypoints.iter().map(|way| way.position).collect())
            .collect()
    }

    /// The **scale line** through whatever the scanner has selected, when that
    /// object's course is known.
    ///
    /// `DrawShipScanPath` (`1058:540c`) draws a line through the selection
    /// along its heading, `warp² × 5` galaxy units each way — the square of
    /// the warp is a year's travel, so the line reaches **five years** back
    /// and five forward — with a mark at every year. It is the answer to
    /// "where will that thing be?", and this project had none of it.
    ///
    /// Three kinds of object have a course to draw:
    ///
    /// * a **fleet**, from the direction and warp stored with the sighting.
    ///   Both are only recorded for a fleet described in part — somebody
    ///   else's — so a fleet of your own gets no scale line and its
    ///   [waypoints](Self::selected_fleet_path) instead;
    /// * a **mineral packet**, whose heading is the vector to the planet it
    ///   was flung at and whose warp is the stored nibble plus four;
    /// * the **Mystery Trader**, likewise towards its destination.
    #[must_use]
    pub fn scan_scale_line(&self) -> Option<ScaleLine> {
        let game = self.game.as_ref()?;
        let at;
        let heading;
        let warp;
        match self.selected_object()? {
            ScanObject::Fleet(index) => {
                let fleet = game.fleets.get(index)?;
                at = fleet.position;
                heading = fleet.direction?;
                warp = i32::from(fleet.warp?);
            }
            ScanObject::Thing(ScanThing::Packet(index)) => {
                let packet = game.packets.get(index)?;
                if packet.warp == 0 {
                    return None;
                }
                let target = self.planet_position(packet.target)?;
                at = packet.position;
                heading = (target.x - at.x, target.y - at.y);
                warp = packet.speed();
            }
            ScanObject::Thing(ScanThing::Trader(index)) => {
                let trader = game.traders.get(index)?;
                at = trader.position;
                heading = (trader.destination.x - at.x, trader.destination.y - at.y);
                warp = i32::from(trader.warp);
            }
            _ => return None,
        }
        if warp <= 0 || (heading.0 == 0 && heading.1 == 0) {
            return None;
        }
        Some(ScaleLine {
            at,
            heading: (i32::from(heading.0), i32::from(heading.1)),
            warp,
        })
    }

    /// Where a planet is, by id.
    fn planet_position(&self, id: u16) -> Option<stars_core::movement::Point> {
        let game = self.game.as_ref()?;
        let id = i16::try_from(id).ok()?;
        game.planets
            .iter()
            .chain(game.known_planets.iter())
            .find(|planet| planet.id == id)?
            .position
    }

    /// The **selected fleet's own path**, leg by leg, as `DrawShipScanPath`
    /// draws it under the waypoints.
    ///
    /// The line runs from waypoint to waypoint in `hpenShip` green, and a leg
    /// the fleet travels **twice** — the same pair of points again later, in
    /// either direction, as a there-and-back shuttle has — is drawn **once**,
    /// in yellow, with the repeat left out. Compare
    /// [`App::fleet_paths`](Self::fleet_paths), which is the overlay drawn for
    /// every fleet; this is the selected one's, drawn over it.
    ///
    /// The original walks its `rgDup` table with the two loops one index
    /// apart, which would colour a leg either side of the doubled one; the
    /// reading that makes them agree — and the only one that draws what the
    /// game plainly draws — is that the entry belongs to the leg **into**
    /// waypoint `i`, which is what this does.
    #[must_use]
    pub fn selected_fleet_path(&self) -> Vec<PathLeg> {
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let Some(fleet) = self
            .selection
            .fleet
            .filter(|_| self.selection.thing.is_none())
            .and_then(|index| game.fleets.get(index))
        else {
            return Vec::new();
        };
        let points: Vec<_> = fleet.waypoints.iter().map(|way| way.position).collect();
        if points.len() < 2 {
            return Vec::new();
        }
        // Which legs are travelled twice: the first of a pair is drawn in
        // yellow and the second not at all.
        let mut doubled = vec![false; points.len()];
        let mut repeat = vec![false; points.len()];
        for i in 1..points.len() {
            if doubled[i] || repeat[i] {
                continue;
            }
            let leg = (points[i - 1], points[i]);
            for j in i + 1..points.len() {
                let other = (points[j - 1], points[j]);
                if other == leg || other == (leg.1, leg.0) {
                    doubled[i] = true;
                    repeat[j] = true;
                }
            }
        }
        (1..points.len())
            .filter(|i| !repeat[*i])
            .map(|i| PathLeg {
                from: points[i - 1],
                to: points[i],
                doubled: doubled[i],
            })
            .collect()
    }

    /// The line from a selected planet to the planet its starbase's **mass
    /// driver** is aimed at.
    ///
    /// `DrawShipScanPath`'s last arm draws this one first, in dark purple, and
    /// then falls into the route line's loop — the `goto` jumps in past the
    /// assignment that would stop it — so a planet with **both** a driver
    /// target and a route shows **both** lines. The planet must actually have
    /// a starbase for the driver line to be considered.
    #[must_use]
    pub fn planet_driver_line(
        &self,
    ) -> Option<(stars_core::movement::Point, stars_core::movement::Point)> {
        if self.selection.on_fleet || self.selection.thing.is_some() {
            return None;
        }
        let id = self.selection.planet?;
        let game = self.game.as_ref()?;
        let planet = game.planets.iter().find(|planet| planet.id == id)?;
        if !planet.starbase {
            return None;
        }
        let from = self.planet_position(u16::try_from(id).ok()?)?;
        let to = self.planet_position(u16::try_from(planet.fling_dest?).ok()?)?;
        Some((from, to))
    }

    /// The line from a selected planet to the planet it **routes** to.
    ///
    /// `DrawShipScanPath`'s last arm: a dark-green line to `PLANET.idRoute`,
    /// so a planet set to send its new fleets somewhere shows where. See
    /// [`Self::planet_driver_line`] for the purple one drawn before it.
    #[must_use]
    pub fn planet_route_line(
        &self,
    ) -> Option<(stars_core::movement::Point, stars_core::movement::Point)> {
        if self.selection.on_fleet || self.selection.thing.is_some() {
            return None;
        }
        let id = self.selection.planet?;
        let from = self.planet_position(u16::try_from(id).ok()?)?;
        let game = self.game.as_ref()?;
        let planet = game.planets.iter().find(|planet| planet.id == id)?;
        let to = self.planet_position(u16::try_from(planet.route_dest?).ok()?)?;
        Some((from, to))
    }

    /// The 11x11 cell a fleet **at the selected point** is drawn with, in place
    /// of its arrow.
    ///
    /// `(0xb, 0x24)` for one of this player's and `(0xb, 0x2f)` for anybody
    /// else's — the two rows of the sheet's arrow glyph, blue and red.
    #[must_use]
    pub fn fleet_selected_cell(&self, fleet: &stars_core::fleet::Fleet) -> (u32, u32) {
        let mine = usize::try_from(fleet.owner).is_ok_and(|owner| owner == self.local_player());
        (0xb, if mine { 0x24 } else { 0x2f })
    }

    /// Whether a fleet is drawn as an **arrow of its own** rather than as a
    /// ring round the planet it orbits.
    ///
    /// `DrawScanner` splits the two: `fl->idPlanet == -1` gets the arrow, and
    /// everything else adds to `rgWhatsHere` for the planet's orbit ring. A
    /// fleet in orbit has no arrow at all.
    #[must_use]
    pub fn fleet_draws_arrow(fleet: &stars_core::fleet::Fleet) -> bool {
        fleet.orbiting.is_none()
    }

    /// Whether the orbit rings are drawn in the view showing.
    ///
    /// The ring arm is guarded by `uVar8 < 3`, so the three views that redraw
    /// the planets themselves — Planet Value, Population and No Player
    /// Information — have no rings.
    #[must_use]
    pub fn orbit_rings_visible(&self) -> bool {
        matches!(
            self.scan_view,
            ScanView::Normal | ScanView::SurfaceMineral | ScanView::MineralConcentration
        )
    }

    /// The discs of the **scanner coverage** overlay, in the order the
    /// original paints them: every normal range first, then every penetrating
    /// one on top.
    ///
    /// `DrawScanner`'s `grbitScan & 0x20` block. It is the **first** thing
    /// drawn after the map is cleared, so the discs lie under the planets and
    /// the fleets rather than over them, and each is a **filled** ellipse —
    /// `hbrRadar`, `RGB(0, 0, 0x7f)`, with a pen of the same colour — not an
    /// outline. The penetrating pass swaps in `hbrRadarNear`,
    /// `RGB(0x60, 0x60, 0)` on any screen deeper than eight colours.
    ///
    /// What contributes:
    ///
    /// * every planet of this player's, at `GetPlanetScannerRange`;
    /// * every fleet of this player's, at `GetFleetScannerRange` — the
    ///   **largest** range among its designs, not a combination of them —
    ///   and only when that range is positive;
    /// * the same two again for the penetrating pass, a planet's radius being
    ///   its **normal range halved** (the code shifts, rather than using the
    ///   penetrating range it was handed; the two agree for every scanner in
    ///   the game);
    /// * and, for a **Packet Physics** race, each of this player's mineral
    ///   packets under way, at the square of its warp — `(stored + 4)²`,
    ///   which `MANUAL.PDF` p. 20-9 states as "the square of the packet's
    ///   warp speed".
    ///
    /// Every radius is scaled by the toolbar's coverage percentage
    /// (`vpctRadarView`) with `MulDiv`, which rounds to nearest.
    #[must_use]
    pub fn scanner_coverage(&self) -> Vec<CoverageDisc> {
        if !self.scan_overlays.scanner_coverage {
            return Vec::new();
        }
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let me = self.local_player();
        let Some(player) = game.players.get(me) else {
            return Vec::new();
        };
        let Ok(mine) = i16::try_from(me) else {
            return Vec::new();
        };
        let race = &player.race;
        let levels = player.research.levels;

        let mut normal: Vec<CoverageDisc> = Vec::new();
        let mut deep: Vec<CoverageDisc> = Vec::new();

        for planet in &game.planets {
            if planet.owner != Some(mine) {
                continue;
            }
            let Some(position) = planet.position else {
                continue;
            };
            let range = self.planet_scan_range(planet, race, &levels);
            if range.normal > 0 {
                normal.push(CoverageDisc {
                    position,
                    radius: self.coverage_scaled(range.normal),
                    penetrating: false,
                });
            }
            if range.penetrating > 0 {
                // The same figure as `range.normal / 2`, which is what a
                // planet's penetrating range always works out to — a
                // penetrating scanner's stored ability is negative and
                // halved, and an AR's is set from the normal range the same
                // way. Written as the range itself so that the two branches
                // say the same thing.
                deep.push(CoverageDisc {
                    position,
                    radius: self.coverage_scaled(range.penetrating),
                    penetrating: true,
                });
            }
        }

        for fleet in &game.fleets {
            if fleet.owner != mine {
                continue;
            }
            let range = self.fleet_scan_range(fleet);
            if range.normal > 0 {
                normal.push(CoverageDisc {
                    position: fleet.position,
                    radius: self.coverage_scaled(range.normal),
                    penetrating: false,
                });
            }
            if range.penetrating > 0 {
                deep.push(CoverageDisc {
                    position: fleet.position,
                    radius: self.coverage_scaled(range.penetrating),
                    penetrating: true,
                });
            }
        }

        if race.prt() == Some(stars_core::race::Prt::Pp) {
            for packet in &game.packets {
                if packet.owner != mine || packet.warp == 0 {
                    continue;
                }
                // The same number as a year's travel, arrived at separately:
                // both are the square of the warp.
                let warp = packet.speed();
                deep.push(CoverageDisc {
                    position: packet.position,
                    radius: self.coverage_scaled(warp * warp),
                    penetrating: true,
                });
            }
        }

        normal.append(&mut deep);
        normal
    }

    /// A planet's two scanner ranges, with the part of the Alternate Reality
    /// rule that needs the starbase filled in.
    ///
    /// `GetPlanetScannerRange` reads the planet's own scanner — `iScanner` of
    /// 31 means it has none — except for an **AR** race, which scans from its
    /// starbase by population and penetrates at half range only when that
    /// starbase's hull is better than `0x22`, the Space Station. The core
    /// routine cannot see the starbase, so that last part is settled here.
    fn planet_scan_range(
        &self,
        planet: &Planet,
        race: &stars_core::Race,
        levels: &[u8; 6],
    ) -> stars_core::scanning::ScannerRange {
        let mut range = stars_core::scanning::planet_scanner_range_for_tech(
            planet,
            race,
            levels,
            planet.scanner.is_some(),
        );
        if race.is_ar() && !race.has_lrt(stars_core::race::lrt::NO_ADV_SCANNER) {
            let big = self.starbase_hull(planet).is_some_and(|hull| hull > 0x22);
            range.penetrating = if big { range.normal / 2 } else { 0 };
        }
        range
    }

    /// A fleet's two scanner ranges: the **largest** of its designs', each
    /// counted separately.
    ///
    /// `GetFleetScannerRange` (`1038:4fb8`) walks the sixteen design slots and
    /// keeps the maximum of each range — the fourth-root combination applies
    /// **within** a design, between its own scanners, and not across the
    /// designs in a fleet.
    #[must_use]
    pub fn fleet_scan_range(
        &self,
        fleet: &stars_core::fleet::Fleet,
    ) -> stars_core::scanning::ScannerRange {
        let mut out = stars_core::scanning::ScannerRange::default();
        let Some(game) = self.game.as_ref() else {
            return out;
        };
        let Some(designs) = usize::try_from(fleet.owner)
            .ok()
            .and_then(|owner| game.designs.get(owner))
        else {
            return out;
        };
        for stack in &fleet.stacks {
            if stack.count <= 0 {
                continue;
            }
            let Some(design) = designs.get(usize::from(stack.design)) else {
                continue;
            };
            let range = design.scanner_range();
            out.normal = out.normal.max(range.normal);
            out.penetrating = out.penetrating.max(range.penetrating);
        }
        out
    }

    /// A coverage radius after the toolbar's percentage is applied.
    ///
    /// `MulDiv(range, vpctRadarView, 100)` when the percentage is under a
    /// hundred, and `MulDiv` rounds to nearest rather than truncating.
    #[must_use]
    pub fn coverage_scaled(&self, range: i32) -> i32 {
        let pct = i32::from(self.scan_coverage_pct);
        if pct >= 100 {
            return range;
        }
        (range * pct + 50) / 100
    }

    /// The two discs the **Planet Value** view draws, outermost first.
    ///
    /// Each is a radius and a colour. The value is `PctPlanetDesirability`,
    /// and when that is negative the view falls back to `PctPlanetOptValue` —
    /// what the planet would be worth **terraformed** — which is what the
    /// yellow pair means. A Claim Adjuster is shown the terraformed value
    /// straight and never sees yellow, because its planets are at their
    /// optimum every year.
    ///
    /// `None` when the planet's environment is not known: there is no value to
    /// show.
    #[must_use]
    pub fn planet_value_discs(&self, planet: &Planet) -> Option<[(f32, [u8; 3]); 2]> {
        let me = self.local_player();
        let player = self.game.as_ref()?.players.get(me)?;
        let race = &player.race;
        let adjuster = race.prt() == Some(stars_core::race::Prt::Ca);
        let known = planet.detail >= stars_core::planet::Detail::Scanned;
        // What the planet would be worth terraformed, which is what
        // `PctPlanetOptValue` measures: the environment moved as far toward the
        // race's ideal as this player's technology reaches.
        let optimum = || {
            let reach = stars_core::terraform::optimal_env(planet, race, player.research.levels);
            stars_core::ai::colonise::pct_planet_opt_value(planet, race, reach)
        };

        let mut terraformed = false;
        let value = if known {
            let value = stars_core::hab::pct_planet_desirability(planet, race);
            if value < 0 {
                let optimum = optimum();
                if optimum >= 0 && !adjuster {
                    terraformed = true;
                }
                optimum
            } else {
                value
            }
        } else if adjuster {
            optimum()
        } else {
            return None;
        };

        // The outer disc grows with the value — or with how hostile it is.
        let radius = if value < 0 { -value / 5 } else { value / 11 };
        let radius = (radius + 2).min(10);
        // The inner one is two smaller, or one when that would be under three.
        let inner = if radius - 2 < 3 {
            radius - 1
        } else {
            radius - 2
        }
        .max(1);
        let (outer_colour, inner_colour) = if value < 0 {
            ([0x60, 0x70, 0x80], [0xff, 0x00, 0x00])
        } else if terraformed {
            ([0x80, 0x80, 0x00], [0xff, 0xff, 0x00])
        } else {
            ([0x00, 0x80, 0x00], [0xff, 0xff, 0xff])
        };
        #[allow(clippy::cast_precision_loss)]
        Some([(radius as f32, outer_colour), (inner as f32, inner_colour)])
    }

    /// The **flag** the Planet Value view plants on an inhabited planet.
    ///
    /// A pole and a banner, in the owner's colour: blue for this player,
    /// yellow for a friend, grey for a neutral and red for an enemy. The
    /// manual (p. 5-13) lumps the last two together — "red flags mark planets
    /// of neutrals and enemies" — but the code keeps them apart, and the code
    /// is what this follows. A planet nobody lives on has no flag.
    #[must_use]
    pub fn planet_value_flag(&self, planet: &Planet) -> Option<[u8; 3]> {
        let owner = usize::try_from(planet.owner?).ok()?;
        let me = self.local_player();
        if owner == me {
            return Some([0x40, 0x80, 0xff]);
        }
        let game = self.game.as_ref()?;
        Some(match stars_core::relations::regard(game, me, owner) {
            stars_core::relations::Relation::Friend => [0xff, 0xd0, 0x40],
            stars_core::relations::Relation::Neutral => [0x80, 0x90, 0xa0],
            stars_core::relations::Relation::Enemy => [0xff, 0x00, 0x00],
        })
    }

    /// The three bars the **mineral** views draw beside a planet.
    ///
    /// Surface minerals are scaled against [`Self::mineral_scale`] and
    /// concentrations by a fifth, both capped at twenty pixels and halved when
    /// the map is zoomed out. A surface reading needs a planet this player has
    /// **been to**; a concentration only one that has been scanned.
    #[must_use]
    pub fn planet_mineral_bars(&self, planet: &Planet, concentration: bool) -> Vec<MineralBar> {
        use stars_core::planet::Detail;
        // A concentration reading only wants a planet that has been scanned;
        // a surface reading wants one this player has been to.
        let enough = if concentration {
            planet.detail >= Detail::Scanned
        } else {
            planet.detail == Detail::Full
        };
        if !enough {
            return Vec::new();
        }
        let small = self.scan_zoom < 0;
        (0..3)
            .map(|mineral| {
                let height = if concentration {
                    i32::from(planet.min_conc[mineral]) / 5
                } else {
                    (planet.surface_min[mineral] + self.mineral_scale / 40)
                        / (self.mineral_scale / 20).max(1)
                }
                .min(20);
                MineralBar {
                    mineral,
                    height: if small { height / 2 } else { height },
                }
            })
            .collect()
    }

    /// Where the mineral bars go, by zoom.
    #[must_use]
    pub fn mineral_bar_layout(&self) -> [i32; 5] {
        MINERAL_BAR_LAYOUT[usize::from(self.scan_zoom < 0)]
    }

    /// The radius of the **Population** view's circle, and its colour.
    ///
    /// The circle is the first of [`POPULATION_STEPS`] the population does not
    /// reach, plus two. Another player's population is not known: the original
    /// takes the planet's own guess field and shifts it left twice, which this
    /// engine does not keep, so their planets are drawn at the smallest size
    /// rather than guessed at.
    ///
    /// A planet nobody lives on falls back to the ordinary mark, which is what
    /// the manual means by "uncolonized planets that you've visited are small
    /// and grey" (p. 5-13).
    #[must_use]
    pub fn planet_population_disc(&self, planet: &Planet) -> Option<(f32, [u8; 3])> {
        use stars_core::planet::Detail;
        let owner = usize::try_from(planet.owner?).ok()?;
        if planet.detail < Detail::Scanned {
            return None;
        }
        let me = self.local_player();
        let population = if owner == me { planet.pop } else { 0 };
        let step = POPULATION_STEPS
            .iter()
            .position(|threshold| population <= *threshold)
            .unwrap_or(POPULATION_STEPS.len());
        let game = self.game.as_ref()?;
        let colour = if owner == me {
            [0x00, 0xc0, 0x00]
        } else if stars_core::relations::regard(game, me, owner)
            == stars_core::relations::Relation::Friend
        {
            [0xff, 0xff, 0x00]
        } else {
            [0xff, 0x00, 0x00]
        };
        #[allow(clippy::cast_precision_loss)]
        Some(((step + 2) as f32, colour))
    }

    /// The **starbase** mark beside a planet, if it has one.
    ///
    /// A filled square at `pt + (3, -4)`, or `pt + (4, -6)` and larger when the
    /// planet is the selected one. It is **blue** for a full starbase and
    /// **yellow** otherwise — `fStarbase` is 2 when the design's hull is
    /// `0x20`, which is 32, the first of the starbase hulls: the **Orbital
    /// Fort**. So a fort is yellow and everything built beyond one is blue.
    #[must_use]
    pub fn planet_starbase_mark(&self, planet: &Planet) -> Option<[u8; 3]> {
        if !planet.starbase {
            return None;
        }
        let full = self.starbase_hull(planet).is_some_and(|hull| hull != 32);
        Some(if full {
            [0x40, 0x80, 0xff]
        } else {
            [0xff, 0xd0, 0x40]
        })
    }

    /// The hull a planet's starbase is built on.
    fn starbase_hull(&self, planet: &Planet) -> Option<i16> {
        let game = self.game.as_ref()?;
        let owner = usize::try_from(planet.owner?).ok()?;
        let design = planet.starbase_design?;
        game.designs
            .get(owner)?
            .get(starbase_slot(design))
            .map(|design| design.hull_id)
    }

    /// What colour a minefield is drawn in.
    ///
    /// `DrawScanner` walks the fields in three groups and gives each its own
    /// colour from `rgcrScanMine`, which `MANUAL.PDF` p. 5-14 names: **yours
    /// blue, a friend's yellow, and anybody else's red**. The map shares one
    /// colour between neutrals and enemies where the menu keeps them apart.
    ///
    /// A field **armed to detonate** is drawn red whoever owns it: it is the
    /// second pass the loop makes over the standard fields, and the only kind
    /// that can be armed.
    #[must_use]
    pub fn minefield_colour(&self, field: &stars_core::minefield::Minefield) -> [u8; 3] {
        if field.detonating {
            return SCAN_OTHER;
        }
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return SCAN_OTHER;
        };
        let Ok(owner) = usize::try_from(field.owner) else {
            return SCAN_OTHER;
        };
        match stars_core::relations::party(game, me, owner) {
            stars_core::relations::Party::Yours => SCAN_YOURS,
            stars_core::relations::Party::Friends => SCAN_FRIEND,
            _ => SCAN_OTHER,
        }
    }

    /// Which of the three pattern brushes a field is filled with.
    ///
    /// One per kind — `rghbrPat[kind]`, resources 460, 461 and 462 — so the
    /// hatch says whether a field is standard, heavy or a speed bump.
    #[must_use]
    pub fn minefield_pattern(field: &stars_core::minefield::Minefield) -> u16 {
        460 + u16::from(field.kind.min(2))
    }

    /// Whether a minefield's centre gets a mark of its own.
    ///
    /// `DrawScanner` looks the centre up in `rgptPlan` and only marks it when
    /// **no planet is there** — a field centred on a planet would have its mark
    /// buried under the planet's dot anyway.
    #[must_use]
    pub fn minefield_centre_marked(&self, field: &stars_core::minefield::Minefield) -> bool {
        self.game.as_ref().is_none_or(|game| {
            !game
                .planets
                .iter()
                .chain(game.known_planets.iter())
                .any(|planet| planet.position == Some(field.position))
        })
    }

    /// Half the width of a mineral packet's mark, by zoom.
    ///
    /// `DrawScanner` picks 2, 3 or 5 from `iScanZoom` — the mark is drawn a
    /// pixel outside that either way, so the shape spans `2r + 3`.
    #[must_use]
    pub fn packet_mark_radius(&self) -> f32 {
        if self.scan_zoom < 1 {
            2.0
        } else if self.scan_zoom < 3 {
            3.0
        } else {
            5.0
        }
    }

    /// Whether a packet is drawn as a **diamond** rather than a square.
    ///
    /// `DrawScanner` switches on the packet's warp field — `(wFlags >> 10) &
    /// 0xf`, which is where the speed is stored — and draws a yellow diamond
    /// when it is zero, a square in the owner's colour otherwise. A packet
    /// with no speed is one that has arrived and is waiting to be caught.
    #[must_use]
    pub fn packet_is_diamond(packet: &stars_core::packet::Packet) -> bool {
        packet.warp == 0
    }

    /// The wormhole at the far end of `index`, when the line between the two
    /// should be drawn.
    ///
    /// `DrawScanner` joins a pair with a line, but only from the **lower** of
    /// the two ids, so it is drawn once, and only for a player who has been
    /// through it (`grbitPlr`, which this engine keeps as `traversed_by`).
    #[must_use]
    pub fn wormhole_line(&self, index: usize) -> Option<usize> {
        let game = self.game.as_ref()?;
        let hole = game.wormholes.get(index)?;
        let me = u32::try_from(self.local_player()).unwrap_or(0);
        if hole.traversed_by & (1u16 << (me & 0x0F)) == 0 {
            return None;
        }
        let partner = hole.partner & 0x01FF;
        if hole.id >= partner {
            return None;
        }
        game.wormholes.iter().position(|other| other.id == partner)
    }

    /// Which way the Mystery Trader is drawn pointing.
    ///
    /// It is a **fleet arrow** — `DrawScanner` selects `hbmpScanShip`, tints it
    /// yellow and orients it with `GetDxDyOrientation` on the way to its
    /// destination — so it uses the same eight-way sheet the fleets do.
    #[must_use]
    pub fn trader_arrow(&self, index: usize) -> Option<u8> {
        let trader = self.game.as_ref()?.traders.get(index)?;
        Some(fleet_arrow(
            trader.destination.x - trader.position.x,
            trader.destination.y - trader.position.y,
        ))
    }

    /// The space objects at a point, in the order the original's `lpThings`
    /// walk finds them: minefields, packets, then wormholes.
    ///
    /// Only what this player can see. A minefield is listed once it has been
    /// detected, and a packet or a wormhole when the player's own view carries
    /// it, which is what the loader wrote into `include`.
    #[must_use]
    pub fn things_at(&self, x: i16, y: i16) -> Vec<ScanThing> {
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let at = stars_core::movement::Point::new(x, y);
        let me = u32::try_from(self.local_player()).unwrap_or(0);
        let seen = 1u16 << (me & 0x0F);
        let mut out = Vec::new();
        for (index, field) in game.minefields.iter().enumerate() {
            if field.position == at
                && (field.detected_by & seen != 0 || field.owner == i16::try_from(me).unwrap_or(-1))
            {
                out.push(ScanThing::Minefield(index));
            }
        }
        for (index, packet) in game.packets.iter().enumerate() {
            if packet.position == at && packet.include {
                out.push(ScanThing::Packet(index));
            }
        }
        for (index, hole) in game.wormholes.iter().enumerate() {
            if hole.position == at && hole.include {
                out.push(ScanThing::Wormhole(index));
            }
        }
        for (index, trader) in game.traders.iter().enumerate() {
            if trader.position == at && trader.include {
                out.push(ScanThing::Trader(index));
            }
        }
        out
    }

    /// What a space object is called, which is what the menu, the pane's title
    /// and the status bar's name cell use.
    ///
    /// `PszGetThingName` (`1038:26de`) has one format per kind, all from the
    /// string table: a minefield is `"%s%s Mine Field"` (`idsSSMineField`)
    /// with the owner and the kind from [`MINEFIELD_KINDS`], a packet is
    /// `"%sMineral Packet"` (`idsSmineralPacket`) — or `Salvage`
    /// (`idsSalvage`, trailing space and all) when it is aimed at no planet —
    /// and a wormhole and the Mystery Trader are named outright.
    ///
    /// The owner prefix is `"%s "` (`DS:0x518`) and is left off **your own**
    /// objects, exactly as a fleet's name leaves it off.
    #[must_use]
    pub fn thing_name(&self, thing: ScanThing) -> String {
        let Some(game) = self.game.as_ref() else {
            return String::new();
        };
        let me = self.local_player();
        let prefix = |owner: i16| -> String {
            match usize::try_from(owner) {
                Ok(owner) if owner != me => match game.players.get(owner) {
                    Some(player) => format!("{} ", player.name),
                    None => format!("player {} ", owner + 1),
                },
                _ => String::new(),
            }
        };
        match thing {
            ScanThing::Minefield(index) => match game.minefields.get(index) {
                Some(field) => {
                    let kind = MINEFIELD_KINDS
                        .get(usize::from(field.kind))
                        .copied()
                        .unwrap_or("Standard");
                    format!("{}{kind} Mine Field", prefix(field.owner))
                }
                None => String::new(),
            },
            ScanThing::Packet(index) => match game.packets.get(index) {
                // A packet aimed at no planet is salvage, and salvage is not
                // named for whoever dropped it.
                Some(packet) if packet.target == 0 => "Salvage".to_string(),
                Some(packet) => format!("{}Mineral Packet", prefix(packet.owner)),
                None => String::new(),
            },
            ScanThing::Wormhole(_) => "Wormhole".to_string(),
            ScanThing::Trader(_) => "Mystery Trader".to_string(),
        }
    }

    /// The scanner's left click, at a galaxy point.
    ///
    /// A new spot selects what is on it. **The spot already selected advances
    /// to the next thing there**, which is what clicking the same place twice
    /// does in the original, and wraps round to the planet at the end. Nothing
    /// changes when the spot holds only the one thing.
    ///
    /// Returns whether the selection moved.
    pub fn scan_click(&mut self, x: i16, y: i16) -> bool {
        let here = self.objects_at(x, y);
        if here.is_empty() {
            return false;
        }
        let at = stars_core::movement::Point::new(x, y);
        let same_spot = self
            .selected_object()
            .and_then(|object| self.object_position(object))
            .is_some_and(|position| position == at);

        let next = if same_spot {
            let current = self.selected_object();
            let index = here.iter().position(|object| Some(*object) == current);
            match index {
                // Round to the next, and back to the planet at the end.
                Some(index) => here[(index + 1) % here.len()],
                // Selected here but not one of the things that cycle — take
                // the first, as the original takes the first fleet.
                None => here[0],
            }
        } else {
            here[0]
        };
        if Some(next) == self.selected_object() {
            return false;
        }
        self.select_object(next);
        true
    }
}

// --- Which way a fleet is pointing ----------------------------------------

/// The sheet the fleet arrows come out of (`hbmpScanShip`, id 88): eight
/// arrows stacked, at 9 pixels in the right-hand column and 7 in the left.
pub const ARROW_SHEET: u16 = 88;

/// The sheet the scanner writes **ship counts** out of — bitmap **249**,
/// loaded as `LoadBitmap(hInst, 0xf9)` when the game starts (`hbmpNumbers`).
///
/// It is 44x7 and one bit deep: eleven 4x7 cells, the ten digits at
/// `x = digit * 4` and a star in the eleventh that the counts never touch. The
/// digits are the **zero** bits, as in every other one-bit sheet, so they are
/// drawn as a stencil and tinted.
pub const DIGIT_SHEET: u16 = 249;
/// How wide one digit is.
pub const DIGIT_WIDTH: u32 = 4;
/// And how tall.
pub const DIGIT_HEIGHT: u32 = 7;

/// The π `GetDxDyOrientation` **adds** to the angle.
///
/// It is a less precise π than the one it then divides by — seven digits
/// against ten. Both are transcribed as the executable holds them rather than
/// folded into `std::f64::consts::PI`: they are two different numbers in the
/// original, and a transcription that tidied them into one would no longer be
/// a transcription.
#[allow(
    clippy::approx_constant,
    reason = "the executable's own seven-digit pi, not an approximation of ours"
)]
const ARROW_PI_ADDED: f64 = 3.1415927;
/// The π it **divides** by.
#[allow(
    clippy::approx_constant,
    reason = "the executable's own ten-digit pi, kept apart from the one above"
)]
const ARROW_PI_DIVISOR: f64 = 3.141592654;

/// Which of the eight arrows points along `(dx, dy)`.
///
/// `GetDxDyOrientation` (`1058:987c`) turns the angle into an octant:
/// `(atan2(dy, dx) + π) × 4 / π + 0.5`, truncated, and then `(9 - n & 7) & 7`
/// to put the sprites in the order the sheet has them. The result runs
/// anticlockwise from south-west:
///
/// | | | | | | | | |
/// |-|-|-|-|-|-|-|-|
/// | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
/// | SW | W | NW | N | NE | E | SE | S |
///
/// **A fleet going nowhere gets arrow 0 as well**, which is the same picture
/// as one heading south-west. The original sets the index to zero before it
/// looks at the angle at all and never distinguishes the two.
#[must_use]
pub fn fleet_arrow(dx: i16, dy: i16) -> u8 {
    if dx == 0 && dy == 0 {
        return 0;
    }
    let angle = f64::from(dy).atan2(f64::from(dx));
    let scaled = (angle + ARROW_PI_ADDED) * 4.0 / ARROW_PI_DIVISOR + 0.5;
    // `ftol` truncates toward zero, and the value here is never negative.
    let n = scaled as i64;
    u8::try_from((9 - (n & 7)) & 7).unwrap_or(0)
}

impl App {
    /// Which way a fleet's arrow points on the map.
    ///
    /// `GetScanFleetOrientation` (`1058:978c`) has two sources and picks by
    /// **whose fleet it is**. A player's own is read from its next waypoint,
    /// and only when the leg has a warp set; anybody else's comes from the
    /// direction stored with the sighting, which is only meaningful when the
    /// fleet was seen moving. Either way a fleet with no known course gets
    /// arrow 0.
    #[must_use]
    pub fn fleet_arrow_of(&self, fleet: &stars_core::fleet::Fleet) -> u8 {
        let me = self.local_player();
        let mine = usize::try_from(fleet.owner).is_ok_and(|owner| owner == me);
        let (dx, dy) = if mine {
            match fleet.waypoints.get(1).filter(|leg| leg.warp > 0) {
                Some(leg) => (
                    leg.position.x - fleet.position.x,
                    leg.position.y - fleet.position.y,
                ),
                None => (0, 0),
            }
        } else {
            fleet.direction.unwrap_or((0, 0))
        };
        fleet_arrow(dx, dy)
    }

    /// Where that arrow sits in the sheet, and how big it is.
    ///
    /// The right-hand column holds the 9-pixel arrows and the left the
    /// 7-pixel ones; the scanner uses the smaller once it is zoomed out past
    /// life size.
    #[must_use]
    pub fn fleet_arrow_cell(&self, arrow: u8) -> (u32, u32, u32) {
        let side: u32 = if self.scan_zoom < 0 { 7 } else { 9 };
        let x = if self.scan_zoom < 0 { 0 } else { 7 };
        (x, u32::from(arrow) * side, side)
    }
}

// --- Player colours -------------------------------------------------------

/// A ship count the scanner writes on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipCount {
    /// Where it goes.
    pub position: stars_core::movement::Point,
    /// How many, already capped at 999.
    pub ships: i32,
    /// The one player whose fleets these are, when they all belong to one.
    /// `None` when several players have fleets at the spot.
    pub owner: Option<usize>,
    /// How many pixels above the location's own point the number sits — the
    /// `y` `DrawScanner` hands `DrawScanFleetCount`, which differs by the mark
    /// the fleet under it was given.
    pub above: i16,
}

impl App {
    /// The ship counts to write on the map, one per **location**.
    ///
    /// `DrawScanFleetCount` (`1058:47d2`) is handed one fleet and walks the
    /// **circular list** `LinkFleets` (`1038:1bb4`) builds out of the fleets
    /// sharing a point, adding up what `CShipsScanVis` counts of each. So the
    /// number is per location, which is what the manual means by "the number
    /// of ships at a location". On its way round it sets `fDone` on every
    /// fleet in the ring, which is how the other fleets at the spot are kept
    /// from writing the same number again.
    ///
    /// Two limits come with it: the total is **capped at 999**, and a spot
    /// totalling nothing is not written at all. Each fleet contributes only
    /// what the ship filters allow, so filtering can leave a location with no
    /// number even though ships are there.
    #[must_use]
    pub fn ship_counts(&self) -> Vec<ShipCount> {
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let mut totals: std::collections::BTreeMap<(i16, i16), (i64, Option<usize>, bool)> =
            std::collections::BTreeMap::new();
        // Which fleet's mark the number is written above: the first one at the
        // point whose arm of the loop draws a count at all.
        let mut anchors: std::collections::BTreeMap<(i16, i16), i16> =
            std::collections::BTreeMap::new();
        for fleet in &game.fleets {
            let ships = i64::from(self.filtered_ship_count(fleet));
            if ships <= 0 {
                continue;
            }
            let owner = usize::try_from(fleet.owner).ok();
            let entry = totals
                .entry((fleet.position.x, fleet.position.y))
                .or_insert((0, owner, true));
            entry.0 += ships;
            if entry.1 != owner {
                // More than one player's fleets here.
                entry.2 = false;
            }
            if let Some(above) = self.ship_count_anchor(fleet) {
                anchors
                    .entry((fleet.position.x, fleet.position.y))
                    .or_insert(above);
            }
        }
        totals
            .into_iter()
            .filter_map(|((x, y), (ships, owner, one_owner))| {
                Some(ShipCount {
                    position: stars_core::movement::Point::new(x, y),
                    ships: i32::try_from(ships.min(999)).unwrap_or(999),
                    owner: if one_owner { owner } else { None },
                    above: *anchors.get(&(x, y))?,
                })
            })
            .collect()
    }

    /// How far above a fleet's own point the count for its location is
    /// written, or `None` when that fleet's arm of the loop writes none.
    ///
    /// `DrawScanner` calls `DrawScanFleetCount` from all three of its arms and
    /// passes each a different `y`: `pt.y - ptD.y/2 - 2` above a deep-space
    /// arrow, `pt.y - 7` above the glyph a fleet on the selected point gets,
    /// and `pt.y - 5 - 2` or `pt.y - 9 - 2` above an orbit ring, by which of
    /// the two sizes it is.
    ///
    /// The orbit call sits **inside the ring arm**, which is guarded by
    /// `uVar8 < 3`. So in Planet Value, Population and No Player Information a
    /// fleet in orbit writes no number at all — one in deep space at the same
    /// point still does.
    #[must_use]
    pub fn ship_count_anchor(&self, fleet: &stars_core::fleet::Fleet) -> Option<i16> {
        let selected = Some(fleet.position) == self.selected_point();
        if fleet.orbiting.is_some() {
            if !self.orbit_rings_visible() {
                return None;
            }
            return Some(if selected { 9 + 2 } else { 5 + 2 });
        }
        if selected {
            return Some(7);
        }
        let (_, _, side) = self.fleet_arrow_cell(self.fleet_arrow_of(fleet));
        i16::try_from(side / 2 + 2).ok()
    }

    /// Where each digit of a count goes, as an offset from the location's own
    /// x and the digit to draw there.
    ///
    /// `DrawScanFleetCount` lays them out by hand, five pixels apart, and the
    /// three cases do not share a left edge: one digit sits at `x - 1`, two at
    /// `x - 4` and `x + 1`, three at `x - 6`, `x - 1` and `x + 4`. The top of
    /// every digit is seven pixels above the `y` it was handed.
    #[must_use]
    pub fn ship_count_digits(ships: i32) -> Vec<(i16, u8)> {
        let mut left = ships.clamp(0, 999);
        if left < 1 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut x: i16 = -1;
        if left > 99 {
            out.push((-6, u8::try_from(left / 100).unwrap_or(0)));
            left %= 100;
            x = 2;
        }
        if left > 9 || !out.is_empty() {
            out.push((x - 3, u8::try_from(left / 10).unwrap_or(0)));
            x += 2;
            left %= 10;
        }
        out.push((x, u8::try_from(left).unwrap_or(0)));
        out
    }

    /// What colour to write a ship count in.
    ///
    /// White unless **Player Colors** is on, the fleets at that spot all
    /// belong to one player, and that player is not this one — the original
    /// keeps your own numbers white whatever the setting.
    #[must_use]
    pub fn ship_count_colour(&self, count: &ShipCount) -> Option<usize> {
        if !self.scan_overlays.player_colours {
            return None;
        }
        let owner = count.owner?;
        (owner != self.local_player()).then_some(owner)
    }

    /// What colour to write a planet's name in.
    ///
    /// `None` means the ordinary colour, which is **white**: `DrawScanner`
    /// sets `SetTextColor(hdc, 0xffffff)` before the loop (`1058:2d74`) and
    /// puts it back after every coloured name, and an **unowned** planet is
    /// never given a colour at all — the routine only reaches for one once it
    /// has established the planet has an owner. `Some(None)` is white too —
    /// this player's own, which the original writes as literal white rather
    /// than out of the colour table. `Some(Some(p))` is player `p`'s colour
    /// from `rgcrPlrHistory`.
    #[must_use]
    pub fn planet_name_colour(&self, owner: Option<i16>) -> Option<Option<usize>> {
        if !self.scan_overlays.player_colours {
            return None;
        }
        let owner = usize::try_from(owner?).ok()?;
        Some((owner != self.local_player()).then_some(owner))
    }

    /// Whether planet names are drawn at all at this zoom.
    ///
    /// The original hides them below `iScanZoom > -2` (`1058:2f63`), where the
    /// dots are too close together for a name to mean anything.
    #[must_use]
    pub fn planet_names_visible(&self) -> bool {
        self.scan_overlays.names && self.scan_zoom > -2
    }

    /// How a planet name is written at the zoom showing.
    ///
    /// `DrawScanner` picks the font out of a jump table on `iScanZoom`
    /// (`1058:2d63`), and the four it can land on are four different globals:
    /// `rghfontArial6[0]` at -1, `rghfontArial8[0]` at 0, 1 and 2,
    /// `rghfontArial8[1]` at 3 and `rghfontArial10[1]` at 4. `FCreateFonts`
    /// (`1000:0ab2`) builds each array from the string table at `idsArial2 + i`
    /// — the constant `0x0537` is there in our own binary — and never touches
    /// `lfWeight`, so the weight is in the **face name**: index 0 is `Arial`
    /// and index 1 `Arial Bold`. The two zoomed-in sizes are therefore bold.
    ///
    /// The size is in points, converted with `MulDiv(points, LOGPIXELSY, 72)`;
    /// at the 96 dpi the original ran on that is four thirds of a pixel per
    /// point, which is what [`PlanetNameStyle::pixels`] carries.
    #[must_use]
    pub fn planet_name_style(&self) -> PlanetNameStyle {
        let (points, bold) = match self.scan_zoom {
            i8::MIN..=-1 => (6.0, false),
            0..=2 => (8.0, false),
            3 => (8.0, true),
            _ => (10.0, true),
        };
        PlanetNameStyle {
            points,
            bold,
            below: self.planet_name_below(),
        }
    }

    /// How far below a planet's own point its name is written.
    ///
    /// `CtrTextOut(hdc, pt.x, pt.y + 5 + iVar21, …)` — centred on the planet
    /// and five pixels under it, `iVar21` being **11 more** when the zoom is 3
    /// or better *and* the view is Population (`1058:2db1`), which is the one
    /// view that draws something of its own under the planet for the name to
    /// clear.
    #[must_use]
    pub fn planet_name_below(&self) -> i16 {
        let population = self.scan_view == ScanView::Population && self.scan_zoom >= 3;
        if population {
            5 + 11
        } else {
            5
        }
    }
}

/// How a planet's name is written: which font, and how far under the planet.
///
/// See [`App::planet_name_style`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanetNameStyle {
    /// The point size the original asks Windows for.
    pub points: f32,
    /// Whether it asks for the **Arial Bold** face rather than Arial.
    pub bold: bool,
    /// Pixels below the planet's own point that the top of the name sits at.
    pub below: i16,
}

impl PlanetNameStyle {
    /// The size in pixels: `MulDiv(points, 96, 72)`, the conversion the
    /// original makes against the screen's `LOGPIXELSY`.
    #[must_use]
    pub fn pixels(&self) -> f32 {
        self.points * 4.0 / 3.0
    }
}

// --- The View menu --------------------------------------------------------

/// View (Window Layout): how much room the frame gives the scanner.
///
/// `iWindowLayout`, set straight from the menu id — `wParam - IDM_VIEW_LAYOUT_0`
/// — after which the original resizes its tiles and refits the frame's
/// children. The three are the same three the menu names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowLayout {
    /// The most room for the map.
    #[default]
    Large = 0,
    /// The middle one.
    Medium = 1,
    /// The least — the only one the original treats specially, passing
    /// `iWindowLayout == 2` to `EnsureTileSize`.
    Small = 2,
}

impl WindowLayout {
    /// All three, in the menu's order.
    pub const ALL: [WindowLayout; 3] = [
        WindowLayout::Large,
        WindowLayout::Medium,
        WindowLayout::Small,
    ];

    /// The name the menu gives it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            WindowLayout::Large => "Large Screen",
            WindowLayout::Medium => "Medium Screen",
            WindowLayout::Small => "Small Screen",
        }
    }
}

impl App {
    /// Whether the scanner's toolbar is showing.
    #[must_use]
    pub fn toolbar_visible(&self) -> bool {
        !self.toolbar_hidden
    }

    /// The game's settings, as the View (Game Parameters) window lists them.
    ///
    /// Everything here comes out of the `.xy`'s game block, which is the only
    /// place it is kept — a `.mN` or `.hst` alone knows none of it, and the
    /// list is then empty rather than invented.
    #[must_use]
    pub fn game_parameters_rows(&self) -> Vec<(String, String)> {
        use stars_core::newgame::{Density, Size, StartDistance};
        use stars_formats::game_flag;

        let Some(info) = self.universe.as_ref().and_then(|u| u.game().ok()) else {
            return Vec::new();
        };
        let class = |value: i16, names: &[&'static str]| -> String {
            usize::try_from(value)
                .ok()
                .and_then(|index| names.get(index))
                .map_or_else(|| format!("class {value}"), |name| (*name).to_string())
        };
        let sizes: Vec<&'static str> = Size::ALL.iter().map(|s| s.name()).collect();
        let densities: Vec<&'static str> = Density::ALL.iter().map(|d| d.name()).collect();
        let distances: Vec<&'static str> = StartDistance::ALL.iter().map(|d| d.name()).collect();

        // The year comes from the **save**, not the universe file: the `.xy`
        // is written once when the game is created and its turn counter stays
        // at zero for ever after. All four years of the `no-random-events`
        // fixture carry `turn = 0`, which is what settles it.
        let year = self.game.as_ref().map_or(2400, stars_core::GameState::year);
        let mut rows = vec![
            ("Name".to_string(), info.name.clone()),
            ("Year".to_string(), year.to_string()),
            ("Universe size".to_string(), class(info.size, &sizes)),
            ("Density".to_string(), class(info.density, &densities)),
            (
                "Player positions".to_string(),
                class(info.start_distance, &distances),
            ),
            ("Players".to_string(), info.players.to_string()),
            ("Planets".to_string(), info.planets.to_string()),
        ];

        // The options, in the Advanced Game dialog's own captions and its own
        // order — checkboxes `0x3f8`..`0x3fd` and `0x41a` on resource 390,
        // each read from the bit `NewGameDlg` reads it from. Only the ones
        // that are on are listed, which is how the original's page reads.
        //
        // Two of the nine flags have no checkbox here: `SINGLE_PLAYER` and
        // `TUTORIAL` belong to the New Game dialog in front of this one.
        let options = [
            (game_flag::EXTRA_FUEL, "Beginner: Maximum Minerals"),
            (game_flag::SLOW_TECH, "Slower Tech Advances"),
            (game_flag::BBS_PLAY, "Accelerated BBS Play"),
            (game_flag::NO_RANDOM, "No Random Events"),
            (game_flag::AIS_BAND, "Computer Players Form Alliances"),
            (game_flag::VIS_SCORES, "Public Player Scores"),
            (game_flag::CLUMPING, "Galaxy Clumping"),
            (game_flag::SINGLE_PLAYER, "One human player"),
            (game_flag::TUTORIAL, "Tutorial"),
        ];
        let on: Vec<&str> = options
            .iter()
            .filter(|(bit, _)| info.flags & bit != 0)
            .map(|(_, name)| *name)
            .collect();
        rows.push((
            "Options".to_string(),
            if on.is_empty() {
                "none".to_string()
            } else {
                on.join(", ")
            },
        ));
        rows
    }
}

impl App {
    /// The victory conditions the Game Parameters window lists, which are the
    /// Score sheet's own.
    #[must_use]
    pub fn game_parameters_conditions(&self) -> Vec<stars_core::scoresheet::Condition> {
        self.score_conditions()
    }
}

// --- The race viewer ------------------------------------------------------

/// One page of the race viewer, mirroring one page of the race wizard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RacePage {
    /// What the page is about.
    pub title: &'static str,
    /// Its rows, as label and value.
    pub rows: Vec<(String, String)>,
}

impl App {
    /// A player's race, laid out as the race wizard's six pages.
    ///
    /// View (Race) opens the wizard itself on the player's own race, read-only
    /// (`IDM_RACE_EDIT1`, F8). This shows the same six pages' worth of settings
    /// without the wizard: dialogs 146 to 151 are mostly **owner-drawn** — the
    /// habitability sliders and the economy bars are painted rather than laid
    /// out as controls — so what is reproduced here is what each page *says*,
    /// not how it looks.
    ///
    /// Returns nothing for a player that is not there.
    #[must_use]
    pub fn race_pages(&self, player: usize) -> Vec<RacePage> {
        use stars_core::race::{lrt, RaceStat};
        use stars_core::research::TechField;

        let Some(record) = self.game.as_ref().and_then(|game| game.players.get(player)) else {
            return Vec::new();
        };
        let race = &record.race;
        let yes_no = |on: bool| if on { "yes" } else { "no" }.to_string();

        // 1 — who they are.
        let mut pages = vec![RacePage {
            title: "Race",
            rows: vec![
                ("Race name".to_string(), record.name.clone()),
                (
                    "Plural race name".to_string(),
                    if record.plural_name.is_empty() {
                        "—".to_string()
                    } else {
                        record.plural_name.clone()
                    },
                ),
                (
                    "Password".to_string(),
                    if record.password == 0 {
                        "none".to_string()
                    } else {
                        "set".to_string()
                    },
                ),
            ],
        }];

        // 2 — where they can live. A negative upper bound means immune, which
        // is why the row is one thing or the other rather than a range and a
        // flag.
        let mut habitability = Vec::new();
        for (index, name) in ["Gravity", "Temperature", "Radiation"].iter().enumerate() {
            let value = if race.is_immune(index) {
                "immune".to_string()
            } else {
                format!(
                    "{} to {}, ideal {}",
                    env_text(index, race.env_min[index]),
                    env_text(index, race.env_max[index]),
                    env_text(index, race.env_center[index])
                )
            };
            habitability.push(((*name).to_string(), value));
        }
        habitability.push((
            "Maximum growth rate".to_string(),
            format!("{}%", race.pct_ideal_growth),
        ));
        pages.push(RacePage {
            title: "Habitability",
            rows: habitability,
        });

        // 3 — what they build things with.
        pages.push(RacePage {
            title: "Economy",
            rows: vec![
                (
                    "Colonists per resource".to_string(),
                    format!("{}00", race.stat(RaceStat::ResGen)),
                ),
                (
                    "Resources per 10 factories".to_string(),
                    race.stat(RaceStat::FactProd).to_string(),
                ),
                (
                    "Cost of a factory".to_string(),
                    format!("{} resources", race.stat(RaceStat::FactBuild)),
                ),
                (
                    "Factories per 10,000 colonists".to_string(),
                    race.stat(RaceStat::FactOperate).to_string(),
                ),
                (
                    "Minerals per 10 mines".to_string(),
                    race.stat(RaceStat::MineProd).to_string(),
                ),
                (
                    "Cost of a mine".to_string(),
                    format!("{} resources", race.stat(RaceStat::MineBuild)),
                ),
                (
                    "Mines per 10,000 colonists".to_string(),
                    race.stat(RaceStat::MineOperate).to_string(),
                ),
                (
                    "Factories cost 1kT less germanium".to_string(),
                    yes_no(race.has_lrt(lrt::CHEAP_FACT)),
                ),
            ],
        });

        // 4 — the one primary trait.
        pages.push(RacePage {
            title: "Primary Racial Trait",
            rows: vec![(
                "Trait".to_string(),
                race.prt().map_or_else(
                    || format!("unknown ({})", race.stat(RaceStat::MajorAdv)),
                    |prt| prt.name().to_string(),
                ),
            )],
        });

        // 5 — the fourteen lesser ones, all listed so that what is *not* taken
        // is as plain as what is.
        pages.push(RacePage {
            title: "Lesser Racial Traits",
            rows: lrt::ALL
                .iter()
                .map(|bit| {
                    (
                        lrt::name(*bit).unwrap_or("?").to_string(),
                        yes_no(race.has_lrt(*bit)),
                    )
                })
                .collect(),
        });

        // 6 — what research costs, field by field. The stored value is a
        // three-way setting and the wizard spells out all three.
        let mut research: Vec<(String, String)> = TechField::ALL
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let setting = race.attrs[RaceStat::TechBonus1 as usize + index];
                let text = match setting {
                    0 => "costs 75% extra",
                    1 => "costs the standard amount",
                    2 => "costs 50% less",
                    _ => "unknown",
                };
                (field.name().to_string(), text.to_string())
            })
            .collect();
        research.push((
            "All techs start at 3".to_string(),
            yes_no(race.has_lrt(lrt::TECH3)),
        ));
        pages.push(RacePage {
            title: "Research Costs",
            rows: research,
        });

        pages
    }
}

impl App {
    /// Open the race viewer on a player's race (View (Race), F8).
    pub fn open_race_viewer(&mut self, player: usize) {
        if self
            .game
            .as_ref()
            .is_some_and(|game| player < game.players.len())
        {
            self.race_viewer = Some((player, 0));
        }
    }

    /// Close it.
    pub fn close_race_viewer(&mut self) {
        self.race_viewer = None;
    }

    /// Turn a page, the way the wizard's Back and Next do — but stopping at
    /// the ends rather than wrapping, because a wizard's Back and Next stop.
    pub fn race_viewer_page(&mut self, forward: bool) {
        let Some((player, page)) = self.race_viewer else {
            return;
        };
        let last = self.race_pages(player).len().saturating_sub(1);
        let page = if forward {
            (page + 1).min(last)
        } else {
            page.saturating_sub(1)
        };
        self.race_viewer = Some((player, page));
    }
}

// --- The Custom Race Wizard -----------------------------------------------

/// A race being designed.
///
/// `RaceCreationWizard` (`10e0:0000`), File (Custom Race Wizard). The same six
/// pages the viewer shows, with the settings editable and the **advantage
/// points** counted as they change — which is what the wizard is really for:
/// every choice spends or refunds from one budget, and a race is only legal
/// when what is left is not negative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaceWizard {
    /// The race being built.
    pub race: stars_core::Race,
    /// Its singular name.
    pub name: String,
    /// Its plural name.
    pub plural: String,
    /// Which race emblem it wears (`PLAYER.iPlrBmp`, 0..=31).
    pub emblem: u8,
    /// The turn password, as typed. Stored as a salt, never as itself — see
    /// [`stars_formats::password`] — so this is only ever what was typed into
    /// this wizard, and an existing race's password cannot be shown back.
    pub password: String,
    /// Which of the six pages is showing.
    pub page: usize,
}

impl App {
    /// Open the wizard on a fresh Humanoid, which is where the original starts.
    pub fn open_race_wizard(&mut self) {
        self.race_wizard = Some(RaceWizard {
            race: stars_core::Race::humanoid(),
            name: String::new(),
            plural: String::new(),
            emblem: 0,
            password: String::new(),
            page: 0,
        });
        self.race_wizard_load_preset(0);
    }

    /// Open it on a copy of a player's race.
    pub fn open_race_wizard_from(&mut self, player: usize) {
        let Some(record) = self.game.as_ref().and_then(|game| game.players.get(player)) else {
            return;
        };
        self.race_wizard = Some(RaceWizard {
            race: record.race.clone(),
            name: record.name.clone(),
            plural: record.plural_name.clone(),
            emblem: record.logo,
            password: String::new(),
            page: 0,
        });
    }

    /// Load one of the seven predefined races, as the buttons on page 1 do.
    ///
    /// The race and its emblem are replaced; the **names only if the race is
    /// still called after one of the seven**. `RaceWizardDlg1` compares the
    /// name box against the seven strings before refilling it, so a name the
    /// player typed survives pressing the buttons.
    pub fn race_wizard_load_preset(&mut self, index: usize) {
        let Some(preset) = stars_core::presets::preset(index) else {
            return;
        };
        let Some(wizard) = self.race_wizard.as_mut() else {
            return;
        };
        let stock = wizard.name.is_empty()
            || stars_core::presets::ALL
                .iter()
                .any(|other| other.name == wizard.name);
        wizard.race = preset.race.clone();
        wizard.emblem = preset.emblem;
        if stock {
            wizard.name = preset.name.to_string();
            wizard.plural = preset.plural.to_string();
        }
    }

    /// Which of the eight buttons on page 1 is checked.
    ///
    /// They are radio buttons, not commands: the original works out which one
    /// by comparing the race being designed against each entry of `vrgplrDef`
    /// (`__fmemcmp` over the first `0x80` bytes of the player struct, which is
    /// everything but the two names), and checks `Custom` when none matches.
    #[must_use]
    pub fn race_wizard_selected_preset(&self) -> Option<usize> {
        let wizard = self.race_wizard.as_ref()?;
        stars_core::presets::ALL
            .iter()
            .position(|preset| preset.race == wizard.race && preset.emblem == wizard.emblem)
    }

    /// Whether the wizard can move off page 1.
    ///
    /// It cannot while `Random` is chosen — the original disables `Next >` for
    /// that one button — because a random race is not one there is anything to
    /// edit.
    #[must_use]
    pub fn race_wizard_can_go_on(&self) -> bool {
        let random = stars_core::presets::ALL.len() - 1;
        self.race_wizard_selected_preset() != Some(random)
    }

    /// The window caption, which the original numbers: string `0x010e`,
    /// `"Custom Race Wizard - Step %d of 6"`.
    #[must_use]
    pub fn race_wizard_title(&self) -> String {
        let step = self.race_wizard.as_ref().map_or(1, |w| w.page + 1);
        format!("Custom Race Wizard - Step {step} of {RACE_WIZARD_PAGES}")
    }

    /// Choose the race emblem.
    pub fn race_wizard_set_emblem(&mut self, emblem: u8) {
        if let Some(wizard) = self.race_wizard.as_mut() {
            wizard.emblem = emblem & 0x1F;
        }
    }

    /// Choose what the leftover advantage points are spent on: `0` surface
    /// minerals, `1` concentrations, `2` mines, `3` factories, `4` defences.
    pub fn race_wizard_set_leftover(&mut self, choice: i16) {
        if let Some(wizard) = self.race_wizard.as_mut() {
            wizard.race.attrs[stars_core::race::RaceStat::UseLeftover as usize] =
                choice.clamp(0, 4);
        }
    }

    /// The race as a `.rN` file, ready to write.
    ///
    /// `None` while the wizard is closed; an error only if a name is too long
    /// for the length byte that frames it.
    pub fn race_wizard_file(&self) -> Option<stars_formats::Result<Vec<u8>>> {
        let wizard = self.race_wizard.as_ref()?;
        Some(stars_core::save::race_file(
            &wizard.race,
            &wizard.name,
            &wizard.plural,
            wizard.emblem,
        ))
    }

    /// Why the wizard will not save this race, or `None` when it is legal.
    ///
    /// The original refuses the same thing and says so in a message box
    /// (string `0x0515`); the wording here is this project's own.
    #[must_use]
    pub fn race_wizard_refusal(&self) -> Option<String> {
        let points = self.race_wizard_points();
        (points < 0).then(|| {
            format!(
                "This race is {} advantage points over budget. A race can only be \
                 saved once the balance is back to nothing or better.",
                -points
            )
        })
    }

    /// Close it, throwing the race away.
    pub fn close_race_wizard(&mut self) {
        self.race_wizard = None;
    }

    /// Turn a page. Like the viewer's, these stop at the ends.
    pub fn race_wizard_page(&mut self, forward: bool) {
        let Some(wizard) = self.race_wizard.as_mut() else {
            return;
        };
        wizard.page = if forward {
            (wizard.page + 1).min(RACE_WIZARD_PAGES - 1)
        } else {
            wizard.page.saturating_sub(1)
        };
    }

    /// What the race being designed has left to spend.
    ///
    /// Negative means it costs more than the budget allows, and the original
    /// draws the figure in red when it is — see
    /// [`stars_core::advantage_points`].
    #[must_use]
    pub fn race_wizard_points(&self) -> i16 {
        self.race_wizard
            .as_ref()
            .map_or(0, |wizard| stars_core::advantage_points(&wizard.race))
    }

    /// Whether the race as it stands could be played.
    #[must_use]
    pub fn race_wizard_is_legal(&self) -> bool {
        self.race_wizard_points() >= 0
    }

    /// Choose the primary racial trait.
    pub fn race_wizard_set_prt(&mut self, prt: stars_core::race::Prt) {
        if let Some(wizard) = self.race_wizard.as_mut() {
            wizard.race.attrs[stars_core::race::RaceStat::MajorAdv as usize] = prt as i16;
        }
    }

    /// Turn one lesser racial trait on or off.
    pub fn race_wizard_toggle_lrt(&mut self, bit: u32) {
        if let Some(wizard) = self.race_wizard.as_mut() {
            wizard.race.lrt_bits ^= 1 << bit;
        }
    }

    /// Set one environment axis, or make the race immune to it.
    ///
    /// Immunity is stored as a **negative upper bound**, which is how the
    /// simulation recognises it, so switching it on and off has to put a real
    /// range back rather than leave the axis half-set.
    pub fn race_wizard_set_immune(&mut self, axis: usize, immune: bool) {
        let Some(wizard) = self.race_wizard.as_mut() else {
            return;
        };
        if axis > 2 {
            return;
        }
        if immune {
            wizard.race.env_max[axis] = -1;
        } else if wizard.race.env_max[axis] < 0 {
            // Back to the middle third, which is where a fresh axis sits.
            wizard.race.env_center[axis] = 50;
            wizard.race.env_min[axis] = 35;
            wizard.race.env_max[axis] = 65;
        }
    }

    /// Move one bound of an environment axis, in clicks.
    ///
    /// The three are kept in order — low, centre, high — because a race whose
    /// bounds crossed would have a habitable range of nothing.
    pub fn race_wizard_set_env(&mut self, axis: usize, which: usize, clicks: i8) {
        let Some(wizard) = self.race_wizard.as_mut() else {
            return;
        };
        if axis > 2 || wizard.race.env_max[axis] < 0 {
            return;
        }
        let clicks = clicks.clamp(0, 100);
        match which {
            0 => wizard.race.env_min[axis] = clicks.min(wizard.race.env_center[axis]),
            1 => {
                wizard.race.env_center[axis] =
                    clicks.clamp(wizard.race.env_min[axis], wizard.race.env_max[axis]);
            }
            _ => wizard.race.env_max[axis] = clicks.max(wizard.race.env_center[axis]),
        }
    }

    /// Set the maximum growth rate, which the points model clamps to `1..=20`.
    pub fn race_wizard_set_growth(&mut self, percent: i8) {
        if let Some(wizard) = self.race_wizard.as_mut() {
            wizard.race.pct_ideal_growth = percent.clamp(1, 20);
        }
    }

    /// Set one economy figure.
    ///
    /// The original's sliders have bounds this project has not recovered, so
    /// the only limits here are the ones the points model itself imposes — it
    /// stops counting colonists-per-resource above 25 — and what the file can
    /// store. What really constrains a race is the budget, which the counter
    /// shows.
    pub fn race_wizard_set_stat(&mut self, stat: stars_core::race::RaceStat, value: i16) {
        if let Some(wizard) = self.race_wizard.as_mut() {
            wizard.race.attrs[stat as usize] = value.clamp(1, 100);
        }
    }

    /// Set what one research field costs: `0` extra, `1` standard, `2` less.
    pub fn race_wizard_set_research(&mut self, field: usize, setting: i16) {
        if let Some(wizard) = self.race_wizard.as_mut() {
            if field < 6 {
                wizard.race.attrs[stars_core::race::RaceStat::TechBonus1 as usize + field] =
                    setting.clamp(0, 2);
            }
        }
    }

    /// Turn the two checkbox traits on or off — they live far up the trait
    /// word and belong to other pages than the fourteen.
    pub fn race_wizard_set_flag(&mut self, bit: u32, on: bool) {
        if let Some(wizard) = self.race_wizard.as_mut() {
            if on {
                wizard.race.lrt_bits |= 1 << bit;
            } else {
                wizard.race.lrt_bits &= !(1 << bit);
            }
        }
    }
}

/// How many pages the wizard has.
pub const RACE_WIZARD_PAGES: usize = 6;

/// The Battle Plans dialog (`BattlePlansDlg`, `IDD_BATTLE_PLANS`).
///
/// Commands (Battle Plans...), **F6**. A player's plans are a short list — at
/// most sixteen — and this edits one of them at a time: its tactic, its two
/// target classes, who it will attack and whether it dumps cargo. Every change
/// is logged as it is made, which is what `LogChangeBtlplan` does when the
/// original leaves a plan.
///
/// See `docs/ui/battle-plans.md`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BattlePlans {
    /// Which plan is showing.
    pub selected: usize,
    /// The rename box, while it is up, holding what has been typed. The
    /// original puts a modal dialog (`IDD_RENAME`) here, and both `Rename...`
    /// and `Copy` go through it.
    pub rename: Option<String>,
    /// Set while the delete warning is up: the original asks before deleting a
    /// plan that fleets are using (string `0x035b`).
    pub confirm_delete: bool,
}

/// The name a copied plan gets.
///
/// `BattlePlansDlg` bumps a trailing `" (n)"` — `'9'` wraps to `'0'` — and
/// appends `" (2)"` when there is none. It does neither to a name of 28
/// characters or more, which is the length it checks before touching it.
#[must_use]
pub fn copied_plan_name(name: &str) -> String {
    const LIMIT: usize = 28;
    if name.len() >= LIMIT {
        return name.to_string();
    }
    let bytes = name.as_bytes();
    if bytes.len() >= 3 {
        let tail = &bytes[bytes.len() - 3..];
        if tail[0] == b'(' && tail[2] == b')' && tail[1].is_ascii_digit() {
            let mut out = name.to_string();
            let digit = if tail[1] == b'9' {
                '0'
            } else {
                (tail[1] + 1) as char
            };
            out.replace_range(name.len() - 2..name.len() - 1, &digit.to_string());
            return out;
        }
    }
    format!("{name} (2)")
}

/// The most plans the dialog will make.
///
/// `Copy` refuses once the player has fifteen, though the host accepts a
/// sixteenth from a log record.
pub const MAX_BATTLE_PLANS: usize = 15;

impl App {
    /// Open the dialog.
    ///
    /// It opens on the selected fleet's plan when a fleet is selected, which is
    /// what `WM_INITDIALOG` does with `sel.fl.iplan`; otherwise on the first.
    pub fn open_battle_plans(&mut self) {
        let selected = self
            .selection
            .fleet
            .and_then(|index| Some(self.game.as_ref()?.fleets.get(index)?.battle_plan))
            .map(usize::from)
            .filter(|slot| *slot < self.battle_plan_count())
            .unwrap_or(0);
        self.battle_plans = Some(BattlePlans {
            selected,
            rename: None,
            confirm_delete: false,
        });
    }

    /// Close it.
    pub fn close_battle_plans(&mut self) {
        self.battle_plans = None;
    }

    /// How many plans the local player has.
    #[must_use]
    pub fn battle_plan_count(&self) -> usize {
        self.game.as_ref().map_or(0, |game| {
            game.players
                .get(self.local_player())
                .map_or(0, |player| player.battle_plans.len())
        })
    }

    /// The local player's plans.
    #[must_use]
    pub fn battle_plan_list(&self) -> &[stars_formats::BattlePlanRecord] {
        self.game
            .as_ref()
            .and_then(|game| game.players.get(self.local_player()))
            .map_or(&[], |player| player.battle_plans.as_slice())
    }

    /// The plan the dialog is showing.
    #[must_use]
    pub fn selected_battle_plan(&self) -> Option<&stars_formats::BattlePlanRecord> {
        let slot = self.battle_plans.as_ref()?.selected;
        self.battle_plan_list().get(slot)
    }

    /// Show another plan.
    pub fn select_battle_plan(&mut self, slot: usize) {
        let count = self.battle_plan_count();
        if let Some(dialog) = self.battle_plans.as_mut() {
            if slot < count {
                dialog.selected = slot;
                dialog.rename = None;
                dialog.confirm_delete = false;
            }
        }
    }

    /// Edit the plan on show, through `edit`, and log the result.
    ///
    /// Returns whether the change was accepted; the bounds are the host's.
    fn edit_battle_plan(
        &mut self,
        edit: impl FnOnce(&mut stars_formats::BattlePlanRecord),
    ) -> bool {
        let Some(slot) = self.battle_plans.as_ref().map(|d| d.selected) else {
            return false;
        };
        let Some(mut plan) = self.battle_plan_list().get(slot).cloned() else {
            return false;
        };
        edit(&mut plan);
        self.set_battle_plan_definition(slot, &plan)
    }

    /// Choose the tactic, leaving the flags in the same byte alone.
    pub fn set_battle_plan_tactic(&mut self, tactic: stars_core::battle::Tactic) -> bool {
        self.edit_battle_plan(|plan| plan.set_tactic(tactic as u8))
    }

    /// Choose the primary or secondary target class.
    pub fn set_battle_plan_target(
        &mut self,
        primary: bool,
        class: stars_core::battle::TargetClass,
    ) -> bool {
        self.edit_battle_plan(|plan| {
            if primary {
                plan.primary_target = class as u8;
            } else {
                plan.secondary_target = class as u8;
            }
        })
    }

    /// Choose who the plan attacks, as the stored byte.
    pub fn set_battle_plan_attack_who(&mut self, who: u8) -> bool {
        self.edit_battle_plan(|plan| plan.attack_who = who)
    }

    /// Turn *dump cargo* on or off.
    pub fn set_battle_plan_dump_cargo(&mut self, on: bool) -> bool {
        self.edit_battle_plan(|plan| plan.set_dump_cargo(on))
    }

    /// Rename the plan on show.
    ///
    /// Plan 0 cannot be renamed — the original disables the button for it, and
    /// the fleet panes name that plan in their own right.
    pub fn rename_battle_plan(&mut self, name: &str) -> bool {
        if self.battle_plans.as_ref().is_none_or(|d| d.selected == 0) {
            return false;
        }
        let name = name.to_string();
        self.edit_battle_plan(|plan| plan.name = name)
    }

    /// Copy the plan on show to a new slot at the end of the list.
    ///
    /// Refuses once the player has [`MAX_BATTLE_PLANS`]. The copy is selected
    /// and its rename box opened, which is what the original does — `Copy`
    /// falls straight through into the rename dialog.
    pub fn copy_battle_plan(&mut self) -> bool {
        let count = self.battle_plan_count();
        let Some(dialog) = self.battle_plans.as_ref() else {
            return false;
        };
        if count >= MAX_BATTLE_PLANS {
            return false;
        }
        let Some(mut plan) = self.battle_plan_list().get(dialog.selected).cloned() else {
            return false;
        };
        plan.name = copied_plan_name(&plan.name);
        if !self.set_battle_plan_definition(count, &plan) {
            return false;
        }
        if let Some(dialog) = self.battle_plans.as_mut() {
            dialog.selected = count;
            dialog.rename = Some(plan.name.clone());
        }
        true
    }

    /// How many of the local player's fleets use a plan.
    ///
    /// The original warns before deleting one that is in use, because the
    /// fleets using it are moved to the plan before it.
    #[must_use]
    pub fn fleets_using_battle_plan(&self, slot: usize) -> usize {
        let owner = i16::try_from(self.local_player()).unwrap_or(-1);
        self.game.as_ref().map_or(0, |game| {
            game.fleets
                .iter()
                .filter(|f| f.owner == owner && usize::from(f.battle_plan) == slot)
                .count()
        })
    }

    /// Delete the plan on show and select the one before it.
    ///
    /// Plan 0 cannot be deleted, as the original's disabled button says.
    pub fn delete_selected_battle_plan(&mut self) -> bool {
        let Some(slot) = self.battle_plans.as_ref().map(|d| d.selected) else {
            return false;
        };
        if slot == 0 || !self.delete_battle_plan(slot) {
            return false;
        }
        if let Some(dialog) = self.battle_plans.as_mut() {
            dialog.selected = slot - 1;
            dialog.confirm_delete = false;
            dialog.rename = None;
        }
        true
    }

    /// What the *Attack Who* combo offers, as `(stored value, caption)`.
    ///
    /// Four fixed choices (strings `0x78`..`0x7b`) and then **every other**
    /// player by name, stored as `4 + player`. A single-player game offers only
    /// `Everyone`, which is what the original leaves in the combo before
    /// disabling it.
    #[must_use]
    pub fn battle_plan_attack_options(&self) -> Vec<(u8, String)> {
        const FIXED: [&str; 4] = ["Nobody", "Enemies", "Neutrals & Enemies", "Everyone"];
        let single = self
            .game
            .as_ref()
            .is_none_or(|game| game.single_player || game.players.len() < 2);
        if single {
            return vec![(3, "Everyone".to_string())];
        }
        let me = self.local_player();
        let mut out: Vec<(u8, String)> = FIXED
            .iter()
            .enumerate()
            .map(|(value, name)| (u8::try_from(value).unwrap_or(0), (*name).to_string()))
            .collect();
        let Some(game) = self.game.as_ref() else {
            return out;
        };
        for player in 0..game.players.len() {
            if player == me {
                continue;
            }
            out.push((
                u8::try_from(player + 4).unwrap_or(u8::MAX),
                self.player_name(player),
            ));
        }
        out
    }
}

/// The Change Password dialog (`NewPasswordDlg`, `IDD_NEW_PASSWORD`).
///
/// Commands (Change Password...), the last item of that menu. Two boxes — the
/// password and the same password again — and nothing else: what is kept is a
/// salt of what was typed, so there is nothing to show back and no old
/// password to ask for.
///
/// See `docs/ui/change-password.md`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PasswordDialog {
    /// `New Password:`.
    pub new: String,
    /// `Retype Password:`.
    pub retype: String,
    /// What went wrong with the last attempt, if anything.
    pub error: Option<String>,
    /// Whether this is the **host's** password rather than a player's, which
    /// the original decides by `idPlayer == -1`. It changes the caption, the
    /// note and where the salt goes.
    pub host: bool,
}

impl App {
    /// Open the dialog, empty, for the local player's password.
    pub fn open_password_dialog(&mut self) {
        self.password_dialog = Some(PasswordDialog::default());
    }

    /// Open it for the **host's** password, which is what the Host Mode
    /// dialog's `Password...` button does.
    pub fn open_host_password_dialog(&mut self) {
        self.password_dialog = Some(PasswordDialog {
            host: true,
            ..PasswordDialog::default()
        });
    }

    /// The dialog's caption: the original renames it in host mode (string
    /// `0x035e`).
    #[must_use]
    pub fn password_title(&self) -> &'static str {
        if self.password_dialog.as_ref().is_some_and(|d| d.host) {
            "Change Host Password"
        } else {
            "Change Password"
        }
    }

    /// Whether the game has a host password.
    #[must_use]
    pub fn has_host_password(&self) -> bool {
        self.game
            .as_ref()
            .is_some_and(|game| game.host_password != 0)
    }

    /// Set or clear the **host's** password.
    ///
    /// It goes in the host file rather than a player block — a type-36 record
    /// after the player blocks — and takes effect at once, because the host
    /// writes its own file rather than submitting a turn.
    ///
    /// Returns whether it changed.
    pub fn set_host_password(&mut self, text: &str) -> bool {
        let salt = stars_formats::password_salt(text);
        let Some(game) = self.game.as_mut() else {
            return false;
        };
        if game.host_password == salt {
            return false;
        }
        game.host_password = salt;
        self.host_password_edited = true;
        self.dirty = true;
        self.password_given = (salt != 0).then_some(salt);
        true
    }

    /// Close it, keeping nothing.
    pub fn close_password_dialog(&mut self) {
        self.password_dialog = None;
    }

    /// Whether the local player has a turn password.
    #[must_use]
    pub fn has_password(&self) -> bool {
        let me = self.local_player();
        self.game
            .as_ref()
            .and_then(|game| game.players.get(me))
            .is_some_and(|player| player.password != 0)
    }

    /// The note the dialog carries under its two boxes.
    ///
    /// The original picks between two (strings `0x035c` and `0x035d`) by
    /// whether it is in host mode: a host's password takes effect at once
    /// because the host writes its own file there and then, while a player's
    /// travels in the turn they submit and so only binds from the next turn.
    /// The wording is this project's own, as the game's message text always is.
    #[must_use]
    pub fn password_note(&self) -> &'static str {
        if self.password_dialog.as_ref().is_some_and(|d| d.host) {
            "The new password takes effect as soon as the game is saved."
        } else {
            "The new password takes effect with the next turn, not this one."
        }
    }

    /// Accept what is typed: check the two boxes against each other and set
    /// the password.
    ///
    /// Returns whether the dialog should close. The two are compared **by
    /// salt** rather than by text, which is what `NewPasswordDlg` does — two
    /// different strings that fold to the same salt are the same password as
    /// far as the game is ever concerned. An empty pair clears the password.
    pub fn submit_password(&mut self) -> bool {
        let Some(dialog) = self.password_dialog.as_ref() else {
            return false;
        };
        let (new, retype, host) = (dialog.new.clone(), dialog.retype.clone(), dialog.host);
        if stars_formats::password_salt(&new) != stars_formats::password_salt(&retype) {
            if let Some(dialog) = self.password_dialog.as_mut() {
                dialog.error =
                    Some("The two boxes do not hold the same password. Type it again.".to_string());
                dialog.new.clear();
                dialog.retype.clear();
            }
            return false;
        }
        if host {
            self.set_host_password(&new);
        } else {
            self.set_password(&new);
        }
        self.password_dialog = None;
        true
    }
}

/// A save read off disk but not yet installed.
///
/// It exists for one reason: a file whose player has a password is read before
/// the password is asked for, and has to wait somewhere until it is given.
pub struct Loaded {
    state: GameState,
    universe: Option<Universe>,
    battles: Vec<BattleRecord>,
    file: StarsFile,
    path: std::path::PathBuf,
    /// The salt of the password guarding it; `0` for none.
    password: u32,
}

/// The prompt that asks for a turn password (`PasswordDlg`, `IDD_PASSWORD`).
///
/// See `docs/ui/change-password.md`.
pub struct PasswordPrompt {
    /// The salt the typed password has to fold to.
    pub salt: u32,
    /// What has been typed.
    pub typed: String,
    /// Whether the last attempt was wrong.
    pub error: Option<String>,
    /// The save waiting on it.
    pending: Box<Loaded>,
}

impl App {
    /// Whether a file guarded by `salt` needs the prompt.
    ///
    /// `FCheckPassword` (`1040:58d8`) in order: no password at all, the same
    /// password as the last one accepted, a computer player, or a matching
    /// `DefaultPassword` in `stars.ini` — any of those and nothing is asked.
    #[must_use]
    pub fn password_needed(&self, salt: u32) -> bool {
        if salt == 0 || self.password_given == Some(salt) {
            return false;
        }
        if !self.default_password.is_empty()
            && stars_formats::password_salt(&self.default_password) == salt
        {
            return false;
        }
        true
    }

    /// How long the prompt makes the player wait before trying again, in
    /// milliseconds.
    ///
    /// `PasswordDlg` calls `Delay` with one of three constants, chosen by how
    /// many passwords have been got wrong this session: a second under ten,
    /// five seconds under a hundred, ten seconds after that. It is the only
    /// thing standing between the salt and a dictionary, and it is reproduced
    /// here as a wait before the next attempt is accepted rather than as a
    /// frozen window.
    #[must_use]
    pub fn password_retry_delay_ms(&self) -> u64 {
        match self.password_failures {
            0..=9 => 1_000,
            10..=99 => 5_000,
            _ => 10_000,
        }
    }

    /// Answer the prompt. Returns whether the password was right.
    ///
    /// Right, and the save it was holding is opened; wrong, and the prompt
    /// stays with the box cleared, the failure counted and the wait before the
    /// next attempt started. `now` is the frontend's clock, in seconds.
    pub fn submit_password_prompt(&mut self, now: f64) -> bool {
        let Some(prompt) = self.password_prompt.as_ref() else {
            return false;
        };
        if stars_formats::password_salt(&prompt.typed) != prompt.salt {
            self.password_failures = self.password_failures.saturating_add(1);
            #[allow(clippy::cast_precision_loss)]
            let delay = self.password_retry_delay_ms() as f64 / 1000.0;
            self.password_retry_at = Some(now + delay);
            if let Some(prompt) = self.password_prompt.as_mut() {
                prompt.typed.clear();
                prompt.error = Some("That is not the password. Try again.".to_string());
            }
            return false;
        }
        self.password_retry_at = None;
        let prompt = self.password_prompt.take().expect("checked just above");
        // `PasswordDlg` remembers the salt it accepted, which is what stops the
        // same password being asked for twice in one session.
        self.password_given = Some(prompt.salt);
        self.install(*prompt.pending);
        true
    }

    /// How long is left of the wait after a wrong password, in seconds.
    ///
    /// `now` is the frontend's clock. Zero when there is nothing to wait for.
    #[must_use]
    pub fn password_wait_left(&self, now: f64) -> f64 {
        let Some(until) = self.password_retry_at else {
            return 0.0;
        };
        (until - now).max(0.0)
    }

    /// Give up on the prompt, and on the save behind it.
    ///
    /// The original's loader treats a cancelled prompt as a failed load, so
    /// whatever was open stays open and the file is dropped.
    pub fn cancel_password_prompt(&mut self) {
        self.password_prompt = None;
    }
}

/// Where one player's turn has got to, as the Host Mode dialog reports it.
///
/// `CFindTurnsOutstanding` (`mdi.c`) works this out for every player and puts
/// the answer in `rgOut`, which indexes seven consecutive strings from
/// `idsTurned` (`0x02cc`) — with the dead one at `0x02cb`, an index of `-1`.
/// The dialog draws the first two in dark green and the rest in dark red.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnStatus {
    /// The player is dead and is not waited for (`rgOut = -1`).
    Dead,
    /// The orders are in — or the player is a computer player, which the host
    /// never waits for.
    TurnedIn,
    /// Nothing has arrived.
    StillOut,
    /// Something arrived, but it is not a finished turn.
    PartiallyDone,
    /// A file arrived that could not be read.
    Corrupted,
    /// A file for another year.
    WrongYear,
    /// A file for another game.
    WrongGame,
}

impl TurnStatus {
    /// What the dialog writes after the player's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Dead => "dead",
            Self::TurnedIn => "turned in",
            Self::StillOut => "still out",
            Self::PartiallyDone => "partially done",
            Self::Corrupted => "corrupted",
            Self::WrongYear => "not on the right year",
            Self::WrongGame => "not in the right game",
        }
    }

    /// Whether the host is still waiting on this player.
    ///
    /// Only the two the dialog draws in green are not outstanding, and a dead
    /// player is not waited for.
    #[must_use]
    pub fn outstanding(self) -> bool {
        !matches!(self, Self::Dead | Self::TurnedIn)
    }
}

impl App {
    /// The game's name, as the `.xy` records it.
    ///
    /// The Host Mode dialog puts it at the top; the original takes it from
    /// `game.szName`, which is what the universe file carries.
    #[must_use]
    pub fn game_name(&self) -> String {
        self.universe
            .as_ref()
            .and_then(|u| u.game().ok())
            .map(|info| info.name)
            .unwrap_or_default()
    }

    /// The base name of the open game's files (`szBase`), without an extension.
    #[must_use]
    pub fn host_file_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_stem())
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    /// Open the Host Mode dialog.
    pub fn open_host_mode(&mut self) {
        self.host_mode = true;
    }

    /// Close it.
    pub fn close_host_mode(&mut self) {
        self.host_mode = false;
    }

    /// Where each player's turn has got to.
    ///
    /// A computer player is never waited for, and neither is a dead one. For
    /// everybody else the `.xN` beside the game is looked at: missing is
    /// *still out*, a file for another game or another year says so, one that
    /// will not decode is *corrupted*, and one whose header does not carry the
    /// submitted flag is *partially done* — that flag is `gd.fPartialTurn` on
    /// the original's side.
    #[must_use]
    pub fn turn_status(&self, player: usize) -> TurnStatus {
        let Some(game) = self.game.as_ref() else {
            return TurnStatus::StillOut;
        };
        let Some(record) = game.players.get(player) else {
            return TurnStatus::StillOut;
        };
        if record.dead {
            return TurnStatus::Dead;
        }
        if !matches!(record.control, stars_core::ai::Control::Human) {
            return TurnStatus::TurnedIn;
        }
        let Some(path) = self.path.as_ref() else {
            return TurnStatus::StillOut;
        };
        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            return TurnStatus::StillOut;
        };
        let Ok(bytes) = std::fs::read(directory.join(format!("{stem}.x{}", player + 1))) else {
            return TurnStatus::StillOut;
        };
        let Ok(decoded) = StarsFile::decode(&bytes) else {
            return TurnStatus::Corrupted;
        };
        let header = &decoded.latest_segment().header;
        if header.game_id != game.seed {
            return TurnStatus::WrongGame;
        }
        if i16::try_from(header.turn).ok() != Some(game.turn) {
            return TurnStatus::WrongYear;
        }
        if !header.flag_done {
            return TurnStatus::PartiallyDone;
        }
        TurnStatus::TurnedIn
    }

    /// One row of the host dialog's player list, as `DrawHostDialog2`
    /// (`1020:6240`) writes it.
    ///
    /// Two things this project had wrong. The number is `"#%d:"` (`idsD2`),
    /// not a bare figure; and the rest is a **sentence** — `"%s are %s."`
    /// (`idsSS`) with the player's **plural** name and the status word, so a
    /// row reads `The Humanoids are turned in.` A player marked `fHacker`
    /// gets `" - HACKER"` after it.
    ///
    /// When the game carries **`fNoHostNames`** the names are left out
    /// altogether and the row is `" %s"` — the status alone. A host who should
    /// not know who is who still sees who is waited for.
    #[must_use]
    pub fn host_row(&self, player: usize) -> (String, String) {
        let status = self.turn_status(player);
        let text = if self.no_host_names {
            format!(" {}", status.name())
        } else {
            // A good many player blocks store no plural, so the singular
            // stands in rather than leaving the sentence with a hole in it.
            let plural = self
                .game
                .as_ref()
                .and_then(|game| game.players.get(player))
                .map(|p| {
                    if p.plural_name.is_empty() {
                        p.name.clone()
                    } else {
                        p.plural_name.clone()
                    }
                })
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("player {}", player + 1));
            format!("{plural} are {}.", status.name())
        };
        // The original appends `" - HACKER"` for a player marked `fHacker`;
        // this engine does not carry that flag, so nothing is appended.
        (format!("#{}:", player + 1), text)
    }

    /// Every player's status, in player order.
    #[must_use]
    pub fn turn_statuses(&self) -> Vec<TurnStatus> {
        let count = self.game.as_ref().map_or(0, |game| game.players.len());
        (0..count).map(|player| self.turn_status(player)).collect()
    }

    /// How many turns the host is still waiting on (`CFindTurnsOutstanding`).
    #[must_use]
    pub fn turns_outstanding(&self) -> usize {
        self.turn_statuses()
            .iter()
            .filter(|status| status.outstanding())
            .count()
    }

    /// The year the next generation will produce.
    #[must_use]
    pub fn next_year(&self) -> i32 {
        self.game.as_ref().map_or(2400, |game| game.year() + 1)
    }

    /// How many turns `Generate Now` will run, given the modifiers held down.
    ///
    /// The original reads them with `GetAsyncKeyState` as it starts: plain is
    /// one turn, and Ctrl, Shift or both force a run of them (`iPassCnt` 99, 9
    /// and 999). It asks before doing any of it.
    #[must_use]
    pub fn generate_passes(shift: bool, control: bool) -> u16 {
        match (shift, control) {
            (false, false) => 1,
            (false, true) => 99,
            (true, false) => 9,
            (true, true) => 999,
        }
    }

    /// The beat host mode runs on, in seconds.
    ///
    /// `SetTimer(NULL, 0x0D, 10000, HostTimerProc)`: the host looks for new
    /// turn files every ten seconds, and that is also how often the dialog
    /// redraws.
    pub const HOST_TICK_SECONDS: f64 = 10.0;

    /// Whether auto-generating is possible at all.
    ///
    /// `gd.fAllAis` — every player is a computer player — is the one thing
    /// that stops it: there is nobody to wait for, so the host would generate
    /// year after year for ever. The original disables the button and says so
    /// (string `0x02c8`).
    #[must_use]
    pub fn auto_generate_blocked(&self) -> bool {
        self.game.as_ref().is_none_or(|game| {
            game.players
                .iter()
                .all(|player| !matches!(player.control, stars_core::ai::Control::Human))
        })
    }

    /// Start or stop watching for turns.
    pub fn set_auto_generate(&mut self, on: bool, now: f64) {
        self.auto_generate = on && !self.auto_generate_blocked();
        self.auto_generate_checked = self.auto_generate.then_some(now);
    }

    /// Whether the watch is due to look again.
    #[must_use]
    pub fn auto_generate_due(&self, now: f64) -> bool {
        self.auto_generate
            && self
                .auto_generate_checked
                .is_none_or(|last| now - last >= Self::HOST_TICK_SECONDS)
    }

    /// Note that the watch has just looked.
    pub fn auto_generate_looked(&mut self, now: f64) {
        self.auto_generate_checked = Some(now);
    }

    /// What host mode reports while it waits, which the original writes into
    /// the frame's title bar (strings `0x031b`, `0x0421` and `0x031c`).
    #[must_use]
    pub fn host_title(&self) -> String {
        let out = self.turns_outstanding();
        format!(
            "Host Mode {out} Player{} Out",
            if out == 1 { "" } else { "s" }
        )
    }

    /// Generate `passes` turns in a row.
    pub fn generate_turns(&mut self, passes: u16) {
        for _ in 0..passes.max(1) {
            if self.game.is_none() {
                return;
            }
            self.generate_turn();
        }
    }
}

/// One fleet in the pane's last tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneFleet {
    /// Where it is in the game's fleet list.
    pub index: usize,
    /// Its owner and id, which together are what the dropdown remembers: a
    /// fleet id is only unique within one player's fleets.
    pub key: (i16, u16),
    /// Its name, as the game writes it.
    pub name: String,
    /// How many ships it has.
    pub ships: i32,
    /// Whether it is the local player's.
    pub mine: bool,
}

/// What the pane's last tile draws its two gauges from.
///
/// The fuel gauge is one bar; the cargo gauge is four, one per mineral and one
/// for colonists, which is why the original draws them with different routines
/// (`1050:44b6` and `1110:044e`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FleetGauges {
    /// Fuel aboard, in mg.
    pub fuel: i32,
    /// What the fleet's tanks hold.
    pub fuel_capacity: i32,
    /// Ironium, boranium and germanium aboard, in kT.
    pub minerals: [i32; 3],
    /// Colonists aboard, in kT.
    pub colonists: i32,
    /// What the fleet's holds take.
    pub cargo_capacity: i32,
}

impl FleetGauges {
    /// Everything in the holds.
    #[must_use]
    pub fn cargo(&self) -> i32 {
        self.minerals.iter().sum::<i32>() + self.colonists
    }
}

/// How settled a wormhole reads.
///
/// **Not** the stored `iStable`: the pane shows one of seven words indexed by
/// `PctWormholeMoves`, the chance the end jumps this year — so the player is
/// shown the formula's answer rather than the field. See
/// `docs/ui/mine-survey-pane.md`.
#[must_use]
pub fn wormhole_stability(hole: &stars_core::wormhole::Wormhole) -> &'static str {
    const WORDS: [&str; 7] = [
        "Rock Solid",
        "Stable",
        "Mostly Stable",
        "Average",
        "Slightly Volatile",
        "Volatile",
        "Extremely Volatile",
    ];
    let chance = stars_core::wormhole::jump_chance(hole.stability, hole.years_still);
    WORDS
        .get(usize::try_from(chance).unwrap_or(0))
        .copied()
        .unwrap_or("Average")
}

// --- The tutorial ---------------------------------------------------------

/// Compare a count the three ways the tutorial's arms compare them.
fn compare(held: usize, want: usize, cmp: crate::tutorial::Cmp) -> bool {
    match cmp {
        crate::tutorial::Cmp::Fewer => held < want,
        crate::tutorial::Cmp::Exactly => held == want,
        crate::tutorial::Cmp::NotExactly => held != want,
        crate::tutorial::Cmp::AtLeast => held >= want,
    }
}

impl App {
    /// Whether one of the tutorial's checks is satisfied.
    ///
    /// These are the fifteen `FCheck*` verbs of `FTutorTaskDone`
    /// (`10f8:0fbc`), each reduced to the question it actually asks. The help
    /// topic each sets on failure is the original's, and is what the page's
    /// Help button would open.
    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn tutor_check(&self, check: &crate::tutorial::Check) -> bool {
        use crate::tutorial::{grobj, Check, ANY};

        let Some(game) = self.game.as_ref() else {
            return false;
        };
        let me = self.local_player();
        // A fleet is named by its **id**, which is unique per player, so the
        // lookup is by id among the local player's fleets.
        let by_id = |id: u16| {
            game.fleets
                .iter()
                .find(|f| f.id == id && usize::try_from(f.owner).is_ok_and(|o| o == me))
        };

        match check {
            Check::Selection { class, id } => match *class {
                grobj::PLANET => self.selection.planet == Some(*id) && !self.selection.on_fleet,
                grobj::FLEET => {
                    self.selection.on_fleet
                        && self
                            .selection
                            .fleet
                            .and_then(|index| game.fleets.get(index))
                            .is_some_and(|f| i16::try_from(f.id).is_ok_and(|got| got == *id))
                }
                _ => false,
            },
            // The summary pane follows `sel.scan`, which is the same thing
            // this engine keeps in `selection` — the difference in the
            // original is which of the two the scanner last wrote.
            Check::Summary { class, id } => match *class {
                grobj::PLANET => self.selection.planet == Some(*id),
                grobj::FLEET => self
                    .selection
                    .fleet
                    .and_then(|index| game.fleets.get(index))
                    .is_some_and(|f| i16::try_from(f.id).is_ok_and(|got| got == *id)),
                grobj::THING => self.selection.thing.is_some(),
                _ => false,
            },
            // `9999` means every message read; the pane's index having run
            // past the last is how that shows here.
            Check::Messages {
                message,
                kind,
                filter,
            } => {
                let count = i32::try_from(self.messages().len()).unwrap_or(0);
                let read = if *message == 9999 {
                    count == 0 || self.message_index >= count - 1
                } else {
                    *message < 0 || self.message_index >= *message
                };
                if !read {
                    return false;
                }
                match (kind, filter) {
                    // No kind: reading is the whole question.
                    (None, _) => true,
                    // With `fFilter`, the question is whether that kind has
                    // been **filtered out** — "Filter it out by clicking the
                    // blue check mark in the upper left hand corner of the
                    // Messages pane."
                    (Some(id), true) => self
                        .messages()
                        .iter()
                        .find(|m| m.id == *id)
                        .zip(game.players.get(me))
                        .is_some_and(|(m, p)| m.hidden_by(&p.message_filter)),
                    // Without it, whether the message in front is one.
                    (Some(id), false) => self
                        .messages()
                        .get(usize::try_from(self.message_index).unwrap_or(0))
                        .is_some_and(|m| m.id == *id),
                }
            }
            Check::FleetWaypoint {
                fleet,
                order,
                class,
                id,
                task,
                warp,
            } => by_id(*fleet).is_some_and(|f| {
                f.waypoints.get(*order).is_some_and(|leg| {
                    let right_place =
                        *id == ANY || (leg.target_class == *class && leg.target == Some(*id));
                    let right_task = *task == ANY || u16::from(leg.task) == *task;
                    let right_warp = *warp == ANY || u16::from(leg.warp) == *warp;
                    right_place && right_task && right_warp
                })
            }),
            // Colonize is the same question with the task pinned, plus the
            // rule that a fleet already at the target must be carrying
            // something to put down.
            Check::ColonizeWaypoint { fleet, id, warp } => {
                self.tutor_check(&Check::FleetWaypoint {
                    fleet: *fleet,
                    order: 1,
                    class: grobj::PLANET,
                    id: *id,
                    task: u16::from(stars_formats::task::COLONIZE),
                    warp: *warp,
                })
            }
            Check::Cargo {
                fleet,
                minerals,
                colonists,
            } => by_id(*fleet)
                .is_some_and(|f| f.cargo.minerals == *minerals && f.cargo.colonists == *colonists),
            Check::Queue {
                planet,
                slot,
                ship,
                item,
                count,
                no_research,
            } => game
                .planets
                .iter()
                .find(|p| p.id == *planet)
                .and_then(|p| Some((p, p.queue.get(*slot)?)))
                .is_some_and(|(planet, entry)| {
                    entry.ship == *ship
                        && entry.item == *item
                        && i32::from(*count) == entry.count
                        && no_research.is_none_or(|want| planet.no_research == want)
                }),
            Check::Research { field, next, pct } => game.players.get(me).is_some_and(|p| {
                p.research.current_field == *field
                    && p.research.next_field.raw() == *next
                    && p.research_pct == *pct
            }),
            Check::Scanner { view, zoom } => {
                let view_ok = view.is_none_or(|want| {
                    if want < 6 {
                        u16::from(self.scan_view as u8) == want
                    } else {
                        // A mask of overlay bits: every one of them must be on.
                        self.grbit_scan() & want == want
                    }
                });
                view_ok && zoom.is_none_or(|want| want == self.scan_zoom)
            }
            Check::PlanetRoute { planet, to } => game
                .planets
                .iter()
                .find(|p| p.id == *planet)
                .is_some_and(|p| p.route_dest == Some(*to)),
            // Only the action of each cargo is compared, unless it is one
            // that carries a meaningful figure.
            Check::TransportWaypoint {
                fleet,
                order,
                id,
                warp,
                goal,
            } => {
                self.tutor_check(&Check::FleetWaypoint {
                    fleet: *fleet,
                    order: *order,
                    class: grobj::PLANET,
                    id: *id,
                    task: u16::from(stars_formats::task::TRANSPORT),
                    warp: *warp,
                }) && by_id(*fleet).is_some_and(|f| {
                    f.waypoints.get(*order).is_some_and(|leg| {
                        let held = stars_formats::TransportTask::decode(&leg.task_data);
                        held.is_some_and(|held| {
                            held.items
                                .iter()
                                .zip(goal.iter())
                                .all(|(had, want)| had.action == *want)
                        })
                    })
                })
            }
            Check::RepeatOrders { fleet } => by_id(*fleet).is_some_and(|f| f.repeat_orders),
            Check::FleetCount { count, cmp } => compare(self.own_fleets().len(), *count, *cmp),
            Check::FleetExists { fleet, exists } => by_id(*fleet).is_some() == *exists,
            // `iItem` packs the count in its high byte and the component
            // index in its low one, which is how `FCheckBuilderPart`
            // compares the two separately.
            Check::Designer { open } => self.designer.is_some() == *open,
            Check::SavedDesignSlot { design, slot, item } => game
                .designs
                .get(me)
                .and_then(|designs| designs.get(*design))
                .and_then(|d| d.slots.get(*slot))
                .is_some_and(|fitted| fitted.item == *item),
            Check::DesignSlot { slot, item, count } => self
                .designer
                .as_ref()
                .and_then(|d| d.editing.as_ref())
                .and_then(|e| e.design.slots.get(*slot))
                .is_some_and(|fitted| {
                    fitted.item == u8::try_from(*item & 0xff).unwrap_or(0) && fitted.count >= *count
                }),
            Check::Fuel { fleet, amount } => by_id(*fleet).is_some_and(|f| f.cargo.fuel == *amount),
            Check::DesignCount { count, cmp } => game
                .designs
                .get(me)
                .is_some_and(|designs| compare(designs.len(), *count, *cmp)),
            Check::Zip { slot, goal } => self.zip_orders.get(*slot).is_some_and(|saved| {
                !saved.name.is_empty()
                    && saved
                        .items
                        .iter()
                        .zip(goal.iter())
                        .all(|((action, _), want)| action == want)
            }),
            Check::BattleVcr { open } => self.vcr.is_some() == *open,
            Check::Browser { open } => self.browser.is_some() == *open,
            Check::ResearchDialog { open } => self.research_dialog.is_some() == *open,
            // `vprptCur`: the arms test the pointer, so any of the four
            // reports satisfies it.
            Check::ReportOpen => self.open_report_kind().is_some(),
            Check::ReportSort {
                column,
                ascending,
                subsort,
            } => self.open_report_kind().is_some_and(|report| {
                let state = self.reports.state(report);
                state.sort == *column
                    && ascending.is_none_or(|want| state.ascending == want)
                    && subsort.is_none_or(|want| state.subsort == want)
            }),
            Check::Template { slot } => self
                .production_templates()
                .get(*slot)
                .is_some_and(|t| t.queue.is_some()),
            Check::QueueLength { planet, count, cmp } => game
                .planets
                .iter()
                .find(|p| p.id == *planet)
                .is_some_and(|p| compare(p.queue.len(), *count, *cmp)),
            Check::FleetOrders { fleet, count, cmp } => {
                by_id(*fleet).is_some_and(|f| compare(f.waypoints.len(), *count, *cmp))
            }
            Check::ShipBuilder { starbase, design } => self.designer.as_ref().is_some_and(|d| {
                starbase.is_none_or(|want| d.starbase == want)
                    && design.is_none_or(|want| d.selected == want)
            }),
        }
    }

    /// The scanner's state as the original's own `grbitScan` word.
    ///
    /// The bits are `ExecuteButton`'s (`1068:0db6`), which sets exactly one
    /// per toolbar button: the six views share the **low nibble** — switching
    /// keeps `grbitScan & 0x3ff0` and drops the old view — and each toggle
    /// owns a bit of its own.
    ///
    /// | bit | button |
    /// |-----|--------|
    /// | `0x000f` | the chosen view, 0 to 5 |
    /// | `0x0010` | `Add Way Points Mode` |
    /// | `0x0020` | `Scanner Coverage Overlay` |
    /// | `0x0040` | `Mine Fields Overlay` |
    /// | `0x0080` | `Fleet Paths Overlay` |
    /// | `0x0100` | `Idle Fleets Filter` |
    /// | `0x0200` | `Ship Design Filter` |
    /// | `0x0400` | `Planet Names Overlay` |
    /// | `0x0800` | `Enemy Ship Class Filter` |
    /// | `0x1000` | `Ship Counts Overlay` |
    /// | `0x2000` | `Player Colors`, which is the View menu's own |
    /// Put the scanner back the way a `grbitScan` says.
    ///
    /// The inverse of [`Self::grbit_scan`], for restoring the scanner from
    /// `stars.ini`. A view the game does not have reads as the plain one.
    pub fn set_grbit_scan(&mut self, bits: u16) {
        self.scan_view = match bits & 0x000f {
            1 => ScanView::SurfaceMineral,
            2 => ScanView::MineralConcentration,
            3 => ScanView::PlanetValue,
            4 => ScanView::Population,
            5 => ScanView::NoPlayerInfo,
            _ => ScanView::Normal,
        };
        self.add_waypoints = bits & 0x0010 != 0;
        self.scan_overlays = ScanOverlays {
            scanner_coverage: bits & 0x0020 != 0,
            minefields: bits & 0x0040 != 0,
            fleet_paths: bits & 0x0080 != 0,
            idle_fleets: bits & 0x0100 != 0,
            ship_design_filter: bits & 0x0200 != 0,
            names: bits & 0x0400 != 0,
            enemy_class_filter: bits & 0x0800 != 0,
            ship_counts: bits & 0x1000 != 0,
            player_colours: bits & 0x2000 != 0,
        };
    }

    #[must_use]
    pub fn grbit_scan(&self) -> u16 {
        let mut bits = u16::from(self.scan_view as u8) & 0x000f;
        for (on, bit) in [
            (self.add_waypoints, 0x0010),
            (self.scan_overlays.scanner_coverage, 0x0020),
            (self.scan_overlays.minefields, 0x0040),
            (self.scan_overlays.fleet_paths, 0x0080),
            (self.scan_overlays.idle_fleets, 0x0100),
            (self.scan_overlays.ship_design_filter, 0x0200),
            (self.scan_overlays.names, 0x0400),
            (self.scan_overlays.enemy_class_filter, 0x0800),
            (self.scan_overlays.ship_counts, 0x1000),
            (self.scan_overlays.player_colours, 0x2000),
        ] {
            if on {
                bits |= bit;
            }
        }
        bits
    }
}

impl App {
    /// The step the tutorial is on, if it is running.
    #[must_use]
    pub fn tutor_step(&self) -> Option<&'static crate::tutorial::Step> {
        let tutor = self.tutor.as_ref()?;
        crate::tutorial::step(tutor.idt)
    }

    /// Whether the page showing has had its task done.
    ///
    /// `FTutorTaskDone` (`10f8:0fbc`) asks about a page only in **its own
    /// year** — it is a `switch (game.turn)` — so a page belonging to another
    /// year is not done, whatever the galaxy looks like. A page with nothing
    /// to do passes at once.
    #[must_use]
    pub fn tutor_task_done(&self) -> bool {
        let Some(step) = self.tutor_step() else {
            return false;
        };
        let turn = self.game.as_ref().map_or(-1_i16, |game| game.turn);
        if step.turn != turn {
            return false;
        }
        if step
            .escape
            .as_ref()
            .is_some_and(|check| self.tutor_check(check))
        {
            return true;
        }
        step.stages
            .iter()
            .filter(|stage| stage.gates)
            .all(|stage| stage.check.as_ref().is_none_or(|c| self.tutor_check(c)))
    }

    /// Which paragraph the page emboldens: the first rung not yet satisfied,
    /// or the last when they all are.
    #[must_use]
    pub fn tutor_bold(&self) -> Option<usize> {
        let step = self.tutor_step()?;
        let turn = self.game.as_ref().map_or(-1_i16, |game| game.turn);
        if step.turn != turn {
            return step.stages.first().map(|stage| stage.bold);
        }
        step.stages
            .iter()
            .find(|stage| stage.check.as_ref().is_some_and(|c| !self.tutor_check(c)))
            .or_else(|| step.stages.last())
            .map(|stage| stage.bold)
    }

    /// Step the tutorial on if the page's task is done.
    ///
    /// `AdvanceTutor` (`10f8:0a30`): while the task is done, add eight to
    /// `idt` and ask again — so a page satisfied in advance is skipped rather
    /// than shown — and past the last paragraph of page eighty the tutorial
    /// ends. Returns whether the page changed.
    pub fn advance_tutor(&mut self) -> bool {
        let Some(tutor) = self.tutor.as_ref() else {
            return false;
        };
        if tutor.finished {
            return false;
        }
        let was = tutor.idt;
        while self.tutor_task_done() {
            let Some(tutor) = self.tutor.as_mut() else {
                return false;
            };
            tutor.idt += stars_formats::tutorial::PARAGRAPHS_PER_PAGE;
            tutor.error = None;
            tutor.bold = tutor.idt;
            if tutor.idt > crate::tutorial::LAST_PARAGRAPH {
                tutor.finished = true;
                break;
            }
        }
        // A page that has moved brings the window back up, which is
        // `AdvanceTutor`'s own `ShowTutor(1)`.
        let moved = self.tutor.as_ref().is_some_and(|t| t.idt != was);
        if moved {
            self.show_tutor();
        }
        // Whatever page we have landed on says which of its paragraphs to
        // embolden.
        if let Some(bold) = self.tutor_bold() {
            if let Some(tutor) = self.tutor.as_mut() {
                tutor.bold = bold;
            }
        }
        self.tutor.as_ref().is_some_and(|t| t.idt != was)
    }

    /// Begin the tutorial at its first page.
    ///
    /// `StartTutor` (`10f8:06b4`) zeroes the whole of `tutor` and then runs
    /// the same skipping loop, so a game already past the opening pages opens
    /// on the first page that still has something to do.
    pub fn start_tutor(&mut self) {
        self.tutor = Some(crate::tutorial::Tutor::default());
        self.advance_tutor();
    }

    /// Stop it (`EndTutor`, `10f8:0c02`).
    pub fn end_tutor(&mut self) {
        self.tutor = None;
    }

    /// The paragraphs of the page showing, read out of the player's copy of
    /// the game.
    ///
    /// `None` when the tutorial is not running, or when no copy of the
    /// original has been found — its words are the game's own and none of
    /// them are in this program.
    #[must_use]
    pub fn tutor_page(&self) -> Option<Vec<String>> {
        let tutor = self.tutor.as_ref()?;
        let art = self.art.as_ref()?;
        stars_formats::tutorial::page(art.executable(), tutor.page())
    }
}

// --- Walking your own fleets ----------------------------------------------

impl App {
    /// Your own fleets, in the order the pane's Prev and Next walk them.
    ///
    /// `SelectAdjFleet` (`1050:3d32`) steps through `vlprgidFleet`, the local
    /// player's own list, not through every fleet on the map — somebody
    /// else's is never stepped onto.
    #[must_use]
    pub fn own_fleets(&self) -> Vec<usize> {
        let me = self.local_player();
        self.game.as_ref().map_or_else(Vec::new, |game| {
            game.fleets
                .iter()
                .enumerate()
                .filter(|(_, f)| usize::try_from(f.owner).is_ok_and(|o| o == me))
                .map(|(index, _)| index)
                .collect()
        })
    }

    /// Step to the fleet before or after the one selected, wrapping round.
    ///
    /// `SelectAdjFleet` with a non-zero `dInc`: find where the current fleet
    /// sits in your own list, move by `delta`, and **wrap** — past the end
    /// goes to the first, before the start goes to the last. With nothing
    /// selected it starts at the first. Returns whether the selection moved.
    ///
    /// The original also recentres the scanner on the fleet it lands on
    /// (`CtrPointScan`); this frontend's map has no scroll to recentre.
    pub fn select_adjacent_fleet(&mut self, delta: i32) -> bool {
        let fleets = self.own_fleets();
        if fleets.is_empty() {
            return false;
        }
        let here = self
            .selection
            .fleet
            .and_then(|index| fleets.iter().position(|f| *f == index));
        let count = i32::try_from(fleets.len()).unwrap_or(1);
        let next = match here {
            Some(at) => {
                let moved = i32::try_from(at).unwrap_or(0) + delta;
                if moved >= count {
                    0
                } else if moved < 0 {
                    count - 1
                } else {
                    moved
                }
            }
            None => 0,
        };
        let index = fleets[usize::try_from(next).unwrap_or(0)];
        if self.selection.fleet == Some(index) && self.selection.on_fleet {
            return false;
        }
        self.select_object(ScanObject::Fleet(index));
        true
    }

    /// Select one fleet by its id, as the tile's **Goto** button does.
    ///
    /// `SelectAdjFleet` with `dInc == 0`: no stepping, just go there.
    pub fn goto_fleet(&mut self, id: u16) -> bool {
        let me = self.local_player();
        let Some(index) = self.game.as_ref().and_then(|game| {
            game.fleets
                .iter()
                .position(|f| f.id == id && usize::try_from(f.owner).is_ok_and(|o| o == me))
        }) else {
            return false;
        };
        self.select_object(ScanObject::Fleet(index));
        true
    }

    /// Go to the planet the selected fleet is orbiting, as the location
    /// tile's **Goto** button does (`SelectAdjPlanet(0, sel.fl.idPlanet)`).
    pub fn goto_orbited_planet(&mut self) -> bool {
        let Some(id) = self
            .selection
            .fleet
            .and_then(|index| self.game.as_ref()?.fleets.get(index))
            .and_then(|fleet| fleet.orbiting)
            .and_then(|id| i16::try_from(id).ok())
        else {
            return false;
        };
        self.select_object(ScanObject::Planet(id));
        true
    }
}

impl App {
    /// Put every ship of a fleet into a fleet of its own.
    ///
    /// `FFleetSplitAll` (`1038:3a00`) walks the sixteen design slots and,
    /// for **every ship after the first overall**, makes a new fleet holding
    /// one of them, moving a share of the cargo across as it goes
    /// (`FleetTransferCargoBalance`). The fleet you were commanding keeps the
    /// first ship and nothing else.
    ///
    /// Returns how many new fleets were made — the original returns whether
    /// there was more than one ship to begin with, which is the same
    /// question.
    pub fn split_all(&mut self, fleet: usize) -> usize {
        let Some(stacks) = self
            .game
            .as_ref()
            .and_then(|game| game.fleets.get(fleet))
            .map(|f| f.stacks.clone())
        else {
            return 0;
        };
        // One ship stays behind; every other becomes a fleet.
        let mut first = true;
        let mut made = 0;
        for stack in stacks {
            let mut peel = stack.count;
            if first {
                peel -= 1;
                first = false;
            }
            for _ in 0..peel {
                if self.split_fleet(fleet, stack.design, 1) {
                    made += 1;
                }
            }
        }
        made
    }

    /// How many ships the selected fleet holds altogether.
    #[must_use]
    pub fn pane_fleet_ships(&self) -> i32 {
        self.pane_fleet()
            .map_or(0, |fleet| fleet.stacks.iter().map(|s| s.count).sum())
    }
}

// --- Zip orders: the blue diamond's menu ----------------------------------

/// One of the four saved cargo orders the blue diamond offers.
///
/// `vrgZip` in the original: four slots of `0x18` bytes, each a validity
/// byte, the same five `ITEMACTION` words a Transport waypoint carries, and a
/// name kept beside them at `0x526e + i * 0x18`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ZipOrder {
    /// What it is called. An empty slot has no name and shows as
    /// `<Unused n>` (string `0x4be`).
    pub name: String,
    /// The five cargo instructions, ironium first and fuel last, exactly as a
    /// Transport waypoint stores them.
    pub items: [(stars_formats::XferAction, u16); 5],
}

impl App {
    /// How many custom order slots there are (`vrgZip`).
    pub const ZIP_ORDERS: usize = 4;

    /// What the blue diamond's menu offers, in the original's order.
    ///
    /// Two built-in orders, then the four custom slots, then `<Customize>`
    /// (string `0x4c0`) which opens the dialog that fills them in. An empty
    /// slot reads `<Unused n>`.
    #[must_use]
    pub fn zip_menu(&self) -> Vec<String> {
        let mut out = vec!["QuikLoad".to_string(), "QuikDrop".to_string()];
        for (index, slot) in self.zip_orders.iter().enumerate() {
            out.push(if slot.name.is_empty() {
                format!("<Unused {}>", index + 1)
            } else {
                slot.name.clone()
            });
        }
        out.push("<Customize>".to_string());
        out
    }

    /// Load everything there is to load — the first of the two built-in
    /// orders.
    ///
    /// The tutorial names both by what they do: *"select QuikDrop to empty
    /// the freighter's hold at 90210"*, so QuikDrop unloads everything and
    /// QuikLoad is its opposite.
    pub fn zip_quik(&mut self, load: bool) -> bool {
        let action = if load {
            stars_formats::XferAction::LoadAll
        } else {
            stars_formats::XferAction::UnloadAll
        };
        let mut any = false;
        for slot in 0..5 {
            any |= self.set_waypoint_transport(slot, action, 0);
        }
        any
    }

    /// Apply one of the four saved orders to the waypoint in hand.
    pub fn zip_apply(&mut self, index: usize) -> bool {
        let Some(order) = self.zip_orders.get(index).cloned() else {
            return false;
        };
        if order.name.is_empty() {
            return false;
        }
        let mut any = false;
        for (slot, (action, quantity)) in order.items.iter().enumerate() {
            any |= self.set_waypoint_transport(slot, *action, *quantity);
        }
        any
    }

    /// **Import**: copy the waypoint in hand's cargo table into a slot and
    /// name it (`ZipOrderDlg`'s `0x816`).
    ///
    /// An empty name is replaced with `Custom n`, as the original does.
    pub fn zip_import(&mut self, index: usize, name: &str) -> bool {
        if index >= Self::ZIP_ORDERS || self.task_leg().is_none() {
            return false;
        }
        let items = std::array::from_fn(|slot| self.waypoint_transport(slot));
        let name = if name.trim().is_empty() {
            format!("Custom {}", index + 1)
        } else {
            name.trim().to_string()
        };
        self.zip_orders[index] = ZipOrder { name, items };
        true
    }

    /// **Delete**: empty a slot (`ZipOrderDlg`'s `0x817`).
    pub fn zip_delete(&mut self, index: usize) -> bool {
        let Some(slot) = self.zip_orders.get_mut(index) else {
            return false;
        };
        if slot.name.is_empty() {
            return false;
        }
        *slot = ZipOrder::default();
        true
    }
}

impl App {
    /// Whether the game in hand is a single-player one (`GAME` flag word at
    /// `+0x10`, bit 2).
    ///
    /// `InitializeMenu` (`1020:5560`) greys four menu items on it, because
    /// each is about a game with other people in it.
    #[must_use]
    pub fn single_player(&self) -> bool {
        self.game.as_ref().is_some_and(|game| game.single_player)
    }

    /// Whether a menu item is alive, as the `WM_INITMENU` handler decides.
    ///
    /// `InitializeMenu` greys five items, each on its own condition rather
    /// than on one blanket "is a game open":
    ///
    /// | item | greyed when |
    /// |------|-------------|
    /// | `&Generate` (`0x69`) | no game |
    /// | `&Wait for New` (`0x6a`) | no game, or single-player |
    /// | `&Player Relations...` (`0x7de`) | no game, or single-player |
    /// | `Save &And Submit` (`0xedb`) | no game, or single-player |
    /// | `&Change Password...` (`0x10e`) | no game, or single-player **with no password set** |
    ///
    /// Change Password is the interesting one: a single-player game can
    /// still take the password **off**, so the item stays alive while
    /// `lSaltCur` is non-zero.
    #[must_use]
    pub fn menu_item_enabled(&self, item: MenuItem) -> bool {
        let playing = self.game.is_some() && self.setup.is_none();
        if !playing {
            return false;
        }
        let alone = self.single_player();
        match item {
            MenuItem::Generate => true,
            MenuItem::WaitForNew | MenuItem::PlayerRelations | MenuItem::SaveAndSubmit => !alone,
            MenuItem::ChangePassword => !alone || self.current_password_salt() != 0,
        }
    }

    /// The salt of the password on the player whose turn this is
    /// (`lSaltCur`); `0` when there is none.
    #[must_use]
    pub fn current_password_salt(&self) -> u32 {
        self.game
            .as_ref()
            .and_then(|game| game.players.get(self.local_player()))
            .map_or(0, |player| player.password)
    }

    /// Whether a screen is one of the original's four **reports**.
    ///
    /// `SortReportCache` (`1108:589c`) knows four: planets, your fleets,
    /// everybody else's fleets, and battles. This project's Players screen
    /// is its own and is **not** one of them, so F3 does not deal it into
    /// the cycle — though Esc closes it like anything else.
    #[must_use]
    pub fn is_report(screen: Screen) -> bool {
        matches!(
            screen,
            Screen::Planets | Screen::Fleets | Screen::EnemyFleets | Screen::Battles
        )
    }

    /// Which report window is up, which is the original's `vprptCur`.
    ///
    /// Derived from the screen rather than stored: the four report windows
    /// are this project's four report screens, and the Players screen and
    /// the map are neither.
    #[must_use]
    pub fn open_report_kind(&self) -> Option<crate::report::Report> {
        match self.screen {
            Screen::Planets => Some(crate::report::Report::Planets),
            Screen::Fleets => Some(crate::report::Report::Fleets),
            Screen::EnemyFleets => Some(crate::report::Report::EnemyFleets),
            Screen::Battles => Some(crate::report::Report::Battles),
            Screen::Galaxy | Screen::Players => None,
        }
    }

    /// **F3**, which walks round the four reports and back to the map.
    ///
    /// All four Report entries show `F3` after their caption, but that text
    /// is only text: the accelerator is one key with an id of its own,
    /// `0x8fe`, which is on no menu. `CommandHandler` (`1020:448f`) turns it
    /// into one of the four by asking what is open —
    ///
    /// ```c
    /// if (hwndReportDlg == 0)            wParam = 0x8fd;  /* Planets */
    /// else if (vprptCur == &vrptPlanet)  wParam = 0x8ff;  /* Fleets */
    /// else if (vprptCur == &vrptFleet)   wParam = 0x900;  /* Others' */
    /// else                               wParam = 0x901;  /* Battles */
    /// ```
    ///
    /// — and the open path then closes Battles rather than reopening it
    /// (`1020:4727`). So the key **cycles**: nothing, planets, your fleets,
    /// everybody else's, battles, nothing again.
    pub fn open_report(&mut self) {
        self.screen = match self.screen {
            Screen::Planets => Screen::Fleets,
            Screen::Fleets => Screen::EnemyFleets,
            Screen::EnemyFleets => Screen::Battles,
            Screen::Battles => Screen::Galaxy,
            // Nothing open — the map, or this project's own Players screen.
            Screen::Galaxy | Screen::Players => Screen::Planets,
        };
    }

    /// Pick a report from the Report menu.
    ///
    /// Asking for a report while one is open closes that one first, and
    /// **Battles asked for while Battles is open just closes it**: the one
    /// item of the four that toggles (`1020:4727` tests `wParam == 0x901`
    /// against `vprptCur == &vrptBattle` and skips the reopen).
    pub fn choose_report(&mut self, screen: Screen) {
        self.screen = if screen == Screen::Battles && self.screen == Screen::Battles {
            Screen::Galaxy
        } else {
            screen
        };
    }

    /// Close whatever screen is up and go back to the map, as **Esc** does —
    /// "Hit the Esc key to close the Planet Summary Report."
    ///
    /// Returns whether there was something to close. The Players screen is
    /// not one of the original's reports but Esc leaves it the same way.
    pub fn close_report(&mut self) -> bool {
        if self.screen == Screen::Galaxy {
            return false;
        }
        self.screen = Screen::Galaxy;
        true
    }

    /// Go to a screen.
    pub fn show_screen(&mut self, screen: Screen) {
        self.screen = screen;
    }
}

impl App {
    /// Move fuel between the fleet the pane is about and another fleet at the
    /// same place.
    ///
    /// The **Other Fleets Here** tile's fuel gauge is draggable, and dragging
    /// it moves fuel between the two fleets — page 56 of the tutorial: *"Click
    /// and drag in the fuel gauge in the Other Fleets Here tile until Teamster
    /// #4 has 383mg of fuel."*
    ///
    /// `wanted` is what the **pane's** fleet should end up with. Returns how
    /// much actually moved, which is limited by what the other fleet has and
    /// by what this one can hold.
    pub fn drag_fleet_fuel(&mut self, other: usize, wanted: i32) -> i32 {
        use stars_formats::{CargoTransfer, LogRecord};

        let Some(mine) = self.survey_subject().fleet_index() else {
            return 0;
        };
        if mine == other {
            return 0;
        }
        let me = self.local_player();
        let Some(game) = self.game.as_ref() else {
            return 0;
        };
        let (Some(a), Some(b)) = (game.fleets.get(mine), game.fleets.get(other)) else {
            return 0;
        };
        // Both have to be yours: the original draws no gauges at all for a
        // fleet it does not know in full.
        if !usize::try_from(a.owner).is_ok_and(|o| o == me)
            || !usize::try_from(b.owner).is_ok_and(|o| o == me)
        {
            return 0;
        }
        let designs = game.designs.get(me).map_or(&[][..], Vec::as_slice);
        let room = a.fuel_capacity(designs) - a.cargo.fuel;
        // Positive: fuel comes across to the pane's fleet.
        let moved = (wanted - a.cargo.fuel).clamp(-a.cargo.fuel, room.min(b.cargo.fuel));
        if moved == 0 {
            return 0;
        }
        let owner = u16::try_from(a.owner.max(0)).unwrap_or(0);
        let source = (owner << 9) | (a.id & 0x1ff);
        let destination = (owner << 9) | (b.id & 0x1ff);

        // Moved here rather than through `apply_cargo_transfer`: that
        // routine is the turn engine's, and applies a *turn's* transfer with
        // the engine's own clamping. This is the client moving fuel between
        // two fleets sitting together, which the original does at once.
        let Some(game) = self.game.as_mut() else {
            return 0;
        };
        game.fleets[mine].cargo.fuel += moved;
        game.fleets[other].cargo.fuel -= moved;

        // Fuel is the fifth cargo kind, so bit 4 of the mask. The quantity
        // is what the fleet named first gives up.
        self.orders.push(LogRecord::cargo(&CargoTransfer {
            id1: source,
            id2: destination,
            grobj1: FLEET_CLASS,
            grobj2: FLEET_CLASS,
            items_mask: 1 << 4,
            quantities: vec![-moved],
            quantity_bytes: Vec::new(),
        }));
        self.dirty = true;
        moved
    }
}

impl App {
    /// Build the tutorial's sample game and start the tutorial on it.
    ///
    /// `StartTutor` (`10f8:06b4`) does this when no game is loaded: it calls
    /// `CreateTutorWorld` and then runs the tutorial's own skipping loop, so
    /// the first page shown is the first with something still to do.
    ///
    /// It also fixes the scanner: `grbitScan = 0x4e0` — the normal view with
    /// scanner coverage, mine fields and fleet paths on — and picks a zoom
    /// from the screen's width. Both are reproduced.
    ///
    /// # Errors
    ///
    /// A message suitable for showing to the player.
    pub fn create_tutor_world(&mut self, screen_width: u32) -> Result<(), String> {
        let (config, seed) = stars_core::newgame::tutorial();
        self.new_game_seeded(&config, seed)?;

        // `grbitScan = 0x4e0`: the low nibble is zero, so the normal view,
        // and bits 5, 6, 7 and 10 are coverage, mine fields, fleet paths and
        // planet names.
        self.scan_view = ScanView::Normal;
        self.scan_overlays.scanner_coverage = true;
        self.scan_overlays.minefields = true;
        self.scan_overlays.fleet_paths = true;
        self.scan_overlays.names = true;

        // `iScanZoom` from `GetSystemMetrics(SM_CXSCREEN)`: under 800 is 0,
        // under 1024 is 1, under 1280 is 2, and 3 above that.
        self.scan_zoom = match screen_width {
            0..=799 => 0,
            800..=1023 => 1,
            1024..=1279 => 2,
            _ => 3,
        };

        self.start_tutor();
        Ok(())
    }
}

impl App {
    /// **Hide** the tutor window without stopping the tutorial.
    ///
    /// `TutorDlg`'s `Hide` (`10f8:0000`, `wParam == 2`) calls `ShowTutor(0)`
    /// and, the first time, puts up a notice saying how to get the window
    /// back — string `0x51a`, *"To make the tutorial reappear complete your
    /// task or choose Tutorial from the Help menu."* The bit that remembers
    /// it has been said is cleared afterwards, so it is said once.
    ///
    /// Returns the notice, when this is the time to show it.
    pub fn hide_tutor(&mut self) -> Option<&'static str> {
        let tutor = self.tutor.as_mut()?;
        tutor.hidden = true;
        if tutor.told_how_to_return {
            return None;
        }
        tutor.told_how_to_return = true;
        Some(
            "The tutorial is still running. Finish what the page asked for, or choose \
             Tutorial from the Help menu, to bring it back.",
        )
    }

    /// Put it back up.
    ///
    /// `AdvanceTutor` calls `ShowTutor(1)` whenever it moves to a new page,
    /// so finishing a page brings a hidden window back by itself.
    pub fn show_tutor(&mut self) {
        if let Some(tutor) = self.tutor.as_mut() {
            tutor.hidden = false;
        }
    }
}
