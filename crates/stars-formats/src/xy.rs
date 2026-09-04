//! `.xy` universe file: header + game-info + the packed planet array.
//!
//! Unlike the other formats, a `.xy` file is **not** a clean sequence of blocks
//! to EOF. It is:
//!
//! ```text
//! [ FileHeaderBlock (type 8, plaintext, 16-byte payload) ]
//! [ game-info block  (type 7, encrypted, 64-byte payload) ]
//! [ planet region: N * 4-byte planet records + trailer ]  <- to EOF
//! ```
//!
//! The number of planets **N** is not derived from the file length; it is read
//! from the game-info block (a little-endian `u16` at offset 10, verified
//! against `24/128/160/360/540`-planet real files). After the `N` records an
//! optional **trailer** follows: an **in-game** `.xy` carries a 2-byte trailer
//! (`00 00`), while a **standalone** universe-definition file carries a 4-byte
//! trailer (`02 00 <players> 00`). The trailer is preserved verbatim so every
//! file round-trips.
//!
//! The planet region is **not** block-framed and — unlike every other payload —
//! is stored *without* the stream cipher: parsing the raw on-disk bytes as
//! 4-byte records yields sane, in-bounds, non-overlapping coordinates.
//!
//! Each 4-byte record is a little-endian `u32` partitioned as (confirmed
//! against six real universes and matching the community `struct position`):
//!
//! ```text
//! bits  0..9   x offset   (10 bits) — delta added to the running x
//! bits 10..21  y          (12 bits) — absolute y coordinate
//! bits 22..31  name id     (10 bits) — index into the master planet-name table
//! ```
//!
//! `x` is **not** stored absolutely: each record's `xoffset` is added to a
//! running total, so a planet's absolute x is the sum of all `xoffset`s up to
//! and including it. This is why planets are stored in non-decreasing x order
//! (the original tools require `x` to never decrease planet-to-planet). With
//! this reconstruction every planet in every sample universe has a unique
//! position, its `y` lies in a band whose width matches the universe size
//! class, and its `name id` resolves to a unique entry in the master name table
//! (see `docs/formats/xy.md` and [`crate::names`]).

use crate::block::{join_blocks, Block, FILE_HEADER_BLOCK};
use crate::header::FileHeader;
use crate::names::planet_name;
use crate::{FormatError, Result};

/// Type id of the `.xy` game-info block (a "planets" block per the registry).
pub const GAME_INFO_BLOCK: u8 = 7;

/// Size in bytes of one packed planet record in the `.xy` planet region.
pub const PLANET_RECORD_LEN: usize = 4;

/// Offset of the planet-count `u16` within the decrypted game-info payload.
pub const GAME_INFO_PLANET_COUNT_OFFSET: usize = 10;

/// Offset of the player-count byte within the decrypted game-info payload.
///
/// The player count is the low 5 bits of this byte.
pub const GAME_INFO_PLAYER_COUNT_OFFSET: usize = 8;

/// A single planet's packed record, unpacked from a 4-byte `.xy` word.
///
/// The fields are stored **exactly as on disk**, so [`PlanetPosition::to_word`]
/// is a byte-exact inverse of [`PlanetPosition::from_word`]. Note that
/// [`PlanetPosition::x_offset`] is a *delta*, not an absolute coordinate — use
/// [`Universe::planets_resolved`] to obtain absolute positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetPosition {
    /// X offset (low 10 bits): the amount added to the running x total.
    pub x_offset: u16,
    /// Y coordinate (next 12 bits): absolute.
    pub y: u16,
    /// Planet name index (high 10 bits): an index into the master name table.
    pub name_index: u16,
}

impl PlanetPosition {
    /// Unpack a 4-byte record word into its fields.
    #[must_use]
    pub fn from_word(word: u32) -> Self {
        Self {
            x_offset: (word & 0x03FF) as u16,
            y: ((word >> 10) & 0x0FFF) as u16,
            name_index: ((word >> 22) & 0x03FF) as u16,
        }
    }

    /// Re-pack these fields into their 4-byte record word.
    #[must_use]
    pub fn to_word(self) -> u32 {
        (u32::from(self.x_offset) & 0x03FF)
            | ((u32::from(self.y) & 0x0FFF) << 10)
            | ((u32::from(self.name_index) & 0x03FF) << 22)
    }
}

/// A planet with its **absolute** position and resolved name.
///
/// Produced by [`Universe::planets_resolved`] after summing the per-record
/// `x_offset`s and looking each `name_index` up in the master name table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Planet {
    /// Planet id (0-based index within the universe).
    pub id: u16,
    /// Absolute x coordinate (running sum of `x_offset`s up to this planet).
    pub x: u32,
    /// Absolute y coordinate.
    pub y: u16,
    /// The 10-bit name index stored in the record.
    pub name_index: u16,
    /// The resolved planet name, or `None` if the index is outside the table.
    pub name: Option<&'static str>,
}

/// The base every planet's x coordinate is measured from.
///
/// The `.xy` stores x as a chain of 10-bit deltas, and the chain starts at 1000
/// rather than 0. Confirmed against the engine's own coordinates: a fleet the
/// game records as orbiting a planet must sit exactly on it, and across
/// **43,769 orbiting-fleet readings** in the two sixteen-player games the
/// difference `fleet - planet` was `(1000, 0)` every single time — never any
/// other value, and never any offset in y.
///
/// This could not be caught by the round-trip tests, which re-emit the same
/// packed deltas whatever base they are read against.
pub const X_BASE: u32 = 1000;

/// A parsed `.xy` universe file.
///
/// [`Universe::decode`] and [`Universe::encode`] are byte-exact inverses for
/// real files, so `.xy` round-trips like the fully-framed formats.
#[derive(Debug, Clone)]
pub struct Universe {
    /// The parsed file header.
    pub header: FileHeader,
    /// Raw 16-byte header payload, kept verbatim to re-emit the plaintext block.
    header_payload: Vec<u8>,
    /// Decrypted game-info (type-7) payload.
    pub game_info: Vec<u8>,
    /// The packed planet records, in planet-id order.
    pub planets: Vec<PlanetPosition>,
    /// Any bytes following the planet records, kept verbatim.
    ///
    /// A 2-byte `00 00` for an in-game `.xy`; a 4-byte `02 00 <players> 00` for
    /// a standalone universe-definition file (see the module docs).
    pub trailer: Vec<u8>,
}

impl Universe {
    /// Decode a `.xy` file into its header, game-info and planet array.
    ///
    /// # Errors
    ///
    /// - [`FormatError::UnexpectedEof`] if the file is too short to contain the
    ///   header and game-info blocks.
    /// - [`FormatError::Malformed`] if the leading block is not a header block,
    ///   the game-info block is not type [`GAME_INFO_BLOCK`], the game-info
    ///   payload is too short to hold the planet count, or the planet region is
    ///   too short for the declared number of 4-byte records.
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

        // The planet count is authoritative from the game-info block, not the
        // file length (the region carries a trailer).
        if game_info.len() < GAME_INFO_PLANET_COUNT_OFFSET + 2 {
            return Err(FormatError::Malformed(format!(
                ".xy game-info block ({} bytes) too short for the planet count",
                game_info.len()
            )));
        }
        let planet_count = u16::from_le_bytes([
            game_info[GAME_INFO_PLANET_COUNT_OFFSET],
            game_info[GAME_INFO_PLANET_COUNT_OFFSET + 1],
        ]) as usize;

        // --- planet region: N * 4-byte records + trailer (no leading header) ---
        let body = &bytes[after_gi..];
        let records_len = planet_count * PLANET_RECORD_LEN;
        if body.len() < records_len {
            return Err(FormatError::UnexpectedEof {
                offset: after_gi,
                needed: records_len - body.len(),
            });
        }
        let planets = (0..planet_count)
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
        let trailer = body[records_len..].to_vec();

        Ok(Self {
            header,
            header_payload: hdr_payload.to_vec(),
            game_info,
            planets,
            trailer,
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

        for planet in &self.planets {
            out.extend_from_slice(&planet.to_word().to_le_bytes());
        }
        out.extend_from_slice(&self.trailer);
        Ok(out)
    }

    /// The number of planets in the universe.
    #[must_use]
    pub fn planet_count(&self) -> usize {
        self.planets.len()
    }

    /// The number of players declared in the game-info block.
    ///
    /// Read from the low 5 bits of game-info offset
    /// [`GAME_INFO_PLAYER_COUNT_OFFSET`]. Verified against the sample game
    /// (3 players) and self-consistent with the standalone universe files'
    /// trailer.
    #[must_use]
    pub fn player_count(&self) -> u8 {
        self.game_info
            .get(GAME_INFO_PLAYER_COUNT_OFFSET)
            .map_or(0, |b| b & 0x1F)
    }

    /// Resolve every planet to its **absolute** position and name.
    ///
    /// Absolute x is [`X_BASE`] plus the running sum of the per-record
    /// `x_offset`s; `y` and the name index are taken verbatim, and the name is
    /// looked up in the master planet-name table
    /// ([`crate::names::planet_name`]).
    #[must_use]
    pub fn planets_resolved(&self) -> Vec<Planet> {
        let mut x = X_BASE;
        self.planets
            .iter()
            .enumerate()
            .map(|(i, p)| {
                x += u32::from(p.x_offset);
                Planet {
                    id: i as u16,
                    x,
                    y: p.y,
                    name_index: p.name_index,
                    name: planet_name(p.name_index),
                }
            })
            .collect()
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
            // 10 + 12 + 10 = 32, so the full 32-bit word round-trips.
            assert_eq!(
                PlanetPosition::from_word(word).to_word(),
                word,
                "{word:#010x}"
            );
        }
    }

    #[test]
    fn planet_position_field_split() {
        // x_offset=5, y=10, name=3  ->  5 | (10<<10) | (3<<22)
        let p = PlanetPosition {
            x_offset: 5,
            y: 10,
            name_index: 3,
        };
        let w = p.to_word();
        assert_eq!(w, 5 | (10 << 10) | (3 << 22));
        assert_eq!(PlanetPosition::from_word(w), p);
    }

    #[test]
    fn field_widths_are_masked() {
        // y is 12 bits and name is 10 bits; oversized inputs are truncated.
        let p = PlanetPosition {
            x_offset: 0x3FF,
            y: 0x0FFF,
            name_index: 0x03FF,
        };
        assert_eq!(PlanetPosition::from_word(p.to_word()), p);
        // The top y bit (0x1000) and top name bits are dropped.
        let clipped = PlanetPosition {
            x_offset: 0,
            y: 0x1000,
            name_index: 0x0400,
        };
        assert_eq!(clipped.to_word(), 0);
    }
}
