//! The built-in computer players.
//!
//! Stars! ships six computer personalities at four difficulties each, as a
//! 6x4 table of fully-formed races (`vrgplrComp`, reached through
//! `LpplrComp(idAi, lvlAi)`). Choosing "Turindrones, Tough" in the New Game
//! wizard picks one of these; nothing is generated.
//!
//! The names are the game's own (`vrgszComputerPlayers` and
//! `vrgszComputerLevel` in `globals.c`), and the personalities line up with the
//! `DoAiTurn` dispatch — Robotoids is Hyper Expansion, Turindrones Super
//! Stealth, Automitrons Inner Strength, Rototills Claim Adjuster, Cybertrons
//! Packet Physics and Macinti Alternate Reality.
//!
//! ## The flags above bit 13
//!
//! Each entry's `grbitAttr` sets bits outside the fourteen lesser racial traits
//! a player picks in the wizard: bit 29 (`ibitRaceTech3`) and bit 31
//! (`ibitRaceCheapFact`) on several entries. Those are **not** an artefact of
//! the reconstruction. A race record stores its lesser traits in a sixteen-bit
//! field, but it stores those two separately, as bits 5 and 7 of the checkbox
//! byte at offset 81 — and the turn-0 fixture's two computer players, which are
//! `Turindrones, Standard` field for field, carry the Cheap Factories checkbox
//! exactly as this table's bit 31 says they should.
//!
//! The whole `grbitAttr` is therefore kept. See `docs/formulas/new-game.md`
//! for the one thing that does **not** then add up: the leftover advantage
//! points those players' homeworlds were stocked with.

use crate::ai::{AiPersonality, Control};
use crate::newgame::NewPlayer;
use crate::race::Race;

/// One built-in computer player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opponent {
    /// Which personality, `0..6`, in `vrgplrComp`'s order.
    pub personality: usize,
    /// Which difficulty, `0..4` (Easy to Expert).
    pub level: usize,
    /// The race it plays.
    pub race: Race,
}

impl Opponent {
    /// The personality this opponent's turn is run by.
    #[must_use]
    pub fn ai(&self) -> Option<AiPersonality> {
        Some(match self.personality {
            0 => AiPersonality::Robotoid,
            1 => AiPersonality::TurinDrone,
            2 => AiPersonality::Automitron,
            3 => AiPersonality::Rototill,
            4 => AiPersonality::Cyber,
            5 => AiPersonality::Macinti,
            _ => return None,
        })
    }

    /// The name the game shows, e.g. `"Turindrones, Standard"`.
    #[must_use]
    pub fn name(&self) -> String {
        format!(
            "{}, {}",
            PERSONALITY_NAMES
                .get(self.personality)
                .copied()
                .unwrap_or("Random"),
            LEVEL_NAMES.get(self.level).copied().unwrap_or("Random")
        )
    }

    /// This opponent as a player of a game about to be created.
    #[must_use]
    pub fn as_player(&self) -> NewPlayer {
        NewPlayer {
            race: self.race.clone(),
            name: PERSONALITY_SINGULAR
                .get(self.personality)
                .copied()
                .unwrap_or("Humanoid")
                .to_string(),
            plural_name: PERSONALITY_NAMES
                .get(self.personality)
                .copied()
                .unwrap_or("Humanoids")
                .to_string(),
            control: Control::Computer {
                personality: self.ai(),
                #[allow(clippy::cast_possible_truncation)]
                skill_bits: self.level as u8,
            },
        }
    }
}

/// The computer personalities, in the game's order (`vrgszComputerPlayers`).
pub const PERSONALITY_NAMES: [&str; 6] = [
    "Robotoids",
    "Turindrones",
    "Automitrons",
    "Rototills",
    "Cybertrons",
    "Macinti",
];

/// The singular of each personality's name, for the race name a computer
/// player's file carries. The game's own table is plural.
pub const PERSONALITY_SINGULAR: [&str; 6] = [
    "Robotoid",
    "Turindrone",
    "Automitron",
    "Rototill",
    "Cybertron",
    "Macinti",
];

/// The difficulty levels, in the game's order (`vrgszComputerLevel`).
pub const LEVEL_NAMES: [&str; 4] = ["Easy", "Standard", "Tough", "Expert"];

/// Look one opponent up by personality and difficulty.
#[must_use]
pub fn opponent(personality: usize, level: usize) -> Option<&'static Opponent> {
    ALL.iter()
        .find(|o| o.personality == personality && o.level == level)
}

/// Every built-in computer player, personality-major (`vrgplrComp`).
pub static ALL: [Opponent; 24] = [
    Opponent {
        personality: 0,
        level: 0,
        race: Race {
            attrs: [10, 12, 10, 16, 10, 5, 10, 0, 1, 1, 2, 1, 1, 0, 0, 0],
            lrt_bits: 0x00001341,
            env_center: [0, 0, 0],
            env_min: [0, 0, 0],
            env_max: [-1, -1, -1],
            pct_ideal_growth: 5,
        },
    },
    Opponent {
        personality: 0,
        level: 1,
        race: Race {
            attrs: [9, 13, 9, 16, 10, 4, 11, 0, 1, 1, 2, 1, 1, 0, 0, 0],
            lrt_bits: 0x00000341,
            env_center: [0, 0, 0],
            env_min: [0, 0, 0],
            env_max: [-1, -1, -1],
            pct_ideal_growth: 6,
        },
    },
    Opponent {
        personality: 0,
        level: 2,
        race: Race {
            attrs: [8, 13, 9, 18, 10, 4, 12, 0, 1, 2, 2, 2, 1, 0, 0, 0],
            lrt_bits: 0x80000261,
            env_center: [0, 0, 0],
            env_min: [0, 0, 0],
            env_max: [-1, -1, -1],
            pct_ideal_growth: 6,
        },
    },
    Opponent {
        personality: 0,
        level: 3,
        race: Race {
            attrs: [8, 13, 9, 16, 10, 4, 8, 0, 1, 2, 1, 2, 1, 0, 0, 0],
            lrt_bits: 0x80000261,
            env_center: [0, 0, 0],
            env_min: [0, 0, 0],
            env_max: [-1, -1, -1],
            pct_ideal_growth: 7,
        },
    },
    Opponent {
        personality: 1,
        level: 0,
        race: Race {
            attrs: [10, 9, 10, 9, 9, 5, 8, 0, 1, 0, 1, 1, 1, 0, 1, 0],
            lrt_bits: 0x00002045,
            env_center: [58, 35, 65],
            env_min: [27, 7, 35],
            env_max: [89, 63, 95],
            pct_ideal_growth: 14,
        },
    },
    Opponent {
        personality: 1,
        level: 1,
        race: Race {
            attrs: [10, 10, 10, 10, 10, 5, 9, 0, 1, 1, 1, 1, 1, 1, 1, 0],
            lrt_bits: 0x80002045,
            env_center: [62, 33, 61],
            env_min: [32, 6, 26],
            env_max: [92, 60, 96],
            pct_ideal_growth: 14,
        },
    },
    Opponent {
        personality: 1,
        level: 2,
        race: Race {
            attrs: [9, 11, 10, 10, 10, 5, 9, 0, 0, 1, 0, 1, 1, 1, 1, 0],
            lrt_bits: 0x80002045,
            env_center: [63, 28, 62],
            env_min: [31, 4, 30],
            env_max: [95, 52, 94],
            pct_ideal_growth: 14,
        },
    },
    Opponent {
        personality: 1,
        level: 3,
        race: Race {
            attrs: [8, 15, 10, 25, 10, 5, 9, 0, 0, 0, 0, 0, 0, 0, 1, 0],
            lrt_bits: 0xa0002045,
            env_center: [62, 29, 0],
            env_min: [31, 5, 0],
            env_max: [93, 53, -1],
            pct_ideal_growth: 15,
        },
    },
    Opponent {
        personality: 2,
        level: 0,
        race: Race {
            attrs: [9, 11, 10, 14, 11, 6, 14, 1, 0, 0, 0, 0, 0, 0, 4, 0],
            lrt_bits: 0x20000f10,
            env_center: [35, 60, 38],
            env_min: [7, 26, 5],
            env_max: [63, 94, 71],
            pct_ideal_growth: 15,
        },
    },
    Opponent {
        personality: 2,
        level: 1,
        race: Race {
            attrs: [8, 13, 9, 14, 10, 6, 14, 1, 0, 0, 0, 0, 0, 0, 4, 0],
            lrt_bits: 0xa0000f10,
            env_center: [35, 60, 38],
            env_min: [7, 26, 5],
            env_max: [63, 94, 71],
            pct_ideal_growth: 15,
        },
    },
    Opponent {
        personality: 2,
        level: 2,
        race: Race {
            attrs: [8, 14, 9, 15, 14, 5, 15, 1, 0, 0, 0, 0, 0, 0, 4, 0],
            lrt_bits: 0xa0000e10,
            env_center: [35, 60, 38],
            env_min: [7, 26, 5],
            env_max: [63, 94, 71],
            pct_ideal_growth: 15,
        },
    },
    Opponent {
        personality: 2,
        level: 3,
        race: Race {
            attrs: [8, 14, 9, 14, 14, 5, 14, 1, 0, 0, 0, 0, 0, 0, 4, 0],
            lrt_bits: 0xa0000e10,
            env_center: [35, 0, 50],
            env_min: [7, 0, 0],
            env_max: [63, -1, 100],
            pct_ideal_growth: 16,
        },
    },
    Opponent {
        personality: 3,
        level: 0,
        race: Race {
            attrs: [10, 10, 10, 10, 10, 5, 10, 1, 0, 0, 0, 0, 0, 2, 3, 0],
            lrt_bits: 0x20001f02,
            env_center: [50, 50, 50],
            env_min: [32, 31, 31],
            env_max: [68, 69, 69],
            pct_ideal_growth: 15,
        },
    },
    Opponent {
        personality: 3,
        level: 1,
        race: Race {
            attrs: [8, 12, 10, 12, 14, 5, 12, 1, 0, 0, 0, 0, 0, 2, 3, 0],
            lrt_bits: 0x20001e02,
            env_center: [50, 50, 50],
            env_min: [32, 31, 31],
            env_max: [68, 69, 69],
            pct_ideal_growth: 15,
        },
    },
    Opponent {
        personality: 3,
        level: 2,
        race: Race {
            attrs: [8, 12, 10, 12, 14, 5, 12, 1, 0, 0, 0, 0, 0, 2, 3, 0],
            lrt_bits: 0x20001e02,
            env_center: [50, 50, 50],
            env_min: [23, 24, 25],
            env_max: [77, 76, 75],
            pct_ideal_growth: 15,
        },
    },
    Opponent {
        personality: 3,
        level: 3,
        race: Race {
            attrs: [8, 15, 10, 15, 15, 5, 15, 1, 0, 0, 0, 0, 0, 2, 3, 0],
            lrt_bits: 0x20001e02,
            env_center: [0, 50, 50],
            env_min: [0, 24, 25],
            env_max: [-1, 76, 75],
            pct_ideal_growth: 15,
        },
    },
    Opponent {
        personality: 4,
        level: 0,
        race: Race {
            attrs: [10, 9, 18, 9, 9, 10, 8, 1, 1, 0, 0, 1, 0, 0, 6, 0],
            lrt_bits: 0x20000a03,
            env_center: [50, 50, 50],
            env_min: [22, 22, 22],
            env_max: [78, 78, 78],
            pct_ideal_growth: 12,
        },
    },
    Opponent {
        personality: 4,
        level: 1,
        race: Race {
            attrs: [10, 10, 13, 19, 10, 10, 7, 1, 1, 0, 0, 1, 1, 1, 6, 0],
            lrt_bits: 0x20000e03,
            env_center: [50, 50, 50],
            env_min: [19, 19, 19],
            env_max: [81, 81, 81],
            pct_ideal_growth: 17,
        },
    },
    Opponent {
        personality: 4,
        level: 2,
        race: Race {
            attrs: [10, 14, 10, 20, 10, 10, 6, 1, 1, 1, 0, 1, 1, 2, 6, 0],
            lrt_bits: 0xa0000e43,
            env_center: [50, 50, 50],
            env_min: [18, 18, 18],
            env_max: [82, 82, 82],
            pct_ideal_growth: 17,
        },
    },
    Opponent {
        personality: 4,
        level: 3,
        race: Race {
            attrs: [10, 15, 9, 25, 10, 10, 5, 1, 2, 2, 0, 2, 1, 1, 6, 0],
            lrt_bits: 0xa0000e43,
            env_center: [50, 50, 50],
            env_min: [17, 17, 17],
            env_max: [83, 83, 83],
            pct_ideal_growth: 19,
        },
    },
    Opponent {
        personality: 5,
        level: 0,
        race: Race {
            attrs: [16, 10, 10, 10, 10, 5, 10, 0, 1, 1, 1, 1, 0, 1, 8, 0],
            lrt_bits: 0x0000011b,
            env_center: [50, 50, 50],
            env_min: [20, 20, 20],
            env_max: [80, 80, 80],
            pct_ideal_growth: 10,
        },
    },
    Opponent {
        personality: 5,
        level: 1,
        race: Race {
            attrs: [12, 10, 10, 10, 10, 5, 10, 0, 2, 1, 1, 1, 0, 1, 8, 0],
            lrt_bits: 0x0000001b,
            env_center: [50, 50, 50],
            env_min: [15, 15, 15],
            env_max: [85, 85, 85],
            pct_ideal_growth: 14,
        },
    },
    Opponent {
        personality: 5,
        level: 2,
        race: Race {
            attrs: [10, 10, 10, 10, 10, 5, 10, 0, 2, 1, 1, 1, 1, 1, 8, 0],
            lrt_bits: 0x0000007f,
            env_center: [50, 50, 50],
            env_min: [15, 15, 15],
            env_max: [85, 85, 85],
            pct_ideal_growth: 17,
        },
    },
    Opponent {
        personality: 5,
        level: 3,
        race: Race {
            attrs: [10, 10, 10, 10, 10, 5, 10, 0, 2, 1, 1, 2, 1, 1, 8, 0],
            lrt_bits: 0x0000007f,
            env_center: [50, 50, 50],
            env_min: [15, 15, 15],
            env_max: [85, 85, 85],
            pct_ideal_growth: 20,
        },
    },
];
