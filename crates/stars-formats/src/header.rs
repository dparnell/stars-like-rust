//! The plaintext **file-header block** (`type 8`) that begins every Stars! file.
//!
//! The header carries the game id, version, turn, owning player and the
//! encryption salt. It is the only block (besides the footer) stored in
//! plaintext, and it is what seeds the [`StarsRng`] used to decrypt every other
//! block (see [`FileHeader::init_rng`]).
//!
//! Layout of the 16-byte header-block **payload** (all integers little-endian):
//!
//! | Offset | Size | Field     | Notes                                            |
//! |-------:|-----:|-----------|--------------------------------------------------|
//! | 0      | 4    | magic     | ASCII `"J3J3"`                                    |
//! | 4      | 4    | `game_id` | random per-game id, identical across a game's files |
//! | 8      | 2    | version   | packed `major(4) | minor(7) | increment(5)`       |
//! | 10     | 2    | turn      | year = `2400 + turn`                              |
//! | 12     | 2    | player    | low 5 bits = player index; high 11 bits = salt   |
//! | 14     | 2    | dts       | low byte = file type; bits 8..12 = flags          |
//!
//! Verified against real `.xy`, `.mN` and `.hst` files (identical `game_id`,
//! per-player numbering, and a distinguishing file-type tag).

use crate::crypt::{StarsRng, PRIMES};
use crate::{FormatError, Result};

/// The kind of Stars! file, taken from the header's file-type tag (`dts` low
/// byte). Matches the original `dt*` enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// `.xy` — the shared universe definition.
    Universe,
    /// `.xN` — a player's submitted orders ("log").
    Orders,
    /// `.hst` — the host state file.
    Host,
    /// `.mN` — a player's turn/state file.
    Turn,
    /// `.hN` — a player's history file.
    History,
    /// `.rN` — a race definition file.
    Race,
    /// An unrecognised file-type tag.
    Unknown(u8),
}

impl FileType {
    /// Decode the file-type tag (the low byte of the header `dts` word).
    #[must_use]
    pub fn from_tag(tag: u8) -> Self {
        match tag {
            0 => FileType::Universe,
            1 => FileType::Orders,
            2 => FileType::Host,
            3 => FileType::Turn,
            4 => FileType::History,
            5 => FileType::Race,
            other => FileType::Unknown(other),
        }
    }
}

/// A parsed Stars! file-header block.
#[derive(Debug, Clone)]
pub struct FileHeader {
    /// The `"J3J3"` magic.
    pub magic: [u8; 4],
    /// Per-game random id; identical across every file belonging to one game.
    pub game_id: u32,
    /// Version major component (e.g. `2`).
    pub version_major: u16,
    /// Version minor component.
    pub version_minor: u16,
    /// Version increment/build component.
    pub version_increment: u16,
    /// Turn counter; the in-game year is `2400 + turn`.
    pub turn: u16,
    /// Owning player index (0-based). `31` denotes a shared/host file.
    pub player: u8,
    /// 11-bit encryption salt (the high bits of the header's player word).
    pub salt: u16,
    /// The kind of file this header introduces.
    pub file_type: FileType,
    /// `.x` only: the turn has been submitted.
    pub flag_done: bool,
    /// The host is currently using this file.
    pub flag_in_use: bool,
    /// `.m` only: multiple turns are included.
    pub flag_multi: bool,
    /// The game is over.
    pub flag_game_over: bool,
    /// Produced by the shareware edition.
    pub flag_shareware: bool,
}

impl FileHeader {
    /// The `"J3J3"` signature expected at the start of the header payload.
    pub const MAGIC: [u8; 4] = *b"J3J3";

    /// Parse a header from the **payload** of the file-header block (type 8).
    ///
    /// # Errors
    ///
    /// - [`FormatError::UnexpectedEof`] if fewer than 16 bytes are supplied.
    /// - [`FormatError::BadMagic`] if the `"J3J3"` signature is missing.
    pub fn parse(payload: &[u8]) -> Result<Self> {
        if payload.len() < 16 {
            return Err(FormatError::UnexpectedEof {
                offset: payload.len(),
                needed: 16 - payload.len(),
            });
        }
        let magic = [payload[0], payload[1], payload[2], payload[3]];
        if magic != Self::MAGIC {
            return Err(FormatError::BadMagic {
                expected: u32::from_le_bytes(Self::MAGIC),
                found: u32::from_le_bytes(magic),
            });
        }

        let game_id = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
        let version = u16::from_le_bytes([payload[8], payload[9]]);
        let turn = u16::from_le_bytes([payload[10], payload[11]]);
        let player_word = u16::from_le_bytes([payload[12], payload[13]]);
        let dts = u16::from_le_bytes([payload[14], payload[15]]);

        // Version: major(4) | minor(7) | increment(5), MSB-first.
        let version_major = version >> 12;
        let version_minor = (version >> 5) & 0x7F;
        let version_increment = version & 0x1F;

        // Player word: low 5 bits = player index, high 11 bits = salt.
        let player = (player_word & 0x1F) as u8;
        let salt = player_word >> 5;

        // dts: low byte = file type; bits 8..12 = flags.
        let file_type = FileType::from_tag((dts & 0xFF) as u8);

        Ok(Self {
            magic,
            game_id,
            version_major,
            version_minor,
            version_increment,
            turn,
            player,
            salt,
            file_type,
            flag_done: (dts >> 8) & 1 == 1,
            flag_in_use: (dts >> 9) & 1 == 1,
            flag_multi: (dts >> 10) & 1 == 1,
            flag_game_over: (dts >> 11) & 1 == 1,
            flag_shareware: (dts >> 12) & 1 == 1,
        })
    }

    /// Construct the [`StarsRng`] that decrypts/encrypts this file's blocks.
    ///
    /// Two entries of the [`PRIMES`] table (indexed by the salt) seed the
    /// generator, which is then warmed up a number of rounds derived from the
    /// game id, turn and player.
    #[must_use]
    pub fn init_rng(&self) -> StarsRng {
        let salt = u32::from(self.salt);
        let mut index1 = (salt & 0x1F) as usize;
        let mut index2 = ((salt >> 5) & 0x1F) as usize;
        if salt >> 10 == 1 {
            index1 += 32;
        } else {
            index2 += 32;
        }

        let part_game = (self.game_id & 0x3) + 1;
        let part_turn = (u32::from(self.turn) & 0x3) + 1;
        let part_player = (u32::from(self.player) & 0x3) + 1;
        let rounds = part_game * part_turn * part_player + u32::from(self.flag_shareware);

        StarsRng::new(PRIMES[index1], PRIMES[index2], rounds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_payload() -> Vec<u8> {
        // magic "J3J3", game_id=0x2a031dd8, ver=0x2840, turn=0,
        // player word 0x4900 (player 0, salt 0x248), dts=3 (Turn file).
        let mut p = Vec::new();
        p.extend_from_slice(b"J3J3");
        p.extend_from_slice(&0x2a03_1dd8u32.to_le_bytes());
        p.extend_from_slice(&0x2840u16.to_le_bytes());
        p.extend_from_slice(&0u16.to_le_bytes());
        p.extend_from_slice(&0x4900u16.to_le_bytes());
        p.extend_from_slice(&3u16.to_le_bytes());
        p
    }

    #[test]
    fn parses_known_header_fields() {
        let h = FileHeader::parse(&header_payload()).unwrap();
        assert_eq!(h.magic, *b"J3J3");
        assert_eq!(h.game_id, 0x2a03_1dd8);
        assert_eq!(h.version_major, 2);
        assert_eq!(h.turn, 0);
        assert_eq!(h.player, 0);
        assert_eq!(h.salt, 0x248);
        assert_eq!(h.file_type, FileType::Turn);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut p = header_payload();
        p[0] = b'X';
        assert!(matches!(
            FileHeader::parse(&p),
            Err(FormatError::BadMagic { .. })
        ));
    }

    #[test]
    fn rejects_short_payload() {
        assert!(matches!(
            FileHeader::parse(b"J3J3"),
            Err(FormatError::UnexpectedEof { .. })
        ));
    }
}
