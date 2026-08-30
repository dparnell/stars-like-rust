//! # stars-formats
//!
//! Reader/writer layer for the original **Stars!** on-disk file formats.
//!
//! This crate is the project's first correctness anchor: every supported
//! format is reverse-engineered from the original binary (see `docs/`) and
//! implemented here as a pair of pure functions with a round-trip contract:
//!
//! ```text
//! write(read(bytes)) == bytes   // for real sample files
//! ```
//!
//! Scope: **data only**. This crate contains typed models and
//! (de)serialization. It contains no game logic, no rendering, no file I/O
//! against the real filesystem (callers pass in `&[u8]`), and no platform
//! APIs. Those live in `stars-core` and the frontend crates.
//!
//! ## Layers
//!
//! Every Stars! file is a flat sequence of *blocks* (see [`block`]). Three
//! layers are implemented and verified against real game files:
//!
//! 1. **Framing** ([`block`]) — the 16-bit type/size header word + payload.
//! 2. **Header** ([`header`]) — the plaintext file-header block that seeds
//!    the cipher.
//! 3. **Payload (de)cryption** ([`crypt`]) — the Stars! PRNG stream cipher.
//!
//! [`file::StarsFile`] combines them: [`StarsFile::decode`] returns blocks with
//! decrypted payloads and [`StarsFile::encode`] reproduces the original bytes.
//! The per-format *record* layouts (interpreting each decrypted block) are the
//! remaining work.
//!
//! ## Supported formats
//!
//! | Extension | Meaning             | Status                                   |
//! |-----------|---------------------|------------------------------------------|
//! | (all)     | Block framing       | implemented (round-trips)                |
//! | (all)     | Header + cipher     | implemented (byte-perfect on real files) |
//! | `.mN`     | Player state        | round-trips; typed [`PlayerRecord`]/[`PlanetRecord`]/[`FleetRecord`]/[`WaypointRecord`]/[`DesignRecord`]/[`BattlePlanRecord`]/[`ProductionQueueRecord`]/[`ScoreRecord`] |
//! | `.hst`    | Host state          | round-trips; typed [`PlayerRecord`]/[`PlanetRecord`]/[`FleetRecord`]/[`WaypointRecord`]/[`DesignRecord`]/[`BattlePlanRecord`]/[`ProductionQueueRecord`] |
//! | `.hN`     | Player history      | decode/encode round-trips; typed [`HistoryHeader`]/[`ScoreRecord`]; other records: WIP |
//! | `.xN`     | Player orders       | decode/encode round-trips; records: WIP  |
//! | `.rN`     | Race definition     | round-trips; typed [`RaceRecord`] (hab, growth, research, PRT, LRT) — `docs/formats/race-r.md` |
//! | `.xy`     | Universe definition | round-trips; header, game-info & planet array (coords + names) decoded |
//!
//! See `docs/formats/blocks.md` for the reverse-engineering notes and the
//! status of the encryption/payload work.

#![forbid(unsafe_code)]

pub mod battleplan;
pub mod block;
pub mod crypt;
pub mod design;
pub mod file;
pub mod fleet;
pub mod header;
pub mod history;
pub mod names;
pub mod planet;
pub mod player;
pub mod production;
pub mod race;
pub mod records;
pub mod score;
pub mod strings;
pub mod waypoint;
pub mod xy;

pub use battleplan::{battle_plan_records, BattlePlanRecord};
pub use block::{
    join_blocks, split_blocks, Block, BlockType, BLOCK_SIZE_MASK, BLOCK_TYPE_SHIFT,
    FILE_HEADER_BLOCK, MAX_BLOCK_SIZE, MAX_BLOCK_TYPE,
};
pub use crypt::{StarsRng, PRIMES};
pub use design::{design_records, DesignRecord, Slot};
pub use file::{StarsFile, FILE_FOOTER_BLOCK};
pub use fleet::{fleet_records, Cargo, FleetRecord, ShipDamage, ShipStack};
pub use header::{FileHeader, FileType};
pub use history::{history_header, HistoryHeader};
pub use names::{planet_name, planet_name_count};
pub use planet::{
    planet_records, Concentration, Environment, Installations, Minerals, PlanetRecord, Starbase,
};
pub use player::{player_records, PlayerRecord};
pub use production::{production_queue_records, ProductionQueueRecord, QueueItem};
pub use race::{Economy, HabRange, Lrt, Prt, RaceRecord};
pub use records::{planet_headers, PlanetHeader, MINIMAL_PLANET_LEN};
pub use score::{score_records, ScoreRecord, VictoryConditions};
pub use strings::{decode_field as decode_stars_string, decode_packed as decode_stars_packed};
pub use waypoint::{waypoint_records, WaypointRecord};
pub use xy::{Planet, PlanetPosition, Universe};

use thiserror::Error;

/// Errors produced while parsing or serializing a Stars! file.
///
/// Reverse-engineered parsers must fail with a typed error (never panic) on
/// corrupt, truncated, or unknown-version inputs so that frontends can report
/// the problem gracefully.
#[derive(Debug, Error)]
pub enum FormatError {
    /// The input ended before a required field could be read.
    #[error("unexpected end of input: needed {needed} more byte(s) at offset {offset}")]
    UnexpectedEof {
        /// Byte offset at which the read was attempted.
        offset: usize,
        /// Number of additional bytes that were required.
        needed: usize,
    },

    /// A magic number / signature did not match the expected value.
    #[error("bad magic: expected {expected:#06x}, found {found:#06x}")]
    BadMagic {
        /// The signature the parser expected.
        expected: u32,
        /// The signature actually found in the input.
        found: u32,
    },

    /// The file declared a format version this crate does not (yet) support.
    #[error("unsupported version: {0}")]
    UnsupportedVersion(u32),

    /// A catch-all for reverse-engineering gaps, invalid field combinations,
    /// or values that violate a documented invariant.
    #[error("malformed data: {0}")]
    Malformed(String),
}

/// Convenience result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, FormatError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_error_is_displayable() {
        let err = FormatError::UnexpectedEof {
            offset: 4,
            needed: 2,
        };
        assert!(err.to_string().contains("offset 4"));
    }
}
