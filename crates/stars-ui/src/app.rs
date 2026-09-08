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

use crate::vcr::Vcr;

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
    /// Battle playback.
    Battles,
    /// Player and race summary.
    Players,
}

impl Screen {
    /// Every screen, in the order a frontend should offer them.
    pub const ALL: [Screen; 5] = [
        Screen::Galaxy,
        Screen::Planets,
        Screen::Fleets,
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
            Screen::Battles => "Battles",
            Screen::Players => "Players",
        }
    }
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
/// The object class for "no target at all" — a bare coordinate.
const NO_TARGET_CLASS: u8 = 4;

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
}

/// What the Find dialog found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindResult {
    /// A planet, by id.
    Planet(i16),
    /// A fleet, by index into [`GameState::fleets`].
    Fleet(usize),
}

/// What the scanner's status bar has to say.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusBar {
    /// What is there: a planet, a fleet, an object, or `Deep Space`.
    pub name: String,
    /// Where it is.
    pub x: i16,
    /// Where it is.
    pub y: i16,
    /// How far that is from the other end of the tape, when one is stretched.
    pub distance: Option<String>,
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
    /// `Add Way Points Mode`: whether clicking the map gives the selected
    /// fleet orders instead of selecting what is under the pointer.
    pub add_waypoints: bool,
    /// Which waypoint a drag is moving, while one is under way.
    pub dragging_waypoint: Option<usize>,
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
            scan_minefield_filter: 0xf,
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
        let mut rng = stars_core::rng::Rng::randomize(config.id);
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
            grobj: if waypoint.target.is_some() {
                PLANET_CLASS
            } else {
                NO_TARGET_CLASS
            },
            valid_task: waypoint.task != 0,
            flags_high: 0,
            task_data: waypoint
                .transport
                .as_ref()
                .map(stars_formats::TransportTask::encode)
                .unwrap_or_default(),
        };
        self.orders.push(LogRecord::waypoint(&order, insert));
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
            .and_then(|d| d.get(usize::from(planet.starbase_design?)))
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
                    crate::views::planets::item_name(entry.item)
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
    /// objects together; this searches the same three, nearest first.
    #[must_use]
    pub fn nearest_object(
        &self,
        at: stars_core::movement::Point,
        reach: f64,
    ) -> Option<(String, stars_core::movement::Point)> {
        let game = self.game.as_ref()?;
        let mut best: Option<(f64, String, stars_core::movement::Point)> = None;
        let mut consider = |name: String, p: stars_core::movement::Point| {
            let d = stars_core::movement::distance(p, at);
            if d <= reach && best.as_ref().is_none_or(|(b, _, _)| d < *b) {
                best = Some((d, name, p));
            }
        };
        for planet in game.planets.iter().chain(game.known_planets.iter()) {
            if let Some(p) = planet.position {
                consider(self.planet_name(planet.id), p);
            }
        }
        for (index, fleet) in game.fleets.iter().enumerate() {
            if fleet.stacks.is_empty() {
                continue;
            }
            consider(self.fleet_display_name(index), fleet.position);
        }
        for field in &game.minefields {
            consider(format!("Mine Field #{}", field.id), field.position);
        }
        for packet in &game.packets {
            consider(format!("Mineral Packet #{}", packet.id), packet.position);
        }
        for hole in &game.wormholes {
            consider(format!("Wormhole #{}", hole.id), hole.position);
        }
        for trader in &game.traders {
            consider("Mystery Trader".to_string(), trader.position);
        }
        best.map(|(_, name, p)| (name, p))
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

    /// What the scanner's status bar says: what is under the point, where it
    /// is, and — while the tape is stretched — how far that is from the other
    /// end (`DrawScannerSBar`, `1058:62d8`).
    ///
    /// The four cells are the original's: the object's id, its x, its y, and
    /// its name, with the distance on a second line.
    #[must_use]
    pub fn status_bar(&self) -> StatusBar {
        // While measuring, the bar follows the tape; otherwise the selection.
        let (from, at) = match self.measuring {
            Some((from, at)) => (Some(from), at),
            None => {
                let at = self
                    .pane_fleet()
                    .map(|f| f.position)
                    .or_else(|| self.pane_planet().and_then(|p| p.position));
                match at {
                    Some(at) => (None, at),
                    None => return StatusBar::default(),
                }
            }
        };
        let found = self.nearest_object(at, 0.5);
        StatusBar {
            name: found
                .as_ref()
                .map_or_else(|| "Deep Space".to_string(), |(name, _)| name.clone()),
            x: at.x,
            y: at.y,
            distance: from.map(|from| distance_text(from, at)),
        }
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

    /// Add a waypoint to the selected fleet, at a point on the map.
    ///
    /// This is what dragging from a fleet does: the leg is appended to whatever
    /// orders it already has, at the warp the client suggests, and the order
    /// log gets the insert the real client writes.
    ///
    /// Returns whether a waypoint was added.
    pub fn add_waypoint(&mut self, x: i16, y: i16) -> bool {
        let Some(index) = self.selection.fleet else {
            return false;
        };
        let me = self.local_player();
        let at = stars_core::movement::Point::new(x, y);
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
        let from = fleet
            .waypoints
            .last()
            .map_or(fleet.position, |w| w.position);
        #[allow(clippy::cast_possible_truncation)]
        let distance = stars_core::movement::distance(from, at) as i32;
        if distance <= 0 {
            return false;
        }
        let warp = self.suggested_warp(index, distance);
        // A waypoint on a planet names it, which is what makes a task there
        // possible at all.
        let target = game
            .planets
            .iter()
            .chain(game.known_planets.iter())
            .find(|p| p.position == Some(at))
            .map(|p| p.id);

        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let Some(fleet) = game.fleets.get_mut(index) else {
            return false;
        };
        fleet.waypoints.push(stars_core::fleet::Waypoint {
            position: at,
            target: target.and_then(|id| u16::try_from(id).ok()),
            target_class: if target.is_some() { 1 } else { 4 },
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
        self.dirty = true;
        true
    }

    /// Move one of the selected fleet's waypoints, as dragging it does.
    ///
    /// Waypoint 0 is where the fleet is and cannot be dragged. Returns whether
    /// anything moved.
    pub fn move_waypoint(&mut self, waypoint: usize, x: i16, y: i16) -> bool {
        let Some(index) = self.selection.fleet else {
            return false;
        };
        if waypoint == 0 {
            return false;
        }
        let me = self.local_player();
        let at = stars_core::movement::Point::new(x, y);
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
        let from = fleet.waypoints[waypoint - 1].position;
        #[allow(clippy::cast_possible_truncation)]
        let distance = stars_core::movement::distance(from, at) as i32;
        let warp = self.suggested_warp(index, distance.max(1));
        let target = game
            .planets
            .iter()
            .chain(game.known_planets.iter())
            .find(|p| p.position == Some(at))
            .map(|p| p.id);

        let Some(game) = self.game.as_mut() else {
            return false;
        };
        let Some(fleet) = game.fleets.get_mut(index) else {
            return false;
        };
        let leg = &mut fleet.waypoints[waypoint];
        leg.position = at;
        leg.target = target.and_then(|id| u16::try_from(id).ok());
        leg.target_class = if target.is_some() { 1 } else { 4 };
        leg.warp = warp;
        if waypoint == 1 {
            fleet.warp = Some(warp);
        }
        self.log_waypoint(index, waypoint, false);
        self.dirty = true;
        true
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
        if fleet.waypoints.len() < 2 {
            fleet.warp = None;
        }
        self.orders.push(stars_formats::LogRecord::delete_waypoint(
            stars_formats::FleetOrderDelete {
                fleet_id: fleet_word,
                order_index: u16::try_from(waypoint).unwrap_or(1),
                delete_extra: false,
            },
        ));
        self.dirty = true;
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
        record
            .waypoints
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, w)| stars_core::movement::distance(w.position, at) <= tolerance)
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
    /// `DrawMineSurvey` (`1028:065a`) switches on the object's `ith` and writes
    /// a few lines for each kind; the rows are in
    /// `docs/ui/mine-survey-pane.md`.
    #[must_use]
    pub fn survey_thing_rows(&self) -> Vec<String> {
        let SurveySubject::Thing(thing) = self.survey_subject() else {
            return Vec::new();
        };
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        match thing {
            ScanThing::Minefield(index) => {
                let Some(field) = game.minefields.get(index) else {
                    return Vec::new();
                };
                let kind = MINEFIELD_KINDS
                    .get(usize::from(field.kind))
                    .copied()
                    .unwrap_or("Mine Field");
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
                let mut rows = vec![
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
                    rows.push(format!("Field:  {which} of {}", mine.len()));
                }
                rows
            }
            ScanThing::Packet(index) => {
                let Some(packet) = game.packets.get(index) else {
                    return Vec::new();
                };
                let mut rows = vec![format!(
                    "Traveling at Warp {}",
                    packet.warp + stars_core::packet::WARP_BIAS
                )];
                let target = game
                    .planets
                    .iter()
                    .chain(game.known_planets.iter())
                    .find(|planet| planet.id == i16::try_from(packet.target).unwrap_or(-1));
                rows.push(format!(
                    "Destination:  {}",
                    target.map_or_else(
                        || "Unknown".to_string(),
                        |planet| self.planet_name(planet.id)
                    )
                ));
                for (name, amount) in ["Ironium", "Boranium", "Germanium"]
                    .iter()
                    .zip(packet.minerals.iter())
                {
                    rows.push(format!("{name}  {amount}kT"));
                }
                rows
            }
            ScanThing::Trader(index) => {
                let Some(trader) = game.traders.get(index) else {
                    return Vec::new();
                };
                let mut rows = Vec::new();
                // The notice comes first and only until this player has traded
                // — `1 << idPlayer & grbitPlr`, the same mask that stops them
                // trading twice. The wording is this project's own, as the
                // game's message text always is; `docs/formulas/wanderers.md`
                // has what it is asking for.
                let me = u32::try_from(self.local_player()).unwrap_or(0);
                if trader.detected_by & (1u16 << (me & 0x0F)) == 0 {
                    rows.push(
                        "Send it a fleet carrying at least 5,000kT of minerals and it will \
                         take the fleet, ships and all, in exchange for technology."
                            .to_string(),
                    );
                }
                rows.push(format!("Trader is traveling at Warp {}.", trader.warp));
                rows
            }
            ScanThing::Wormhole(index) => {
                let Some(hole) = game.wormholes.get(index) else {
                    return Vec::new();
                };
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
                vec![
                    format!("Location:  ({}, {})", hole.position.x, hole.position.y),
                    format!(
                        "Destination:  {}",
                        far.unwrap_or_else(|| "Unknown".to_string())
                    ),
                    format!("Stability:  {}", wormhole_stability(hole)),
                ]
            }
        }
    }

    /// The planet's headline rows: how good it is, who lives there, and how
    /// old the report is.
    #[must_use]
    pub fn survey_planet_rows(&self) -> Vec<(String, String)> {
        let (Some(planet), Some(race)) = (self.pane_planet(), self.pane_race()) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        if planet.detail != stars_core::planet::Detail::Minimal {
            rows.push((
                "Value:".to_string(),
                format!(
                    "{}%",
                    stars_core::hab::pct_planet_desirability(planet, race)
                ),
            ));
        }
        rows.push((
            "Population:".to_string(),
            match (planet.owner, planet.detail) {
                (None, _) => "Uninhabited".to_string(),
                (Some(_), stars_core::planet::Detail::Full) => {
                    comma_format(i64::from(planet.pop) * 100)
                }
                // A planet somebody else holds is only ever an estimate, and
                // one the game will not even guess at without a report.
                (Some(_), stars_core::planet::Detail::Scanned) => {
                    format!("~{}", comma_format(i64::from(planet.pop) * 100))
                }
                (Some(_), stars_core::planet::Detail::Minimal) => "???".to_string(),
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
            rows.push((String::new(), "Report is current".to_string()));
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
        ["Gravity", "Temperature", "Radiation"]
            .iter()
            .enumerate()
            .map(|(index, label)| SurveyBar {
                label: (*label).to_string(),
                value: env_text(index, planet.env[index]),
                at: i32::from(planet.env[index]),
                low: i32::from(race.env_min[index]),
                high: i32::from(race.env_max[index]),
                immune: race.is_immune(index),
            })
            .collect()
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
        ["Ironium", "Boranium", "Germanium"]
            .iter()
            .enumerate()
            .map(|(index, label)| SurveyBar {
                label: (*label).to_string(),
                value: format!("{}kT", planet.surface_min[index]),
                at: planet.surface_min[index],
                low: 0,
                high: i32::from(planet.min_conc[index]),
                immune: false,
            })
            .collect()
    }

    /// A fleet's summary: what it is, what it carries and where it is going.
    #[must_use]
    pub fn survey_fleet_rows(&self) -> Vec<String> {
        let SurveySubject::Fleet(index) = self.survey_subject() else {
            return Vec::new();
        };
        let Some(game) = self.game.as_ref() else {
            return Vec::new();
        };
        let Some(fleet) = game.fleets.get(index) else {
            return Vec::new();
        };
        let designs = game
            .designs
            .get(usize::try_from(fleet.owner).unwrap_or(usize::MAX))
            .map_or(&[][..], Vec::as_slice);

        let ships: i32 = fleet.stacks.iter().map(|s| s.count).sum();
        let mut rows = vec![format!("Ship Count: {ships}")];
        if !designs.is_empty() {
            rows.push(format!("Fleet Mass: {}kT", fleet.mass(designs)));
            rows.push(format!(
                "Fuel: {} of {}",
                fleet.cargo.fuel,
                fleet.fuel_capacity(designs)
            ));
        }
        let cargo: i32 = fleet.cargo.minerals.iter().sum::<i32>() + fleet.cargo.colonists;
        rows.push(format!("Cargo: {cargo}kT"));

        // Where it is going, what it will do there, and how fast.
        let next = fleet.waypoints.get(1);
        rows.push(format!(
            "Next Waypoint: {}",
            next.map_or_else(
                || "(none)".to_string(),
                |w| w.target.map_or_else(
                    || format!("({}, {})", w.position.x, w.position.y),
                    |id| format!("#{id}")
                )
            )
        ));
        if let Some(next) = next {
            rows.push(format!("Waypoint Task: {}", task_name(next.task)));
        }
        rows.push(match fleet.warp {
            Some(0) | None => "Warp Speed: (stopped)".to_string(),
            Some(warp) => format!("Warp Speed: {warp}"),
        });
        rows
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
fn task_name(task: u8) -> &'static str {
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

/// A distance in the words the game uses (`PszGetDistance`, `1038:3f00`).
///
/// The original works in **hundredths of a light year, rounded to nearest** —
/// `(long)(distance * 100 + 0.5)` — and then prints the whole and the remainder
/// with `%ld.%ld`.
///
/// That format has a quirk worth keeping, because it is the game's: the
/// remainder carries **no leading zero**, so three and five hundredths of a
/// light year reads `3.5`, not `3.05`.
#[must_use]
pub fn distance_text(from: stars_core::movement::Point, to: stars_core::movement::Point) -> String {
    #[allow(clippy::cast_possible_truncation)]
    let hundredths = (stars_core::movement::distance(from, to) * 100.0 + 0.5) as i64;
    format!("{}.{} l.y.", hundredths / 100, hundredths % 100)
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
        let starbase = self.designer.as_ref().is_some_and(|d| d.starbase);
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

    /// Who is browsing, for the costs and the requirements.
    #[must_use]
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

/// What the three kinds of minefield are called (`rgszMineFieldTypes`, the
/// table `Field Type:` is indexed into).
pub const MINEFIELD_KINDS: [&str; 3] = ["Mine Field", "Heavy Mine Field", "Speed Bump Field"];

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
    /// Surface minerals are scaled against [`MINERAL_GRAPH_MAX`] and
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
                    (planet.surface_min[mineral] + MINERAL_GRAPH_MAX / 40)
                        / (MINERAL_GRAPH_MAX / 20)
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
            .get(usize::from(design))
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

    /// What a space object is called, which is what the menu and the pane's
    /// title use.
    #[must_use]
    pub fn thing_name(&self, thing: ScanThing) -> String {
        let Some(game) = self.game.as_ref() else {
            return String::new();
        };
        match thing {
            ScanThing::Minefield(index) => match game.minefields.get(index) {
                Some(field) => {
                    let kind = MINEFIELD_KINDS
                        .get(usize::from(field.kind))
                        .copied()
                        .unwrap_or("Mine Field");
                    match usize::try_from(field.owner)
                        .ok()
                        .and_then(|owner| game.players.get(owner))
                    {
                        Some(player) => format!("{} {kind}", player.name),
                        None => kind.to_string(),
                    }
                }
                None => String::new(),
            },
            ScanThing::Packet(index) => match game.packets.get(index) {
                Some(packet) => match usize::try_from(packet.owner)
                    .ok()
                    .and_then(|owner| game.players.get(owner))
                {
                    Some(player) => format!("{} Mineral Packet", player.name),
                    None => "Mineral Packet".to_string(),
                },
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
    /// of ships at a location". On its way round it sets `fNoCount` on every
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
    /// `None` means the ordinary colour, which is what an **unowned** planet
    /// keeps: the original only reaches for a colour when the planet has an
    /// owner. `Some(None)` is white — this player's own. `Some(Some(p))` is
    /// player `p`'s colour.
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
    /// The original hides them below `iScanZoom > -2`, where the dots are too
    /// close together for a name to mean anything.
    #[must_use]
    pub fn planet_names_visible(&self) -> bool {
        self.scan_overlays.names && self.scan_zoom > -2
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

        // The options, named as the New Game wizard names them. Only the ones
        // that are on are listed, which is how the original's page reads.
        let options = [
            (game_flag::EXTRA_FUEL, "Maximum minerals"),
            (game_flag::SLOW_TECH, "Slower tech advances"),
            (game_flag::SINGLE_PLAYER, "One human player"),
            (game_flag::TUTORIAL, "Tutorial"),
            (game_flag::AIS_BAND, "Computer players are handicapped"),
            (game_flag::BBS_PLAY, "Public player (BBS) game"),
            (game_flag::VIS_SCORES, "Public player scores"),
            (game_flag::NO_RANDOM, "No random events"),
            (game_flag::CLUMPING, "Clumped planets"),
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
