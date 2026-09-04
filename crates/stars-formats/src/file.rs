//! Whole-file decode/encode: framing + header + payload (de)cryption.
//!
//! This ties the three lower layers together:
//!
//! 1. [`crate::block`] splits the raw bytes into framed blocks (plaintext
//!    headers, verbatim payloads).
//! 2. [`crate::header::FileHeader`] parses each header block and seeds the
//!    [`crate::crypt::StarsRng`].
//! 3. Every block other than the plaintext header ([`FILE_HEADER_BLOCK`]) and
//!    footer ([`FILE_FOOTER_BLOCK`]) has its payload run through the keystream.
//!
//! ## Files can contain more than one file
//!
//! A `.mN` can hold several complete Stars! files back to back — header,
//! blocks, footer, then another header. It happens when a player receives a
//! new turn before opening the previous one: the new turn is appended rather
//! than replacing what is there. `fixtures/games/exodus/2416/exodus.m6` is one,
//! carrying turns 15 and 16 for player 5.
//!
//! This matters because the keystream is seeded from the header's game id,
//! salt, turn and player, so **each segment must be decrypted with its own
//! keystream**. Decrypting the whole file from the first header yields correct
//! bytes for the first segment and noise for the rest — and, because
//! re-encrypting with the same wrong keystream reproduces the input, a
//! round-trip test cannot detect it. [`StarsFile::segments`] exposes the
//! boundaries.
//!
//! [`StarsFile::decode`] yields blocks with **decrypted** payloads;
//! [`StarsFile::encode`] reverses the process and, for real game files, is
//! byte-for-byte identical to the input (see `tests/real_files.rs`).
//!
//! > **`.xy` note:** universe files are *not* fully block-framed — a game-info
//! > block (type 7) is followed by a raw, unencrypted packed planet array.
//! > [`StarsFile::decode`] therefore returns an error for `.xy`; use the
//! > dedicated [`crate::xy::Universe`] parser instead (it round-trips `.xy`
//! > byte-for-byte). See `docs/formats/xy.md`.

use std::collections::BTreeMap;

use crate::block::{join_blocks, split_blocks, Block, BlockType, FILE_HEADER_BLOCK};
use crate::header::FileHeader;
use crate::{FormatError, Result};

/// Type id of the plaintext file-footer block that ends most Stars! files.
///
/// Like the header, the footer is **not** encrypted; it typically carries the
/// year (`.m`/`.hst`) or a checksum (`.r`).
pub const FILE_FOOTER_BLOCK: u8 = 0;

/// One complete Stars! file within a (possibly concatenated) stream.
#[derive(Debug, Clone)]
pub struct Segment {
    /// The parsed header that starts this segment and seeds its keystream.
    pub header: FileHeader,
    /// Index of this segment's header block within [`StarsFile::blocks`].
    pub start: usize,
    /// One past the index of this segment's last block.
    pub end: usize,
}

/// A decoded Stars! file: its parsed [`FileHeader`] and its blocks with
/// **decrypted** payloads (the header and footer blocks are kept verbatim).
#[derive(Debug, Clone)]
pub struct StarsFile {
    /// The parsed header of the **first** segment.
    ///
    /// Most files have exactly one segment; see [`StarsFile::segments`] when
    /// they do not.
    pub header: FileHeader,
    /// All blocks in file order, with non-header/footer payloads decrypted.
    pub blocks: Vec<Block>,
    /// The concatenated files this stream holds, in order. Always at least one.
    pub segments: Vec<Segment>,
}

impl StarsFile {
    /// Decode raw file bytes into a header plus decrypted blocks.
    ///
    /// # Errors
    ///
    /// - Propagates [`FormatError::UnexpectedEof`] from framing on a truncated
    ///   file (this also currently occurs for `.xy`; see the module note).
    /// - [`FormatError::Malformed`] if the file does not start with a
    ///   [`FILE_HEADER_BLOCK`].
    /// - Propagates header-parse errors (e.g. [`FormatError::BadMagic`]).
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let raw = split_blocks(bytes)?;
        let first = raw
            .first()
            .ok_or_else(|| FormatError::Malformed("file contains no blocks".into()))?;
        if first.type_id != FILE_HEADER_BLOCK {
            return Err(FormatError::Malformed(format!(
                "file does not start with a header block (type {}), found type {}",
                FILE_HEADER_BLOCK, first.type_id
            )));
        }

        let header = FileHeader::parse(&first.data)?;

        let mut blocks = Vec::with_capacity(raw.len());
        let mut segments: Vec<Segment> = Vec::new();
        // Seeded afresh at every header block, because the seed is derived from
        // that header's turn and player.
        let mut rng = header.init_rng();

        for (index, block) in raw.into_iter().enumerate() {
            if block.type_id == FILE_HEADER_BLOCK {
                let parsed = FileHeader::parse(&block.data)?;
                rng = parsed.init_rng();
                if let Some(previous) = segments.last_mut() {
                    previous.end = index;
                }
                segments.push(Segment {
                    header: parsed,
                    start: index,
                    end: index,
                });
                blocks.push(block);
            } else if is_plaintext(block.type_id) {
                blocks.push(block);
            } else {
                let data = rng.apply(&block.data);
                blocks.push(Block::new(block.type_id, data)?);
            }
        }

        if let Some(last) = segments.last_mut() {
            last.end = blocks.len();
        }

        Ok(Self {
            header,
            blocks,
            segments,
        })
    }

    /// The concatenated files this stream holds.
    ///
    /// Nearly every real file has exactly one; a `.mN` holding an unopened turn
    /// plus the next one has two. See the module documentation.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The blocks belonging to one segment, header and footer included.
    #[must_use]
    pub fn segment_blocks(&self, segment: &Segment) -> &[Block] {
        &self.blocks[segment.start..segment.end]
    }

    /// The last segment, which is the current turn when a file holds several.
    #[must_use]
    pub fn latest_segment(&self) -> &Segment {
        self.segments
            .last()
            .expect("decode always records at least one segment")
    }

    /// Re-encode a decoded file back to raw bytes.
    ///
    /// Inverse of [`StarsFile::decode`]: re-encrypts every non-header/footer
    /// payload with a freshly seeded keystream and re-frames the blocks. For
    /// unmodified real files this reproduces the original bytes exactly.
    ///
    /// # Errors
    ///
    /// [`FormatError::Malformed`] if any block no longer fits the header word's
    /// bit fields.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut rng = self.header.init_rng();
        let mut raw = Vec::with_capacity(self.blocks.len());
        for block in &self.blocks {
            if block.type_id == FILE_HEADER_BLOCK {
                // Re-seed exactly as decoding did, so a stream of concatenated
                // files re-encrypts segment by segment.
                rng = FileHeader::parse(&block.data)?.init_rng();
                raw.push(block.clone());
            } else if is_plaintext(block.type_id) {
                raw.push(block.clone());
            } else {
                let data = rng.apply(&block.data);
                raw.push(Block::new(block.type_id, data)?);
            }
        }
        join_blocks(&raw)
    }

    /// Assemble a complete file from a header, a body and a footer payload.
    ///
    /// The inverse of reading a file that was never read: the header block is
    /// written first as plaintext, every body block is framed and encrypted
    /// with the keystream the header seeds, and the footer follows in
    /// plaintext. This is what writes a `.hst` or `.mN` that this crate
    /// produced rather than loaded.
    ///
    /// # Errors
    ///
    /// [`FormatError::Malformed`] if a block's payload is longer than the
    /// 10-bit size field of a block header.
    pub fn build(header: &FileHeader, body: &[Block], footer: Vec<u8>) -> Result<Vec<u8>> {
        let mut rng = header.init_rng();
        let mut raw = Vec::with_capacity(body.len() + 2);
        raw.push(Block::new(FILE_HEADER_BLOCK, header.to_payload().to_vec())?);
        for block in body {
            if is_plaintext(block.type_id) {
                raw.push(block.clone());
            } else {
                raw.push(Block::new(block.type_id, rng.apply(&block.data))?);
            }
        }
        raw.push(Block::new(FILE_FOOTER_BLOCK, footer)?);
        join_blocks(&raw)
    }

    /// Count the blocks in this file grouped by [`BlockType`].
    ///
    /// A quick structural inventory of a decoded file — useful for triaging an
    /// unknown save and for tests that assert a file's block makeup (e.g. a
    /// `.hst` containing one [`BlockType::Player`] per player and one
    /// [`BlockType::Planet`] per planet).
    #[must_use]
    pub fn block_counts(&self) -> BTreeMap<BlockType, usize> {
        let mut counts = BTreeMap::new();
        for block in &self.blocks {
            *counts.entry(block.block_type()).or_insert(0) += 1;
        }
        counts
    }
}

/// Whether a block type is stored in plaintext (header and footer).
fn is_plaintext(type_id: u8) -> bool {
    type_id == FILE_HEADER_BLOCK || type_id == FILE_FOOTER_BLOCK
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypt::StarsRng;

    /// Build a small synthetic but structurally valid encrypted file.
    fn synthetic_file() -> Vec<u8> {
        // Header payload: magic, game_id, ver, turn, player word, dts.
        let mut hdr = Vec::new();
        hdr.extend_from_slice(b"J3J3");
        hdr.extend_from_slice(&0x1234_5678u32.to_le_bytes());
        hdr.extend_from_slice(&0x2840u16.to_le_bytes());
        hdr.extend_from_slice(&0u16.to_le_bytes());
        hdr.extend_from_slice(&0x4900u16.to_le_bytes());
        hdr.extend_from_slice(&3u16.to_le_bytes());

        let header = FileHeader::parse(&hdr).unwrap();
        let mut rng = header.init_rng();

        let plain_a = vec![0x11u8, 0x22, 0x33, 0x44, 0x55];
        let plain_b = vec![0xAAu8; 8];
        let enc_a = rng.apply(&plain_a);
        let enc_b = rng.apply(&plain_b);

        let blocks = vec![
            Block::new(FILE_HEADER_BLOCK, hdr).unwrap(),
            Block::new(6, enc_a).unwrap(),
            Block::new(13, enc_b).unwrap(),
            Block::new(FILE_FOOTER_BLOCK, vec![0x00, 0x00]).unwrap(),
        ];
        join_blocks(&blocks).unwrap()
    }

    #[test]
    fn decode_then_encode_round_trips() {
        let bytes = synthetic_file();
        let file = StarsFile::decode(&bytes).unwrap();
        assert_eq!(file.header.game_id, 0x1234_5678);
        // Payloads are decrypted in-memory.
        assert_eq!(file.blocks[1].data, vec![0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(file.blocks[3].data, vec![0x00, 0x00]); // footer plaintext
                                                           // ...and re-encoding reproduces the original bytes.
        assert_eq!(file.encode().unwrap(), bytes);
    }

    #[test]
    fn rejects_file_not_starting_with_header() {
        // A lone non-header block.
        let mut rng = StarsRng::new(3, 5, 1);
        let enc = rng.apply(&[1, 2, 3, 4]);
        let bytes = join_blocks(&[Block::new(6, enc).unwrap()]).unwrap();
        assert!(matches!(
            StarsFile::decode(&bytes),
            Err(FormatError::Malformed(_))
        ));
    }
}
