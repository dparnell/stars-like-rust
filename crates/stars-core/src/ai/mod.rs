//! Computer players.
//!
//! Stars! ships seven built-in AI opponents. Which one controls a player, and
//! whether a player is computer-controlled at all, is stored in the player
//! record; this module decodes that and describes the order in which the host
//! runs an AI's turn.
//!
//! The identification here is read straight out of `DoAiTurn` (`1088:0000`),
//! whose `switch` on the player's mode word names all seven routines.
//! [`production`] implements the AI's production-queue filling. The rest of the
//! decision-making — colonisation, war, fleet dispatch — is mapped in
//! `docs/formulas/ai.md` but not yet written.

pub mod automitron;
pub mod colonise;
pub mod cyber;
pub mod dispatch;
pub mod macinti;
pub mod parts;
pub mod personality;
pub mod production;
pub mod robotoid;
pub mod rototill;
pub mod ships;
pub mod turindrone;

/// One of the seven computer opponents Stars! ships.
///
/// The values are the `switch` cases in `DoAiTurn` (`1088:0000`), which
/// dispatches on bits 13-15 of the player's mode word. Case 6 is absent from
/// the jump table and falls through to the `default`, which does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiPersonality {
    /// `DoRobotoidAiTurn` (`1088:0312`).
    Robotoid,
    /// `DoTurinDroneAiTurn` (`1088:3670`).
    TurinDrone,
    /// `DoAutomitronAiTurn` (`1098:01e0`).
    Automitron,
    /// `DoRototillAiTurn` (`1098:1e22`).
    Rototill,
    /// `DoCyberAiTurn` (`10a8:002a`).
    Cyber,
    /// `DoMacintiAiTurn` (`10a0:0008`).
    Macinti,
    /// `DoMaidAiTurn` (`1098:0000`).
    Maid,
}

impl AiPersonality {
    /// The personality `DoAiTurn` dispatches to for a mode word, or `None` for
    /// the two values (6, and any value the jump table skips) that fall through
    /// to the `default` case and run no AI at all.
    #[must_use]
    pub fn from_mode(mode: u16) -> Option<Self> {
        Some(match mode >> 13 {
            0 => Self::Robotoid,
            1 => Self::TurinDrone,
            2 => Self::Automitron,
            3 => Self::Rototill,
            4 => Self::Cyber,
            5 => Self::Macinti,
            7 => Self::Maid,
            _ => return None,
        })
    }

    /// The three-bit dispatch value `DoAiTurn` selects on, the inverse of
    /// [`AiPersonality::from_mode`].
    #[must_use]
    pub fn mode(self) -> u8 {
        match self {
            Self::Robotoid => 0,
            Self::TurinDrone => 1,
            Self::Automitron => 2,
            Self::Rototill => 3,
            Self::Cyber => 4,
            Self::Macinti => 5,
            Self::Maid => 7,
        }
    }

    /// The name Stars! shows for this opponent.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Robotoid => "Robotoid",
            Self::TurinDrone => "Turindrone",
            Self::Automitron => "Automitron",
            Self::Rototill => "Rototill",
            Self::Cyber => "Cyber",
            Self::Macinti => "Macinti",
            Self::Maid => "Maid",
        }
    }
}

/// Who controls a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// A person, playing through a `.mN` file.
    Human,
    /// One of the built-in opponents, run by the host.
    Computer {
        /// Which opponent, if the mode word names one that `DoAiTurn` runs.
        personality: Option<AiPersonality>,
        /// The difficulty the player was created at, `0..=3` — Easy,
        /// Standard, Tough, Expert (`vrgszComputerLevel`) — from bits 2-4 of
        /// the flags byte.
        ///
        /// Confirmed against the binary: `GenerateWorld` reads this field as
        /// `(word >> 10) & 7` at `1078:1fb2` and `1078:22c3` and branches on
        /// it at 3 and 2 respectively, and the turn-0 fixtures split exactly
        /// there. See `docs/formulas/new-game.md`.
        skill_bits: u8,
    },
}

impl Control {
    /// Decode the player record's flags byte (offset 7 of a type-6 block).
    ///
    /// That byte is the high half of the player's 16-bit mode word, so
    /// `DoAiTurn`'s `mode >> 13` is this byte's top three bits. Bit 1 marks a
    /// computer player: the fixtures hold `0x01` for the human and `0x27` for
    /// both AIs.
    ///
    /// Bits 2-4 are the **difficulty**, and the field is three bits wide:
    /// `GenerateWorld` reads it as `(word >> 10) & 7` at `1078:22c3`. Only
    /// 0..=3 (Easy, Standard, Tough, Expert) occur.
    #[must_use]
    pub fn from_flags(flags: u8) -> Self {
        if flags & 0x02 == 0 {
            return Self::Human;
        }
        Self::Computer {
            personality: AiPersonality::from_mode(u16::from(flags) << 8),
            skill_bits: (flags >> 2) & 0x07,
        }
    }

    /// The flags byte this control setting is stored as, the inverse of
    /// [`Control::from_flags`].
    ///
    /// Bit 0 is set on every player block in the fixtures; bit 1 marks a
    /// computer player; bits 2-4 hold the difficulty and the top three bits the
    /// personality `DoAiTurn` dispatches on. A computer player with no
    /// recognised personality is written as the `Robotoid` slot, because a
    /// value the jump table skips would leave the host running no AI at all.
    #[must_use]
    pub fn to_flags(self) -> u8 {
        match self {
            Self::Human => 0x01,
            Self::Computer {
                personality,
                skill_bits,
            } => {
                let mode = personality.map_or(0, AiPersonality::mode) & 0x07;
                0x03 | ((skill_bits & 0x07) << 2) | (mode << 5)
            }
        }
    }

    /// Whether this player is run by the host rather than by a person.
    #[must_use]
    pub fn is_computer(self) -> bool {
        matches!(self, Self::Computer { .. })
    }
}

/// One entry of a computer player's starbase history (`vlpbAiData`,
/// twenty bytes: the planet, a count, and room for eight fleet ids).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StarbaseHistoryEntry {
    /// The planet the haulers work from.
    pub planet: i16,
    /// The haulers assigned to it, by fleet id — at most eight.
    pub fleets: Vec<u16>,
}

/// One step of an AI player's turn, in the order `DoAiTurn` runs them.
///
/// `DoAiTurn` reloads the game from disk as the AI player, prepares the shared
/// working state, dispatches to the personality routine, and writes the log and
/// history files back out. The preparation steps are shared by all seven
/// personalities; only [`Self::Personality`] differs between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnStep {
    /// `ComputeShdefPowers` — rate every ship design the AI can see.
    RateShipDesigns,
    /// `MarkPlanetsUnderAttack` — flag the AI's planets that have enemies in
    /// orbit, which the personality routines use to prioritise defence.
    MarkPlanetsUnderAttack,
    /// `IncreaseAIMinefieldSizes` — the AI's minefields grow for free, which is
    /// one of the ways a computer opponent is handed an advantage.
    GrowMinefields,
    /// `InitRandomPlanetList` — collect every planet the player owns and
    /// shuffle it. See [`planet_order`].
    ShufflePlanets,
    /// The personality's own routine.
    Personality,
}

/// The steps of an AI turn, in order.
///
/// Source: `DoAiTurn` (`1088:0000`).
pub const TURN_STEPS: [TurnStep; 5] = [
    TurnStep::RateShipDesigns,
    TurnStep::MarkPlanetsUnderAttack,
    TurnStep::GrowMinefields,
    TurnStep::ShufflePlanets,
    TurnStep::Personality,
];

/// The order in which an AI considers its planets, and the list every
/// per-planet AI routine walks.
///
/// Source: `InitRandomPlanetList` (`1090:a0d7`), one of the four preparation
/// steps in [`TURN_STEPS`]. It collects **every** planet the player owns — it
/// is not a filtered working set — and then shuffles it, so the AI does not
/// always consider its planets in id order.
///
/// The shuffle is a forward Fisher-Yates using the game's own `Random`: for
/// each position `i` below the last, swap with a position drawn from
/// `i .. i + Random(n - i)`. It is skipped when bit 11 of the game flags word
/// is set (the same flag `FFillProdMinesAndFactories` tests when costing
/// factories), which makes an AI turn reproducible for debugging.
///
/// `planets` is the planet ids the player owns, in id order.
#[must_use]
pub fn planet_order(planets: &[i16], rng: &mut crate::rng::Rng, shuffle: bool) -> Vec<i16> {
    let mut out = planets.to_vec();
    if !shuffle {
        return out;
    }
    let n = i16::try_from(out.len()).unwrap_or(i16::MAX);
    for i in 0..out.len().saturating_sub(1) {
        let span = n - i16::try_from(i).unwrap_or(0);
        let j = i + usize::try_from(rng.random(span)).unwrap_or(0);
        if j < out.len() {
            out.swap(i, j);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_flags_decode_as_human() {
        assert_eq!(Control::from_flags(0x01), Control::Human);
    }

    /// The value both AI players carry in `fixtures/incoming/turn1/Game.hst`.
    #[test]
    fn ai_flags_decode_as_turindrone() {
        assert_eq!(
            Control::from_flags(0x27),
            Control::Computer {
                personality: Some(AiPersonality::TurinDrone),
                skill_bits: 1,
            }
        );
    }

    /// The planet list is a permutation of everything the player owns — the
    /// routine filters nothing out.
    #[test]
    fn planet_order_keeps_every_planet() {
        let planets: Vec<i16> = (0..25).collect();
        let mut rng = crate::rng::Rng::randomize(7);
        let shuffled = planet_order(&planets, &mut rng, true);
        let mut sorted = shuffled.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, planets);
        assert_ne!(shuffled, planets, "a 25-planet shuffle should reorder");

        let mut rng = crate::rng::Rng::randomize(7);
        assert_eq!(planet_order(&planets, &mut rng, false), planets);
    }

    /// `DoAiTurn`'s jump table has no case 6; it falls through to `default`.
    #[test]
    fn mode_six_runs_no_ai() {
        assert_eq!(AiPersonality::from_mode(6 << 13), None);
        assert_eq!(AiPersonality::from_mode(7 << 13), Some(AiPersonality::Maid));
    }
}
