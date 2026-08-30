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
//! ## Supported formats (planned)
//!
//! | Extension | Meaning                | Status      |
//! |-----------|------------------------|-------------|
//! | `.xy`     | Universe definition    | not started |
//! | `.mN`     | Player state           | not started |
//! | `.hN`     | Player history         | not started |
//! | `.xN`     | Player orders          | not started |
//! | `.rN`     | Race definition        | not started |
//! | `.hst`    | Host state             | not started |
//!
//! Implementation happens in Step 2 of the delivery plan; this file currently
//! establishes only the shared error type and the module surface.

#![forbid(unsafe_code)]

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
