//! Cargo transfers (block types 1, 2 and 25) and ship transfers (type 23).
//!
//! A cargo transfer is written to the `.x` order file as one block per
//! transfer, with the block *type* choosing how wide each quantity is: type 1
//! stores them as `i8`, type 2 as `i16`, type 25 as `i32`.
//!
//! Type 23, despite the name `rtLogFleetCargoXfer`, does not move cargo at all
//! — it moves **ships** between two fleets of the same player. See
//! [`ShipTransferRecord`].
//!
//! The waypoint's Transport task (task id 1) is not where this lives. Every
//! waypoint in both fixture games carries task 0, because a task is consumed
//! when it executes; the transfer it performed is logged here instead.
//!
//! Source: the `rtLogCargoXfer8/16/32` and `rtLogFleetCargoXfer` arms of the
//! order-log replay (`log.c` in the reconstructed NB09 sources). See
//! `docs/formats/cargo.md`.

use crate::block::Block;

/// Block type for a transfer with 8-bit quantities.
pub const XFER8: u8 = 1;
/// Block type for a transfer with 16-bit quantities.
pub const XFER16: u8 = 2;
/// Block type for a fleet-to-fleet **ship** transfer.
pub const XFER_FLEET: u8 = 23;
/// Block type for a transfer with 32-bit quantities.
pub const XFER32: u8 = 25;

/// How many cargo kinds a transfer block can carry.
///
/// The replay loops `for (i = 0; i < 5; i++)` over the mask in byte 5.
pub const CARGO_KINDS: usize = 5;

/// Index of colonists among the cargo kinds.
///
/// The replay singles this index out for the colonist-drop bookkeeping that
/// turns an unload over a foreign planet into an invasion (`if (i == 3) ...`),
/// which is what fixes the order: ironium, boranium, germanium, colonists,
/// fuel.
pub const COLONISTS: usize = 3;

/// What kind of object one end of a transfer is (`GrobjClass`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrobjClass {
    /// A planet (`grobjPlanet`, 1).
    Planet,
    /// A fleet (`grobjFleet`, 2).
    Fleet,
    /// No object — used for the destination when cargo is simply jettisoned
    /// (`grobjOther`, 4).
    None,
    /// A mineral packet or other "thing" (`grobjThing`, 8).
    Thing,
}

impl GrobjClass {
    /// Decode one nibble of the source/destination byte.
    #[must_use]
    pub fn from_nibble(nibble: u8) -> Option<Self> {
        match nibble {
            1 => Some(Self::Planet),
            2 => Some(Self::Fleet),
            4 => Some(Self::None),
            8 => Some(Self::Thing),
            _ => None,
        }
    }
}

/// One cargo transfer, as the order log records it.
///
/// # The sign convention
///
/// The replay applies `ChgCargo(source, +q)` and `ChgCargo(destination, -q)`,
/// so a **positive** quantity means the *source* gains and the destination
/// loses: a fleet listed as the source with `+25` colonists has **loaded** 25
/// from the planet. Negative unloads.
///
/// Quantities are applied in two passes — every negative first, then every
/// positive — so that space freed by unloading is available to whatever loads
/// afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CargoTransferRecord {
    /// Id of the object named first, which is the transfer's source.
    pub source: u16,
    /// Id of the object named second, the destination.
    pub destination: u16,
    /// What kind of object the source is (low nibble of byte 4).
    pub source_class: Option<GrobjClass>,
    /// What kind the destination is (high nibble of byte 4).
    pub destination_class: Option<GrobjClass>,
    /// Byte 4 verbatim, `source_class | destination_class << 4`.
    pub mode: u8,
    /// Byte 5: which cargo kinds the block carries, one bit each.
    pub selector: u8,
    /// How much of each kind moved, indexed by cargo kind. A kind whose bit is
    /// clear is zero and occupies no bytes in the block.
    pub quantities: [i32; CARGO_KINDS],
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

impl CargoTransferRecord {
    /// Decode a cargo transfer block, given its type.
    ///
    /// Returns `None` for a block that is not one of [`XFER8`], [`XFER16`] or
    /// [`XFER32`], or that is too short for the quantities its mask claims.
    #[must_use]
    pub fn decode(type_id: u8, d: &[u8]) -> Option<Self> {
        let width = match type_id {
            XFER8 => 1usize,
            XFER16 => 2,
            XFER32 => 4,
            _ => return None,
        };
        let source = u16_at(d, 0)?;
        let destination = u16_at(d, 2)?;
        let mode = *d.get(4)?;
        let selector = *d.get(5)?;

        // One quantity per set bit, packed consecutively from byte 6.
        let mut quantities = [0i32; CARGO_KINDS];
        let mut taken = 0usize;
        for (kind, slot) in quantities.iter_mut().enumerate() {
            if selector & (1 << kind) == 0 {
                continue;
            }
            let at = 6 + taken * width;
            *slot = match width {
                1 => i32::from(*d.get(at)? as i8),
                2 => i32::from(u16_at(d, at)? as i16),
                _ => i32::from_le_bytes([
                    *d.get(at)?,
                    *d.get(at + 1)?,
                    *d.get(at + 2)?,
                    *d.get(at + 3)?,
                ]),
            };
            taken += 1;
        }

        Some(Self {
            source,
            destination,
            source_class: GrobjClass::from_nibble(mode & 0x0f),
            destination_class: GrobjClass::from_nibble(mode >> 4),
            mode,
            selector,
            quantities,
        })
    }
}

/// How many ship-design slots a fleet holds, and so how wide the mask is.
pub const DESIGN_SLOTS: usize = 16;

/// A transfer of **ships** between two fleets of the same player (type 23).
///
/// `rtLogFleetCargoXfer` is a misleading name: the replay arm moves
/// `fl.rgcsh[i]`, the count of ships of design `i`, not cargo. It refuses
/// outright when the two fleets have different owners, and rebalances the two
/// fleets' cargo afterwards (`FleetTransferCargoBalance`).
///
/// The mask here is **16 bits** at byte 5, one per design slot, with an `i16`
/// count per set bit from byte 7 — a different layout from the cargo forms,
/// which use an 8-bit mask at byte 5 and pack from byte 6.
///
/// Sign matches the cargo form: the replay does `first += delta` and
/// `second -= delta`, so a negative count moves ships **from** the first fleet
/// **to** the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipTransferRecord {
    /// The fleet named first.
    pub first: u16,
    /// The fleet named second.
    pub second: u16,
    /// Which design slots the block carries, one bit each.
    pub selector: u16,
    /// Ships moved per design slot; positive moves into [`Self::first`].
    pub counts: [i32; DESIGN_SLOTS],
}

impl ShipTransferRecord {
    /// Decode a type-[`XFER_FLEET`] block.
    #[must_use]
    pub fn decode(type_id: u8, d: &[u8]) -> Option<Self> {
        if type_id != XFER_FLEET {
            return None;
        }
        let first = u16_at(d, 0)?;
        let second = u16_at(d, 2)?;
        let selector = u16_at(d, 5)?;
        let mut counts = [0i32; DESIGN_SLOTS];
        let mut taken = 0usize;
        for (slot, count) in counts.iter_mut().enumerate() {
            if selector & (1 << slot) == 0 {
                continue;
            }
            *count = i32::from(u16_at(d, 7 + taken * 2)? as i16);
            taken += 1;
        }
        Some(Self {
            first,
            second,
            selector,
            counts,
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

/// Decode every ship transfer in a run of blocks, in order.
#[must_use]
pub fn ship_transfers_in(blocks: &[Block]) -> Vec<ShipTransferRecord> {
    blocks
        .iter()
        .filter_map(|b| ShipTransferRecord::decode(b.type_id, &b.data))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real records from the `EXODUS.X6` order files.
    #[test]
    fn decodes_the_recorded_transfers() {
        // Fleet 0x0a03 loads 25 colonists from planet 225.
        let eight = CargoTransferRecord::decode(XFER8, &[0x03, 0x0a, 0xe1, 0x00, 0x12, 0x08, 0x19])
            .expect("an 8-bit transfer");
        assert_eq!(eight.source, 0x0a03);
        assert_eq!(eight.destination, 225);
        assert_eq!(eight.source_class, Some(GrobjClass::Fleet));
        assert_eq!(eight.destination_class, Some(GrobjClass::Planet));
        assert_eq!(eight.selector, 0x08);
        assert_eq!(eight.quantities, [0, 0, 0, 25, 0]);

        let sixteen =
            CargoTransferRecord::decode(XFER16, &[0x0c, 0x0a, 0xe1, 0x00, 0x12, 0x08, 0xf4, 0x01])
                .expect("a 16-bit transfer");
        assert_eq!(sixteen.destination, 225);
        assert_eq!(sixteen.quantities[COLONISTS], 500);

        assert_eq!(CargoTransferRecord::decode(99, &[0; 16]), None);
        // Type 23 is not a cargo transfer.
        assert_eq!(CargoTransferRecord::decode(XFER_FLEET, &[0; 16]), None);
    }

    /// Several kinds at once pack their quantities consecutively, and a kind
    /// whose bit is clear takes no room in the block.
    #[test]
    fn a_mask_with_several_bits_packs_the_quantities() {
        // Ironium and germanium only: mask 0b00101, two 16-bit quantities.
        let r = CargoTransferRecord::decode(
            XFER16,
            &[
                0x01,
                0x00,
                0x02,
                0x00,
                0x12,
                0b0000_0101,
                0x0a,
                0x00,
                0xf6,
                0xff,
            ],
        )
        .expect("a two-kind transfer");
        assert_eq!(r.quantities, [10, 0, -10, 0, 0]);
    }

    /// A type-23 block moves ships, not cargo: one ship of design 0 leaves the
    /// first fleet for the second.
    #[test]
    fn a_fleet_transfer_moves_ships() {
        let r = ShipTransferRecord::decode(
            XFER_FLEET,
            &[0x04, 0x0a, 0x06, 0x0a, 0x22, 0x01, 0x00, 0xff, 0xff],
        )
        .expect("a ship transfer");
        assert_eq!(r.first, 0x0a04);
        assert_eq!(r.second, 0x0a06);
        assert_eq!(r.selector, 0x0001);
        assert_eq!(r.counts[0], -1);
        assert!(r.counts[1..].iter().all(|c| *c == 0));
    }
}
