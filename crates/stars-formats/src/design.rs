//! Typed decoder for the **ship/starbase design** block (type id 26) that
//! appears in `.mN`, `.hst` and `.hN` files.
//!
//! A design block describes one hull design a player has created. There are two
//! shapes:
//!
//! * a **full design** (`fullData` bit set) — hull, picture, armor, per-slot
//!   equipment, the turn it was designed and how many have been built/remain; and
//! * a **partial design** — just the identity plus a stored `mass` (used when a
//!   player only *sees* a foreign design, not its internals).
//!
//! The layout was recovered from the stars-4x `starsapi` project
//! (`DesignBlock.java`) and verified against
//! `fixtures/incoming/turn0/Game.hst`, whose starting designs decode to the
//! canonical names/hulls: player 0's *Armed Probe* (hull 4, 3 slots), *Santa
//! Maria* colony ship (hull 15), *Stalwart Defender* (hull 6, 7 slots), and the
//! shared *Starbase* (hull 34, 12 slots, armor 1000).
//!
//! This is an *interpreted, read-only view*; byte-exact write-back still goes
//! through the container in [`crate::file`]. The design's *mass* and *fuel
//! capacity* of a full design are **computed** by the game from a hull/component
//! table that this formats crate does not carry, so they are intentionally not
//! surfaced for full designs (only the directly-stored partial-design mass is).

use crate::block::BlockType;
use crate::file::StarsFile;
use crate::strings;
use crate::{FormatError, Result};

/// One equipment slot of a full design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    /// Tech category bitmask (e.g. engine, beam weapon, armor, scanner …), as
    /// stored in the low bits of the 16-bit category word.
    pub category: u16,
    /// Item id within the category.
    pub item_id: u8,
    /// Number of this item fitted in the slot (`0` = empty slot).
    pub count: u8,
}

/// A decoded ship or starbase design (type-26 block).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignRecord {
    /// Whether this is a full design (internals present) or a partial design
    /// (identity + mass only).
    pub full_design: bool,
    /// Whether the design was transferred from another player.
    pub transferred: bool,
    /// Whether this is a starbase design (vs a ship design).
    pub starbase: bool,
    /// Design slot number (0..=15) within the player's ship or starbase list.
    pub design_number: u8,
    /// Hull id (indexes the game's hull table).
    pub hull_id: u8,
    /// Picture / icon index.
    pub pic: u8,
    /// Armor points — full designs only (`None` for partial designs).
    pub armor: Option<u16>,
    /// Stored mass — partial designs only (`None` for full designs, whose mass
    /// the game computes from the hull + fitted components).
    pub mass: Option<u16>,
    /// The turn (year offset) the design was created — full designs only.
    pub turn_designed: Option<u16>,
    /// Total number of this design ever built — full designs only.
    pub total_built: Option<u32>,
    /// Number of this design currently in existence — full designs only.
    pub total_remaining: Option<u32>,
    /// Equipment slots — full designs only.
    pub slots: Vec<Slot>,
    /// Design name (e.g. `"Armed Probe"`, `"Starbase"`).
    pub name: String,
}

fn read16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn read32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *d.get(o)?,
        *d.get(o + 1)?,
        *d.get(o + 2)?,
        *d.get(o + 3)?,
    ]))
}

impl DesignRecord {
    /// Decode a **decrypted** type-26 design block payload.
    ///
    /// # Errors
    /// Returns [`FormatError::Malformed`] if the payload is truncated or its
    /// declared slot count / name run past the end of the block.
    pub fn from_payload(data: &[u8]) -> Result<Self> {
        let byte0 = *data
            .first()
            .ok_or_else(|| FormatError::Malformed("design block is empty".into()))?;
        let byte1 = *data
            .get(1)
            .ok_or_else(|| FormatError::Malformed("design block truncated at byte 1".into()))?;
        let full_design = byte0 & 0x04 != 0;
        let transferred = byte1 & 0x80 != 0;
        let starbase = byte1 & 0x40 != 0;
        let design_number = (byte1 & 0x3C) >> 2;
        let hull_id = *data
            .get(2)
            .ok_or_else(|| FormatError::Malformed("design block truncated at hull id".into()))?;
        let pic = *data
            .get(3)
            .ok_or_else(|| FormatError::Malformed("design block truncated at pic".into()))?;

        let mut armor = None;
        let mut mass = None;
        let mut turn_designed = None;
        let mut total_built = None;
        let mut total_remaining = None;
        let mut slots = Vec::new();

        let index;
        if full_design {
            armor = Some(
                read16(data, 4)
                    .ok_or_else(|| FormatError::Malformed("design: truncated armor".into()))?,
            );
            let slot_count = *data
                .get(6)
                .ok_or_else(|| FormatError::Malformed("design: truncated slot count".into()))?
                as usize;
            turn_designed =
                Some(read16(data, 7).ok_or_else(|| {
                    FormatError::Malformed("design: truncated turn designed".into())
                })?);
            total_built =
                Some(read32(data, 9).ok_or_else(|| {
                    FormatError::Malformed("design: truncated total built".into())
                })?);
            total_remaining = Some(read32(data, 13).ok_or_else(|| {
                FormatError::Malformed("design: truncated total remaining".into())
            })?);
            let mut i = 17usize;
            for _ in 0..slot_count {
                let category = read16(data, i)
                    .ok_or_else(|| FormatError::Malformed("design: truncated slot".into()))?;
                let item_id = *data
                    .get(i + 2)
                    .ok_or_else(|| FormatError::Malformed("design: truncated slot item".into()))?;
                let count = *data
                    .get(i + 3)
                    .ok_or_else(|| FormatError::Malformed("design: truncated slot count".into()))?;
                slots.push(Slot {
                    category,
                    item_id,
                    count,
                });
                i += 4;
            }
            index = i;
        } else {
            mass = Some(
                read16(data, 4)
                    .ok_or_else(|| FormatError::Malformed("design: truncated mass".into()))?,
            );
            index = 6;
        }

        let name_len = *data
            .get(index)
            .ok_or_else(|| FormatError::Malformed("design: truncated name length".into()))?
            as usize;
        let name_end = index + name_len;
        if name_end >= data.len() && name_len > 0 {
            // The packed name field is [len][len bytes]; it must fit inside the
            // block. (name_len == 0 is allowed: an empty name.)
            return Err(FormatError::Malformed(
                "design: name field runs past end of block".into(),
            ));
        }
        let name = strings::decode_field(&data[index..=name_end.min(data.len() - 1)]);

        Ok(Self {
            full_design,
            transferred,
            starbase,
            design_number,
            hull_id,
            pic,
            armor,
            mass,
            turn_designed,
            total_built,
            total_remaining,
            slots,
            name,
        })
    }
}

/// Decode every design block (type 26) in a decoded [`StarsFile`], in file
/// order.
///
/// # Errors
/// Returns [`FormatError::Malformed`] if any design block is malformed.
pub fn design_records(file: &StarsFile) -> Result<Vec<DesignRecord>> {
    file.blocks
        .iter()
        .filter(|b| b.block_type() == BlockType::Design)
        .map(|b| DesignRecord::from_payload(&b.data))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal partial-design payload with the given name.
    fn partial_payload(name_field: &[u8]) -> Vec<u8> {
        let mut v = vec![
            0x03, // byte0: low 2 bits set, fullData clear
            0x01, // byte1: bit0 set, design 0, not starbase/transferred
            4,    // hull id
            17,   // pic
            10, 0, // mass = 10
        ];
        v.extend_from_slice(name_field);
        v
    }

    #[test]
    fn decodes_partial_design() {
        // Name field: length 1, single packed nibble byte (value irrelevant).
        let d = partial_payload(&[1, 0x00]);
        let rec = DesignRecord::from_payload(&d).unwrap();
        assert!(!rec.full_design);
        assert_eq!(rec.hull_id, 4);
        assert_eq!(rec.pic, 17);
        assert_eq!(rec.mass, Some(10));
        assert_eq!(rec.armor, None);
        assert!(rec.slots.is_empty());
    }

    #[test]
    fn rejects_empty_block() {
        assert!(DesignRecord::from_payload(&[]).is_err());
    }

    #[test]
    fn rejects_truncated_full_design() {
        // fullData set (byte0=0x07) but block ends before armor.
        let d = vec![0x07, 0x01, 4, 17];
        assert!(DesignRecord::from_payload(&d).is_err());
    }
}
