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
    /// Which of the scanner's six views is showing.
    pub scan_view: ScanView,
    /// The scanner's overlays and filters.
    pub scan_overlays: ScanOverlays,
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
    /// What the player screen's password box holds. Never saved anywhere: only
    /// its salt reaches the game, the file and the order log.
    pub password_box: String,
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
        let bytes =
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let file = StarsFile::decode(&bytes)
            .map_err(|e| format!("cannot decode {}: {e}", path.display()))?;

        let (mut state, _) = GameState::from_file(&file);
        let universe = find_universe(path);
        if let Some(universe) = &universe {
            state.apply_universe(universe);
        }

        let header = &file.latest_segment().header;
        let layout = ActionLayout::for_version(header.version_major, header.version_minor);
        self.battles = battle_records_in_with(file.segment_blocks(file.latest_segment()), layout);

        self.selection = Selection {
            planet: state.planets.first().map(|p| p.id),
            fleet: (!state.fleets.is_empty()).then_some(0),
        };
        self.vcr = None;
        self.playing = false;
        self.game = Some(state);
        self.universe = universe;
        self.setup = None;
        self.path = Some(path.to_path_buf());
        self.file = Some(file);
        self.dirty = false;
        self.edited.clear();
        self.renamed.clear();
        self.fleet_edits.clear();
        self.player_edited = false;
        self.battle_plans_edited = false;
        self.orders.clear();
        self.error = None;
        // `ReadPlayerMessages` (`msg.c`) ends by moving the pane to the first
        // message the player has not filtered, which leaves it before the
        // start when every one of them is.
        self.view_filtered = false;
        self.show_first_message();
        Ok(())
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
            if block.type_id == 6 && self.player_edited {
                if let Some(patched) = self.patched_player(game, &block.data) {
                    blocks.push(
                        Block::new(6, patched)
                            .map_err(|e| format!("cannot write a player: {e}"))?,
                    );
                    continue;
                }
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

    /// The **fleets in orbit** tile.
    #[must_use]
    pub fn planet_fleets_tile(&self) -> Vec<String> {
        let (Some(game), Some(planet)) = (self.game.as_ref(), self.pane_planet()) else {
            return Vec::new();
        };
        let Some(at) = planet.position else {
            return Vec::new();
        };
        game.fleets
            .iter()
            .filter(|f| f.position == at && !f.stacks.is_empty())
            .map(|f| {
                let ships: i32 = f.stacks.iter().map(|s| s.count).sum();
                let name = f.name.clone().unwrap_or_else(|| format!("Fleet #{}", f.id));
                format!("{name} ({ships})")
            })
            .collect()
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
        if self.screen == Screen::Fleets {
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
fn env_text(variable: usize, clicks: i8) -> String {
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
            picture: 0,
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
    /// do. Ships have thirty-two pictures and starbases five.
    pub fn designer_next_picture(&mut self, forward: bool) {
        let count: u8 = if self.designer.as_ref().is_some_and(|d| d.starbase) {
            5
        } else {
            32
        };
        if let Some(editing) = self.designer.as_mut().and_then(|d| d.editing.as_mut()) {
            let picture = i32::from(editing.design.picture);
            let step = if forward { 1 } else { i32::from(count) - 1 };
            editing.design.picture = u8::try_from((picture + step) % i32::from(count)).unwrap_or(0);
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
