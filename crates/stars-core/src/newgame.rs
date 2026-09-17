//! Creating a game from nothing: universe generation, homeworlds and starting
//! fleets.
//!
//! This is a transcription of `GenerateWorld` (`create.c`), the routine behind
//! the original's New Game wizard. It runs in five stages:
//!
//! 1. **Scatter and thin.** Rather more planets than the universe wants are
//!    thrown down uniformly, any that land within twelve light years of another
//!    are removed, and the survivors are culled at random down to the target
//!    count. An optional clumping pass then drags planets toward their nearest
//!    neighbour, which is what makes a "clumped" universe look clustered.
//! 2. **Name them**, by drawing without replacement from the 999-entry master
//!    name table.
//! 3. **Give each one an environment and mineral concentrations.** Gravity and
//!    temperature are the sum of two uniform draws, so they cluster around the
//!    middle; radiation is flat. Roughly a third of planets have one mineral
//!    deliberately impoverished.
//! 4. **Place the homeworlds**, first player near the middle and the rest in a
//!    band of distances from each other that the "distance between players"
//!    setting scales. If no arrangement fits, the band widens and the whole
//!    placement restarts.
//! 5. **Set the players up**: starting technology from the primary racial
//!    trait, a homeworld at the exact centre of the race's habitable range with
//!    ten mines, ten factories, ten defences and a starbase, the leftover
//!    advantage points spent on it, and a handful of starting ships.
//!
//! Every planet, homeworld and starting design this produces has been checked
//! against `fixtures/incoming/turn0/`, a real turn-0 game — see
//! `docs/formulas/new-game.md` for what matched and what could not be checked.
//!
//! ## What is deliberately not reproduced
//!
//! * **The Mystery Trader**, which sets out on its own schedule from the
//!   turn generator rather than the generator of the world.
//!
//! ## Seed-identical universes: they are
//!
//! This module used to say they were not, on the grounds that the original
//! sorts its scratch array with `qsort`, whose permutation of equal x
//! coordinates the C standard leaves unspecified. That reasoning was sound
//! and the conclusion was wrong: unspecified by the standard is not the same
//! as undetermined, the runtime is statically linked into the program, and
//! whatever it does it does the same way every time.
//!
//! `crates/stars-core/tests/tutorial_seed.rs` settles it. The tutorial's
//! universe is the one real galaxy whose seed is knowable —
//! `CreateTutorWorld` (`1078:5e5e`) calls `Randomize` with a constant — and
//! generating from that seed reproduces `fixtures/games/tutorial/tutorial.xy`
//! **exactly**: all 24 planets, the same coordinates, the same names.
//!
//! No other fixture can be used the same way, and not because the generator
//! would fail on it: a `.xy` stores its settings but never its seed. An
//! ordinary new game seeds from the clock, so its universe is unreproducible
//! in principle. That is why the claim went untested for so long — the one
//! oracle that exists is the tutorial's.

use crate::advantage::advantage_points;
use crate::ai::Control;
use crate::design::ShipDesign;
use crate::fleet::{Cargo, Fleet, Waypoint};
use crate::hab::pct_planet_desirability;
use crate::movement::Point;
use crate::planet::{Detail, Planet};
use crate::race::{lrt, Prt, Race, RaceStat, STAT_MAX, STAT_MIN};
use crate::research::NextField;
use crate::rng::Rng;
use crate::startup::{self, ship, starbase, upgrade_slots};
use crate::{GameState, Player};

use stars_formats::{game_flag, FileHeader, FileType, GameInfo, Universe};

/// The border every coordinate is measured from (`dGalOff`).
///
/// The same 1000 that [`stars_formats::xy::X_BASE`] documents from the other
/// end: the `.xy` stores x as deltas from it.
pub const GALAXY_OFFSET: i16 = 1000;

/// Closest two planets may be generated (`dGalMinDist`), in light years.
pub const MIN_PLANET_DISTANCE: i16 = 12;

/// The hard ceiling on planets (`cPlanetAbsMax`).
pub const MAX_PLANETS: i16 = 999;

/// How many planet names the master table holds.
const NAME_COUNT: i16 = 999;

/// Universe size (`GAME.mdSize`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i16)]
pub enum Size {
    /// 400 light years across.
    Tiny = 0,
    /// 800 light years across.
    #[default]
    Small = 1,
    /// 1200 light years across.
    Medium = 2,
    /// 1600 light years across.
    Large = 3,
    /// 2000 light years across.
    Huge = 4,
}

impl Size {
    /// Every size, smallest first.
    pub const ALL: [Size; 5] = [
        Size::Tiny,
        Size::Small,
        Size::Medium,
        Size::Large,
        Size::Huge,
    ];

    /// The name the game shows.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Size::Tiny => "Tiny",
            Size::Small => "Small",
            Size::Medium => "Medium",
            Size::Large => "Large",
            Size::Huge => "Huge",
        }
    }

    /// Width of the universe in light years (`dGal`).
    #[must_use]
    pub fn span(self) -> i16 {
        self as i16 * 400 + 400
    }
}

/// Planet density (`GAME.mdDensity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i16)]
pub enum Density {
    /// A quarter fewer planets than normal.
    Sparse = 0,
    /// The baseline.
    #[default]
    Normal = 1,
    /// A quarter more.
    Dense = 2,
    /// Half again, and then a further quarter.
    Packed = 3,
}

impl Density {
    /// Every density, sparsest first.
    pub const ALL: [Density; 4] = [
        Density::Sparse,
        Density::Normal,
        Density::Dense,
        Density::Packed,
    ];

    /// The name the game shows.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Density::Sparse => "Sparse",
            Density::Normal => "Normal",
            Density::Dense => "Dense",
            Density::Packed => "Packed",
        }
    }
}

/// How far apart the players start (`GAME.mdStartDist`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i16)]
pub enum StartDistance {
    /// Neighbours are close.
    Close = 1,
    /// The default spacing.
    #[default]
    Moderate = 2,
    /// As far apart as the universe allows.
    Distant = 3,
}

impl StartDistance {
    /// Every setting, closest first.
    pub const ALL: [StartDistance; 3] = [
        StartDistance::Close,
        StartDistance::Moderate,
        StartDistance::Distant,
    ];

    /// The name the game shows.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            StartDistance::Close => "Close",
            StartDistance::Moderate => "Moderate",
            StartDistance::Distant => "Distant",
        }
    }
}

/// How many planets a universe of this size and density holds (`cPlanMax`).
///
/// Verified exactly against all nine distinct `.xy` fixtures, which between
/// them cover sizes 0-2 and every density.
#[must_use]
pub fn planet_count(size: Size, density: Density) -> i16 {
    let span = i32::from(size.span());
    let mut count = (span * span / 5000) as i16;
    count += (count / 4) * (density as i16 - 1);
    if density as i16 >= 3 {
        count += count / 4;
    }
    count.min(MAX_PLANETS)
}

/// One player in a game about to be created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPlayer {
    /// The race they play.
    pub race: Race,
    /// Whether a person or the computer plays the slot.
    pub control: Control,
    /// The race's singular name, which names the player too.
    pub name: String,
    /// The race's plural name.
    pub plural_name: String,
}

impl NewPlayer {
    /// A human player of `race`, named "Humanoid".
    #[must_use]
    pub fn human(race: Race) -> Self {
        Self {
            race,
            control: Control::Human,
            name: "Humanoid".to_string(),
            plural_name: "Humanoids".to_string(),
        }
    }

    /// A computer player of `race`, named "Humanoid".
    #[must_use]
    pub fn computer(race: Race) -> Self {
        Self {
            race,
            control: Control::Computer {
                personality: None,
                skill_bits: 0,
            },
            name: "Humanoid".to_string(),
            plural_name: "Humanoids".to_string(),
        }
    }
}

/// Everything the New Game wizard asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewGame {
    /// The game's name, as it appears in the `.xy`.
    pub name: String,
    /// The per-game id (`GAME.lid`), which seeds the universe and every
    /// file's cipher. `GenerateWorld` (`1078:43cc`) takes `GetTickCount()`
    /// for it — the clock's milliseconds — so the wizard fills it from the
    /// shell's clock and lets it be typed over; the default here is one
    /// fixed value, for tests that want the same universe every time.
    pub id: u32,
    /// Universe size.
    pub size: Size,
    /// Planet density.
    pub density: Density,
    /// Distance between players' homeworlds.
    pub start_distance: StartDistance,
    /// Clump the planets rather than scattering them evenly.
    pub clumping: bool,
    /// Allow wormholes, artifacts and the Mystery Trader (`!fNoRandom`).
    pub random_events: bool,
    /// Slower tech advances: research costs double.
    pub slow_tech: bool,
    /// Every planet has 100% mineral concentrations (`fExtraFuel`).
    pub unlimited_minerals: bool,
    /// Scores are public (`fVisScores`).
    pub public_scores: bool,
    /// **Accelerated BBS play** (bit 5 of the flag word), which gives every
    /// player a head start. `GenerateWorld` (`1078:0136`) does three things
    /// with it: a mineral concentration under 40 gets five more, the stock
    /// of surface minerals every homeworld starts with is a quarter larger,
    /// and each homeworld's people are multiplied by
    /// `(2 × growth rate + 10) / 10` — four times, for a 15% race. The
    /// tutorial's world is set up this way, which is why its home planet
    /// starts with 100,000 colonists and builds twenty factories in two
    /// years.
    pub accelerated: bool,
    /// The game is **the tutorial** (`fTutorial`, bit 3 of the flag word).
    /// The one thing the engine reads it for: a Jack of All Trades' built-in
    /// scanner is a fixed 40/20 rather than scaled by Electronics
    /// (`GetShdefScannerRange`, `1038:50d0`).
    pub tutorial_game: bool,
    /// The players, in order.
    pub players: Vec<NewPlayer>,
    /// The twenty-four names a random race may be given — the string
    /// table from `idsBerserker` (`0x56e`) on — which the caller reads out
    /// of the game's own resources. Empty, a random race keeps the name it
    /// was handed.
    pub random_names: Vec<String>,
    /// The victory conditions, as `GAME.rgvc` holds them — bit 7 of each
    /// byte whether the condition counts, the rest its setting (see
    /// [`stars_formats::GameInfo::victory_value`]). `None` is the New
    /// Game dialog's own defaults ([`default_victory`]), with the least
    /// years set from the universe size as `InitNewGamePlr` sets it.
    pub victory: Option<[u8; stars_formats::victory::COUNT]>,
}

/// The opponents the simple New Game dialog fills a game with
/// (`InitNewGamePlr`, `1078:6e44`), for a universe of `size` at
/// `difficulty` 0 (easy) to 3 (expert): how many players the game has, and
/// for each computer player their personality and level as
/// `(personality, level)`, with `None` where the original leaves the
/// choice to chance (`0x9b`: a random personality at a random level).
///
/// The count: a tiny universe gets 2 players (3 one time in three at
/// expert); small 3, or 4 one time in `6 − difficulty` at standard and up,
/// or 5 one time in four at expert; medium 7, 6 or 8 by two draws of
/// `Random(7 − difficulty)`, and at expert 9 or 5 one time in ten each;
/// large 12, 11 or 13 the same way, 14 to 15 or 9 to 10 one time in ten at
/// expert; huge 16, 15 or 14, or 11 to 13 one time in ten at expert. The
/// computer players are then dealt personalities by their place in the
/// list — the original's thresholds, kept as they are — and shuffled.
///
/// Returns the computer players in order, each `(personality, level)`,
/// `None` in either place where the byte says to draw one — a personality
/// of 6 (`Random(6)` when the game is made) or a level past 3
/// (`Random(4)`), which is `NewGameWizard`'s reading of the byte.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn simple_game_opponents(
    size: Size,
    difficulty: usize,
    rng: &mut Rng,
) -> Vec<(Option<usize>, Option<usize>)> {
    let roll = |rng: &mut Rng, n: i32| i32::from(rng.random(i16::try_from(n).unwrap_or(1)));
    let level = i32::try_from(difficulty).unwrap_or(0);
    let expert = difficulty == 3;
    let count: i32 = match size {
        Size::Tiny => {
            if expert && roll(rng, 3) == 0 {
                3
            } else {
                2
            }
        }
        Size::Small => {
            if expert && roll(rng, 4) == 0 {
                5
            } else if level < 2 || roll(rng, 6 - level) != 0 {
                3
            } else {
                4
            }
        }
        Size::Medium => {
            if expert && roll(rng, 10) == 0 {
                9
            } else if expert && roll(rng, 10) == 0 {
                5
            } else if level < 2 || roll(rng, 7 - level) != 0 {
                if level < 2 || roll(rng, 7 - level) != 0 {
                    7
                } else {
                    6
                }
            } else {
                8
            }
        }
        Size::Large => {
            if expert && roll(rng, 10) == 0 {
                roll(rng, 2) + 14
            } else if expert && roll(rng, 10) == 0 {
                10 - roll(rng, 2)
            } else if level < 2 || roll(rng, 7 - level) != 0 {
                if level < 2 || roll(rng, 7 - level) != 0 {
                    12
                } else {
                    11
                }
            } else {
                13
            }
        }
        Size::Huge => {
            if expert && roll(rng, 10) == 0 {
                13 - roll(rng, 3)
            } else if level < 2 || roll(rng, 9 - level) != 0 {
                if level < 2 || roll(rng, 7 - level) != 0 {
                    16
                } else {
                    15
                }
            } else {
                14
            }
        }
    };

    // The type bytes: `personality << 2 | 3`, with the level in the top
    // three bits: a personality of 6 or a level of 4 is a draw.
    let byte = |p: usize, l: usize| -> (Option<usize>, Option<usize>) {
        ((p < 6).then_some(p), (l < 4).then_some(l))
    };
    let mut types: Vec<(Option<usize>, Option<usize>)> = Vec::new();
    for i in 1..count {
        let t = match difficulty {
            0 => {
                if i < (count + 1) / 3 + 1 {
                    byte(2, 0)
                } else if i < (count + 1) * 2 / 3 + 1 {
                    byte(3, 0)
                } else if i < (count + 1) * 5 / 6 + 1 {
                    byte(1, 0)
                } else {
                    byte(6, 0)
                }
            }
            1 => {
                if i < (count + 5) * 2 / 7 + 1 {
                    byte(1, 1)
                } else if i < ((count - 1) * 3 + 6) / 7 + 1 {
                    byte(0, 1)
                } else if i < ((count - 1) * 4 + 6) / 7 + 1 {
                    byte(2, 1)
                } else if i < ((count - 1) * 5 + 6) / 7 + 1 {
                    byte(3, 1)
                } else if i < ((count - 1) * 6 + 6) / 7 + 1 {
                    byte(4, 1)
                } else {
                    byte(6, 4)
                }
            }
            2 => {
                if i < (count + 5) * 2 / 7 + 1 {
                    byte(4, 2)
                } else if i < ((count - 1) * 3 + 6) / 7 + 1 {
                    byte(1, 2)
                } else if i < ((count - 1) * 4 + 6) / 7 + 1 {
                    byte(0, 2)
                } else if i < ((count - 1) * 5 + 6) / 7 + 1 {
                    byte(5, 2)
                } else if i < ((count - 1) * 6 + 6) / 7 + 1 {
                    byte(6, 2)
                } else {
                    byte(6, 4)
                }
            }
            _ => {
                if i < (count + 1) / 3 + 1 {
                    byte(0, 3)
                } else if i < ((count - 1) * 6 + 11) / 12 + 1 {
                    byte(5, 3)
                } else if i < ((count - 1) * 5 + 5) / 6 + 1 {
                    byte(4, 3)
                } else {
                    byte(6, 3)
                }
            }
        };
        types.push(t);
    }
    // The shuffle: each place but the last swaps with one further on.
    let n = types.len();
    for i in 0..n.saturating_sub(1) {
        let span = i32::try_from(n - i - 1).unwrap_or(1);
        let j = i + 1 + usize::try_from(roll(rng, span)).unwrap_or(0);
        if j < n {
            types.swap(i, j);
        }
    }
    types
}

/// The victory conditions the New Game wizard starts with
/// (`NewGameWizard`, `1078:6022`, written the moment the first dialog
/// returns): owning 60% of the planets, tech 22 in 4 fields and a score
/// twice the second player's, all counting; a score of 11,000, 100,000
/// resources a year, 100 capital ships and the highest score after 100
/// years set but not counting; one of them enough to win; and the least
/// years `InitNewGamePlr`'s `2 × size` — 30 for a tiny universe, 10 more
/// for each size up.
#[must_use]
pub fn default_victory(size: Size) -> [u8; stars_formats::victory::COUNT] {
    let mut bytes = [0u8; stars_formats::victory::COUNT];
    bytes[..9].copy_from_slice(&[0x88, 0x8e, 0x82, 0x0a, 0x88, 0x09, 0x09, 0x07, 0x01]);
    #[allow(clippy::cast_possible_truncation)]
    {
        bytes[stars_formats::victory::LEAST_YEARS] = (size as u8) << 1;
    }
    bytes
}

impl Default for NewGame {
    fn default() -> Self {
        Self {
            name: "New Game".to_string(),
            id: 0x2a03_1dd8,
            size: Size::default(),
            density: Density::default(),
            start_distance: StartDistance::default(),
            clumping: false,
            random_events: true,
            slow_tech: false,
            unlimited_minerals: false,
            public_scores: false,
            accelerated: false,
            tutorial_game: false,
            players: vec![NewPlayer::human(Race::humanoid())],
            random_names: Vec::new(),
            victory: None,
        }
    }
}

/// Why a game could not be created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewGameError {
    /// A game needs at least one and at most sixteen players.
    PlayerCount(usize),
    /// The universe file could not be assembled.
    Universe(String),
}

impl std::fmt::Display for NewGameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlayerCount(n) => {
                write!(f, "a game needs 1 to 16 players, not {n}")
            }
            Self::Universe(message) => write!(f, "cannot build the universe file: {message}"),
        }
    }
}

impl std::error::Error for NewGameError {}

/// A freshly created game.
#[derive(Debug, Clone)]
pub struct Created {
    /// The game state, at turn 0 (year 2400).
    pub state: GameState,
    /// The matching universe, ready to write as a `.xy`.
    pub universe: Universe,
}

/// The largest number of players a game holds.
pub const MAX_PLAYERS: usize = 16;

/// Create a universe and a starting position for every player.
///
/// # Errors
///
/// [`NewGameError::PlayerCount`] if the player list is empty or longer than
/// [`MAX_PLAYERS`]; [`NewGameError::Universe`] if the generated positions
/// cannot be packed into a `.xy` (which would be a bug here, not bad input).
pub fn generate(config: &NewGame, rng: &mut Rng) -> Result<Created, NewGameError> {
    let players = config.players.len();
    if players == 0 || players > MAX_PLAYERS {
        return Err(NewGameError::PlayerCount(players));
    }

    let positions = scatter_planets(config, rng);
    let names = name_planets(positions.len(), rng);
    let mut planets = describe_planets(config, &positions, &names, rng);
    let homes = place_homeworlds(config, &positions, rng);
    let (state_players, homes) = settle_players(config, &mut planets, &positions, homes, rng);
    let (fleets, designs) = starting_ships(config, &planets, &positions, &homes);
    let wormholes = place_wormholes(config, &positions, &fleets, rng);

    let mut state = GameState::new(config.id);
    state.slow_tech = config.slow_tech;
    state.random_events = config.random_events;
    state.public_scores = config.public_scores;
    state.tutorial_game = config.tutorial_game;
    state.galaxy_size = config.size as i16;
    state.start_distance = config.start_distance as i16;
    state.victory = config
        .victory
        .unwrap_or_else(|| default_victory(config.size));
    state.galaxy_planets = i16::try_from(positions.len()).unwrap_or(i16::MAX);
    state.planets = planets;
    state.players = state_players;
    state.fleets = fleets;
    state.designs = designs;
    state.wormholes = wormholes;
    // `CreateTutorWorld` (`1078:5e5e`) does not run the starting-tech rule
    // for the Berserkers: `tutorial.hst` holds them at Electronics 5 and
    // nothing else, one short of the Propulsion their Improved Fuel
    // Efficiency would give a race in an ordinary new game.
    if config.tutorial_game {
        for player in state.players.iter_mut().filter(|p| p.control.is_computer()) {
            player.research.levels = [0, 0, 0, 0, 5, 0];
        }
    }
    opening_messages(&mut state, &homes);

    let universe = build_universe(config, &positions, &names)?;
    Ok(Created { state, universe })
}

/// What the wizard's "Random" race is called in the string table
/// (`idsRandom2`, `0x532`); a random race so named draws a real one.
pub const RANDOM_RACE_NAME: &str = "Random";
/// How many names a random race chooses from (`Random(0x18)` over
/// `idsBerserker`).
pub const RANDOM_RACE_NAMES: usize = 24;
/// The most tries at balancing a random race's points before giving up
/// and handing out the stock Humanoid instead (`CreateRandomRace`,
/// `0xfb`).
const RANDOM_RACE_TRIES: i32 = 0xfb;
/// The economy a third of random races start from: the Humanoid's
/// (`1120:0de0`), with the leftover policy rolled separately.
const RANDOM_RACE_STOCK_ECONOMY: [i16; 7] = [10, 10, 10, 10, 10, 5, 10];

/// `CreateRandomRace` (`10e0:5b08`): roll a race for a player whose race
/// is the wizard's "Random", and bring its advantage points into
/// `0..=50`.
///
/// The habitability is one of four shapes on `Random(25)`: under 4, immune
/// to all three with a growth of `2 + Random(4)`; under 7, the whole
/// spectrum on all three with `3 + Random(4)`; under 9, each of the first
/// two axes either the whole spectrum or left as it was, the third left
/// as it was, with `2 + Random(5)`; otherwise each axis a band
/// `20 + 2 × Random(40)` wide placed at random, then under 12 one axis
/// made immune, under 14 one made whole-spectrum, under 17 one narrowed
/// to twenty wide, with a growth of `7 + Random(9)`. The six research
/// settings are all normal one time in three, else each `Random(3)`; the
/// primary trait `Random(10)`; the fourteen lesser traits all off one time
/// in four, else each a coin; Expensive Tech Starts at 3 and Cheap
/// Factories a coin each; the economy the Humanoid's one time in three,
/// with a leftover policy of `Random(5)`, else each statistic anywhere in
/// its wizard range. A race still called "Random" takes one of the
/// twenty-four names.
///
/// Then, while the points are outside `0..=50`, up to 251 nudges, each
/// kept only when it brings the points nearer the range: a research
/// setting down or up (`Random(10) < 3`), a lesser trait off or on
/// (`< 6`), an economy statistic down or up (`< 9`), or, on a coin, an
/// axis made immune or an immune axis given a seventy-wide band, else the
/// growth down or up. After the 251st the race is the stock Humanoid under
/// the name it had.
///
/// Returns the name chosen, if the race was still called "Random".
pub fn random_race(race: &mut Race, rng: &mut Rng, name: &str, names: &[String]) -> Option<String> {
    let roll = |rng: &mut Rng, n: i16| i32::from(rng.random(n));
    let set_axis = |race: &mut Race, axis: usize, low: i32, high: i32| {
        #[allow(clippy::cast_possible_truncation)]
        {
            race.env_min[axis] = low as i8;
            race.env_max[axis] = high as i8;
            race.env_center[axis] = (low + (high - low) / 2) as i8;
        }
    };
    let immune = |race: &mut Race, axis: usize| {
        race.env_min[axis] = -1;
        race.env_max[axis] = -1;
        race.env_center[axis] = -1;
    };

    let shape = roll(rng, 25);
    if shape < 4 {
        for axis in 0..3 {
            immune(race, axis);
        }
        #[allow(clippy::cast_possible_truncation)]
        {
            race.pct_ideal_growth = (roll(rng, 4) + 2) as i8;
        }
    } else if shape < 7 {
        for axis in 0..3 {
            set_axis(race, axis, 0, 100);
        }
        #[allow(clippy::cast_possible_truncation)]
        {
            race.pct_ideal_growth = (roll(rng, 4) + 3) as i8;
        }
    } else if shape < 9 {
        for axis in 0..3 {
            let mut coin = roll(rng, 2);
            if axis == 2 && race.env_center[0] == race.env_center[1] {
                coin = i32::from(race.env_center[0] != 0);
            }
            if coin == 0 {
                set_axis(race, axis, 0, 100);
            } else {
                #[allow(clippy::cast_possible_truncation)]
                {
                    race.pct_ideal_growth = (roll(rng, 4) + 2) as i8;
                }
            }
        }
        #[allow(clippy::cast_possible_truncation)]
        {
            race.pct_ideal_growth = (roll(rng, 5) + 2) as i8;
        }
    } else {
        for axis in 0..3 {
            let width = roll(rng, 0x28) * 2 + 0x14;
            let low = roll(rng, i16::try_from(0x65 - width).unwrap_or(1));
            set_axis(race, axis, low, low + width);
        }
        if shape < 0xc {
            let axis = usize::try_from(roll(rng, 3)).unwrap_or(0);
            immune(race, axis);
        } else if shape < 0xe {
            let axis = usize::try_from(roll(rng, 3)).unwrap_or(0);
            set_axis(race, axis, 0, 100);
        } else if shape < 0x11 {
            let axis = usize::try_from(roll(rng, 3)).unwrap_or(0);
            let low = roll(rng, 0x51);
            set_axis(race, axis, low, low + 0x14);
        }
        #[allow(clippy::cast_possible_truncation)]
        {
            race.pct_ideal_growth = (roll(rng, 9) + 7) as i8;
        }
    }

    let set_stat = |race: &mut Race, stat: usize, value: i32| {
        #[allow(clippy::cast_possible_truncation)]
        {
            race.attrs[stat] = (value as i16).clamp(STAT_MIN[stat], STAT_MAX[stat]);
        }
    };
    let all_normal = roll(rng, 3) == 0;
    for stat in RaceStat::TechBonus1 as usize..RaceStat::MajorAdv as usize {
        let value = if all_normal { 1 } else { roll(rng, 3) };
        set_stat(race, stat, value);
    }
    set_stat(race, RaceStat::MajorAdv as usize, roll(rng, 10));
    let all_off = roll(rng, 4) == 0;
    for bit in 0..14u32 {
        let on = if all_off { 0 } else { roll(rng, 2) };
        race.lrt_bits = (race.lrt_bits & !(1 << bit)) | (u32::try_from(on).unwrap_or(0) << bit);
    }
    for bit in [lrt::TECH3, lrt::CHEAP_FACT] {
        let on = roll(rng, 2);
        race.lrt_bits = (race.lrt_bits & !(1 << bit)) | (u32::try_from(on).unwrap_or(0) << bit);
    }
    if roll(rng, 3) == 0 {
        for (stat, value) in RANDOM_RACE_STOCK_ECONOMY.iter().enumerate() {
            race.attrs[stat] = *value;
        }
        set_stat(race, RaceStat::UseLeftover as usize, roll(rng, 5));
    } else {
        for stat in 0..8 {
            let span = i32::from(STAT_MAX[stat]) + 1 - i32::from(STAT_MIN[stat]);
            let value = i32::from(STAT_MIN[stat]) + roll(rng, i16::try_from(span).unwrap_or(1));
            set_stat(race, stat, value);
        }
    }
    let chosen = if name == RANDOM_RACE_NAME {
        let pick =
            usize::try_from(roll(rng, i16::try_from(RANDOM_RACE_NAMES).unwrap_or(1))).unwrap_or(0);
        names.get(pick).cloned()
    } else {
        None
    };

    // How far outside `0..=50` the points are.
    let away = |race: &Race| {
        let points = i32::from(advantage_points(race));
        (points - 50).max(-points)
    };
    let mut tries = 0;
    loop {
        let now = away(race);
        if now <= 0 {
            return chosen;
        }
        if tries >= RANDOM_RACE_TRIES {
            // Give up: the first predefined race (`vrgplrDef[0]`, the
            // Humanoid) under whatever name it had.
            *race = crate::presets::ALL[0].race.clone();
            return chosen;
        }
        tries += 1;
        let what = roll(rng, 10);
        if what < 3 {
            let stat = RaceStat::TechBonus1 as usize + usize::try_from(roll(rng, 6)).unwrap_or(0);
            let was = race.attrs[stat];
            let mut settled = false;
            if was > 0 {
                race.attrs[stat] = was - 1;
                if away(race) < now {
                    settled = true;
                } else {
                    race.attrs[stat] = was;
                }
            }
            if !settled && was < 2 {
                race.attrs[stat] = was + 1;
                if away(race) >= now {
                    race.attrs[stat] = was;
                }
            }
        } else if what < 6 {
            let bit = u32::try_from(roll(rng, 14)).unwrap_or(0);
            let was = race.lrt_bits;
            let mut kept = false;
            for on in 0..2u32 {
                race.lrt_bits = (was & !(1 << bit)) | (on << bit);
                if away(race) < now {
                    kept = true;
                    break;
                }
            }
            if !kept {
                race.lrt_bits = was;
            }
        } else if what < 9 {
            let stat = usize::try_from(roll(rng, 7)).unwrap_or(0);
            let was = race.attrs[stat];
            let mut kept = false;
            for step in [-1, 1] {
                set_stat(race, stat, i32::from(was) + step);
                if away(race) < now {
                    kept = true;
                    break;
                }
            }
            if !kept {
                race.attrs[stat] = was;
            }
        } else if roll(rng, 2) == 0 {
            let axis = usize::try_from(roll(rng, 3)).unwrap_or(0);
            let before = race.clone();
            if race.env_center[axis] < 0 {
                let low = roll(rng, 0x1f);
                set_axis(race, axis, low, low + 0x46);
            } else {
                immune(race, axis);
            }
            if away(race) >= now {
                *race = before;
            }
        } else {
            let was = race.pct_ideal_growth;
            if was > 1 {
                race.pct_ideal_growth = was - 1;
                if away(race) < now {
                    continue;
                }
            }
            if was < 15 {
                race.pct_ideal_growth = was + 1;
                if away(race) < now {
                    continue;
                }
            }
            race.pct_ideal_growth = was;
        }
    }
}

/// How many wormholes a universe of each size starts with, at least
/// (`vrgWormholeMin`, `1078:0000`) and the span of the roll above that
/// (`vrgWormholeVar`, `1078:0006`), by size tiny to huge.
pub const WORMHOLES_MIN: [i16; 5] = [0, 1, 1, 3, 4];
/// See [`WORMHOLES_MIN`].
pub const WORMHOLES_VAR: [i16; 5] = [3, 3, 5, 4, 5];

/// The wormholes a new universe starts with — `GenerateWorld` after the
/// battle plans (`create.c`, the "wormhole creation loop").
///
/// None in a game without random events. Otherwise
/// `min[size] + Random(var[size])` pairs, each end a `THING` of kind
/// wormhole with a stability of `Random(3)` and the other end as its
/// partner; each is placed by up to a hundred tries at
/// `(Random(dGal) + 1000, Random(dGal) + 1000)`, the first spot
/// [`crate::wormhole::position_score`] calls perfect, else the best seen.
/// The first end of a pair is scored before its partner exists.
fn place_wormholes(
    config: &NewGame,
    positions: &[(i16, i16)],
    fleets: &[Fleet],
    rng: &mut Rng,
) -> Vec<crate::wormhole::Wormhole> {
    use crate::wormhole::{position_score, Wormhole};

    if !config.random_events {
        return Vec::new();
    }
    let size = config.size as usize;
    let count = WORMHOLES_MIN[size] + rng.random(WORMHOLES_VAR[size]);
    let span = config.size.span();
    let planets: Vec<Point> = positions.iter().map(|&(x, y)| Point::new(x, y)).collect();
    let fleet_spots: Vec<Point> = fleets.iter().map(|f| f.position).collect();
    // A wormhole's `idFull` is its kind, 2, in the top three bits over its
    // number.
    let id_full = |id: u16| (2u16 << 13) | id;

    let mut holes: Vec<Wormhole> = Vec::new();
    for pair in 0..count {
        let first = u16::try_from(pair * 2).unwrap_or(0);
        for end in 0..2u16 {
            let id = first + end;
            let partner = if end == 1 { first } else { u16::MAX };
            let stability = u8::try_from(rng.random(3)).unwrap_or(0);
            let others: Vec<(u16, Point)> = holes.iter().map(|w| (w.id, w.position)).collect();
            let mut best: Option<(u8, Point)> = None;
            let mut at = Point::new(1000, 1000);
            for _ in 0..100 {
                at = Point::new(
                    i16::try_from(i32::from(rng.random(span)) + 1000).unwrap_or(i16::MAX),
                    i16::try_from(i32::from(rng.random(span)) + 1000).unwrap_or(i16::MAX),
                );
                let score = position_score(
                    at,
                    i32::from(config.size as i16),
                    partner,
                    &others,
                    &planets,
                    &fleet_spots,
                );
                if score == 0 {
                    best = None;
                    break;
                }
                if best.is_none_or(|(worst, _)| score < worst) {
                    best = Some((score, at));
                }
            }
            if let Some((_, spot)) = best {
                at = spot;
            }
            holes.push(Wormhole {
                id,
                position: at,
                stability,
                years_still: 0,
                dest_known: false,
                include: true,
                detected_by: 0,
                traversed_by: 0,
                partner: if end == 1 { id_full(first) } else { 0 },
                turn: 0,
            });
        }
        // The first end learns its partner once the second exists.
        let last = holes.len() - 1;
        holes[last - 1].partner = id_full(first + 1);
    }
    holes
}

/// What a new game has to say before anyone has done anything.
///
/// `GenerateWorld` (`1078:0136`) sends every player the four playing tips,
/// `idm 0x7f` to `0x82` with no object, in one loop, and then in the next —
/// the one that settles each home world — `idmHomePlanetPeopleReadyLeaveNestExplore`
/// (`0xa9`) with the home planet as both object and parameter. So a player's
/// first year opens with five messages, which is what the turn-0 fixture's
/// `Game.m1` carries and what the tutorial's first page asks you to read.
///
/// The original's AI player file from the same year carries none of them,
/// so the writer may well drop them for a computer player; they are queued
/// for every player here, as the routine queues them, and the writer is
/// left to decide.
fn opening_messages(state: &mut GameState, homes: &[usize]) {
    use crate::message::{id, Message};
    for player in 0..state.players.len() {
        for tip in [
            id::TIP_FILTERING,
            id::TIP_WAYPOINTS,
            id::TIP_DESIGNER,
            id::TIP_POPUPS,
        ] {
            state.messages.push(Message {
                player,
                id: tip,
                object: -1,
                params: Vec::new(),
            });
        }
    }
    for (player, home) in homes.iter().enumerate() {
        let Some(id) = state.planets.get(*home).map(|planet| planet.id) else {
            continue;
        };
        state.messages.push(Message {
            player,
            id: id::HOME_PLANET,
            object: id,
            params: vec![id],
        });
    }
}

/// Stage 1: scatter, thin and (optionally) clump, returning positions sorted by
/// ascending x.
fn scatter_planets(config: &NewGame, rng: &mut Rng) -> Vec<(i16, i16)> {
    let span = config.size.span();
    let target = planet_count(config.size, config.density);
    // A seventh more than wanted are thrown down, because the minimum-distance
    // pass will remove some of them.
    let over = (target + target / 7).min(MAX_PLANETS);

    let origin = GALAXY_OFFSET + 10;
    let range = span + 1 - 20;
    let mut points: Vec<(i16, i16)> = (0..over)
        .map(|_| {
            let x = origin + rng.random(range);
            let y = origin + rng.random(range);
            (x, y)
        })
        .collect();
    points.sort_by_key(|p| p.0);

    // Thin: a planet within twelve light years of an earlier one is removed.
    // The original marks a removed planet with y = -100, which also takes it
    // out of every later distance test, so nothing is counted twice.
    let min_sq = MIN_PLANET_DISTANCE * MIN_PLANET_DISTANCE;
    let mut killed = 0;
    for i in 0..points.len() {
        if points[i].1 < 0 {
            continue;
        }
        let line = points[i].0 + MIN_PLANET_DISTANCE;
        for j in i + 1..points.len() {
            if points[j].0 > line {
                break;
            }
            let dy = (points[i].1 - points[j].1).abs();
            if dy > MIN_PLANET_DISTANCE {
                continue;
            }
            let dx = points[i].0 - points[j].0;
            if dx * dx + dy * dy <= min_sq {
                points[j].1 = -100;
                killed += 1;
            }
        }
    }

    // Cull the rest at random until the target count is left.
    let kill_max = over - target;
    let over_usize = points.len();
    while killed < kill_max {
        let i = usize::try_from(rng.random(over)).unwrap_or(0);
        if i >= over_usize || points[i].1 < 0 {
            continue;
        }
        points[i].1 = -100;
        killed += 1;
    }
    points.retain(|p| p.1 >= 0);

    if config.clumping {
        clump(&mut points, rng);
        points.sort_by_key(|p| p.0);
    }
    points
}

/// Drag each planet a fraction of the way toward its nearest neighbour, the
/// further the more (`game.fClumping`).
fn clump(points: &mut [(i16, i16)], rng: &mut Rng) {
    let count = i16::try_from(points.len()).unwrap_or(i16::MAX);
    for _ in 0..points.len() {
        let j = usize::try_from(rng.random(count)).unwrap_or(0);
        if j >= points.len() {
            continue;
        }
        let (x, y) = points[j];

        let mut best = 10_000_000i32;
        let mut nearest = 0usize;
        for (k, other) in points.iter().enumerate() {
            if k == j {
                continue;
            }
            let dx = i32::from(x - other.0);
            let dy = i32::from(y - other.1);
            let d = dx * dx + dy * dy;
            if d < best {
                best = d;
                nearest = k;
            }
        }

        if best <= 12 * 12 {
            continue;
        }
        let (nx, ny) = points[nearest];
        let (numerator, denominator) = if best > 40 * 40 {
            (2, 3)
        } else if best > 25 * 25 {
            (1, 2)
        } else if best > 18 * 18 {
            (1, 3)
        } else {
            (1, 5)
        };
        // `(a * self + b * nearest) / (a + b)` with a = denominator - numerator.
        let keep = denominator - numerator;
        points[j] = (
            (keep * x + numerator * nx) / denominator,
            (keep * y + numerator * ny) / denominator,
        );
    }
}

/// Stage 2: draw a name index for each planet, without repetition.
fn name_planets(count: usize, rng: &mut Rng) -> Vec<u16> {
    let mut used = vec![false; usize::try_from(NAME_COUNT).unwrap_or(999) + 1];
    let mut names = Vec::with_capacity(count);
    for _ in 0..count {
        let mut id = rng.random(NAME_COUNT);
        while used.get(usize::try_from(id).unwrap_or(0)).copied() == Some(true) {
            id += 1;
            if id >= NAME_COUNT {
                id = 0;
            }
        }
        if let Some(slot) = used.get_mut(usize::try_from(id).unwrap_or(0)) {
            *slot = true;
        }
        names.push(u16::try_from(id).unwrap_or(0));
    }
    names
}

/// Stage 3: environment, mineral concentrations and the surface minerals of
/// planet 0, which is the template every homeworld is stocked from.
fn describe_planets(
    config: &NewGame,
    positions: &[(i16, i16)],
    names: &[u16],
    rng: &mut Rng,
) -> Vec<Planet> {
    let mut planets = Vec::with_capacity(positions.len());
    for (i, (x, y)) in positions.iter().enumerate() {
        let mut planet = Planet::unowned(i16::try_from(i).unwrap_or(i16::MAX));
        planet.detail = Detail::Full;
        planet.position = Some(Point::new(*x, *y));
        planet.name = names.get(i).copied().and_then(stars_formats::planet_name);

        if config.random_events {
            planet.artifact = rng.random(3) == 0;
        }

        // Gravity and temperature are the sum of two draws, so they cluster in
        // the middle of the range; radiation is flat.
        let mut env = [0i16; 3];
        env[0] = 1 + rng.random(90);
        env[0] += rng.random(10);
        env[1] = 1 + rng.random(90);
        env[1] += rng.random(10);
        env[2] = 1 + rng.random(99);
        for (axis, value) in env.iter().enumerate() {
            planet.env[axis] = *value as i8;
        }
        planet.env_orig = Some(planet.env);

        for j in 0..3 {
            if config.unlimited_minerals {
                planet.min_conc[j] = 100;
            } else {
                let mut conc = rng.random(45) + rng.random(45) + 31;
                // A highly radioactive world is richer.
                if env[2] >= 90 {
                    conc += rng.random(99 - conc) / 2;
                }
                planet.min_conc[j] = u8::try_from(conc).unwrap_or(u8::MAX);
            }
            planet.min_level[j] = 0;
            planet.surface_min[j] = 0;
            // Accelerated play tops up a thin concentration, before the
            // impoverishing pass below — which can still overwrite it.
            if config.accelerated && planet.min_conc[j] < 40 {
                planet.min_conc[j] += 5;
            }
        }

        // Roughly a third of planets are impoverished in one mineral, and a
        // ninth of those in several.
        let mut limit = if config.unlimited_minerals {
            100
        } else {
            rng.random(27)
        };
        if limit < 18 {
            if limit >= 9 {
                let quantity = rng.random(30);
                let which = usize::try_from(rng.random(3)).unwrap_or(0);
                planet.min_conc[which] = u8::try_from(1 + quantity).unwrap_or(1);
            } else {
                limit += 1;
                while limit < 16 {
                    let quantity = rng.random(30);
                    let which = usize::try_from(rng.random(3)).unwrap_or(0);
                    planet.min_conc[which] = u8::try_from(1 + quantity).unwrap_or(1);
                    limit <<= 1;
                }
            }
        }
        planets.push(planet);
    }

    // Planet 0's surface minerals are the stock every homeworld starts with.
    if let Some(first) = planets.first_mut() {
        for j in 0..3 {
            let conc = i16::from(first.min_conc[j]);
            let mut amount = i32::from(rng.random(conc.saturating_mul(10)) + 10);
            if amount < 200 {
                amount += 155 + i32::from(rng.random(150));
            }
            if config.accelerated {
                amount += amount / 4;
            }
            first.surface_min[j] = amount;
        }
    }
    planets
}

/// Stage 4: choose one planet per player to be their homeworld.
///
/// Player 0 goes near the middle of the map; the rest go inside a border that
/// narrows with the player count, at a distance from every player already
/// placed that lies inside a band around the ideal. If a player cannot be
/// placed, the band widens and every player is placed again from scratch.
fn place_homeworlds(config: &NewGame, positions: &[(i16, i16)], rng: &mut Rng) -> Vec<usize> {
    let span = config.size.span();
    let count = i16::try_from(positions.len()).unwrap_or(1).max(1);
    let players = config.players.len();

    let floor = i32::from(span) * 6;
    let mut ideal = i32::from(span) * i32::from(span) / players as i32 - floor;
    ideal = if ideal < 0 { 0 } else { ideal * 9 / 10 };
    ideal = ideal * i32::from(config.start_distance as i16) / 3 + floor;
    let mut min_sq = ideal * 9 / 10;
    let mut max_sq = ideal * 7 / 6;

    // How far in from the edge the other players' homeworlds must sit. The
    // original computes these with `muldiv_i16`, which multiplies in 32 bits;
    // 2000 * 19 does not fit a 16-bit register.
    let muldiv = |value: i16, numerator: i32, denominator: i32| -> i16 {
        i16::try_from(i32::from(value) * numerator / denominator).unwrap_or(i16::MAX)
    };
    let (border_low, border_high) = if players > 4 {
        (
            span / 20 + GALAXY_OFFSET,
            muldiv(span, 19, 20) + GALAXY_OFFSET,
        )
    } else if players > 2 {
        (
            span / 10 + GALAXY_OFFSET,
            muldiv(span, 9, 10) + GALAXY_OFFSET,
        )
    } else {
        (
            muldiv(span, 3, 20) + GALAXY_OFFSET,
            muldiv(span, 17, 20) + GALAXY_OFFSET,
        )
    };

    let distance_sq = |a: (i16, i16), b: (i16, i16)| -> i32 {
        let dx = i32::from(a.0 - b.0);
        let dy = i32::from(a.1 - b.1);
        dx * dx + dy * dy
    };

    // The original loops until it succeeds; the band only ever widens, so it
    // always terminates. The cap is belt and braces against a degenerate
    // universe (one planet, sixteen players) and simply accepts the last
    // arrangement.
    let mut homes = vec![0usize; players];
    'retry: for _ in 0..1000 {
        // Player 0: up to fifty tries for a planet in the middle half of the
        // map, otherwise the closest to it that came up.
        let centre_low = span / 4 + GALAXY_OFFSET;
        let centre_high = muldiv(span, 3, 4) + GALAXY_OFFSET;
        let mut best = i32::MAX;
        let mut nearest = 0usize;
        let mut tries = 0;
        while tries < 50 {
            homes[0] = usize::try_from(rng.random(count)).unwrap_or(0) % positions.len();
            let (x, y) = positions[homes[0]];
            let dx = (centre_low - x).max(0).max(x - centre_high);
            let dy = (centre_low - y).max(0).max(y - centre_high);
            if dx == 0 && dy == 0 {
                break;
            }
            let d = i32::from(dx) * i32::from(dx) + i32::from(dy) * i32::from(dy);
            if d < best {
                best = d;
                nearest = homes[0];
            }
            tries += 1;
        }
        if tries == 50 {
            homes[0] = nearest;
        }

        for i in 1..players {
            let fits = |candidate: usize, homes: &[usize]| -> bool {
                let pt = positions[candidate];
                if pt.0 < border_low
                    || pt.1 < border_low
                    || pt.0 > border_high
                    || pt.1 > border_high
                {
                    return false;
                }
                let mut in_band = false;
                for placed in &homes[..i] {
                    let d = distance_sq(pt, positions[*placed]);
                    if d < 1 || d < min_sq {
                        return false;
                    }
                    if d <= max_sq {
                        in_band = true;
                    }
                }
                in_band
            };

            let mut placed = false;
            for _ in 0..50 {
                homes[i] = usize::try_from(rng.random(count)).unwrap_or(0) % positions.len();
                if fits(homes[i], &homes) {
                    placed = true;
                    break;
                }
            }
            if placed {
                continue;
            }

            // Random search failed: walk the whole array from where we stopped.
            let start = homes[i];
            let mut candidate = start;
            loop {
                candidate = (candidate + 1) % positions.len();
                if candidate == start {
                    // Nothing fits anywhere: widen the band and start over.
                    let step = ideal / 35;
                    min_sq -= step;
                    max_sq += step;
                    continue 'retry;
                }
                if fits(candidate, &homes) {
                    homes[i] = candidate;
                    break;
                }
            }
        }
        return homes;
    }
    homes
}

/// Stage 5: starting technology, the homeworld itself, and what the leftover
/// advantage points buy.
fn settle_players(
    config: &NewGame,
    planets: &mut [Planet],
    positions: &[(i16, i16)],
    mut homes: Vec<usize>,
    rng: &mut Rng,
) -> (Vec<Player>, Vec<usize>) {
    let count = config.players.len();
    let mut players: Vec<Player> = Vec::with_capacity(count);
    let single_player = config
        .players
        .iter()
        .filter(|p| matches!(p.control, Control::Human))
        .count()
        == 1;

    // Which player gets which of the chosen planets is shuffled, so the
    // central one does not always fall to player 0.
    for i in 0..count {
        let pick = usize::try_from(rng.random(i16::try_from(count - i).unwrap_or(1))).unwrap_or(0);
        homes.swap(i + pick.min(count - i - 1), i);

        let mut race = config.players[i].race.clone();
        let mut name = config.players[i].name.clone();
        let mut plural = config.players[i].plural_name.clone();
        // A race carrying `ibitRaceAIPlayer` — the wizard's "Random" — is
        // rolled here (`GenerateWorld`, `1078:13d0`).
        if race.has_lrt(lrt::AI_PLAYER) {
            if let Some(chosen) = random_race(&mut race, rng, &name, &config.random_names) {
                plural = format!("{chosen}s");
                name = chosen;
            }
        }
        let mut player = Player::new(race);
        player.control = config.players[i].control;
        player.name = name;
        player.plural_name = plural;
        // The five stock battle plans, in the player's own number
        // (`InitBattlePlan`, `1078:…`), the Default plan attacking
        // everyone in a single-player game.
        player.battle_plans = crate::default_battle_plans(i);
        if single_player {
            if let Some(plan) = player.battle_plans.first_mut() {
                plan.attack_who = crate::combat::attack_who::EVERYONE;
            }
        }
        player.relations = vec![0; count];
        player.research.levels = starting_tech(&player.race);
        player.research.current_field = 0;
        player.research.next_field = NextField::Same;
        player.research_pct = 15;
        players.push(player);
    }

    for (i, player) in players.iter_mut().enumerate() {
        let home_index = homes[i];
        let race = player.race.clone();
        let owner = i16::try_from(i).unwrap_or(0);

        // The stock every homeworld starts with is planet 0's, with a floor
        // under the mineral concentrations.
        let template_surface = planets[0].surface_min;
        let template_conc = planets[0].min_conc;

        let home = &mut planets[home_index];
        home.owner = Some(owner);
        home.homeworld = true;
        home.starbase = true;
        // `PLANET.isb` indexes the player's **starbase** design list, not the
        // combined design array: 0 is the first starbase design, which lives at
        // `startup::FIRST_STARBASE_SLOT` in `GameState::designs`.
        home.starbase_design = Some(0);
        home.artifact = false;
        home.factories = 10;
        home.mines = 10;
        home.defenses = 10;
        home.pop = if race.has_lrt(lrt::LOW_STARTING_POP) {
            175
        } else {
            250
        };
        for j in 0..3 {
            home.surface_min[j] = template_surface[j];
            // Floored at 30 — or at 25 while the tutorial runs, a branch on
            // bit 11 of the client's `gd` word at `1078:1db6`: `tutorial.hst`
            // has both home worlds at `[25, 70, 84]` from a template of 24.
            // (Mining still yields as if a home world were at 30, see
            // `mining.rs`.)
            home.min_conc[j] = template_conc[j].max(if config.tutorial_game { 25 } else { 30 });
        }

        // The homeworld sits at the exact middle of the race's habitable band
        // on every axis it cares about, and anywhere at all on one it is
        // immune to.
        for j in 0..3 {
            let value = if race.is_immune(j) {
                rng.random(99) + 1
            } else {
                i16::from(race.env_min[j])
                    + (i16::from(race.env_max[j]) - i16::from(race.env_min[j])) / 2
            };
            home.env[j] = value as i8;
        }
        home.env_orig = Some(home.env);

        spend_leftover_points(home, &race, config.players[i].control);
        // A computer player from Expert upward starts with a tenth more
        // colonists (`GenerateWorld` at `1078:1fbd`).
        if ai_level(config.players[i].control) >= POPULATION_BONUS_LEVEL {
            home.pop += home.pop / 10;
        }
        // Accelerated play: `pop * (2 * PctTrueMaxGrowth + 10) / 10`
        // (`GenerateWorld`, after the computer player's tenth and before the
        // leftover points are spent).
        if config.accelerated {
            let growth = i32::from(crate::population::pct_true_max_growth(&race));
            home.pop = home.pop * (2 * growth + 10) / 10;
        }

        // Alternate Reality lives on its starbase, so its homeworld has no
        // planetary installations at all.
        if race.prt() == Some(Prt::Ar) {
            home.mines = 0;
            home.factories = 0;
            home.defenses = 0;
            home.starbase_design = Some(1);
        }
    }

    // Packet Physics and Inner Tech get a second, already-colonised planet.
    for i in 0..count {
        let race = players[i].race.clone();
        if !matches!(race.prt(), Some(Prt::Pp) | Some(Prt::It)) || config.size == Size::Tiny {
            continue;
        }
        let Some(index) = second_planet(config, planets, positions, homes[i], &race, rng) else {
            continue;
        };
        let owner = i16::try_from(i).unwrap_or(0);
        let home_pop = planets[homes[i]].pop;
        planets[index].owner = Some(owner);
        planets[index].starbase = true;
        planets[index].starbase_design = Some(1);
        planets[index].pop = home_pop * 2 / 5;
        planets[homes[i]].pop = home_pop * 4 / 5;
        for j in 0..3 {
            planets[index].surface_min[j] = i32::from(rng.random(200) + 100);
        }
    }

    (players, homes)
}

/// Pick the planet a Packet Physics or Inner Tech race is given as a second
/// colony, and make it habitable enough to be worth having.
fn second_planet(
    config: &NewGame,
    planets: &mut [Planet],
    positions: &[(i16, i16)],
    home: usize,
    race: &Race,
    rng: &mut Rng,
) -> Option<usize> {
    let span = i32::from(config.size.span());
    let min_sq = (span * 15 / 100) * (span * 15 / 100);
    let max_sq = (span * 23 / 100) * (span * 23 / 100);
    let (hx, hy) = positions[home];

    let mut picked: Option<usize> = None;
    let mut closest: Option<usize> = None;
    let mut best = 10_000_000i32;
    let mut fitting = 0i16;
    for (i, planet) in planets.iter().enumerate() {
        if planet.owner.is_some() {
            continue;
        }
        let (x, y) = positions[i];
        let dx = i32::from(x - hx);
        let dy = i32::from(y - hy);
        let d = dx * dx + dy * dy;
        if d >= min_sq && d <= max_sq {
            fitting += 1;
            if rng.random(fitting) == 0 {
                picked = Some(i);
            }
        } else if picked.is_none() && d < best {
            best = d;
            closest = Some(i);
        }
    }
    let index = picked.or(closest)?;

    // Reroll its environment until it is worth settling, then give up and copy
    // the homeworld's.
    let mut tries = 0;
    while pct_planet_desirability(&planets[index], race) < 10 {
        if tries >= 100 {
            planets[index].env = planets[home].env;
            planets[index].env_orig = Some(planets[index].env);
            break;
        }
        tries += 1;
        for j in 0..3 {
            planets[index].env[j] = (rng.random(97) + 2) as i8;
        }
        planets[index].env_orig = Some(planets[index].env);
    }
    Some(index)
}

/// Spend the leftover advantage points on the homeworld, in whatever currency
/// the race's "leftover points go to" setting names.
///
/// A **person** spends what their race did not: `min(50, CAdvantagePoints)`.
/// A **computer player** always spends the full fifty, whatever its race costs
/// — which is one of the ways the built-in opponents are handed an advantage,
/// and why several of them are priced well over the budget a player is held to.
///
/// Source: `GenerateWorld` at `1078:1f48`-`1078:1f98`. The reconstructed
/// `create.c` folds the unconditional `iT = 50` into the same test as the
/// population bonus, giving `if (fAi && lvlAi > 2)`; the disassembly shows two
/// nested tests, the outer one on `fAi` alone:
///
/// ```text
/// 1078:1f8b  SHR AX, 9 / AND AX, 1     ; fAi
/// 1078:1f98  MOV [iT], 0x32            ; unconditionally 50
/// 1078:1fb0  SHR AX, 10 / AND AX, 7    ; lvlAi
/// 1078:1fb5  CMP AX, 3 / JNC           ; only then, the population bonus
/// ```
pub fn spend_leftover_points(home: &mut Planet, race: &Race, control: Control) {
    let computer = matches!(control, Control::Computer { .. });
    let points = if computer {
        MAX_LEFTOVER_POINTS
    } else {
        advantage_points(race).min(MAX_LEFTOVER_POINTS)
    };
    // A computer player from Tough upward spends on mineral concentrations as
    // well, even when its race would put the leftovers into surface minerals
    // (`GenerateWorld`'s jump to `LConcentrations` at `1078:22ce`, taken when
    // `lvlAi >= 2`).
    let also_concentrations = ai_level(control) >= CONCENTRATION_BONUS_LEVEL;
    spend_points(home, race, points, also_concentrations);
}

/// The largest leftover balance a homeworld is ever stocked with.
pub const MAX_LEFTOVER_POINTS: i16 = 50;

/// The difficulty from which a computer player's homeworld gets the mineral
/// concentration bonus as well as whatever its race asked for.
pub const CONCENTRATION_BONUS_LEVEL: u8 = 2;

/// The difficulty from which a computer player's homeworld starts with a tenth
/// more colonists.
pub const POPULATION_BONUS_LEVEL: u8 = 3;

/// A player's difficulty, or `0` for a person.
fn ai_level(control: Control) -> u8 {
    match control {
        Control::Human => 0,
        Control::Computer { skill_bits, .. } => skill_bits,
    }
}

/// Spend a **given** number of leftover points, which is the rule on its own.
///
/// Split out from [`spend_leftover_points`] so the rule can be checked against
/// the turn-0 fixtures' own numbers without depending on how many points the
/// player was given.
pub fn spend_points(home: &mut Planet, race: &Race, points: i16, also_concentrations: bool) {
    if points <= 0 {
        return;
    }
    let setting = race.stat(RaceStat::UseLeftover);
    if setting == 0 {
        // Surface minerals: a quarter of ten times the points to each mineral,
        // and the remainder plus another quarter to whichever is scarcest.
        let lowest = if home.surface_min[0] < home.surface_min[1] {
            usize::from(home.surface_min[0] >= home.surface_min[2]) * 2
        } else if home.surface_min[1] < home.surface_min[2] {
            1
        } else {
            2
        };
        let total = points * 10;
        let remainder = total & 3;
        let share = total >> 2;
        home.surface_min[lowest] += i32::from(share + remainder);
        for amount in &mut home.surface_min {
            *amount += i32::from(share);
        }
    }
    if setting == 1 || (setting == 0 && also_concentrations) {
        // Mineral concentrations: half the points to the scarcest, a quarter
        // to all three.
        let mut step = if points < 3 { 1 } else { points / 2 };
        let mut lowest = 0;
        for j in 1..3 {
            if home.min_conc[j] < home.min_conc[lowest] {
                lowest = j;
            }
        }
        home.min_conc[lowest] = add_conc(home.min_conc[lowest], step);
        step = (step + 1) / 2;
        for conc in &mut home.min_conc {
            *conc = add_conc(*conc, step);
        }
    }
    match setting {
        2 => home.mines += points / 2,
        3 => home.factories += points / 5,
        4 => home.defenses += (points + 5) / 10,
        _ => {}
    }
}

/// Add to a mineral concentration without wrapping past the byte the game
/// stores it in.
fn add_conc(conc: u8, step: i16) -> u8 {
    u8::try_from(i16::from(conc) + step).unwrap_or(u8::MAX)
}

/// The technology a race starts with, from its primary trait and its lesser
/// traits.
#[must_use]
pub fn starting_tech(race: &Race) -> [u8; 6] {
    use crate::research::TechField::{
        Biotechnology, Construction, Electronics, Energy, Propulsion, Weapons,
    };
    let mut tech = [0u8; 6];
    let mut set = |field: crate::research::TechField, level: u8| tech[field as usize] = level;

    match race.prt() {
        Some(Prt::Ss) => set(Electronics, 5),
        Some(Prt::Wm) => {
            set(Weapons, 6);
            set(Propulsion, 1);
            set(Energy, 1);
        }
        Some(Prt::Ca) => {
            set(Biotechnology, 6);
            set(Construction, 2);
            set(Energy, 1);
            set(Weapons, 1);
            set(Propulsion, 1);
        }
        Some(Prt::Sd) => {
            set(Propulsion, 2);
            set(Biotechnology, 2);
        }
        Some(Prt::Pp) => set(Energy, 4),
        Some(Prt::It) => {
            set(Propulsion, 5);
            set(Construction, 5);
        }
        Some(Prt::Ar) => set(Energy, 1),
        Some(Prt::Joat) => tech = [3; 6],
        _ => {}
    }

    // "Expensive tech starts at level 3" raises every field that has no
    // cheap/expensive setting of its own — level 4 for Jack of All Trades.
    if race.has_lrt(lrt::TECH3) {
        let floor = if race.prt() == Some(Prt::Joat) { 4 } else { 3 };
        for (field, level) in tech.iter_mut().enumerate() {
            if *level < floor && race.attrs[RaceStat::TechBonus1 as usize + field] == 0 {
                *level = floor;
            }
        }
    }
    if race.has_lrt(lrt::CHEAP_ENGINES) {
        tech[Propulsion as usize] += 1;
    }
    if race.has_lrt(lrt::IFE) {
        tech[Propulsion as usize] += 1;
    }
    tech
}

/// A stock race with the given primary trait: the Humanoid economy, a
/// symmetric habitable band and no lesser traits.
///
/// The original's New Game wizard starts every custom race from this shape,
/// and it is what the built-in "Humanoid" is: every economy figure at its
/// baseline, growth 15%, normal research costs in all six fields, and a
/// habitable range of 15 to 85 clicks centred on 50.
#[must_use]
pub fn stock_race(prt: Prt) -> Race {
    let mut race = Race::humanoid();
    race.attrs[RaceStat::MajorAdv as usize] = prt as i16;
    race
}

/// Build each player's starting designs and the fleets that hold them.
fn starting_ships(
    config: &NewGame,
    planets: &[Planet],
    positions: &[(i16, i16)],
    homes: &[usize],
) -> (Vec<Fleet>, Vec<Vec<ShipDesign>>) {
    let mut fleets = Vec::new();
    let mut all_designs = Vec::with_capacity(config.players.len());

    for (i, home_index) in homes.iter().enumerate() {
        let race = &config.players[i].race;
        let prt = race.prt();
        let levels = starting_tech(race);
        let owner = i16::try_from(i).unwrap_or(0);
        let (x, y) = positions[*home_index];
        let planet_id = u16::try_from(planets[*home_index].id).unwrap_or(0);
        let human = matches!(config.players[i].control, Control::Human);

        // Which templates, and how many ships of each.
        let mut wanted: Vec<(usize, i32)> = Vec::new();
        let push = |t: usize, n: i32, wanted: &mut Vec<(usize, i32)>| wanted.push((t, n));

        match prt {
            Some(Prt::Pp) => push(ship::LONG_RANGE_SCOUT, 1, &mut wanted),
            Some(Prt::Wm) => {
                push(ship::ARMED_PROBE, 1, &mut wanted);
                if levels[3] > 2 {
                    push(ship::STALWART_DEFENDER, 1, &mut wanted);
                    push(ship::GADFLY, 1, &mut wanted);
                }
            }
            Some(Prt::Joat) => {
                push(ship::ARMED_PROBE, 1, &mut wanted);
                push(ship::LONG_RANGE_SCOUT, 1, &mut wanted);
            }
            Some(Prt::Ss) => {
                let scout = if levels[0] < 2 {
                    ship::SMAUGARIAN_PEEPING_TOM
                } else {
                    ship::SHADOW_SLEUTH
                };
                push(scout, 1, &mut wanted);
                if human {
                    push(ship::SHADOW_TRANSPORT, 1, &mut wanted);
                }
            }
            _ => push(ship::SMAUGARIAN_PEEPING_TOM, 1, &mut wanted),
        }

        match prt {
            Some(Prt::He) => push(ship::SPORE_CLOUD, 3, &mut wanted),
            Some(Prt::It) => push(ship::MAYFLOWER, 1, &mut wanted),
            Some(Prt::Ar) => push(ship::PINTA, 1, &mut wanted),
            _ => push(ship::SANTA_MARIA, 1, &mut wanted),
        }

        match prt {
            Some(Prt::Sd) => {
                push(ship::LITTLE_HEN, 1, &mut wanted);
                push(ship::SPEED_TURTLE, 1, &mut wanted);
            }
            Some(Prt::Ca) => push(ship::CHANGE_OF_HEART, 1, &mut wanted),
            Some(Prt::It) => {
                push(ship::STALWART_DEFENDER, 1, &mut wanted);
                push(ship::SWASHBUCKLER, 1, &mut wanted);
            }
            Some(Prt::Joat) => {
                let freighter = if levels[3] < 4 {
                    ship::TEAMSTER
                } else {
                    ship::SWASHBUCKLER
                };
                push(freighter, 1, &mut wanted);
                push(ship::STALWART_DEFENDER, 1, &mut wanted);
                push(ship::COTTON_PICKER, 1, &mut wanted);
            }
            _ => {}
        }

        if race.has_lrt(lrt::ARM) && !race.has_lrt(lrt::OBRM) {
            push(ship::POTATO_BUG, 2, &mut wanted);
        }

        // One design per template, one fleet per ship.
        let mut designs: Vec<ShipDesign> = Vec::new();
        let mut next_fleet = 0u16;
        for (template, ships) in wanted {
            let mut design = startup::SHIPS[template].design();
            upgrade_slots(&mut design, &levels);
            let slot = u8::try_from(designs.len()).unwrap_or(0);
            designs.push(design);

            for _ in 0..ships {
                fleets.push(Fleet {
                    name: None,
                    repeat_orders: false,
                    direction: None,
                    id: next_fleet,
                    owner,
                    position: Point::new(x, y),
                    orbiting: Some(planet_id),
                    stacks: vec![crate::fleet::ShipStack {
                        design: slot,
                        count: 1,
                        damaged_pct: 0,
                        damage_pct: 0,
                    }],
                    cargo: Cargo::default(),
                    battle_plan: 0,
                    warp: None,
                    waypoints: vec![Waypoint {
                        position: Point::new(x, y),
                        target: Some(planet_id),
                        target_class: 1,
                        warp: 0,
                        task: 0,
                        transport: None,
                        task_data: Vec::new(),
                    }],
                });
                next_fleet += 1;
            }
        }

        // The starbase design goes in slot 16, where the game keeps it.
        let base = match prt {
            Some(Prt::Ar) => starbase::STARTER_COLONY,
            _ => starbase::STARBASE,
        };
        while designs.len() < usize::from(startup::FIRST_STARBASE_SLOT) {
            designs.push(ShipDesign {
                name: String::new(),
                picture: 0,
                stored_armor: 0,
                obsolete: false,
                designed: 0,
                built: 0,
                hull_id: -1,
                slots: Vec::new(),
            });
        }
        designs.push(startup::STARBASES[base].design());
        // Packet Physics arms its starbase with a mass driver; Inner Tech with
        // a stargate. Both also get the matching second design.
        match prt {
            Some(Prt::Pp) => {
                if let Some(first) = designs[usize::from(startup::FIRST_STARBASE_SLOT)]
                    .slots
                    .first_mut()
                {
                    first.item = 7;
                    first.count = 1;
                }
                designs.push(startup::STARBASES[starbase::ACCELERATOR_PLATFORM].design());
            }
            Some(Prt::It) => {
                if let Some(first) = designs[usize::from(startup::FIRST_STARBASE_SLOT)]
                    .slots
                    .first_mut()
                {
                    first.item = 0;
                    first.count = 1;
                }
                designs.push(startup::STARBASES[starbase::PORTHOLE_TO_BEYOND].design());
            }
            Some(Prt::Ar) => {
                designs.push(startup::STARBASES[starbase::STARBASE].design());
            }
            _ => {}
        }

        // A fleet's tanks start full.
        let mut fuel_designs = designs.clone();
        fuel_designs.truncate(usize::from(startup::FIRST_STARBASE_SLOT));
        for fleet in fleets.iter_mut().filter(|f| f.owner == owner) {
            fleet.cargo.fuel = fleet.fuel_capacity(&fuel_designs);
        }

        all_designs.push(designs);
    }

    (fleets, all_designs)
}

/// Assemble the `.xy` file for a generated universe.
fn build_universe(
    config: &NewGame,
    positions: &[(i16, i16)],
    names: &[u16],
) -> Result<Universe, NewGameError> {
    let mut flags = 0u16;
    if config.slow_tech {
        flags |= game_flag::SLOW_TECH;
    }
    if config.unlimited_minerals {
        flags |= game_flag::EXTRA_FUEL;
    }
    if config.public_scores {
        flags |= game_flag::VIS_SCORES;
    }
    if !config.random_events {
        flags |= game_flag::NO_RANDOM;
    }
    if config.clumping {
        flags |= game_flag::CLUMPING;
    }
    if config.accelerated {
        flags |= game_flag::BBS_PLAY;
    }
    if config.tutorial_game {
        flags |= game_flag::TUTORIAL;
    }
    if config
        .players
        .iter()
        .filter(|p| matches!(p.control, Control::Human))
        .count()
        == 1
    {
        flags |= game_flag::SINGLE_PLAYER;
    }

    let mut raw = vec![0; GameInfo::LEN];
    let victory = config
        .victory
        .unwrap_or_else(|| default_victory(config.size));
    raw[stars_formats::victory::OFFSET..stars_formats::victory::OFFSET + victory.len()]
        .copy_from_slice(&victory);
    let info = GameInfo {
        id: config.id,
        size: config.size as i16,
        density: config.density as i16,
        players: i16::try_from(config.players.len()).unwrap_or(1),
        planets: i16::try_from(positions.len()).unwrap_or(0),
        start_distance: config.start_distance as i16,
        flags,
        turn: 0,
        name: config.name.clone(),
        raw,
    };

    // A `.xy` belongs to no one player; the game writes it as player 31.
    let header = FileHeader::new(config.id, FileType::Universe, 31, 0, 0x248);
    let packed: Vec<(u16, u16)> = positions
        .iter()
        .map(|(x, y)| (*x as u16, *y as u16))
        .collect();
    Universe::create(header, &info, &packed, names)
        .map_err(|e| NewGameError::Universe(e.to_string()))
}

/// The Berserkers' race, as `tutorial.hst` records it: not one of the
/// wizard's presets but the world's own — a Humanoid with a few points
/// moved, the lesser traits `0x2045`, a narrower habitable range and 14%
/// growth. With it the new game gives them what the host file gives them:
/// a Smaugarian Peeping Tom, a Santa Maria and two Potato Bugs (ARM).
#[must_use]
pub fn berserker() -> Race {
    Race {
        attrs: [10, 9, 10, 9, 9, 5, 8, 0, 1, 0, 1, 1, 1, 0, 1, 0],
        lrt_bits: 0x2045,
        env_center: [58, 35, 65],
        env_min: [27, 7, 35],
        env_max: [89, 63, 95],
        pct_ideal_growth: 14,
    }
}

/// The tutorial's sample game.
///
/// `CreateTutorWorld` (`1078:5e5e`) does not hand-place anything: it fills in
/// a `GAME` by hand and then calls `GenerateWorld` like any other new game.
/// Everything about the tutorial's galaxy therefore comes from these
/// settings and one seed.
///
/// | field | value |
/// |-------|-------|
/// | `cPlayer` | 2 |
/// | `mdSize` | 0 — tiny, 400 light years |
/// | `mdDensity` | 0 — sparse |
/// | `mdStartDist` | 1 — close |
/// | flags | `0xe8` |
/// | `lid` | `0x008cef49` |
/// | `rgvc[7]`, `rgvc[8]` | `0x80`, `0x81` |
/// | seed | `0x499602d2` — 1,234,567,890 |
///
/// The flag word is worth reading out: `0xe8` is bits 3, 5, 6 and 7 —
/// **tutorial**, BBS play, visible scores and **no random events**. Bit 2,
/// single-player, is *not* set here; a game with one human gets it later.
///
/// Player 0 is the default race named `Humanoid`; player 1 is a computer
/// player named `Berserker`.
///
/// # The galaxy is the original's
///
/// `crates/stars-core/tests/tutorial_seed.rs` generates from these settings
/// and this seed and matches every planet of `fixtures/games/tutorial/` —
/// names and coordinates — so the pages' planet ids mean what they say.
/// Bit 5 is **accelerated BBS play**, and it is what makes the tutorial's
/// home planets start with 100,000 colonists rather than 25,000; see
/// [`NewGame::accelerated`].
#[must_use]
pub fn tutorial() -> (NewGame, u32) {
    let config = NewGame {
        name: "Tutorial Game".to_string(),
        id: 0x008c_ef49,
        size: Size::Tiny,
        density: Density::Sparse,
        start_distance: StartDistance::Close,
        clumping: false,
        // Bit 7 of the flag word is `fNoRandom`, and it is set.
        random_events: false,
        slow_tech: false,
        unlimited_minerals: false,
        // Bit 6, `fVisScores`.
        public_scores: true,
        // Bit 5, accelerated BBS play.
        accelerated: true,
        tutorial_game: true,
        players: vec![
            NewPlayer {
                race: Race::humanoid(),
                control: Control::Human,
                name: "Humanoid".to_string(),
                plural_name: "Humanoids".to_string(),
            },
            NewPlayer {
                race: berserker(),
                // `tutorial.hst` gives the Berserkers `0x27`: a TurinDrone.
                control: Control::Computer {
                    personality: Some(crate::ai::AiPersonality::TurinDrone),
                    skill_bits: 0,
                },
                name: "Berserker".to_string(),
                plural_name: "Berserkers".to_string(),
            },
        ],
        random_names: Vec::new(),
        // `CreateTutorWorld`: only the highest score after a hundred
        // years counts, and one condition is enough.
        victory: Some({
            let mut v = [0u8; stars_formats::victory::COUNT];
            v[7] = 0x80;
            v[8] = 0x81;
            v
        }),
    };
    (config, 0x4996_02d2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The planet counts the game shows in its own wizard.
    #[test]
    fn planet_counts_are_the_published_ones() {
        assert_eq!(planet_count(Size::Tiny, Density::Sparse), 24);
        assert_eq!(planet_count(Size::Small, Density::Normal), 128);
        assert_eq!(planet_count(Size::Small, Density::Dense), 160);
        assert_eq!(planet_count(Size::Medium, Density::Normal), 288);
        assert_eq!(planet_count(Size::Medium, Density::Dense), 360);
        assert_eq!(planet_count(Size::Medium, Density::Packed), 540);
        assert!(planet_count(Size::Huge, Density::Packed) <= MAX_PLANETS);
    }

    /// The universe is 400 light years per size step.
    #[test]
    fn sizes_span_what_they_say() {
        assert_eq!(Size::Tiny.span(), 400);
        assert_eq!(Size::Huge.span(), 2000);
    }

    /// Starting technology, trait by trait.
    #[test]
    fn primary_traits_start_where_they_should() {
        use crate::research::TechField::{
            Biotechnology, Construction, Electronics, Energy, Propulsion, Weapons,
        };
        let tech = |prt| starting_tech(&stock_race(prt));

        assert_eq!(tech(Prt::Joat), [3; 6]);
        assert_eq!(tech(Prt::Ss)[Electronics as usize], 5);
        assert_eq!(tech(Prt::Wm)[Weapons as usize], 6);
        assert_eq!(tech(Prt::Ca)[Biotechnology as usize], 6);
        assert_eq!(tech(Prt::Ca)[Construction as usize], 2);
        assert_eq!(tech(Prt::Sd)[Propulsion as usize], 2);
        assert_eq!(tech(Prt::Pp)[Energy as usize], 4);
        assert_eq!(tech(Prt::It)[Propulsion as usize], 5);
        assert_eq!(tech(Prt::It)[Construction as usize], 5);
        assert_eq!(tech(Prt::Ar)[Energy as usize], 1);
        assert_eq!(tech(Prt::Is), [0; 6], "Inner Strength starts at nothing");
    }

    /// Improved Fuel Efficiency and Cheap Engines each add a propulsion level.
    #[test]
    fn engine_traits_add_propulsion() {
        let mut race = stock_race(Prt::Is);
        race.lrt_bits |= 1 << lrt::IFE;
        assert_eq!(starting_tech(&race)[2], 1);
        race.lrt_bits |= 1 << lrt::CHEAP_ENGINES;
        assert_eq!(starting_tech(&race)[2], 2);
    }

    /// Low Starting Population is 175 rather than 250 hundreds of colonists.
    #[test]
    fn low_starting_population_settles_fewer() {
        let mut race = stock_race(Prt::Is);
        race.lrt_bits |= 1 << lrt::LOW_STARTING_POP;
        let config = NewGame {
            players: vec![NewPlayer::human(race)],
            ..NewGame::default()
        };
        let mut rng = Rng::randomize(config.id);
        let made = generate(&config, &mut rng).expect("generates");
        let home = made
            .state
            .planets
            .iter()
            .find(|p| p.homeworld)
            .expect("a homeworld");
        assert_eq!(home.pop, 175);
    }

    /// A homeworld sits at the exact middle of the race's habitable band.
    #[test]
    fn a_homeworld_is_perfectly_habitable() {
        let mut race = stock_race(Prt::Is);
        race.env_center = [30, 70, 50];
        race.env_min = [10, 50, 20];
        race.env_max = [50, 90, 80];
        let config = NewGame {
            players: vec![NewPlayer::human(race)],
            ..NewGame::default()
        };
        let mut rng = Rng::randomize(config.id);
        let made = generate(&config, &mut rng).expect("generates");
        let home = made
            .state
            .planets
            .iter()
            .find(|p| p.homeworld)
            .expect("a homeworld");
        assert_eq!(home.env, [30, 70, 50]);
        assert_eq!(home.env_orig, Some([30, 70, 50]));
    }

    /// Every player is given a different homeworld, however many there are.
    #[test]
    fn homeworlds_never_collide() {
        for players in 1..=MAX_PLAYERS {
            let config = NewGame {
                size: Size::Small,
                players: (0..players)
                    .map(|_| NewPlayer::human(Race::humanoid()))
                    .collect(),
                ..NewGame::default()
            };
            let mut rng = Rng::randomize(0x0bad_c0de);
            let made = generate(&config, &mut rng).expect("generates");
            let homes: std::collections::BTreeSet<i16> = made
                .state
                .planets
                .iter()
                .filter(|p| p.homeworld)
                .map(|p| p.id)
                .collect();
            assert_eq!(homes.len(), players, "{players} players");
        }
    }

    /// Names are drawn without replacement.
    #[test]
    fn every_planet_gets_its_own_name() {
        let config = NewGame {
            size: Size::Medium,
            density: Density::Packed,
            players: vec![NewPlayer::human(Race::humanoid())],
            ..NewGame::default()
        };
        let mut rng = Rng::randomize(0x5eed);
        let made = generate(&config, &mut rng).expect("generates");
        let names: std::collections::BTreeSet<&str> =
            made.state.planets.iter().filter_map(|p| p.name).collect();
        assert_eq!(names.len(), made.state.planets.len());
    }

    /// Unlimited minerals means exactly that.
    #[test]
    fn unlimited_minerals_maxes_every_concentration() {
        let config = NewGame {
            unlimited_minerals: true,
            players: vec![NewPlayer::human(Race::humanoid())],
            ..NewGame::default()
        };
        let mut rng = Rng::randomize(config.id);
        let made = generate(&config, &mut rng).expect("generates");
        for planet in made.state.planets.iter().filter(|p| p.owner.is_none()) {
            assert_eq!(planet.min_conc, [100, 100, 100]);
        }
    }

    /// No random events means no artifacts.
    #[test]
    fn a_game_without_random_events_hides_no_artifacts() {
        let config = NewGame {
            random_events: false,
            players: vec![NewPlayer::human(Race::humanoid())],
            ..NewGame::default()
        };
        let mut rng = Rng::randomize(config.id);
        let made = generate(&config, &mut rng).expect("generates");
        assert!(made.state.planets.iter().all(|p| !p.artifact));
    }

    /// A game needs between one and sixteen players.
    #[test]
    fn the_player_count_is_checked() {
        let mut rng = Rng::randomize(1);
        let empty = NewGame {
            players: Vec::new(),
            ..NewGame::default()
        };
        assert_eq!(
            generate(&empty, &mut rng).err(),
            Some(NewGameError::PlayerCount(0))
        );
        let crowded = NewGame {
            players: (0..17)
                .map(|_| NewPlayer::human(Race::humanoid()))
                .collect(),
            ..NewGame::default()
        };
        assert_eq!(
            generate(&crowded, &mut rng).err(),
            Some(NewGameError::PlayerCount(17))
        );
    }
}
