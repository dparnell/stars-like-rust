//! Typed decoder for the **production queue** blocks (type id 28,
//! `PRODUCTION_QUEUE`, and type id 29, `PRODUCTION_QUEUE_CHANGE`, the `.xN`
//! order variant) that appear in `.mN`/`.hst`/`.xN` files.
//!
//! A production queue is a planet's build list. It is stored as a run of packed
//! 32-bit items, each holding a build quantity, the item being built, and how
//! far along the first of them is:
//!
//! ```text
//! bits  0.. 9  count      (10 bits)
//! bits 10..19  item       (10 bits)
//! bits 20..31  completion (12 bits)
//! ```
//!
//! The type-28 form is a bare list of items; the `.xN` order form (type 29)
//! prefixes a 2-byte planet id.
//!
//! The layout was recovered from the stars-4x `decompiled` project
//! (`Structures/Structure28.xml` / `Structure29.xml`) and verified against
//! `fixtures/incoming/turn1/Game.hst`, whose type-28 blocks decode to
//! auto-build item runs (e.g. five auto-factory items, the first partly built,
//! plus an auto-mine item).
//!
//! Like the other record decoders this is an *interpreted, read-only view*;
//! byte-exact write-back still goes through the container in [`crate::file`].

use crate::block::BlockType;
use crate::file::StarsFile;

/// One entry in a production queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueItem {
    /// How many of the item to build (10-bit field).
    pub count: u16,
    /// The item id being built (10-bit field). Ids `< 256` are ship/starbase
    /// designs and other concrete items; ids `>= 256` are the "auto build"
    /// pseudo-items (auto factories/mines/defenses/etc.). Exposed raw.
    pub item: u16,
    /// Resources/minerals already applied to the *first* unit of this item
    /// (12-bit field); a rough completion indicator.
    pub completion: u16,
}

/// A decoded production queue (type-28 or type-29 block).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionQueueRecord {
    /// Planet id this queue belongs to — only present for the `.xN` order form
    /// (type 29); `None` for the type-28 form (whose association is positional).
    pub planet_id: Option<u16>,
    /// The queued items, in build order.
    pub items: Vec<QueueItem>,
}

fn read32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *d.get(o)?,
        *d.get(o + 1)?,
        *d.get(o + 2)?,
        *d.get(o + 3)?,
    ]))
}

fn decode_items(data: &[u8]) -> Vec<QueueItem> {
    let mut items = Vec::with_capacity(data.len() / 4);
    let mut i = 0;
    while i + 4 <= data.len() {
        let w = read32(data, i).unwrap_or(0);
        items.push(QueueItem {
            count: (w & 0x3FF) as u16,
            item: ((w >> 10) & 0x3FF) as u16,
            completion: ((w >> 20) & 0xFFF) as u16,
        });
        i += 4;
    }
    items
}

impl ProductionQueueRecord {
    /// Decode a **decrypted** type-28 production-queue block payload (a bare
    /// list of items, with no planet id).
    #[must_use]
    pub fn decode(data: &[u8]) -> Self {
        Self {
            planet_id: None,
            items: decode_items(data),
        }
    }

    /// Decode a **decrypted** type-29 production-queue *change* block payload
    /// (the `.xN` order form), which prefixes a 2-byte planet id.
    ///
    /// Returns `None` if the payload is shorter than the 2-byte planet id.
    #[must_use]
    pub fn decode_change(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        let planet_id = u16::from_le_bytes([data[0], data[1]]);
        Some(Self {
            planet_id: Some(planet_id),
            items: decode_items(&data[2..]),
        })
    }
}

/// Decode every production-queue block (type 28) in a decoded [`StarsFile`], in
/// file order.
///
/// This decodes only the type-28 form found in `.mN`/`.hst`; the type-29 `.xN`
/// order form is decoded on demand via [`ProductionQueueRecord::decode_change`].
#[must_use]
pub fn production_queue_records(file: &StarsFile) -> Vec<ProductionQueueRecord> {
    file.blocks
        .iter()
        .filter(|b| b.block_type() == BlockType::ProductionQueue)
        .map(|b| ProductionQueueRecord::decode(&b.data))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a queue item into its packed 32-bit word (test helper).
    fn item_word(count: u32, item: u32, completion: u32) -> [u8; 4] {
        let w = (count & 0x3FF) | ((item & 0x3FF) << 10) | ((completion & 0xFFF) << 20);
        w.to_le_bytes()
    }

    #[test]
    fn decodes_items() {
        let mut d = Vec::new();
        d.extend_from_slice(&item_word(1, 256, 81));
        d.extend_from_slice(&item_word(5, 274, 0));
        let q = ProductionQueueRecord::decode(&d);
        assert_eq!(q.planet_id, None);
        assert_eq!(q.items.len(), 2);
        assert_eq!(q.items[0].count, 1);
        assert_eq!(q.items[0].item, 256);
        assert_eq!(q.items[0].completion, 81);
        assert_eq!(q.items[1].count, 5);
        assert_eq!(q.items[1].item, 274);
    }

    #[test]
    fn ignores_trailing_partial_word() {
        let mut d = item_word(2, 3, 4).to_vec();
        d.extend_from_slice(&[0xAA, 0xBB]); // stray 2 bytes, not a full item
        let q = ProductionQueueRecord::decode(&d);
        assert_eq!(q.items.len(), 1);
    }

    #[test]
    fn decodes_change_with_planet_id() {
        let mut d = 42u16.to_le_bytes().to_vec();
        d.extend_from_slice(&item_word(1, 256, 0));
        let q = ProductionQueueRecord::decode_change(&d).unwrap();
        assert_eq!(q.planet_id, Some(42));
        assert_eq!(q.items.len(), 1);
    }

    #[test]
    fn rejects_truncated_change() {
        assert!(ProductionQueueRecord::decode_change(&[0x01]).is_none());
    }
}
