//! `.xy` universe file: header + game-info + the packed planet array.
//!
//! Unlike the other formats, a `.xy` file is **not** a clean sequence of blocks
//! to EOF. It is:
//!
//! ```text
//! [ FileHeaderBlock (type 8, plaintext, 16-byte payload) ]
//! [ game-info block  (type 7, encrypted, 64-byte payload) ]
//! [ planet region: 2-byte header + N * 4-byte planet records ]  <- to EOF
//! ```
//!
//! The planet region is **not** block-framed and — unlike every other payload —
//! is stored *without* the stream cipher: parsing the raw on-disk bytes as
//! 4-byte records yields sane, in-bounds, non-overlapping coordinates, whereas
//! decrypting them first yields noise (see `docs/formats/xy.md`).
//!
//! Each 4-byte record is a little-endian `u32` partitioned as:
//!
//! ```text
//! bits  0..9   x coordinate   (10 bits)
//! bits 10..19  y coordinate   (10 bits)
//! bits 20..31  planet name index (12 bits)
//! ```
//!
//! The 10/10/12 *partition* and the 2-byte region header are confirmed by the
//! record count (exactly `N` planets, matching the `.hst`) and by the values
//! being cleanly bounded and non-overlapping at this offset (and broken at any
//! other). The axis assignment (which 10-bit field is x vs y) follows the
//! community convention and is not independently confirmed; it does not affect
//! byte-accuracy, which [`Universe::encode`] reproduces exactly.

use crate::block::{join_blocks, Block, FILE_HEADER_BLOCK};
use crate::header::FileHeader;
use crate::{FormatError, Result};

/// Type id of the `.xy` game-info block (a "planets" block per the registry).
pub const GAME_INFO_BLOCK: u8 = 7;

/// Size in bytes of one packed planet record in the `.xy` planet region.
pub const PLANET_RECORD_LEN: usize = 4;

/// A single planet's fixed position, unpacked from a 4-byte `.xy` record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetPosition {
    /// X coordinate (low 10 bits of the record).
    pub x: u16,
    /// Y coordinate (next 10 bits of the record).
    pub y: u16,
    /// Planet name index (high 12 bits of the record).
    pub name_index: u16,
}

impl PlanetPosition {
    /// Unpack a 4-byte record word into a position.
    #[must_use]
    pub fn from_word(word: u32) -> Self {
        Self {
            x: (word & 0x03FF) as u16,
            y: ((word >> 10) & 0x03FF) as u16,
            name_index: ((word >> 20) & 0x0FFF) as u16,
        }
    }

    /// Re-pack this position into its 4-byte record word.
    #[must_use]
    pub fn to_word(self) -> u32 {
        (u32::from(self.x) & 0x03FF)
            | ((u32::from(self.y) & 0x03FF) << 10)
            | ((u32::from(self.name_index) & 0x0FFF) << 20)
    }
}

/// A parsed `.xy` universe file.
///
/// [`Universe::decode`] and [`Universe::encode`] are byte-exact inverses for
/// real files, so `.xy` now round-trips like the fully-framed formats.
#[derive(Debug, Clone)]
pub struct Universe {
    /// The parsed file header.
    pub header: FileHeader,
    /// Raw 16-byte header payload, kept verbatim to re-emit the plaintext block.
    header_payload: Vec<u8>,
    /// Decrypted game-info (type-7) payload.
    pub game_info: Vec<u8>,
    /// The 2-byte header that precedes the planet records (meaning TBD).
    pub region_header: [u8; 2],
    /// The fixed planet positions, in planet-id order.
    pub planets: Vec<PlanetPosition>,
}

impl Universe {
    /// Decode a `.xy` file into its header, game-info and planet array.
    ///
    /// # Errors
    ///
    /// - [`FormatError::UnexpectedEof`] if the file is too short to contain the
    ///   header and game-info blocks.
    /// - [`FormatError::Malformed`] if the leading block is not a header block,
    ///   the game-info block is not type [`GAME_INFO_BLOCK`], or the planet
    ///   region is not a whole number of 4-byte records after its 2-byte header.
    /// - Propagates header-parse errors (e.g. [`FormatError::BadMagic`]).
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        // --- header block (type 8, plaintext) ---
        let (hdr_type, hdr_payload, after_hdr) = read_block(bytes, 0)?;
        if hdr_type != FILE_HEADER_BLOCK {
            return Err(FormatError::Malformed(format!(
                ".xy does not start with a header block (type {FILE_HEADER_BLOCK}), found {hdr_type}"
            )));
        }
        let header = FileHeader::parse(hdr_payload)?;

        // --- game-info block (type 7, encrypted) ---
        let (gi_type, gi_payload, after_gi) = read_block(bytes, after_hdr)?;
        if gi_type != GAME_INFO_BLOCK {
            return Err(FormatError::Malformed(format!(
                ".xy second block is not the game-info block (type {GAME_INFO_BLOCK}), found {gi_type}"
            )));
        }
        let mut rng = header.init_rng();
        let game_info = rng.apply(gi_payload);

        // --- planet region: 2-byte header + N * 4-byte records (plaintext) ---
        let region = &bytes[after_gi..];
        if region.len() < 2 {
            return Err(FormatError::UnexpectedEof {
                offset: after_gi,
                needed: 2 - region.len(),
            });
        }
        let region_header = [region[0], region[1]];
        let body = &region[2..];
        if !body.len().is_multiple_of(PLANET_RECORD_LEN) {
            return Err(FormatError::Malformed(format!(
                ".xy planet region body ({} bytes) is not a whole number of {PLANET_RECORD_LEN}-byte records",
                body.len()
            )));
        }
        let planets = (0..body.len() / PLANET_RECORD_LEN)
            .map(|i| {
                let o = i * PLANET_RECORD_LEN;
                PlanetPosition::from_word(u32::from_le_bytes([
                    body[o],
                    body[o + 1],
                    body[o + 2],
                    body[o + 3],
                ]))
            })
            .collect();

        Ok(Self {
            header,
            header_payload: hdr_payload.to_vec(),
            game_info,
            region_header,
            planets,
        })
    }

    /// Re-encode this universe back to raw `.xy` bytes.
    ///
    /// Byte-for-byte inverse of [`Universe::decode`] for unmodified real files.
    ///
    /// # Errors
    ///
    /// [`FormatError::Malformed`] if a block no longer fits the header word.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut rng = self.header.init_rng();
        let header_block = Block::new(FILE_HEADER_BLOCK, self.header_payload.clone())?;
        let game_info_block = Block::new(GAME_INFO_BLOCK, rng.apply(&self.game_info))?;
        let mut out = join_blocks(&[header_block, game_info_block])?;

        out.extend_from_slice(&self.region_header);
        for planet in &self.planets {
            out.extend_from_slice(&planet.to_word().to_le_bytes());
        }
        Ok(out)
    }

    /// The number of planets in the universe.
    #[must_use]
    pub fn planet_count(&self) -> usize {
        self.planets.len()
    }
}

/// Read one framed block starting at `offset`, returning `(type_id, payload,
/// next_offset)`.
fn read_block(bytes: &[u8], offset: usize) -> Result<(u8, &[u8], usize)> {
    if offset + 2 > bytes.len() {
        return Err(FormatError::UnexpectedEof {
            offset,
            needed: (offset + 2) - bytes.len(),
        });
    }
    let header = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
    let size = (header & 0x03FF) as usize;
    let type_id = (header >> 10) as u8;
    let start = offset + 2;
    if start + size > bytes.len() {
        return Err(FormatError::UnexpectedEof {
            offset: start,
            needed: (start + size) - bytes.len(),
        });
    }
    Ok((type_id, &bytes[start..start + size], start + size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planet_position_word_round_trips() {
        for word in [0u32, 0x0000_0001, 0x8c05_5555, 0xFFFF_FFFF, 0x1c05_8552] {
            // The name index is only 12 bits, so the top 12 bits survive; the
            // full 32-bit word round-trips because 10+10+12 = 32.
            assert_eq!(
                PlanetPosition::from_word(word).to_word(),
                word,
                "{word:#010x}"
            );
        }
    }

    #[test]
    fn planet_position_field_split() {
        // x=5, y=10, name=3  ->  5 | (10<<10) | (3<<20)
        let p = PlanetPosition {
            x: 5,
            y: 10,
            name_index: 3,
        };
        let w = p.to_word();
        assert_eq!(w, 5 | (10 << 10) | (3 << 20));
        assert_eq!(PlanetPosition::from_word(w), p);
    }
}
