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
//! Unlike the other record decoders this one can also **write**: a production
//! queue is the one thing a player edits that has to reach the file, so
//! [`QueueItem::to_word`] and [`ProductionQueueRecord::encode`] pack the entries
//! back. Everything else still round-trips as opaque bytes through
//! [`crate::file`].

use crate::block::BlockType;
use crate::file::StarsFile;

/// What a queue entry builds: a planetary installation or a ship.
///
/// This is the game's `GrobjClass`, stored in bits 17-19 of the entry. Only
/// these two values appear in a production queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueClass {
    /// A planetary item — the id is a [`crate::production`] `ProdItemType`.
    Planet,
    /// A ship or starbase — the id is a design slot.
    Fleet,
    /// Anything else; the raw 3-bit class is kept so nothing is silently lost.
    Other(u8),
}

/// One entry in a production queue.
///
/// The entry is one 32-bit little-endian word, packed as recovered from
/// `AddItemToQueue` (`1090:407b`), which writes each field with an explicit
/// shift and mask:
///
/// | Bits    | Width | Field                                              |
/// |---------|-------|----------------------------------------------------|
/// | 0-9     | 10    | `count`                                            |
/// | 10-16   | 7     | `item` (shift `0xa`, mask `0x7f`)                  |
/// | 17-19   | 3     | `class` (shift `0x11`, mask `0x7`)                 |
/// | 20-26   | 7     | `completion`, a percentage; zeroed when queued      |
/// | 27-31   | 5     | unknown; zeroed when queued, and 0 in every fixture |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueItem {
    /// How many of the item to build (10-bit field).
    pub count: u16,
    /// The item id being built (7-bit field), interpreted per [`Self::class`].
    pub item: u16,
    /// Whether this entry builds a planetary item or a ship.
    pub class: QueueClass,
    /// How far the *first* unit has been paid for, as a percentage (0-99).
    pub completion: u16,
}

impl QueueItem {
    /// Pack this entry back into its 32-bit word.
    ///
    /// The inverse of the decode above, writing the same fields
    /// `AddItemToQueue` writes. Bits 27-31 are left zero, as the game leaves
    /// them when it queues an item; they are zero in every fixture.
    #[must_use]
    pub fn to_word(self) -> u32 {
        let class = match self.class {
            QueueClass::Planet => 1,
            QueueClass::Fleet => 2,
            QueueClass::Other(raw) => u32::from(raw),
        };
        (u32::from(self.count) & 0x3FF)
            | ((u32::from(self.item) & 0x7F) << 10)
            | ((class & 0x7) << 17)
            | ((u32::from(self.completion) & 0x7F) << 20)
    }
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

fn decode_items(data: &[u8]) -> Vec<QueueItem> {
    let mut items = Vec::with_capacity(data.len() / 4);
    let mut i = 0;
    while i + 4 <= data.len() {
        let w = read32(data, i).unwrap_or(0);
        items.push(QueueItem {
            count: (w & 0x3FF) as u16,
            item: ((w >> 10) & 0x7F) as u16,
            class: match (w >> 17) & 0x7 {
                1 => QueueClass::Planet,
                2 => QueueClass::Fleet,
                other => QueueClass::Other(other as u8),
            },
            completion: ((w >> 20) & 0x7F) as u16,
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

impl ProductionQueueRecord {
    /// Pack the queue back into a type-28 block payload.
    ///
    /// The type-28 form is a bare list of items with no planet id — the
    /// association is positional, the queue belonging to the planet block it
    /// follows. A caller replacing one must keep it in the same place.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.items.len() * 4);
        for item in &self.items {
            out.extend_from_slice(&item.to_word().to_le_bytes());
        }
        out
    }

    /// Pack the queue into the type-29 **order** form, which names its planet.
    ///
    /// Exact inverse of [`ProductionQueueRecord::decode_change`]. A record with
    /// no planet id writes zero, which no real order does.
    #[must_use]
    pub fn encode_change(&self) -> Vec<u8> {
        let mut out = self.planet_id.unwrap_or(0).to_le_bytes().to_vec();
        out.extend_from_slice(&self.encode());
        out
    }
}

/// Decode every production-queue block (type 28) in a decoded [`StarsFile`], in
/// file order.
///
/// This decodes only the type-28 form found in `.mN`/`.hst`; the type-29 `.xN`
/// order form is decoded on demand via [`ProductionQueueRecord::decode_change`].
///
/// A type-28 block carries no planet id of its own, so this bare list cannot
/// say which planet each queue belongs to. Prefer
/// [`production_queues_by_planet`], which recovers the owner from block order.
#[must_use]
pub fn production_queue_records(file: &StarsFile) -> Vec<ProductionQueueRecord> {
    file.blocks
        .iter()
        .filter(|b| b.block_type() == BlockType::ProductionQueue)
        .map(|b| ProductionQueueRecord::decode(&b.data))
        .collect()
}

/// Decode the production-queue blocks in one segment, each paired with the id
/// of the planet it belongs to.
///
/// A type-28 block stores only a list of items; the planet is implied by
/// position, because the queue block immediately follows its own planet block.
/// Only planets that have a queue get a block at all, so the queues cannot be
/// matched to planets by index — in `fixtures/incoming/turn1/Game.hst` the two
/// queues belong to planets 32 and 112 while planet 69, which lies between
/// them, has none.
#[must_use]
pub fn production_queues_by_planet(
    blocks: &[crate::block::Block],
) -> Vec<(u16, ProductionQueueRecord)> {
    let mut out = Vec::new();
    let mut current: Option<u16> = None;
    for b in blocks {
        match b.block_type() {
            BlockType::Planet => current = read16(&b.data, 0).map(|w| w & 0x3FF),
            BlockType::PartialPlanet | BlockType::MinimalPlanet => current = None,
            BlockType::ProductionQueue => {
                if let Some(id) = current.take() {
                    out.push((id, ProductionQueueRecord::decode(&b.data)));
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every queue block in every fixture packs back to the bytes it came from.
    ///
    /// This is the check that makes saving safe: an edited queue is written
    /// with the same encoder, so anything it gets wrong would show up here
    /// first on the thousands of queues the fixtures already contain.
    #[test]
    fn every_recorded_queue_round_trips() {
        use crate::file::StarsFile;
        use std::path::Path;

        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let games = root.join("fixtures/games");
        if !games.is_dir() {
            eprintln!("skipping: no fixtures");
            return;
        }

        let mut checked = 0usize;
        let mut stack = vec![games];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                let Ok(file) = StarsFile::decode(&bytes) else {
                    continue;
                };
                for block in &file.blocks {
                    if block.block_type() != BlockType::ProductionQueue {
                        continue;
                    }
                    let decoded = ProductionQueueRecord::decode(&block.data);
                    assert_eq!(
                        decoded.encode(),
                        block.data,
                        "queue block did not round-trip in {}",
                        path.display()
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 100, "expected many queues, checked {checked}");
        eprintln!("production queues round-tripped: {checked}");
    }

    /// Encode a queue item into its packed 32-bit word (test helper), using the
    /// field layout `AddItemToQueue` writes.
    fn item_word(count: u32, item: u32, class: u32, completion: u32) -> [u8; 4] {
        let w = (count & 0x3FF)
            | ((item & 0x7F) << 10)
            | ((class & 0x7) << 17)
            | ((completion & 0x7F) << 20);
        w.to_le_bytes()
    }

    #[test]
    fn decodes_items() {
        let mut d = Vec::new();
        d.extend_from_slice(&item_word(1, 8, 1, 81)); // auto mines, 81% paid
        d.extend_from_slice(&item_word(5, 3, 2, 0)); // five of ship design 3
        let q = ProductionQueueRecord::decode(&d);
        assert_eq!(q.planet_id, None);
        assert_eq!(q.items.len(), 2);
        assert_eq!(q.items[0].count, 1);
        assert_eq!(q.items[0].item, 8);
        assert_eq!(q.items[0].class, QueueClass::Planet);
        assert_eq!(q.items[0].completion, 81);
        assert_eq!(q.items[1].count, 5);
        assert_eq!(q.items[1].item, 3);
        assert_eq!(q.items[1].class, QueueClass::Fleet);
    }

    /// The real words from the two AI planets in
    /// `fixtures/incoming/turn1/Game.hst`, which is where the layout was
    /// pinned down: five scouts and a starbase, not "auto mines".
    #[test]
    fn decodes_recorded_ai_queue() {
        let mut d = Vec::new();
        for w in [0x0514_0001u32, 0x0004_0001, 0x0004_4801] {
            d.extend_from_slice(&w.to_le_bytes());
        }
        let q = ProductionQueueRecord::decode(&d);
        assert_eq!(
            q.items
                .iter()
                .map(|i| (i.count, i.item, i.class, i.completion))
                .collect::<Vec<_>>(),
            vec![
                (1, 0, QueueClass::Fleet, 81),
                (1, 0, QueueClass::Fleet, 0),
                (1, 18, QueueClass::Fleet, 0),
            ]
        );
    }

    #[test]
    fn ignores_trailing_partial_word() {
        let mut d = item_word(2, 3, 1, 4).to_vec();
        d.extend_from_slice(&[0xAA, 0xBB]); // stray 2 bytes, not a full item
        let q = ProductionQueueRecord::decode(&d);
        assert_eq!(q.items.len(), 1);
    }

    #[test]
    fn decodes_change_with_planet_id() {
        let mut d = 42u16.to_le_bytes().to_vec();
        d.extend_from_slice(&item_word(1, 8, 1, 0));
        let q = ProductionQueueRecord::decode_change(&d).unwrap();
        assert_eq!(q.planet_id, Some(42));
        assert_eq!(q.items.len(), 1);
    }

    #[test]
    fn rejects_truncated_change() {
        assert!(ProductionQueueRecord::decode_change(&[0x01]).is_none());
    }
}
