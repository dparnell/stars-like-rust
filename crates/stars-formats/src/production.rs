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

/// The production queue a newly settled planet starts with (`ZIPPRODQ1`).
///
/// A player keeps a **default queue**: when a planet becomes theirs — settled
/// or taken — the game gives it these items rather than an empty queue, and
/// sets the planet's "no research" flag from the same record. It lives at
/// offset [`DEFAULT_QUEUE_OFFSET`] of the player block and is also what the
/// `.xN` order operation `rtLogPlayerZpq1` (type 46) carries.
///
/// ```c
/// typedef struct _zipprodq1 {
///     uint8_t fNoResearch;   /* +0x00 */
///     uint8_t cpq;           /* +0x01 how many entries follow */
///     PRODQ1  rgpq[12];      /* +0x02 */
/// } ZIPPRODQ1;               /* size 26 */
///
/// typedef struct _prodq1 { uint16_t mdIdle : 6, cQuan : 10; } PRODQ1;
/// ```
///
/// Note the entry packing is **not** the four-byte `PROD` of a planet's own
/// queue: it is two bytes, six bits of item and ten of quantity, and it can
/// only hold planetary items.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefaultQueue {
    /// Whether a planet that starts with this queue is exempt from the
    /// research skim (`fNoResearch`).
    pub no_research: bool,
    /// The entries, in build order.
    pub items: Vec<DefaultQueueItem>,
}

/// One entry of a [`DefaultQueue`] (`PRODQ1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultQueueItem {
    /// The planetary item to build (`mdIdle`, six bits).
    pub item: u8,
    /// How many (`cQuan`, ten bits).
    pub count: u16,
}

/// Offset of the default queue within a player block (`PLAYER.zpq1`).
pub const DEFAULT_QUEUE_OFFSET: usize = 0x56;

/// Size of the default queue as the player block stores it
/// (`sizeof(ZIPPRODQ1)`).
pub const DEFAULT_QUEUE_LEN: usize = 26;

/// How many entries it holds.
pub const DEFAULT_QUEUE_MAX: usize = 12;

impl DefaultQueue {
    /// Decode a `ZIPPRODQ1`, from a player block or an order record.
    ///
    /// The declared entry count is clamped to what the record and the array
    /// can hold, so a short or overstated record decodes to what is there
    /// rather than failing.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        let declared = usize::from(data[1]).min(DEFAULT_QUEUE_MAX);
        let available = (data.len() - 2) / 2;
        Some(Self {
            no_research: data[0] & 1 != 0,
            items: (0..declared.min(available))
                .map(|i| {
                    let word = u16::from_le_bytes([data[2 + i * 2], data[3 + i * 2]]);
                    DefaultQueueItem {
                        item: (word & 0x3F) as u8,
                        count: word >> 6,
                    }
                })
                .collect(),
        })
    }

    /// Encode just the header and the entries, which is what the `.xN` order
    /// record carries: `2 * cpq + 2` bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let items = &self.items[..self.items.len().min(DEFAULT_QUEUE_MAX)];
        let mut out = Vec::with_capacity(2 + items.len() * 2);
        out.push(u8::from(self.no_research));
        #[allow(clippy::cast_possible_truncation)]
        out.push(items.len() as u8);
        for entry in items {
            let word = u16::from(entry.item & 0x3F) | ((entry.count & 0x03FF) << 6);
            out.extend_from_slice(&word.to_le_bytes());
        }
        out
    }

    /// Encode the full fixed-size form the player block holds, zero-padded.
    #[must_use]
    pub fn encode_fixed(&self) -> [u8; DEFAULT_QUEUE_LEN] {
        let mut out = [0u8; DEFAULT_QUEUE_LEN];
        let packed = self.encode();
        let n = packed.len().min(DEFAULT_QUEUE_LEN);
        out[..n].copy_from_slice(&packed[..n]);
        out
    }
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

/// A named **production template** (`ZIPPRODQ`).
///
/// Four of these: a **default**, which is the same record the player block
/// carries as `PLAYER.zpq1` and which is applied on its own to any planet the
/// player settles or takes, and three the player applies by hand. Applying one
/// replaces every auto-build item in a planet's queue with the template's,
/// leaving the ordinary items where they are.
///
/// ```c
/// typedef struct _zipprodq {
///     char      szName[13]; /* +0x00 */
///     uint8_t   fValid;     /* +0x0D */
///     ZIPPRODQ1 zpq1;       /* +0x0E */
/// } ZIPPRODQ;               /* size 40 */
/// ```
///
/// They are **not** in a save file. The game keeps them in `stars.ini`, under
/// the section `ZipOrders` and the keys `ZipOrdersP1`…`ZipOrdersP5`, in the
/// text form [`ProductionTemplate::decode_ini`] reads — which is why they last
/// for as long as the installation does rather than for as long as the game
/// does. Only the default reaches the host, through the player block and the
/// `rtLogPlayerZpq1` order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductionTemplate {
    /// What the player has called it, up to twelve characters.
    pub name: String,
    /// Its contents. `None` for an empty slot (`fValid == 0`), which the game
    /// shows as `<Unused 2>`.
    pub queue: Option<DefaultQueue>,
}

/// How many template slots the `<Customize>` dialog shows: the default and
/// three more (`ZipProdDlg`'s radio buttons `0x431`…`0x434`).
pub const TEMPLATE_SLOTS: usize = 4;

/// How many the `stars.ini` section carries.
///
/// The array is `ZIPPRODQ[5]` and both the reader and the writer walk all five,
/// but the dialog only ever reaches the first four — so the fifth round-trips
/// through the file without ever being usable.
pub const TEMPLATE_INI_SLOTS: usize = 5;

/// The longest name a template may have, which is its 13-byte field less the
/// terminator.
pub const TEMPLATE_NAME_MAX: usize = 12;

impl ProductionTemplate {
    /// Decode one `stars.ini` value.
    ///
    /// The encoding packs the whole record into printable letters: the first
    /// character is the "no research" flag, the second the entry count, then
    /// four characters per entry — the four nibbles of the `PRODQ1` word,
    /// lowest first, each as `'a' + nibble` — and finally the name.
    ///
    /// Returns `None` for anything the game itself would reject: a value
    /// shorter than three characters or longer than 64, a count above twelve,
    /// a body that is not all in `'a'..='p'`, or a name of thirteen characters
    /// or more.
    ///
    /// Two values are clamped rather than rejected, as `InitStars` clamps
    /// them: a quantity above 1020 becomes **1**, and an item id above 6 —
    /// which is to say anything that is not an auto-build item — becomes
    /// **0**.
    #[must_use]
    pub fn decode_ini(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() <= 2 || bytes.len() >= 0x41 {
            return None;
        }
        let count = usize::from(bytes[1].wrapping_sub(b'a'));
        if count > DEFAULT_QUEUE_MAX {
            return None;
        }
        let body = 2 + count * 4;
        if bytes.len() < body || !bytes[..body].iter().all(|c| (b'a'..=b'p').contains(c)) {
            return None;
        }
        let name = &text[body..];
        if name.len() > TEMPLATE_NAME_MAX {
            return None;
        }

        let nibble = |at: usize| u16::from(bytes[at] - b'a');
        let items = (0..count)
            .map(|i| {
                let at = 2 + i * 4;
                let mut word = nibble(at)
                    | (nibble(at + 1) << 4)
                    | (nibble(at + 2) << 8)
                    | (nibble(at + 3) << 12);
                // A quantity past what the field means is taken as one.
                if word >> 6 > 0x3fc {
                    word = (word & 0x3f) | 0x40;
                }
                // Only the auto-build items belong in a template.
                if word & 0x3f > 6 {
                    word &= 0xffc0;
                }
                DefaultQueueItem {
                    item: (word & 0x3f) as u8,
                    count: word >> 6,
                }
            })
            .collect();

        Some(Self {
            name: name.to_string(),
            queue: Some(DefaultQueue {
                no_research: bytes[0] != b'a',
                items,
            }),
        })
    }

    /// Encode one `stars.ini` value. An empty slot writes an empty string,
    /// which is what the game writes and what it reads back as unused.
    #[must_use]
    pub fn encode_ini(&self) -> String {
        let Some(queue) = self.queue.as_ref() else {
            return String::new();
        };
        let items = &queue.items[..queue.items.len().min(DEFAULT_QUEUE_MAX)];
        let mut out = String::with_capacity(2 + items.len() * 4 + self.name.len());
        out.push(char::from(b'a' + u8::from(queue.no_research)));
        #[allow(clippy::cast_possible_truncation)]
        out.push(char::from(b'a' + items.len() as u8));
        for entry in items {
            let word = u16::from(entry.item & 0x3f) | ((entry.count & 0x03ff) << 6);
            for shift in [0, 4, 8, 12] {
                #[allow(clippy::cast_possible_truncation)]
                out.push(char::from(b'a' + ((word >> shift) & 0xf) as u8));
            }
        }
        out.push_str(&self.name[..self.name.len().min(TEMPLATE_NAME_MAX)]);
        out
    }

    /// The `stars.ini` key this slot is stored under: `ZipOrdersP1` upward.
    #[must_use]
    pub fn ini_key(slot: usize) -> String {
        format!("{TEMPLATE_INI_SECTION}P{}", slot + 1)
    }
}

/// The `stars.ini` section the templates share with the fleet zip orders.
pub const TEMPLATE_INI_SECTION: &str = "ZipOrders";
