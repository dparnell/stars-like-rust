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

/// A planet, as far as the deterministic planetary simulation is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planet {
    /// Planet id (index into the universe's planet array).
    pub id: i16,
    /// Owning player index, or `None` when unowned (`PLANET.iPlayer == -1`).
    pub owner: Option<i16>,
    /// Current gravity, temperature and radiation, as clicks in `0..=100`.
    pub env: [i8; MINERALS],
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
    /// Whether this is a player's home world (`fHomeworld`).
    pub homeworld: bool,
    /// Whether the planet has a starbase (`fStarbase`).
    pub starbase: bool,
    /// The planet's production queue, in build order.
    pub queue: Vec<crate::production::QueueItem>,
    /// Whether this planet is exempt from the research skim (`fNoResearch`).
    pub no_research: bool,
}

impl Planet {
    /// An empty, unowned planet with neutral values; a starting point for tests.
    #[must_use]
    pub fn unowned(id: i16) -> Self {
        Self {
            id,
            owner: None,
            env: [50, 50, 50],
            min_conc: [50, 50, 50],
            min_level: [0, 0, 0],
            surface_min: [0, 0, 0],
            pop: 0,
            delta_pop: 0,
            mines: 0,
            factories: 0,
            homeworld: false,
            starbase: false,
            queue: Vec::new(),
            no_research: false,
        }
    }

    /// Population in colonists (the stored value is in units of 100).
    #[must_use]
    pub fn colonists(&self) -> i64 {
        i64::from(self.pop) * 100
    }
}
