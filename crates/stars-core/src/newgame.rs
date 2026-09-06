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
//! * **Seed-identical universes.** The original sorts its scratch array with
//!   the C library's `qsort`, whose permutation of equal x coordinates is not
//!   specified; every later random draw indexes that array, so an identical
//!   seed cannot be made to give an identical universe without reproducing a
//!   1996 Microsoft C runtime. Draw *order* and *distributions* are faithful;
//!   a given seed is not.
//! * **Wormholes and the Mystery Trader**, which live in the `THING` list that
//!   [`GameState`] does not yet model.
//! * **Random races for computer players.** `CreateRandomRace` is not
//!   transcribed, so a computer player is given whatever race the caller
//!   supplies.

use crate::advantage::advantage_points;
use crate::ai::Control;
use crate::design::ShipDesign;
use crate::fleet::{Cargo, Fleet, Waypoint};
use crate::hab::pct_planet_desirability;
use crate::movement::Point;
use crate::planet::{Detail, Planet};
use crate::race::{lrt, Prt, Race, RaceStat};
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
    /// The per-game id (`GAME.lid`), which also seeds every file's cipher.
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
    /// The players, in order.
    pub players: Vec<NewPlayer>,
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
            players: vec![NewPlayer::human(Race::humanoid())],
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

    let mut state = GameState::new(config.id);
    state.slow_tech = config.slow_tech;
    state.planets = planets;
    state.players = state_players;
    state.fleets = fleets;
    state.designs = designs;

    let universe = build_universe(config, &positions, &names)?;
    Ok(Created { state, universe })
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

    // Which player gets which of the chosen planets is shuffled, so the
    // central one does not always fall to player 0.
    for i in 0..count {
        let pick = usize::try_from(rng.random(i16::try_from(count - i).unwrap_or(1))).unwrap_or(0);
        homes.swap(i + pick.min(count - i - 1), i);

        let race = config.players[i].race.clone();
        let mut player = Player::new(race);
        player.control = config.players[i].control;
        player.name.clone_from(&config.players[i].name);
        player
            .plural_name
            .clone_from(&config.players[i].plural_name);
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
            home.min_conc[j] = template_conc[j].max(30);
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
    if config
        .players
        .iter()
        .filter(|p| matches!(p.control, Control::Human))
        .count()
        == 1
    {
        flags |= game_flag::SINGLE_PLAYER;
    }

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
        raw: vec![0; GameInfo::LEN],
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
