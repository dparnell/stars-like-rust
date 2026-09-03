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

pub mod production;

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
        /// Bits 2-3 of the flags byte. Believed to be the difficulty the player
        /// was created at, but *not* confirmed against the binary: every AI in
        /// the fixtures has the same value, so nothing distinguishes this from
        /// any other two-bit field. Exposed raw rather than named.
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
    #[must_use]
    pub fn from_flags(flags: u8) -> Self {
        if flags & 0x02 == 0 {
            return Self::Human;
        }
        Self::Computer {
            personality: AiPersonality::from_mode(u16::from(flags) << 8),
            skill_bits: (flags >> 2) & 0x03,
        }
    }

    /// Whether this player is run by the host rather than by a person.
    #[must_use]
    pub fn is_computer(self) -> bool {
        matches!(self, Self::Computer { .. })
    }
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
    /// `InitRandomPlanetList` — shuffle the planet list, so the AI does not
    /// always consider planets in id order.
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

    /// `DoAiTurn`'s jump table has no case 6; it falls through to `default`.
    #[test]
    fn mode_six_runs_no_ai() {
        assert_eq!(AiPersonality::from_mode(6 << 13), None);
        assert_eq!(AiPersonality::from_mode(7 << 13), Some(AiPersonality::Maid));
    }
}
