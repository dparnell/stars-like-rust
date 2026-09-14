//! What tells the seven computer opponents apart, as far as it has been
//! recovered: the research plan each hands `IroEnsureAi` (`1090:425a`)
//! and the share of resources it puts into research, and the shape of its
//! turn.
//!
//! Every `Do…AiTurn` opens the same way — `IroEnsureAi(plan, count,
//! &ishdefSBLatest, pct)` — and closes the same way, `HandleBasicAiTasks`
//! then `FillProductionQueue`. What lies between is the personality: its
//! design table (`Ensure…Shdefs`), its pass over its planets' queues and
//! its pass over its fleets. Only the TurinDrone's middle is transcribed
//! (`turindrone`); the Maid has none, and the other five run the
//! TurinDrone's in its place until their own are written — see
//! `docs/formulas/ai.md`, *The seven personalities*.

use super::AiPersonality;

/// How much of a personality's turn is its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Research, the basic tasks and the production queues, and nothing
    /// between: `DoMaidAiTurn` (`1098:0000`) is exactly that.
    Basic,
    /// The TurinDrone's full turn (`DoTurinDroneAiTurn`, `1088:3670`).
    TurinDrone,
    /// The Robotoid's full turn (`DoRobotoidAiTurn`, `1088:0312`).
    Robotoid,
    /// A personality whose own middle is not yet transcribed: the
    /// TurinDrone's runs for it, under its own research plan and share.
    StandIn,
}

/// One opponent's recovered constants.
#[derive(Debug, Clone, Copy)]
pub struct Profile {
    /// Which opponent.
    pub personality: AiPersonality,
    /// The research plan: a field in each byte's top three bits and a level
    /// in its low five, worked through in order. Empty for a personality
    /// that hands `IroEnsureAi` a null plan and researches whatever is
    /// lowest.
    pub plan: &'static [u8],
    /// The share of resources put into research, by the year: most
    /// personalities put nothing in for their first years.
    pub pct: fn(i16) -> u8,
    /// How much of the turn is the personality's own.
    pub shape: Shape,
}

/// `DoRobotoidAiTurn` (`1088:0312`): the thirty-six bytes at `1088:02ee`,
/// and 15 % from turn 10.
pub const ROBOTOID_PLAN: &[u8] = &[
    0x42, 0x63, 0x23, 0x64, 0x02, 0x83, 0x46, 0x25, 0x66, 0xa4, 0x85, 0x06, 0x27, 0x6a, 0x06, 0x87,
    0x2a, 0x49, 0x4c, 0x6d, 0x2e, 0x70, 0x09, 0x8a, 0x50, 0xaa, 0x0f, 0x34, 0x54, 0x90, 0xac, 0x38,
    0x93, 0x78, 0x16, 0x7a,
];
/// `DoAutomitronAiTurn` (`1098:01e0`): the eighteen bytes at `1098:01ce`,
/// and 20 % from turn 10.
pub const AUTOMITRON_PLAN: &[u8] = &[
    0x86, 0x64, 0x45, 0x25, 0xa4, 0x04, 0x87, 0x66, 0x47, 0x28, 0xa6, 0x07, 0x8c, 0x6d, 0x49, 0x2b,
    0xa7, 0x0a,
];
/// `DoCyberAiTurn` (`10a8:002a`): the forty-two bytes at `10a8:0000`, and
/// 17 % from the first turn.
pub const CYBER_PLAN: &[u8] = &[
    0x64, 0x42, 0x83, 0xa3, 0x23, 0x46, 0x66, 0x86, 0x0a, 0x6a, 0x48, 0x6d, 0x26, 0xa9, 0x89, 0x27,
    0x49, 0x70, 0x0e, 0x2b, 0x8c, 0xab, 0x4d, 0x72, 0x2f, 0x12, 0x91, 0x31, 0x74, 0x51, 0xb2, 0x95,
    0x17, 0x56, 0x75, 0x37, 0x5a, 0x9a, 0x1a, 0x3a, 0x7a, 0xba,
];
/// `DoMacintiAiTurn` (`10a0:0008`): the eight bytes at `10a0:0000`, and
/// 15 % from the first turn.
pub const MACINTI_PLAN: &[u8] = &[0x03, 0x42, 0x14, 0x71, 0x54, 0x34, 0x77, 0x37];

fn pct_from_ten(turn: i16) -> u8 {
    if turn < 10 {
        0
    } else {
        15
    }
}
fn pct_from_twenty(turn: i16) -> u8 {
    if turn < 20 {
        0
    } else {
        15
    }
}
fn pct_twenty_from_ten(turn: i16) -> u8 {
    if turn < 10 {
        0
    } else {
        20
    }
}
fn pct_fifteen(_turn: i16) -> u8 {
    15
}
fn pct_seventeen(_turn: i16) -> u8 {
    17
}

impl Profile {
    /// The profile of one opponent.
    #[must_use]
    pub fn of(personality: AiPersonality) -> Self {
        match personality {
            AiPersonality::Robotoid => Self {
                personality,
                plan: ROBOTOID_PLAN,
                pct: pct_from_ten,
                shape: Shape::Robotoid,
            },
            AiPersonality::TurinDrone => Self {
                personality,
                plan: super::turindrone::RESEARCH_PLAN,
                pct: pct_fifteen,
                shape: Shape::TurinDrone,
            },
            AiPersonality::Automitron => Self {
                personality,
                plan: AUTOMITRON_PLAN,
                pct: pct_twenty_from_ten,
                shape: Shape::StandIn,
            },
            // `DoRototillAiTurn` (`1098:1e22`) hands `IroEnsureAi` no plan
            // and 15 % from turn 20.
            AiPersonality::Rototill => Self {
                personality,
                plan: &[],
                pct: pct_from_twenty,
                shape: Shape::StandIn,
            },
            AiPersonality::Cyber => Self {
                personality,
                plan: CYBER_PLAN,
                pct: pct_seventeen,
                shape: Shape::StandIn,
            },
            AiPersonality::Macinti => Self {
                personality,
                plan: MACINTI_PLAN,
                pct: pct_fifteen,
                shape: Shape::StandIn,
            },
            // `DoMaidAiTurn` (`1098:0000`): no plan, and 15 % from turn 20.
            AiPersonality::Maid => Self {
                personality,
                plan: &[],
                pct: pct_from_twenty,
                shape: Shape::Basic,
            },
        }
    }

    /// The share of resources this personality puts into research this
    /// year.
    #[must_use]
    pub fn research_pct(&self, turn: i16) -> u8 {
        (self.pct)(turn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_plan_names_a_field_and_a_level() {
        for plan in [ROBOTOID_PLAN, AUTOMITRON_PLAN, CYBER_PLAN, MACINTI_PLAN] {
            for &entry in plan {
                assert!(usize::from(entry >> 5) < 6, "{entry:#04x} names a field");
                assert!(
                    (1..=26).contains(&(entry & 0x1f)),
                    "{entry:#04x} names a level"
                );
            }
        }
    }

    #[test]
    fn the_shares_follow_the_years() {
        assert_eq!(Profile::of(AiPersonality::Robotoid).research_pct(9), 0);
        assert_eq!(Profile::of(AiPersonality::Robotoid).research_pct(10), 15);
        assert_eq!(Profile::of(AiPersonality::Automitron).research_pct(10), 20);
        assert_eq!(Profile::of(AiPersonality::Cyber).research_pct(0), 17);
        assert_eq!(Profile::of(AiPersonality::Macinti).research_pct(0), 15);
        assert_eq!(Profile::of(AiPersonality::Maid).research_pct(19), 0);
        assert_eq!(Profile::of(AiPersonality::Maid).research_pct(20), 15);
        assert_eq!(Profile::of(AiPersonality::Rototill).research_pct(20), 15);
    }
}
