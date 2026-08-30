//! Typed decoder for the **history-file header** block (type id 32,
//! `rtHistHdr`) that begins every `.hN` player-history file.
//!
//! When Stars! opens a history file (`dtHist`) the very first record after the
//! plaintext file header is a `rtHistHdr`; the loader treats its absence as a
//! corrupt file (`file.c`: *"after opening dtHist, expects rt == rtHistHdr"*).
//! The record is a fixed 4-byte struct:
//!
//! ```c
//! typedef struct _rthisthdr {
//!     int16_t cPlanet;      /* +0x00 (2) number of planet records that follow */
//!     int16_t cPlanetExtra; /* +0x02 (2) low 12 bits of the owning player's rgplr word */
//! } RTHISTHDR;              /* size=0x4 */
//! ```
//!
//! The layout is recovered from the NB09 debug symbols of `Stars! 2.7j`
//! (sirgwain's `stars-asm` / `decompiled` project — `structs.h`, `enums.h`,
//! `file.c`, `log.c`; see `docs/formats/nb09-structs.md`) and verified against
//! the real `.hN` fixtures: `cPlanet` equals the number of planet blocks the
//! history file carries — i.e. the planets that player has knowledge of, not
//! the whole universe (e.g. `Game.h1` = 2 planet blocks, `tutorial.h1` = 5).
//!
//! Like the other record decoders this is a read-only *interpreted view*;
//! byte-exact write-back still goes through the container in [`crate::file`].

use crate::block::BlockType;
use crate::file::StarsFile;

/// A decoded history-file header (type-32 `rtHistHdr` block).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryHeader {
    /// Number of planet records that follow in this history file (`cPlanet`),
    /// i.e. the planets the owning player has knowledge of. This is generally
    /// fewer than the universe's total planet count.
    pub planet_count: u16,
    /// `cPlanetExtra` — the low 12 bits of the owning player's `rgplr` word
    /// (game/AI bookkeeping), stored verbatim here.
    pub planet_extra: u16,
}

/// The fixed length of a `rtHistHdr` record.
const RECORD_LEN: usize = 4;

impl HistoryHeader {
    /// Decode a **decrypted** type-32 `rtHistHdr` block payload.
    ///
    /// Returns `None` if the payload is shorter than the fixed 4-byte record.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < RECORD_LEN {
            return None;
        }
        Some(Self {
            planet_count: u16::from_le_bytes([data[0], data[1]]),
            planet_extra: u16::from_le_bytes([data[2], data[3]]),
        })
    }
}

/// Return the history header of a decoded [`StarsFile`], if present.
///
/// A well-formed `.hN` file carries exactly one `rtHistHdr` block; this returns
/// the first one in file order.
#[must_use]
pub fn history_header(file: &StarsFile) -> Option<HistoryHeader> {
    file.blocks
        .iter()
        .find(|b| b.block_type() == BlockType::HistoryHeader)
        .and_then(|b| HistoryHeader::decode(&b.data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_header() {
        let d = [128u16.to_le_bytes(), 5u16.to_le_bytes()].concat();
        let h = HistoryHeader::decode(&d).unwrap();
        assert_eq!(h.planet_count, 128);
        assert_eq!(h.planet_extra, 5);
    }

    #[test]
    fn rejects_truncated() {
        assert!(HistoryHeader::decode(&[0u8; 3]).is_none());
    }
}
