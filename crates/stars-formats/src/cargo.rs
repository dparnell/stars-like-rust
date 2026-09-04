//! Cargo transfer records (block types 1, 2, 23 and 25).
//!
//! A cargo transfer is written to the `.x` order file as one block per
//! transfer, with the block *type* choosing how wide the quantity is: type 1
//! stores it in one byte, type 2 in two, type 25 in four. Type 23 is the
//! fleet-to-fleet form.
//!
//! The waypoint's Transport task (task id 1) is not where this lives. Every
//! waypoint in both fixture games carries task 0, because a task is consumed
//! when it executes; the transfer it performed is logged here instead.

use crate::block::Block;

/// Block type for a transfer with 8-bit quantities.
pub const XFER8: u8 = 1;
/// Block type for a transfer with 16-bit quantities.
pub const XFER16: u8 = 2;
/// Block type for a fleet-to-fleet transfer.
pub const XFER_FLEET: u8 = 23;
/// Block type for a transfer with 32-bit quantities.
pub const XFER32: u8 = 25;

/// One cargo transfer.
///
/// # What is and is not pinned down
///
/// The two ids and the quantity are clear: they vary across the samples and
/// their values are sensible. The two bytes between them are **not**. Every
/// planet transfer in the fixtures carries `mode = 0x12` and `selector = 0x08`,
/// and every fleet transfer `mode = 0x22`, so nothing in the data distinguishes
/// what either byte means. They are exposed raw rather than guessed at.
///
/// The `0x10` between the two `mode` values, and the fact that all 43 planet
/// transfers move colonists, hint that the high nibble picks planet or fleet
/// and `selector` is a cargo bitmask with bit 3 for colonists — but that is a
/// reading of two constants, not evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CargoTransferRecord {
    /// The fleet doing the transferring.
    pub fleet: u16,
    /// The planet or fleet on the other side.
    pub target: u16,
    /// Byte 4. Constant per block type across every fixture sample.
    pub mode: u8,
    /// Byte 5, present only in the planet forms. Constant in every sample.
    pub selector: Option<u8>,
    /// How much moved. Signed: the sign says which way.
    pub quantity: i32,
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

impl CargoTransferRecord {
    /// Decode a transfer block, given its type.
    ///
    /// Returns `None` for a block that is not a transfer, or is too short.
    #[must_use]
    pub fn decode(type_id: u8, d: &[u8]) -> Option<Self> {
        let fleet = u16_at(d, 0)?;
        let target = u16_at(d, 2)?;
        let mode = *d.get(4)?;
        let (selector, quantity) = match type_id {
            XFER8 => (Some(*d.get(5)?), i32::from(*d.get(6)? as i8)),
            XFER16 => (Some(*d.get(5)?), i32::from(u16_at(d, 6)? as i16)),
            XFER_FLEET => (None, i32::from(u16_at(d, 5)? as i16)),
            XFER32 => (
                Some(*d.get(5)?),
                i32::from_le_bytes([*d.get(6)?, *d.get(7)?, *d.get(8)?, *d.get(9)?]),
            ),
            _ => return None,
        };
        Some(Self {
            fleet,
            target,
            mode,
            selector,
            quantity,
        })
    }
}

/// Decode every cargo transfer in a run of blocks, in order.
#[must_use]
pub fn cargo_transfers_in(blocks: &[Block]) -> Vec<CargoTransferRecord> {
    blocks
        .iter()
        .filter_map(|b| CargoTransferRecord::decode(b.type_id, &b.data))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real records from `fixtures/games/exodus/2420/EXODUS.X6` and its
    /// neighbours.
    #[test]
    fn decodes_the_recorded_transfers() {
        let eight = CargoTransferRecord::decode(XFER8, &[0x03, 0x0a, 0xe1, 0x00, 0x12, 0x08, 0x19])
            .expect("an 8-bit transfer");
        assert_eq!(eight.fleet, 0x0a03);
        assert_eq!(eight.target, 225);
        assert_eq!(eight.mode, 0x12);
        assert_eq!(eight.selector, Some(0x08));
        assert_eq!(eight.quantity, 25);

        let sixteen =
            CargoTransferRecord::decode(XFER16, &[0x0c, 0x0a, 0xe1, 0x00, 0x12, 0x08, 0xf4, 0x01])
                .expect("a 16-bit transfer");
        assert_eq!(sixteen.target, 225);
        assert_eq!(sixteen.quantity, 500);

        let fleet = CargoTransferRecord::decode(
            XFER_FLEET,
            &[0x04, 0x0a, 0x06, 0x0a, 0x22, 0x01, 0x00, 0xff, 0xff],
        )
        .expect("a fleet transfer");
        assert_eq!(fleet.fleet, 0x0a04);
        assert_eq!(fleet.target, 0x0a06);
        assert_eq!(fleet.mode, 0x22);
        assert_eq!(fleet.selector, None);
        assert_eq!(fleet.quantity, 1);

        assert_eq!(CargoTransferRecord::decode(99, &[0; 16]), None);
    }
}
