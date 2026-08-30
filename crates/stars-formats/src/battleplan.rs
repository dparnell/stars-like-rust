//! Typed decoder for the **battle plan** block (type id 30, `BATTLE_PLAN`,
//! "Battle Orders") that appears in `.mN`/`.hst` files.
//!
//! Each player carries a small set of battle plans (a fresh game ships the
//! defaults: *Default*, *Kill Starbase*, *Max-Defense*, *Sniper*, *Chicken*).
//! A battle plan describes how a fleet using it behaves in combat: its tactic,
//! primary/secondary target preferences, and which races it will attack.
//!
//! The layout was recovered from the stars-4x `decompiled` project
//! (`Structures/Structure30.xml`) and verified against
//! `fixtures/incoming/turn0/Game.hst`, whose type-30 blocks decode to the
//! canonical default plan names for each player.
//!
//! ## Layout (fixed 5-byte header + packed name)
//!
//! | Offset | Bits / Size | Field                                          |
//! |--------|-------------|------------------------------------------------|
//! | 0      | low 4 bits  | race / player id                               |
//! | 0      | high 4 bits | battle-plan id                                 |
//! | 1      | 1 byte      | tactic                                         |
//! | 2      | low 4 bits  | primary target                                 |
//! | 2      | high 4 bits | secondary target                               |
//! | 3      | 1 byte      | "attack who" (0 = nobody … 3 = everyone, 4+ = a specific race id) |
//! | 4      | 1 byte      | name length                                    |
//! | 5..    | var         | packed name (Stars! nibble encoding)           |
//!
//! Like the other record decoders this is an *interpreted, read-only view*;
//! byte-exact write-back still goes through the container in [`crate::file`].

use crate::block::BlockType;
use crate::file::StarsFile;
use crate::strings;
use crate::{FormatError, Result};

/// The fixed part of a battle-plan block preceding the packed name.
const HEADER_LEN: usize = 4;

/// A decoded battle plan (type-30 block).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattlePlanRecord {
    /// Owning race / player id (low nibble of byte 0).
    pub race_id: u8,
    /// Battle-plan id / slot (high nibble of byte 0).
    pub plan_id: u8,
    /// Tactic value (e.g. disengage / minimize damage / maximize damage …),
    /// exposed raw.
    pub tactic: u8,
    /// Primary target preference (low nibble of byte 2), exposed raw.
    pub primary_target: u8,
    /// Secondary target preference (high nibble of byte 2), exposed raw.
    pub secondary_target: u8,
    /// "Attack who" selector: `0` = nobody, `1` = enemies, `2` = neutrals &
    /// enemies, `3` = everyone, `4..` = a specific race id (`value - 4`).
    pub attack_who: u8,
    /// Plan name (e.g. `"Default"`, `"Kill Starbase"`).
    pub name: String,
}

impl BattlePlanRecord {
    /// Decode a **decrypted** type-30 battle-plan block payload.
    ///
    /// # Errors
    /// Returns [`FormatError::Malformed`] if the payload is shorter than the
    /// 5-byte header or the declared name runs past the end of the block.
    pub fn from_payload(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_LEN + 1 {
            return Err(FormatError::Malformed(format!(
                "battle-plan block ({} bytes) shorter than the {}-byte header",
                data.len(),
                HEADER_LEN + 1
            )));
        }
        let race_id = data[0] & 0x0F;
        let plan_id = data[0] >> 4;
        let tactic = data[1];
        let primary_target = data[2] & 0x0F;
        let secondary_target = data[2] >> 4;
        let attack_who = data[3];

        let name_len = data[HEADER_LEN] as usize;
        let name_end = HEADER_LEN + name_len;
        if name_end >= data.len() && name_len > 0 {
            return Err(FormatError::Malformed(
                "battle-plan: name field runs past end of block".into(),
            ));
        }
        let name = strings::decode_field(&data[HEADER_LEN..=name_end.min(data.len() - 1)]);

        Ok(Self {
            race_id,
            plan_id,
            tactic,
            primary_target,
            secondary_target,
            attack_who,
            name,
        })
    }
}

/// Decode every battle-plan block (type 30) in a decoded [`StarsFile`], in file
/// order.
///
/// # Errors
/// Returns [`FormatError::Malformed`] if any battle-plan block is malformed.
pub fn battle_plan_records(file: &StarsFile) -> Result<Vec<BattlePlanRecord>> {
    file.blocks
        .iter()
        .filter(|b| b.block_type() == BlockType::BattlePlan)
        .map(|b| BattlePlanRecord::from_payload(&b.data))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_truncated() {
        assert!(BattlePlanRecord::from_payload(&[0u8; 4]).is_err());
    }

    #[test]
    fn splits_header_nibbles() {
        // race 2, plan 5, tactic 4, primary 3, secondary 1, attack-who 2,
        // empty name (length 0).
        let d = vec![0x52, 0x04, 0x13, 0x02, 0x00];
        let p = BattlePlanRecord::from_payload(&d).unwrap();
        assert_eq!(p.race_id, 2);
        assert_eq!(p.plan_id, 5);
        assert_eq!(p.tactic, 4);
        assert_eq!(p.primary_target, 3);
        assert_eq!(p.secondary_target, 1);
        assert_eq!(p.attack_who, 2);
        assert_eq!(p.name, "");
    }

    #[test]
    fn rejects_name_past_end() {
        // name length says 10 but no bytes follow.
        let d = vec![0x00, 0x00, 0x00, 0x00, 0x0A];
        assert!(BattlePlanRecord::from_payload(&d).is_err());
    }
}
