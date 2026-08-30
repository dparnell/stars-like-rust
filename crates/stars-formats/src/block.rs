//! Block framing layer shared by **every** Stars! on-disk file.
//!
//! All Stars! files (`.xy`, `.mN`, `.hN`, `.xN`, `.rN`, `.hst`) are a flat
//! sequence of *blocks*. Each block is introduced by a little-endian 16-bit
//! header word packing a type id and a payload length, followed by the raw
//! payload bytes:
//!
//! ```text
//! +--------------------------------+----------------------+
//! | header word (u16, little-end.) | payload (`size` bytes)|
//! +--------------------------------+----------------------+
//!   bits 15..10 = type id (6 bits, 0..=63)
//!   bits  9.. 0 = payload size (10 bits, 0..=1023)
//! ```
//!
//! This module implements only the **container framing** — splitting a file
//! into blocks and re-serialising blocks back into a file. It intentionally
//! does *not* interpret or decrypt payloads: the first block is a plaintext
//! [`FILE_HEADER_BLOCK`] and the remaining blocks' payloads are obfuscated with
//! the Stars! stream cipher, which is decoded in a separate layer (see
//! `docs/formats/blocks.md`).
//!
//! ## Why the framing round-trips byte-for-byte
//!
//! Framing copies each payload verbatim; it never touches the (possibly
//! encrypted) payload bytes. Therefore
//!
//! ```text
//! join_blocks(&split_blocks(bytes)?)? == bytes
//! ```
//!
//! holds for **real** game files as well as synthetic ones, giving us a
//! genuine byte-accurate round-trip anchor before any payload is decoded.

use crate::{FormatError, Result};

/// Mask selecting the payload-size bits (low 10 bits) of a block header word.
pub const BLOCK_SIZE_MASK: u16 = 0x03FF;

/// Bit position of the type-id field (high 6 bits) within a block header word.
pub const BLOCK_TYPE_SHIFT: u16 = 10;

/// Largest payload length representable in the 10-bit size field.
pub const MAX_BLOCK_SIZE: usize = BLOCK_SIZE_MASK as usize;

/// Largest type id representable in the 6-bit type field.
pub const MAX_BLOCK_TYPE: u8 = (0xFFFF >> BLOCK_TYPE_SHIFT) as u8;

/// Type id of the plaintext file-header block that begins every Stars! file.
///
/// The header block carries the game id, turn, player, version and the salt
/// used to seed the stream cipher for the remaining blocks. It is the one block
/// whose payload is stored unencrypted. (Confirmed as the well-known Stars!
/// value `8`; the full type registry is populated as payloads are decoded — see
/// `docs/formats/blocks.md`.)
pub const FILE_HEADER_BLOCK: u8 = 8;

/// A single framed block: its 6-bit type id plus the raw payload bytes.
///
/// The payload is stored exactly as it appears on disk (still encrypted for
/// non-header blocks). Payload interpretation belongs to higher layers.
#[derive(Clone, PartialEq, Eq)]
pub struct Block {
    /// Block type id (`0..=63`).
    pub type_id: u8,
    /// Raw payload bytes (`0..=1023` bytes), copied verbatim from the file.
    pub data: Vec<u8>,
}

/// The kind of a block, resolved from its 6-bit type id.
///
/// The names come from the community-documented Stars! block registry (see the
/// table in `docs/formats/blocks.md`), cross-checked against the block types
/// that actually appear in our real sample files (`.hst`/`.mN`/`.xN`/`.rN`).
/// Only [`BlockType::FileHeader`] (8) and [`BlockType::FileFooter`] (0) are
/// stored in plaintext; everything else is encrypted.
///
/// Unknown ids are preserved via [`BlockType::Other`] so framing/round-tripping
/// never depends on the registry being complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BlockType {
    /// 0 — file footer (plaintext); carries a year or checksum.
    FileFooter,
    /// 6 — player/race record (full player in `.mN`/`.hst`, race in `.rN`).
    Player,
    /// 7 — `.xy` game-info / planets header block.
    Planets,
    /// 8 — file header (plaintext); seeds the stream cipher.
    FileHeader,
    /// 12 — events.
    Events,
    /// 13 — planet record.
    Planet,
    /// 14 — partial planet record.
    PartialPlanet,
    /// 15 — minimal planet record (`rtPlanetC`).
    MinimalPlanet,
    /// 16 — fleet record.
    Fleet,
    /// 17 — partial fleet record.
    PartialFleet,
    /// 20 — waypoint.
    Waypoint,
    /// 21 — fleet name.
    FleetName,
    /// 26 — ship/starbase design.
    Design,
    /// 28 — production queue.
    ProductionQueue,
    /// 30 — battle plan.
    BattlePlan,
    /// 43 — generic object.
    Object,
    /// Any type id without a dedicated variant yet.
    Other(u8),
}

impl BlockType {
    /// Resolve a raw 6-bit type id to a [`BlockType`].
    #[must_use]
    pub fn from_id(id: u8) -> Self {
        match id {
            0 => Self::FileFooter,
            6 => Self::Player,
            7 => Self::Planets,
            8 => Self::FileHeader,
            12 => Self::Events,
            13 => Self::Planet,
            14 => Self::PartialPlanet,
            15 => Self::MinimalPlanet,
            16 => Self::Fleet,
            17 => Self::PartialFleet,
            20 => Self::Waypoint,
            21 => Self::FleetName,
            26 => Self::Design,
            28 => Self::ProductionQueue,
            30 => Self::BattlePlan,
            43 => Self::Object,
            other => Self::Other(other),
        }
    }

    /// The raw 6-bit type id this variant encodes to.
    #[must_use]
    pub fn id(self) -> u8 {
        match self {
            Self::FileFooter => 0,
            Self::Player => 6,
            Self::Planets => 7,
            Self::FileHeader => 8,
            Self::Events => 12,
            Self::Planet => 13,
            Self::PartialPlanet => 14,
            Self::MinimalPlanet => 15,
            Self::Fleet => 16,
            Self::PartialFleet => 17,
            Self::Waypoint => 20,
            Self::FleetName => 21,
            Self::Design => 26,
            Self::ProductionQueue => 28,
            Self::BattlePlan => 30,
            Self::Object => 43,
            Self::Other(id) => id,
        }
    }

    /// Whether blocks of this type are stored in plaintext (header and footer).
    #[must_use]
    pub fn is_plaintext(self) -> bool {
        matches!(self, Self::FileHeader | Self::FileFooter)
    }
}

impl Block {
    /// Create a block, validating that the type id and payload length fit in
    /// the header word's 6-bit and 10-bit fields respectively.
    pub fn new(type_id: u8, data: Vec<u8>) -> Result<Self> {
        if type_id > MAX_BLOCK_TYPE {
            return Err(FormatError::Malformed(format!(
                "block type id {type_id} does not fit in 6 bits (max {MAX_BLOCK_TYPE})"
            )));
        }
        if data.len() > MAX_BLOCK_SIZE {
            return Err(FormatError::Malformed(format!(
                "block payload of {} bytes exceeds the 10-bit size field (max {MAX_BLOCK_SIZE})",
                data.len()
            )));
        }
        Ok(Self { type_id, data })
    }

    /// Payload length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the payload is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Whether this is the plaintext [`FILE_HEADER_BLOCK`].
    #[must_use]
    pub fn is_file_header(&self) -> bool {
        self.type_id == FILE_HEADER_BLOCK
    }

    /// The [`BlockType`] this block's type id resolves to.
    #[must_use]
    pub fn block_type(&self) -> BlockType {
        BlockType::from_id(self.type_id)
    }
}

impl core::fmt::Debug for Block {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Block")
            .field("type_id", &self.type_id)
            .field("len", &self.data.len())
            .finish()
    }
}

/// Split a raw Stars! file into its sequence of framed blocks.
///
/// Reads each 16-bit little-endian header word, unpacks the 6-bit type id and
/// 10-bit payload size, and copies the payload verbatim.
///
/// # Errors
///
/// Returns [`FormatError::UnexpectedEof`] if the input ends in the middle of a
/// header word or a declared payload (i.e. a truncated file).
pub fn split_blocks(bytes: &[u8]) -> Result<Vec<Block>> {
    let mut blocks = Vec::new();
    let mut offset = 0usize;

    while offset < bytes.len() {
        // Header word.
        if offset + 2 > bytes.len() {
            return Err(FormatError::UnexpectedEof {
                offset,
                needed: 2 - (bytes.len() - offset),
            });
        }
        let header = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;

        let size = (header & BLOCK_SIZE_MASK) as usize;
        let type_id = (header >> BLOCK_TYPE_SHIFT) as u8;

        // Payload.
        if offset + size > bytes.len() {
            return Err(FormatError::UnexpectedEof {
                offset,
                needed: (offset + size) - bytes.len(),
            });
        }
        let data = bytes[offset..offset + size].to_vec();
        offset += size;

        blocks.push(Block { type_id, data });
    }

    Ok(blocks)
}

/// Serialise a sequence of blocks back into a raw Stars! file.
///
/// Inverse of [`split_blocks`]: writes each block's header word followed by its
/// payload.
///
/// # Errors
///
/// Returns [`FormatError::Malformed`] if any block's type id or payload length
/// does not fit the header word's bit fields.
pub fn join_blocks(blocks: &[Block]) -> Result<Vec<u8>> {
    let total: usize = blocks.iter().map(|b| 2 + b.data.len()).sum();
    let mut out = Vec::with_capacity(total);

    for block in blocks {
        if block.type_id > MAX_BLOCK_TYPE {
            return Err(FormatError::Malformed(format!(
                "block type id {} does not fit in 6 bits (max {MAX_BLOCK_TYPE})",
                block.type_id
            )));
        }
        if block.data.len() > MAX_BLOCK_SIZE {
            return Err(FormatError::Malformed(format!(
                "block payload of {} bytes exceeds the 10-bit size field (max {MAX_BLOCK_SIZE})",
                block.data.len()
            )));
        }

        let header = (u16::from(block.type_id) << BLOCK_TYPE_SHIFT)
            | (block.data.len() as u16 & BLOCK_SIZE_MASK);
        out.extend_from_slice(&header.to_le_bytes());
        out.extend_from_slice(&block.data);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal but structurally valid stream: a header block followed
    /// by two arbitrary payload blocks.
    fn sample_stream() -> Vec<u8> {
        let blocks = vec![
            Block::new(FILE_HEADER_BLOCK, b"J3J3\x01\x02\x03\x04".to_vec()).unwrap(),
            Block::new(1, vec![0xAA; 5]).unwrap(),
            Block::new(63, vec![]).unwrap(),
        ];
        join_blocks(&blocks).unwrap()
    }

    #[test]
    fn header_bit_layout_is_type_hi6_size_lo10() {
        // type_id = 8, size = 4  =>  (8 << 10) | 4 = 0x2004, little-endian 04 20
        let bytes = join_blocks(&[Block::new(8, vec![1, 2, 3, 4]).unwrap()]).unwrap();
        assert_eq!(&bytes[0..2], &[0x04, 0x20]);
        assert_eq!(&bytes[2..], &[1, 2, 3, 4]);
    }

    #[test]
    fn split_then_join_round_trips() {
        let bytes = sample_stream();
        let blocks = split_blocks(&bytes).unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].type_id, FILE_HEADER_BLOCK);
        assert!(blocks[0].is_file_header());
        assert_eq!(blocks[1].len(), 5);
        assert_eq!(blocks[2].len(), 0);
        assert_eq!(join_blocks(&blocks).unwrap(), bytes);
    }

    #[test]
    fn empty_input_yields_no_blocks() {
        assert!(split_blocks(&[]).unwrap().is_empty());
        assert!(join_blocks(&[]).unwrap().is_empty());
    }

    #[test]
    fn max_size_payload_round_trips() {
        let block = Block::new(5, vec![0x5A; MAX_BLOCK_SIZE]).unwrap();
        let bytes = join_blocks(std::slice::from_ref(&block)).unwrap();
        assert_eq!(split_blocks(&bytes).unwrap(), vec![block]);
    }

    #[test]
    fn oversized_payload_is_rejected() {
        assert!(Block::new(0, vec![0; MAX_BLOCK_SIZE + 1]).is_err());
        let bad = Block {
            type_id: 0,
            data: vec![0; MAX_BLOCK_SIZE + 1],
        };
        assert!(join_blocks(&[bad]).is_err());
    }

    #[test]
    fn truncated_header_is_eof() {
        // Single stray byte: not enough for a 2-byte header word.
        let err = split_blocks(&[0x00]).unwrap_err();
        assert!(matches!(err, FormatError::UnexpectedEof { .. }));
    }

    #[test]
    fn truncated_payload_is_eof() {
        // Header claims 4 payload bytes but only 1 is present.
        let bytes = [0x04, 0x20, 0xFF];
        let err = split_blocks(&bytes).unwrap_err();
        assert!(matches!(err, FormatError::UnexpectedEof { .. }));
    }

    #[test]
    fn block_type_id_round_trips_for_all_ids() {
        // `from_id`/`id` are inverse for every 6-bit type id, including the
        // `Other` fallback for ids without a dedicated variant.
        for id in 0..=MAX_BLOCK_TYPE {
            assert_eq!(BlockType::from_id(id).id(), id, "type id {id}");
        }
    }

    #[test]
    fn block_type_named_variants_and_plaintext() {
        assert_eq!(BlockType::from_id(8), BlockType::FileHeader);
        assert_eq!(BlockType::from_id(0), BlockType::FileFooter);
        assert_eq!(BlockType::from_id(13), BlockType::Planet);
        assert_eq!(BlockType::from_id(16), BlockType::Fleet);
        assert_eq!(BlockType::from_id(63), BlockType::Other(63));
        // Only header/footer are plaintext.
        assert!(BlockType::FileHeader.is_plaintext());
        assert!(BlockType::FileFooter.is_plaintext());
        assert!(!BlockType::Planet.is_plaintext());
        assert!(!BlockType::Other(63).is_plaintext());
    }

    #[test]
    fn block_exposes_its_block_type() {
        let b = Block::new(13, vec![0; 4]).unwrap();
        assert_eq!(b.block_type(), BlockType::Planet);
    }
}
