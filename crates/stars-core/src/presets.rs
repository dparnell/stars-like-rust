//! The races the race wizard starts from.
//!
//! Page 1 of the Custom Race Wizard offers eight buttons — seven named races
//! and `Custom` — and pressing one loads that race into the wizard, ready to be
//! changed. The seven are `vrgplrDef` (`1120:0da2`), a `PLAYER[7]` in the data
//! segment, and they are exactly the seven `.r1` files that ship with the game.
//!
//! The names are the game's own, string ids `0x0567` to `0x056d`, which are
//! also the button captions: the game spells two of them differently from the
//! files they are shipped in (`Nucleotid` in `nucleoid.r1`, `Antetheral` in
//! `antetherial.r1`), and the string table is what the wizard fills the name
//! box with.
//!
//! Each entry is checked against its shipped file in
//! `crates/stars-core/tests/race_writing.rs`: writing the preset out has to
//! produce the same race record the original wrote.
//!
//! `Random` is the seventh, and it is a real race rather than a re-roll: it is
//! the template the New Game dialog's random players are built from, and the
//! only one carrying `ibitRaceAIPlayer`.

use crate::race::Race;

/// One race the wizard can start from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    /// The singular name, which is also the wizard's button caption.
    pub name: &'static str,
    /// The plural name.
    pub plural: &'static str,
    /// Which race emblem it wears (`PLAYER.iPlrBmp`).
    pub emblem: u8,
    /// The race itself.
    pub race: Race,
}

/// Look one up by its position on the wizard's first page.
#[must_use]
pub fn preset(index: usize) -> Option<&'static Preset> {
    ALL.get(index)
}

/// The seven predefined races, in `vrgplrDef` order — which is the order the
/// wizard's buttons are in.
pub static ALL: [Preset; 7] = [
    Preset {
        name: "Humanoid",
        plural: "Humanoids",
        emblem: 1,
        race: Race {
            attrs: [10, 10, 10, 10, 10, 5, 10, 0, 1, 1, 1, 1, 1, 1, 9, 0],
            lrt_bits: 0x0000_0000,
            env_center: [50, 50, 50],
            env_min: [15, 15, 15],
            env_max: [85, 85, 85],
            pct_ideal_growth: 15,
        },
    },
    Preset {
        name: "Rabbitoid",
        plural: "Rabbitoids",
        emblem: 12,
        race: Race {
            attrs: [10, 10, 9, 17, 10, 9, 10, 4, 0, 0, 2, 1, 1, 2, 7, 0],
            lrt_bits: 0x8000_0503,
            env_center: [33, 58, 33],
            env_min: [10, 35, 13],
            env_max: [56, 81, 53],
            pct_ideal_growth: 20,
        },
    },
    Preset {
        name: "Insectoid",
        plural: "Insectoids",
        emblem: 4,
        race: Race {
            attrs: [10, 10, 10, 10, 9, 10, 6, 1, 2, 2, 2, 2, 1, 0, 2, 0],
            lrt_bits: 0x0000_2108,
            // Immune to gravity: the axis is stored as three `-1`s.
            env_center: [-1, 50, 85],
            env_min: [-1, 0, 70],
            env_max: [-1, 100, 100],
            pct_ideal_growth: 10,
        },
    },
    Preset {
        name: "Nucleotid",
        plural: "Nucleotids",
        emblem: 25,
        race: Race {
            attrs: [9, 10, 10, 10, 10, 15, 5, 3, 0, 0, 0, 0, 0, 0, 1, 0],
            lrt_bits: 0x2000_000c,
            env_center: [-1, 50, 50],
            env_min: [-1, 12, 0],
            env_max: [-1, 88, 100],
            pct_ideal_growth: 10,
        },
    },
    Preset {
        name: "Silicanoid",
        plural: "Silicanoids",
        emblem: 5,
        race: Race {
            attrs: [8, 12, 12, 15, 10, 9, 10, 3, 1, 1, 2, 2, 1, 0, 0, 0],
            lrt_bits: 0x0000_1221,
            // Immune to all three.
            env_center: [-1, -1, -1],
            env_min: [-1, -1, -1],
            env_max: [-1, -1, -1],
            pct_ideal_growth: 6,
        },
    },
    Preset {
        name: "Antetheral",
        plural: "Antetherals",
        emblem: 18,
        race: Race {
            attrs: [7, 11, 10, 18, 10, 10, 10, 0, 2, 0, 2, 2, 2, 2, 5, 0],
            lrt_bits: 0x0000_05c4,
            env_center: [15, 50, 85],
            env_min: [0, 0, 70],
            env_max: [30, 100, 100],
            pct_ideal_growth: 7,
        },
    },
    Preset {
        name: "Random",
        plural: "Randoms",
        emblem: 31,
        race: Race {
            attrs: [10, 10, 10, 10, 10, 3, 10, 0, 1, 1, 1, 1, 1, 1, 0, 0],
            lrt_bits: 0x4000_0000,
            env_center: [50, 50, 50],
            env_min: [17, 17, 17],
            env_max: [83, 83, 83],
            pct_ideal_growth: 15,
        },
    },
];
