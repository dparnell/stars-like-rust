//! Interpreted **record** layer over the decrypted blocks of a [`StarsFile`].
//!
//! [`crate::block`], [`crate::header`] and [`crate::crypt`] recover the raw
//! *container* (framing + plaintext header + decrypted payloads). This module
//! begins interpreting the payloads of individual blocks into typed records.
//!
//! It grows conservatively: only fields that are **verified against the real
//! sample files** are exposed here.
//!
//! This module holds the lightweight *inventory* view of the planet blocks (id,
//! owner and length, used for quick sanity checks). The full field-by-field
//! decode of a planet record lives in [`crate::planet`].

use crate::block::BlockType;
use crate::file::StarsFile;

/// Header word common to every [`BlockType::Planet`] (type 13) record.
///
/// A planet block starts with a little-endian 16-bit word whose **low 11 bits**
/// are the planet number and whose **high 5 bits** are the owning player
/// (`31` = unowned). This split is taken from TotalHost's `StarsPlanet.pl` and
/// verified against `Game.hst`, whose 128 planet blocks decode to the
/// contiguous ids `0..=127` with exactly three owned homeworlds (owners 0, 1,
/// 2).
///
/// The rest of the payload carries the planet's attributes; see
/// [`crate::planet::PlanetRecord`] for the full decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetHeader {
    /// Planet number (0-based), from the low 11 bits of the header word.
    pub id: u16,
    /// Owning player (0-based), or `None` if the planet is unowned (owner
    /// field `31`), from the high 5 bits of the header word.
    pub owner: Option<u8>,
    /// Total payload length of the planet block, in bytes.
    ///
    /// The minimal record is 11 bytes; inhabited planets (including homeworlds)
    /// are longer (33 bytes in the sample game).
    pub payload_len: usize,
}

impl PlanetHeader {
    /// Parse the header word of a planet-block payload.
    ///
    /// Returns `None` if the payload is too short to contain the 2-byte header
    /// word.
    #[must_use]
    pub fn parse(payload: &[u8]) -> Option<Self> {
        if payload.len() < 2 {
            return None;
        }
        let word = u16::from_le_bytes([payload[0], payload[1]]);
        let owner_raw = (word >> 11) & 0x1F;
        Some(Self {
            id: word & 0x07FF,
            owner: if owner_raw == 31 {
                None
            } else {
                Some(owner_raw as u8)
            },
            payload_len: payload.len(),
        })
    }

    /// Whether this planet carries the extended (inhabited/homeworld) record
    /// rather than the 11-byte minimal one.
    #[must_use]
    pub fn is_extended(&self) -> bool {
        self.payload_len > MINIMAL_PLANET_LEN
    }
}

/// Payload length of a minimal (undiscovered) planet record.
pub const MINIMAL_PLANET_LEN: usize = 11;

/// Collect the [`PlanetHeader`] of every planet block (type 13) in the file,
/// in file order.
///
/// For a universe file's host state (`.hst`) this yields one entry per planet
/// in the galaxy; the ids form a contiguous `0..planet_count` sequence.
#[must_use]
pub fn planet_headers(file: &StarsFile) -> Vec<PlanetHeader> {
    file.blocks
        .iter()
        .filter(|b| b.block_type() == BlockType::Planet)
        .filter_map(|b| PlanetHeader::parse(&b.data))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_planet_header() {
        // A minimal (unowned) planet header word is `(31 << 11) | id`, i.e.
        // `0xF800 | id`, as seen in every 11-byte planet block of `Game.hst`.
        let word: u16 = 0xF800 | 5;
        let mut payload = word.to_le_bytes().to_vec();
        payload.extend_from_slice(&[0u8; MINIMAL_PLANET_LEN - 2]);
        let h = PlanetHeader::parse(&payload).unwrap();
        assert_eq!(h.id, 5);
        assert_eq!(h.owner, None); // owner field 31 = unowned
        assert_eq!(h.payload_len, MINIMAL_PLANET_LEN);
        assert!(!h.is_extended());
    }

    #[test]
    fn parses_extended_planet_header() {
        // Homeworld id 32 owned by player 2: id in the low 11 bits, owner 2 in
        // the high 5 bits => (2 << 11) | 32 = 0x1020. Extended 33-byte payload.
        let word: u16 = (2 << 11) | 32;
        let mut payload = word.to_le_bytes().to_vec();
        payload.extend_from_slice(&[0u8; 31]);
        let h = PlanetHeader::parse(&payload).unwrap();
        assert_eq!(h.id, 32);
        assert_eq!(h.owner, Some(2));
        assert!(h.is_extended());
    }

    #[test]
    fn rejects_too_short_payload() {
        assert!(PlanetHeader::parse(&[0x00]).is_none());
        assert!(PlanetHeader::parse(&[]).is_none());
    }
}
