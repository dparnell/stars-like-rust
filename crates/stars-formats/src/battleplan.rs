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
//! | 1      | low 4 bits  | tactic                                         |
//! | 1      | bit 6       | deleted ([`PLAN_DELETED`])                     |
//! | 1      | bit 7       | dump cargo ([`PLAN_DUMP_CARGO`])               |
//! | 2      | low 4 bits  | primary target                                 |
//! | 2      | high 4 bits | secondary target                               |
//! | 3      | 1 byte      | "attack who" (0 = nobody … 3 = everyone, 4+ = a specific race id) |
//! | 4      | 1 byte      | name length                                    |
//! | 5..    | var         | packed name (Stars! nibble encoding)           |
//!
//! [`BattlePlanRecord::encode`] is an exact inverse of
//! [`BattlePlanRecord::from_payload`], packed name included.

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
    /// Any bytes after the name field, kept so the block re-encodes exactly.
    pub trailing: Vec<u8>,
}

/// Bit 6 of byte 1: the plan has been **deleted**.
///
/// Byte 1 is not a whole tactic value. The replay arm reads the tactic from its
/// low nibble (`1048:c31c`, bounded to `0..=6`) and this bit from its high one
/// (`1048:c2d4`, testing bit 14 of the first *word*). Every plan in the
/// fixtures has a zero high nibble, so the field decodes the same either way;
/// it is kept whole here and read through [`BattlePlanRecord::tactic_nibble`]
/// and [`BattlePlanRecord::deleted`].
pub const PLAN_DELETED: u8 = 0x40;

/// Bit 7 of byte 1: **dump cargo** at the start of the battle.
///
/// The `fDumpCargo` bit of `BTLPLAN.iplr` (`iplr:4, iplan:4, mdTactic:4,
/// unused1:2, fDelete:1, fDumpCargo:1`), and the Battle Plans dialog's
/// `D&ump Cargo` checkbox — `BattlePlansDlg` reads it as `wFlags >> 15` and
/// writes it back into the same bit. `MANUAL.PDF` p. 15-14: "Dump Cargo —
/// Jettison cargo at the start of battle."
pub const PLAN_DUMP_CARGO: u8 = 0x80;

impl BattlePlanRecord {
    /// The tactic proper: the low nibble of [`Self::tactic`]. The original
    /// refuses a value above 6.
    #[must_use]
    pub fn tactic_nibble(&self) -> u8 {
        self.tactic & 0x0F
    }

    /// Whether byte 1 marks the plan deleted ([`PLAN_DELETED`]).
    #[must_use]
    pub fn deleted(&self) -> bool {
        self.tactic & PLAN_DELETED != 0
    }

    /// Whether the plan jettisons its cargo at the start of a battle
    /// ([`PLAN_DUMP_CARGO`]).
    #[must_use]
    pub fn dump_cargo(&self) -> bool {
        self.tactic & PLAN_DUMP_CARGO != 0
    }

    /// Set the tactic nibble, leaving the flags above it alone.
    pub fn set_tactic(&mut self, tactic: u8) {
        self.tactic = (self.tactic & 0xF0) | (tactic & 0x0F);
    }

    /// Set or clear [`PLAN_DUMP_CARGO`].
    pub fn set_dump_cargo(&mut self, on: bool) {
        if on {
            self.tactic |= PLAN_DUMP_CARGO;
        } else {
            self.tactic &= !PLAN_DUMP_CARGO;
        }
    }

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
            trailing: data
                .get(HEADER_LEN + name_len + 1..)
                .unwrap_or_default()
                .to_vec(),
        })
    }

    /// Re-encode this battle plan as a type-30 block payload.
    ///
    /// # Errors
    /// [`FormatError::Malformed`] if the name does not fit its length byte.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(16);
        out.push((self.race_id & 0x0F) | (self.plan_id << 4));
        out.push(self.tactic);
        out.push((self.primary_target & 0x0F) | (self.secondary_target << 4));
        out.push(self.attack_who);
        out.extend_from_slice(&strings::encode_field(&self.name)?);
        out.extend_from_slice(&self.trailing);
        Ok(out)
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
