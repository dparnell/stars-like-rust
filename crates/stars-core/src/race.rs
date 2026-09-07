//! Race attributes: the economic stats, primary trait and lesser traits that
//! parameterise every planetary formula.
//!
//! The layout mirrors the game's own `PLAYER` struct (`stars-types.h`,
//! recovered from the NB09 debug symbols): the sixteen economy bytes in
//! `rgAttr` are indexed by the `RaceStat` enum, the primary racial trait is
//! `rgAttr[rsMajorAdv]`, and the lesser traits are bits in `grbitAttr`.
//! `stars_formats::RaceRecord` decodes all of these from a `.rN`/`.mN` file —
//! see `docs/formats/race-r.md`.

/// Index into a race's sixteen economy attribute bytes (`PLAYER.rgAttr`).
///
/// The order is the game's `RaceStat` enum and is exactly the order the fields
/// appear at offsets 62..=68 of a race record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RaceStat {
    /// Colonists per resource, in thousands (Humanoid `10` = 1 resource per 1000 colonists).
    ResGen = 0,
    /// Resources produced per 10 factories (Humanoid `10`).
    FactProd = 1,
    /// Resource cost to build one factory (Humanoid `10`).
    FactBuild = 2,
    /// Factories operable per 10,000 colonists (Humanoid `10`).
    FactOperate = 3,
    /// Minerals produced per 10 mines (Humanoid `10`).
    MineProd = 4,
    /// Resource cost to build one mine (Humanoid `5`).
    MineBuild = 5,
    /// Mines operable per 10,000 colonists (Humanoid `10`).
    MineOperate = 6,
    /// Leftover-resource policy.
    UseLeftover = 7,
    /// Per-field research cost setting for Energy: `0` costs 75% extra, `1`
    /// normal, `2` costs 50% less. The remaining five fields follow at
    /// consecutive indices.
    TechBonus1 = 8,
    /// The primary racial trait (see [`Prt`]).
    MajorAdv = 14,
}

/// Primary Racial Trait (the game's `RaceAttribute` enum; internal names in
/// parentheses are the ones the binary uses).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum Prt {
    /// Hyper Expansion (`raCheapCol`): double growth rate, half maximum population.
    He = 0,
    /// Super Stealth (`raStealth`).
    Ss = 1,
    /// War Monger (`raAttack`).
    Wm = 2,
    /// Claim Adjuster (`raTerra`).
    Ca = 3,
    /// Inner Strength (`raDefend`).
    Is = 4,
    /// Space Demolition (`raMines`).
    Sd = 5,
    /// Packet Physics (`raMassAccel`).
    Pp = 6,
    /// Interstellar Traveler (`raStargate`): stargates, and the only race
    /// that may build the longer-ranged ones.
    It = 7,
    /// Alternate Reality (`raMacintosh`): lives on starbases, mines by robot.
    Ar = 8,
    /// Jack of All Trades (`raNone`): +20% maximum population.
    Joat = 9,
}

impl Prt {
    /// Every primary racial trait, in the game's own order.
    pub const ALL: [Prt; 10] = [
        Prt::He,
        Prt::Ss,
        Prt::Wm,
        Prt::Ca,
        Prt::Is,
        Prt::Sd,
        Prt::Pp,
        Prt::It,
        Prt::Ar,
        Prt::Joat,
    ];

    /// The name the game shows.
    ///
    /// The captions of the ten radio buttons on page 4 of the race wizard
    /// (`IDD_RACE_WIZARD_4`, ids `0x10f`–`0x118`), hyphens and all. `It` is
    /// **Interstellar Traveler**; this table called it "Inner Tech", which is
    /// this project's own shorthand for the trait and not a name the game ever
    /// shows.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Prt::He => "Hyper-Expansion",
            Prt::Ss => "Super Stealth",
            Prt::Wm => "War Monger",
            Prt::Ca => "Claim Adjuster",
            Prt::Is => "Inner-Strength",
            Prt::Sd => "Space Demolition",
            Prt::Pp => "Packet Physics",
            Prt::It => "Interstellar Traveler",
            Prt::Ar => "Alternate Reality",
            Prt::Joat => "Jack of All Trades",
        }
    }

    /// The two-letter abbreviation players use.
    #[must_use]
    pub fn abbrev(self) -> &'static str {
        match self {
            Prt::He => "HE",
            Prt::Ss => "SS",
            Prt::Wm => "WM",
            Prt::Ca => "CA",
            Prt::Is => "IS",
            Prt::Sd => "SD",
            Prt::Pp => "PP",
            Prt::It => "IT",
            Prt::Ar => "AR",
            Prt::Joat => "JoaT",
        }
    }

    /// Convert the stored `rgAttr[rsMajorAdv]` byte.
    #[must_use]
    pub fn from_raw(v: i16) -> Option<Self> {
        Some(match v {
            0 => Self::He,
            1 => Self::Ss,
            2 => Self::Wm,
            3 => Self::Ca,
            4 => Self::Is,
            5 => Self::Sd,
            6 => Self::Pp,
            7 => Self::It,
            8 => Self::Ar,
            9 => Self::Joat,
            _ => return None,
        })
    }
}

/// Lesser Racial Trait bit positions within `PLAYER.grbitAttr` (`RaceGrbit`).
pub mod lrt {
    /// Improved Fuel Efficiency: engines use 15% less fuel.
    pub const IFE: u32 = 0;
    /// Total Terraforming.
    pub const TT: u32 = 1;
    /// Advanced Remote Mining.
    pub const ARM: u32 = 2;
    /// Improved Starbases.
    pub const ISB: u32 = 3;
    /// Generalized Research.
    pub const GENERALIZED_RESEARCH: u32 = 4;
    /// Ultimate Recycling.
    pub const ULTIMATE_RECYCLING: u32 = 5;
    /// Mineral Alchemy.
    pub const MINERAL_ALCHEMY: u32 = 6;
    /// No Ramscoop Engines.
    pub const NO_RAMSCOOPS: u32 = 7;
    /// Cheap Engines.
    pub const CHEAP_ENGINES: u32 = 8;
    /// Only Basic Remote Mining: +10% maximum population.
    pub const OBRM: u32 = 9;
    /// No Advanced Scanners: conventional scanner ranges doubled, no penetration.
    pub const NO_ADV_SCANNER: u32 = 10;
    /// Low Starting Population.
    pub const LOW_STARTING_POP: u32 = 11;
    /// Bleeding Edge Technology.
    pub const BLEEDING_EDGE_TECH: u32 = 12;
    /// Regenerating Shields.
    pub const REGENERATING_SHIELDS: u32 = 13;
    /// Expensive tech starts at level 3.
    pub const TECH3: u32 = 29;
    /// `ibitRaceAIPlayer`: the race is one the computer plays.
    ///
    /// Not a trait and not a wizard setting — it marks the templates the AI
    /// opponents are built from, and is the only bit the shipped `random.r1`
    /// carries.
    pub const AI_PLAYER: u32 = 30;
    /// Factories cost one less germanium.
    pub const CHEAP_FACT: u32 = 31;

    /// The **fourteen** lesser racial traits, in the order the race wizard's
    /// fifth page lists them — which is bit order.
    ///
    /// `TECH3` and `CHEAP_FACT` are not among them: they live far up the same
    /// word at bits 29 and 31 and belong to other pages of the wizard.
    pub const ALL: [u32; 14] = [
        IFE,
        TT,
        ARM,
        ISB,
        GENERALIZED_RESEARCH,
        ULTIMATE_RECYCLING,
        MINERAL_ALCHEMY,
        NO_RAMSCOOPS,
        CHEAP_ENGINES,
        OBRM,
        NO_ADV_SCANNER,
        LOW_STARTING_POP,
        BLEEDING_EDGE_TECH,
        REGENERATING_SHIELDS,
    ];

    /// What a lesser racial trait is called.
    ///
    /// The names sit at fourteen consecutive string ids, `0x0132` to `0x013f`,
    /// one per bit in this order — which is what fixes bit 5 as Ultimate
    /// Recycling, a trait this table had no name for at all until the race
    /// viewer needed to list all fourteen.
    #[must_use]
    pub fn name(bit: u32) -> Option<&'static str> {
        Some(match bit {
            IFE => "Improved Fuel Efficiency",
            TT => "Total Terraforming",
            ARM => "Advanced Remote Mining",
            ISB => "Improved Starbases",
            GENERALIZED_RESEARCH => "Generalized Research",
            ULTIMATE_RECYCLING => "Ultimate Recycling",
            MINERAL_ALCHEMY => "Mineral Alchemy",
            NO_RAMSCOOPS => "No Ramscoop Engines",
            CHEAP_ENGINES => "Cheap Engines",
            OBRM => "Only Basic Remote Mining",
            NO_ADV_SCANNER => "No Advanced Scanners",
            LOW_STARTING_POP => "Low Starting Population",
            BLEEDING_EDGE_TECH => "Bleeding Edge Technology",
            REGENERATING_SHIELDS => "Regenerating Shields",
            _ => return None,
        })
    }
}

/// A race, as far as the planetary simulation is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Race {
    /// The sixteen economy bytes, indexed by [`RaceStat`].
    pub attrs: [i16; 16],
    /// Lesser racial trait bits (`PLAYER.grbitAttr`), indexed by [`lrt`].
    pub lrt_bits: u32,
    /// Centre (ideal) of the habitable range for gravity, temperature, radiation.
    pub env_center: [i8; 3],
    /// Lower bound of the habitable range.
    pub env_min: [i8; 3],
    /// Upper bound; a negative value means the race is **immune** to that variable.
    pub env_max: [i8; 3],
    /// Maximum colonist growth rate per year, as a percentage (1..=20).
    pub pct_ideal_growth: i8,
}

impl Race {
    /// Read one economy attribute (the game's `GetRaceStat`).
    #[must_use]
    pub fn stat(&self, stat: RaceStat) -> i16 {
        self.attrs[stat as usize]
    }

    /// The primary racial trait, or `None` if the stored value is out of range.
    #[must_use]
    pub fn prt(&self) -> Option<Prt> {
        Prt::from_raw(self.stat(RaceStat::MajorAdv))
    }

    /// Test one lesser racial trait bit (the game's `GetRaceGrbit`).
    #[must_use]
    pub fn has_lrt(&self, bit: u32) -> bool {
        self.lrt_bits & (1 << bit) != 0
    }

    /// True for Alternate Reality races, which use an entirely different
    /// population, mining and scanning model.
    #[must_use]
    pub fn is_ar(&self) -> bool {
        self.prt() == Some(Prt::Ar)
    }

    /// Whether the race is immune to one environment variable.
    ///
    /// A negative upper bound marks immunity: every value of that variable is
    /// ideal, so it never needs terraforming and never limits habitability.
    #[must_use]
    pub fn is_immune(&self, variable: usize) -> bool {
        self.env_max.get(variable).is_some_and(|m| *m < 0)
    }

    /// A Humanoid-like default: every economy stat at its baseline, a
    /// symmetric habitable range, and no lesser traits. Useful for tests.
    #[must_use]
    pub fn humanoid() -> Self {
        let mut attrs = [0i16; 16];
        attrs[RaceStat::ResGen as usize] = 10;
        attrs[RaceStat::FactProd as usize] = 10;
        attrs[RaceStat::FactBuild as usize] = 10;
        attrs[RaceStat::FactOperate as usize] = 10;
        attrs[RaceStat::MineProd as usize] = 10;
        attrs[RaceStat::MineBuild as usize] = 5;
        attrs[RaceStat::MineOperate as usize] = 10;
        // `1` is the normal per-field research cost; `0` would mean "costs 75%
        // extra" in all six fields, which is not what a baseline race is.
        for field in 0..6 {
            attrs[RaceStat::TechBonus1 as usize + field] = 1;
        }
        attrs[RaceStat::MajorAdv as usize] = Prt::Joat as i16;
        Self {
            attrs,
            lrt_bits: 0,
            // Environment values are "clicks" in `0..=100`, with `-1`
            // (stored `0xFF`) meaning immune — see `docs/formats/race-r.md`.
            // The stock Humanoid is perfectly centred at 50 with a half-width
            // of 35 on all three variables.
            env_center: [50, 50, 50],
            env_min: [15, 15, 15],
            env_max: [85, 85, 85],
            pct_ideal_growth: 15,
        }
    }
}
