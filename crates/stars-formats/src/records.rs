//! Interpreted **record** layer over the decrypted blocks of a [`StarsFile`].
//!
//! [`crate::block`], [`crate::header`] and [`crate::crypt`] recover the raw
//! *container* (framing + plaintext header + decrypted payloads). This module
//! begins interpreting the payloads of individual blocks into typed records.
//!
//! It grows conservatively: only fields that are **verified against the real
//! sample files** are exposed here. Fields whose meaning is still a hypothesis
//! (e.g. the exact mineral-concentration vs. environment byte split of a planet
//! record, or the `.xy` coordinate packing) are documented in
//! `docs/formats/hst.md` / `docs/formats/xy.md` and deliberately *not* surfaced
//! as typed fields until confirmed, so this crate stays a correctness anchor.

use crate::block::BlockType;
use crate::file::StarsFile;

/// Header word common to every [`BlockType::Planet`] (type 13) record.
///
/// A planet block starts with a little-endian 16-bit word whose low 10 bits are
/// the planet number and whose high 6 bits are a field-presence/flags mask.
/// This is verified against `Game.hst`, whose 128 planet blocks decode to the
/// contiguous ids `0..=127`.
///
/// The remaining payload bytes carry the planet's attributes (mineral
/// concentrations, environment, and — for inhabited planets — population,
/// installations and a starbase). Their exact layout is still being recovered
/// (see `docs/formats/hst.md`), so only the id, flags and payload length are
/// exposed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetHeader {
    /// Planet number (0-based), from the low 10 bits of the header word.
    pub id: u16,
    /// Field-presence / flags mask, from the high 6 bits of the header word.
    ///
    /// Minimal (undiscovered) planet records use `62`; inhabited/homeworld
    /// records use smaller values and carry a longer payload.
    pub flags: u8,
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
        Some(Self {
            id: word & 0x03FF,
            flags: (word >> 10) as u8,
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
        // A minimal planet header word is `0xF800 | id` (flags 62, id in the
        // low 10 bits), as seen in every 11-byte planet block of `Game.hst`.
        let word: u16 = 0xF800 | 5;
        let mut payload = word.to_le_bytes().to_vec();
        payload.extend_from_slice(&[0u8; MINIMAL_PLANET_LEN - 2]);
        let h = PlanetHeader::parse(&payload).unwrap();
        assert_eq!(h.id, 5);
        assert_eq!(h.flags, 62); // 0xF800 >> 10
        assert_eq!(h.payload_len, MINIMAL_PLANET_LEN);
        assert!(!h.is_extended());
    }

    #[test]
    fn parses_extended_planet_header() {
        // Homeworld id 32 (word 0x1020), extended 33-byte payload.
        let word: u16 = 0x1020;
        let mut payload = word.to_le_bytes().to_vec();
        payload.extend_from_slice(&[0u8; 31]);
        let h = PlanetHeader::parse(&payload).unwrap();
        assert_eq!(h.id, 32);
        assert!(h.is_extended());
    }

    #[test]
    fn rejects_too_short_payload() {
        assert!(PlanetHeader::parse(&[0x00]).is_none());
        assert!(PlanetHeader::parse(&[]).is_none());
    }
}
