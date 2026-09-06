//! Typed decoder for the **player scores** block (type id 45, `PLAYER_SCORES`)
//! that appears in `.mN` and `.hN` files.
//!
//! Each score block is one player's public scoreboard row for the turn: rank,
//! total score, resources, planet/starbase/ship counts and a tech-level total,
//! together with the victory-condition flags that player has met.
//!
//! The 24-byte layout was recovered from the stars-4x `decompiled` project
//! (`Structures/Structure45.xml`) and verified against
//! `fixtures/incoming/turn1/Game.m1`, whose single score block decodes to
//! player 0 at rank 1 with 1 planet, 1 starbase and 4 unarmed / 2 escort
//! ships on turn 1.
//!
//! Like the other record decoders this is an *interpreted, read-only view*;
//! byte-exact write-back still goes through the container in [`crate::file`].

use crate::block::BlockType;
use crate::file::StarsFile;

/// The victory conditions a player has attained, packed into the high bits of
/// the first word of a score block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VictoryConditions {
    /// Owns at least the required number of planets.
    pub owns_planets: bool,
    /// Attained the required tech level in the required number of fields.
    pub attains_tech: bool,
    /// Exceeds the configured absolute score.
    pub exceeds_score: bool,
    /// Exceeds second place by the configured margin.
    pub exceeds_second_place: bool,
    /// Has the required production capacity.
    pub production_capacity: bool,
    /// Owns the required number of capital ships.
    pub capital_ships: bool,
    /// Has held the highest score for the required number of years.
    pub highest_score: bool,
}

impl VictoryConditions {
    /// The flags as the block packs them, `1 << 0` for the planet condition
    /// through `1 << 6` — that is, shifted down from bit 6 of the first word.
    ///
    /// The order is the one the game's own condition table is in, so a bit
    /// here is `1 << condition` for the constants in [`crate::victory`], with
    /// `TECH_FIELDS` — which is a second value for the tech condition rather
    /// than a condition of its own — left out.
    #[must_use]
    pub fn bits(self) -> u16 {
        u16::from(self.owns_planets)
            | u16::from(self.attains_tech) << 1
            | u16::from(self.exceeds_score) << 2
            | u16::from(self.exceeds_second_place) << 3
            | u16::from(self.production_capacity) << 4
            | u16::from(self.capital_ships) << 5
            | u16::from(self.highest_score) << 6
    }
}

/// A decoded player-scores record (type-45 block).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreRecord {
    /// Player id (0-based), low **five** bits of the first word
    /// (`SCOREX.iPlayer:5`).
    pub player_id: u8,
    /// `fValid`, bit 5: the row carries figures. A player file holds a row for
    /// every player but fills in only the ones this player is allowed to see,
    /// so a row without this bit is a blank column on the score sheet rather
    /// than a player with nothing.
    pub known: bool,
    /// Victory conditions this player has met.
    pub victory: VictoryConditions,
    /// `fWinner`, bit 14: this player has met enough conditions to win.
    /// `UpdatePlayerScores` (`10b8:6258`) sets it, and the score sheet draws
    /// the name blue.
    pub winner: bool,
    /// `fHistory`, bit 15: the row is one year of a `.hN` file's timeline
    /// rather than the current standing. On such a row [`Self::rank`] is the
    /// **turn** the row describes — see [`Self::turn`].
    pub history: bool,
    /// Overall rank (1 = leader) — or, on a history row, the turn.
    pub rank: u16,
    /// Total score.
    pub score: u32,
    /// Total resources produced.
    pub resources: u32,
    /// Number of planets owned.
    pub planets: u16,
    /// Number of starbases.
    pub starbases: u16,
    /// Number of unarmed ships.
    pub unarmed_ships: u16,
    /// Number of escort (lightly armed) ships.
    pub escort_ships: u16,
    /// Number of capital ships.
    pub capital_ships: u16,
    /// Sum of tech levels across all six fields.
    pub tech_levels: u16,
}

/// The fixed length of a score block.
const RECORD_LEN: usize = 24;

fn read16(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}

fn read32(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

impl ScoreRecord {
    /// Decode a **decrypted** type-45 score block payload.
    ///
    /// Returns `None` if the payload is shorter than the fixed 24-byte record.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < RECORD_LEN {
            return None;
        }
        let word0 = read16(data, 0);
        let player_id = (word0 & 0x1F) as u8;
        // Victory-condition bits begin at bit 6 of the first word.
        let vc = word0 >> 6;
        let victory = VictoryConditions {
            owns_planets: vc & (1 << 0) != 0,
            attains_tech: vc & (1 << 1) != 0,
            exceeds_score: vc & (1 << 2) != 0,
            exceeds_second_place: vc & (1 << 3) != 0,
            production_capacity: vc & (1 << 4) != 0,
            capital_ships: vc & (1 << 5) != 0,
            highest_score: vc & (1 << 6) != 0,
        };
        Some(Self {
            player_id,
            known: word0 & (1 << 5) != 0,
            victory,
            winner: word0 & (1 << 14) != 0,
            history: word0 & (1 << 15) != 0,
            rank: read16(data, 2),
            score: read32(data, 4),
            resources: read32(data, 8),
            planets: read16(data, 12),
            starbases: read16(data, 14),
            unarmed_ships: read16(data, 16),
            escort_ships: read16(data, 18),
            capital_ships: read16(data, 20),
            tech_levels: read16(data, 22),
        })
    }

    /// The turn this row describes, on a history row.
    ///
    /// The second word of a score block is a union: a rank on the current
    /// standing a `.mN` carries, and the turn on the rows a `.hN` keeps. Only
    /// `fHistory` tells the two apart.
    #[must_use]
    pub fn turn(self) -> Option<u16> {
        self.history.then_some(self.rank)
    }
}

/// Decode every score block (type 45) in a decoded [`StarsFile`], in file
/// order.
#[must_use]
pub fn score_records(file: &StarsFile) -> Vec<ScoreRecord> {
    file.blocks
        .iter()
        .filter(|b| b.block_type() == BlockType::PlayerScores)
        .filter_map(|b| ScoreRecord::decode(&b.data))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        // player 0, rank 1, score 25, resources 39, 1 planet, 1 starbase,
        // 4 unarmed, 2 escort, 0 capital, tech total 18.
        let mut d = Vec::new();
        d.extend_from_slice(&0x0000u16.to_le_bytes()); // player 0, no VC bits
        d.extend_from_slice(&1u16.to_le_bytes()); // rank
        d.extend_from_slice(&25u32.to_le_bytes()); // score
        d.extend_from_slice(&39u32.to_le_bytes()); // resources
        d.extend_from_slice(&1u16.to_le_bytes()); // planets
        d.extend_from_slice(&1u16.to_le_bytes()); // starbases
        d.extend_from_slice(&4u16.to_le_bytes()); // unarmed
        d.extend_from_slice(&2u16.to_le_bytes()); // escort
        d.extend_from_slice(&0u16.to_le_bytes()); // capital
        d.extend_from_slice(&18u16.to_le_bytes()); // tech
        d
    }

    #[test]
    fn decodes_score() {
        let s = ScoreRecord::decode(&sample()).unwrap();
        assert_eq!(s.player_id, 0);
        assert_eq!(s.rank, 1);
        assert_eq!(s.score, 25);
        assert_eq!(s.resources, 39);
        assert_eq!(s.planets, 1);
        assert_eq!(s.starbases, 1);
        assert_eq!(s.unarmed_ships, 4);
        assert_eq!(s.escort_ships, 2);
        assert_eq!(s.capital_ships, 0);
        assert_eq!(s.tech_levels, 18);
        assert_eq!(s.victory, VictoryConditions::default());
    }

    #[test]
    fn decodes_victory_bits() {
        let mut d = sample();
        // set exceeds_score (bit 2 of the VC field, i.e. bit 8 of word0).
        d[1] |= 1 << 0; // word0 high byte bit0 == overall bit 8 == VC bit 2
        let s = ScoreRecord::decode(&d).unwrap();
        assert!(s.victory.exceeds_score);
    }

    /// The rest of the first word: five bits of player, then the flags the
    /// score sheet reads.
    #[test]
    fn decodes_the_flags_around_the_player_id() {
        let mut d = sample();
        // player 17, valid, winner, and a history row: 0x11 | 0x20 | 0x4000
        // | 0x8000.
        d[0..2].copy_from_slice(&0xC031u16.to_le_bytes());
        d[2..4].copy_from_slice(&37u16.to_le_bytes());
        let s = ScoreRecord::decode(&d).unwrap();
        assert_eq!(s.player_id, 17, "five bits, not four");
        assert!(s.known);
        assert!(s.winner);
        assert!(s.history);
        assert_eq!(s.turn(), Some(37), "the second word is a turn here");

        // Without `fHistory` the same word is a rank.
        d[0..2].copy_from_slice(&0x0020u16.to_le_bytes());
        let s = ScoreRecord::decode(&d).unwrap();
        assert!(!s.history);
        assert!(!s.winner);
        assert_eq!(s.turn(), None);
        assert_eq!(s.rank, 37);
    }

    /// The flags pack back into the order the game's condition table is in.
    #[test]
    fn victory_bits_pack_in_condition_order() {
        assert_eq!(VictoryConditions::default().bits(), 0);
        let all = VictoryConditions {
            owns_planets: true,
            attains_tech: true,
            exceeds_score: true,
            exceeds_second_place: true,
            production_capacity: true,
            capital_ships: true,
            highest_score: true,
        };
        assert_eq!(all.bits(), 0x7F);
        let one = VictoryConditions {
            highest_score: true,
            ..VictoryConditions::default()
        };
        assert_eq!(one.bits(), 1 << 6);
    }

    #[test]
    fn rejects_truncated() {
        assert!(ScoreRecord::decode(&[0u8; 23]).is_none());
    }
}
