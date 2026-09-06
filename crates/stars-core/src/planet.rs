//! Planet state, mirroring the fields of the game's `PLANET` struct that the
//! planetary simulation reads and writes.
//!
//! Field names and offsets come from the NB09 debug symbols (`PLANET`,
//! size `0x38`); `stars_formats::PlanetRecord` decodes them from
//! `.hst`/`.mN`/`.hN` files — see `docs/formats/planet.md`.

/// The three minerals, in the order they appear on disk and in every formula.
pub const MINERALS: usize = 3;

/// Index of ironium in mineral arrays.
pub const IRONIUM: usize = 0;
/// Index of boranium in mineral arrays.
pub const BORANIUM: usize = 1;
/// Index of germanium in mineral arrays.
pub const GERMANIUM: usize = 2;

/// How much a record said about a planet.
///
/// Source: the `det` field of a planet block — see `docs/formats/planet.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Detail {
    /// Header only: the planet's id, and whether anyone holds it.
    Minimal,
    /// Environment and mineral concentrations, but no population or
    /// installations. This is a planet the player has scanned but does not own.
    Scanned,
    /// Everything: population, installations, surface minerals. Only a planet
    /// the player owns is recorded this way.
    Full,
}

impl Detail {
    /// Whether the record carries population and installations, and so can be
    /// simulated rather than merely known about.
    #[must_use]
    pub fn is_full(self) -> bool {
        self == Self::Full
    }
}

/// A planet, as far as the deterministic planetary simulation is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planet {
    /// Planet id (index into the universe's planet array).
    pub id: i16,
    /// How much of this planet the file actually recorded.
    ///
    /// A player's own file describes the planets it owns in full and everything
    /// else at whatever detail it has scanned. Only a [`Detail::Full`] planet
    /// carries a trustworthy population and installation count; the others are
    /// present so that code which needs to know *what a player knows* — the AI's
    /// colonisation search above all — can see them at all.
    pub detail: Detail,
    /// Owning player index, or `None` when unowned (`PLANET.iPlayer == -1`).
    pub owner: Option<i16>,
    /// Current gravity, temperature and radiation, as clicks in `0..=100`.
    pub env: [i8; MINERALS],
    /// The environment before any terraforming (`rgEnvVarOrig`), when the
    /// planet records it. Terraforming reach is measured from these, not from
    /// [`Self::env`] — see [`crate::terraform`].
    pub env_orig: Option<[i8; MINERALS]>,
    /// Mineral concentration under the surface, `0..=100+` (`rgMinConc`).
    pub min_conc: [u8; MINERALS],
    /// Sub-concentration decay accumulator, in 1/256ths (`rgpctMinLevel`).
    ///
    /// A stored `0` means "full", i.e. 256 — see [`crate::mining`].
    pub min_level: [u8; MINERALS],
    /// Minerals on the surface, in kT (`rgwtMin[0..3]`).
    pub surface_min: [i32; MINERALS],
    /// Population, in units of **100 colonists** (`rgwtMin[3]`).
    ///
    /// A value of `10_000` is 1,000,000 colonists.
    pub pop: i32,
    /// Fractional-population accumulator in `0..100` (`PLANET.rgbImp[0]`,
    /// the `iDeltaPop:8` bitfield) — see [`crate::population`].
    pub delta_pop: u8,
    /// Mines built on the planet (`cMines`).
    pub mines: i16,
    /// Factories built on the planet (`cFactories`).
    pub factories: i16,
    /// Planetary defences built (`cDefenses`). These stop bombing — see
    /// [`crate::bombing::pct_survive`].
    pub defenses: i16,
    /// Whether this is a player's home world (`fHomeworld`).
    pub homeworld: bool,
    /// Whether the planet has a starbase (`fStarbase`).
    pub starbase: bool,
    /// Whether an undiscovered Mystery Trader artifact lies here
    /// (`fIsArtifact`). Settling the planet finds it — see
    /// [`crate::ground::artifact_bonus`] — and clears the flag.
    pub artifact: bool,
    /// The starbase's design index (`isb`), when one is present.
    ///
    /// The AI treats a value above 9 as "no usable starbase" — only 0-9 are
    /// real starbase designs. See [`crate::ai::ships`].
    pub starbase_design: Option<u8>,
    /// The planet's production queue, in build order.
    pub queue: Vec<crate::production::QueueItem>,
    /// The planetary scanner built here, as an index into
    /// [`crate::components::PLANETARY`] — or `None` when the planet has none
    /// (`PLANET.iScanner`, five bits, where the game stores **31** for "no
    /// scanner").
    ///
    /// A planet scans only once one has been built, and it then upgrades
    /// itself as technology arrives, which is why the production inventory
    /// offers a scanner exactly once. Alternate Reality is the exception: its
    /// starbases scan and it never builds one.
    pub scanner: Option<u8>,
    /// Whether this planet is exempt from the research skim (`fNoResearch`).
    pub no_research: bool,
    /// Where fleets built here, or given the Route task, are sent
    /// (`PLANET.idRoute`), or `None` when no route is set.
    ///
    /// The file stores it **one-based** so that `0` can mean "none"; this is
    /// the planet id itself. Read by the Route waypoint task — see
    /// [`crate::orders::execute_arrival_tasks`].
    pub route_dest: Option<i16>,
    /// Where the planet sits in the universe.
    ///
    /// Planet coordinates live in the `.xy` universe file, not in the per-player
    /// `.mN` or the `.hst`, so this is `None` until
    /// [`crate::GameState::apply_universe`] has been given one. Anything that
    /// needs a position — placing a newly built fleet, measuring a distance —
    /// must handle its absence rather than assume the origin.
    pub position: Option<crate::movement::Point>,
    /// The planet's name, from the `.xy` file's name table.
    pub name: Option<&'static str>,
}

impl Planet {
    /// An empty, unowned planet with neutral values; a starting point for tests.
    #[must_use]
    pub fn unowned(id: i16) -> Self {
        Self {
            id,
            detail: Detail::Full,
            owner: None,
            env: [50, 50, 50],
            env_orig: None,
            min_conc: [50, 50, 50],
            min_level: [0, 0, 0],
            surface_min: [0, 0, 0],
            pop: 0,
            delta_pop: 0,
            mines: 0,
            factories: 0,
            defenses: 0,
            homeworld: false,
            starbase: false,
            starbase_design: None,
            artifact: false,
            queue: Vec::new(),
            scanner: None,
            no_research: false,
            route_dest: None,
            position: None,
            name: None,
        }
    }

    /// Population in colonists (the stored value is in units of 100).
    #[must_use]
    pub fn colonists(&self) -> i64 {
        i64::from(self.pop) * 100
    }
}
