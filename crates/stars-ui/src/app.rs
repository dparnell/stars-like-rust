//! The shared application state every frontend drives.
//!
//! This is deliberately free of any rendering: it holds the loaded game, what
//! the player has selected, and the derived facts the screens need, so that the
//! desktop and web shells can differ only in how they draw it.

use std::path::{Path, PathBuf};

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
        let Some(planet) = self.pane_planet() else {
            return Vec::new();
        };
        if planet.queue.is_empty() {
            return vec!["--- Queue is Empty ---".to_string()];
        }
        planet
            .queue
            .iter()
            .map(|entry| {
                let name = if entry.ship {
                    self.game
                        .as_ref()
                        .and_then(|g| g.designs.get(self.local_player()))
                        .and_then(|d| d.get(usize::from(entry.item)))
                        .filter(|d| d.hull_id >= 0 && !d.name.is_empty())
                        .map_or_else(|| format!("Design #{}", entry.item), |d| d.name.clone())
                } else {
                    crate::views::planets::item_name(entry.item)
                };
                format!("{} {}", entry.count, name)
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
            SurveySubject::Fleet(index) => {
                let name = self
                    .game
                    .as_ref()
                    .and_then(|g| g.fleets.get(index))
                    .map_or_else(String::new, |f| {
                        f.name.clone().unwrap_or_else(|| format!("Fleet #{}", f.id))
                    });
                format!("{name} Summary")
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
            (item::MINE, "mines"),
            (item::FACTORY, "factories"),
            (item::DEFENSE, "defences"),
            (item::ALCHEMY, "mineral alchemy"),
            (item::MAX_TERRAFORM, "terraforming"),
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
        assert!(!build_list_for(Prt::Ca).contains(&item::MAX_TERRAFORM));
        assert!(build_list_for(Prt::Ca).contains(&item::MINE));
        // An Alternate Reality race builds no planetary installation at all.
        let ar = build_list_for(Prt::Ar);
        assert!(!ar.contains(&item::MINE));
        assert!(!ar.contains(&item::FACTORY));
        assert!(!ar.contains(&item::DEFENSE));
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
