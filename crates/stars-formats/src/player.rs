//! Typed decoder for the **player** block (type id 6) as it appears in a
//! `.mN` player file or a `.hst` host file.
//!
//! The type-6 block is dual-purpose: in a `.rN` race file it carries a bare
//! race definition (decoded by [`crate::race::RaceRecord`]), while in a
//! `.mN`/`.hst` file it is a *full player* record with an 8-byte header in
//! front of the very same race struct. This module decodes that header and
//! reuses [`RaceRecord`] for the embedded race fields, so the two views stay in
//! sync.
//!
//! The layout was recovered from the stars-4x `starsapi` project
//! (`PlayerBlock.java`) and verified against `fixtures/incoming/turn0/Game.hst`,
//! whose three player blocks decode to player 0 = *Humanoid*/*Humanoids*
//! (PRT JOAT, 6 ship designs, 1 starbase design) and the two AI players
//! *Tritizoid* and *Golem*.
//!
//! ## Header layout (8 bytes)
//!
//! | Offset | Bits          | Field                                            |
//! |--------|---------------|--------------------------------------------------|
//! | 0      | all           | player number (0-based)                          |
//! | 1      | all           | ship design count                               |
//! | 2–3    | 10 bits       | planet count (`d[2] + ((d[3] & 3) << 8)`)        |
//! | 4–5    | 10 bits       | fleet count (`d[4] + ((d[5] & 3) << 8)`)         |
//! | 5      | bits 4–7      | starbase design count                            |
//! | 6      | bit 2         | `fullData` flag (race struct present)            |
//! | 6      | bits 3–7      | logo index                                       |
//! | 7      | all           | flags byte (`1` for humans, `39` for AI)         |
//!
//! When `fullData` is set, the 0x68-byte race struct follows at offset 8, then
//! a length-prefixed player-relations table at offset 0x70, then the packed
//! singular/plural names. Because the race struct sits at offset 8, every race
//! field lands at the *same absolute offset* as in a `.rN` file, so
//! [`RaceRecord::from_payload`] decodes it directly.
//!
//! Like the other record decoders this is an *interpreted, read-only view*;
//! byte-exact write-back still goes through the container in [`crate::file`].

use crate::file::StarsFile;
use crate::race::RaceRecord;
use crate::{FormatError, Result};

/// The minimum length of a player block (the fixed 8-byte header).
const HEADER_LEN: usize = 8;

/// A decoded full-player record from a `.mN`/`.hst` type-6 block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRecord {
    /// Player number (0-based).
    pub player_number: u8,
    /// Number of ship designs this player has defined.
    pub ship_design_count: u8,
    /// Number of starbase designs this player has defined.
    pub starbase_design_count: u8,
    /// Number of planets owned/known for this player (10-bit field).
    pub planets: u16,
    /// Number of fleets for this player (10-bit field).
    pub fleets: u16,
    /// Race logo/emblem index (0..=31).
    pub logo: u8,
    /// Whether the block carries the full race struct (`fullData` flag). This
    /// is set for real player files; when clear only the header and names are
    /// present.
    pub full_data: bool,
    /// The trailing flags byte (offset 7): `1` for a human player, `39` for the
    /// AI players in our fixtures. Exposed raw as its exact meaning is not yet
    /// pinned down.
    pub flags_byte: u8,
    /// Player-relations table (one byte per player: `0` neutral, `1` friend,
    /// `2` enemy), present only when [`full_data`](Self::full_data) is set.
    pub player_relations: Vec<u8>,
    /// The embedded race definition (habitability, growth, PRT, LRT, economy,
    /// names), decoded via [`RaceRecord`]. Present only when
    /// [`full_data`](Self::full_data) is set.
    pub race: Option<RaceRecord>,
    /// Singular race/player name (e.g. `"Humanoid"`).
    pub singular_name: String,
    /// Plural race/player name (e.g. `"Humanoids"`); may be empty.
    pub plural_name: String,
}

impl PlayerRecord {
    /// Decode a **decrypted** type-6 player block payload.
    ///
    /// # Errors
    /// Returns [`FormatError::Malformed`] if the payload is shorter than the
    /// 8-byte header or the declared full-data / relations region runs past the
    /// end of the block.
    pub fn from_payload(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_LEN {
            return Err(FormatError::Malformed(format!(
                "player block ({} bytes) shorter than the {HEADER_LEN}-byte header",
                data.len()
            )));
        }

        let player_number = data[0];
        let ship_design_count = data[1];
        let planets = u16::from(data[2]) + (u16::from(data[3] & 0x03) << 8);
        let fleets = u16::from(data[4]) + (u16::from(data[5] & 0x03) << 8);
        let starbase_design_count = (data[5] & 0xF0) >> 4;
        let logo = data[6] >> 3;
        let full_data = data[6] & 0x04 != 0;
        let flags_byte = data[7];

        // `fullData` region: 0x68-byte race struct at offset 8, then a
        // length-prefixed player-relations table at offset 0x70.
        let mut player_relations = Vec::new();
        let mut race = None;
        if full_data {
            let relations_index = 0x70;
            let &relations_len = data.get(relations_index).ok_or_else(|| {
                FormatError::Malformed(
                    "player block: fullData flag set but block ends before player relations".into(),
                )
            })?;
            let start = relations_index + 1;
            let end = start + relations_len as usize;
            if end > data.len() {
                return Err(FormatError::Malformed(
                    "player block: player-relations table runs past end of block".into(),
                ));
            }
            player_relations = data[start..end].to_vec();
            // The race struct occupies the same absolute offsets as a `.rN`
            // file, so `RaceRecord` decodes the whole payload directly.
            race = Some(RaceRecord::from_payload(data)?);
        }

        // Names: reuse the race decoder's result when available (it already
        // handles the fullData player-relations framing); otherwise fall back
        // to the short-record layout (names start right after the header).
        let (singular_name, plural_name) = match &race {
            Some(r) => (r.singular_name.clone(), r.plural_name.clone()),
            None => decode_short_names(data),
        };

        Ok(Self {
            player_number,
            ship_design_count,
            starbase_design_count,
            planets,
            fleets,
            logo,
            full_data,
            flags_byte,
            player_relations,
            race,
            singular_name,
            plural_name,
        })
    }
}

/// Decode the packed singular/plural names of a **short** (non-fullData) player
/// block, where the names start immediately after the 8-byte header.
fn decode_short_names(data: &[u8]) -> (String, String) {
    use crate::strings;
    let index = HEADER_LEN;
    let Some(&singular_len) = data.get(index) else {
        return (String::new(), String::new());
    };
    let singular_end = index + singular_len as usize;
    if singular_end >= data.len() {
        return (String::new(), String::new());
    }
    let singular = strings::decode_field(&data[index..=singular_end]);
    let plural = strings::decode_field(&data[singular_end + 1..]);
    (singular, plural)
}

/// Decode every player block (type 6) in a decoded [`StarsFile`], in file
/// order.
///
/// For a `.hst` host file this yields one [`PlayerRecord`] per player.
///
/// # Errors
/// Returns [`FormatError::Malformed`] if any player block is malformed.
pub fn player_records(file: &StarsFile) -> Result<Vec<PlayerRecord>> {
    file.blocks
        .iter()
        .filter(|b| b.type_id == 6)
        .map(|b| PlayerRecord::from_payload(&b.data))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_truncated_header() {
        assert!(PlayerRecord::from_payload(&[0u8; 4]).is_err());
    }

    #[test]
    fn decodes_short_player_header() {
        // A non-fullData header: player 2, 3 ship designs, 1 starbase design,
        // 5 planets, 4 fleets, logo 3.
        let mut d = vec![0u8; HEADER_LEN];
        d[0] = 2;
        d[1] = 3;
        d[2] = 5;
        d[4] = 4;
        d[5] = 1 << 4; // starbase design count = 1, fleets high bits 0
        d[6] = 3 << 3; // logo 3, fullData clear
        d[7] = 1;
        let p = PlayerRecord::from_payload(&d).unwrap();
        assert_eq!(p.player_number, 2);
        assert_eq!(p.ship_design_count, 3);
        assert_eq!(p.starbase_design_count, 1);
        assert_eq!(p.planets, 5);
        assert_eq!(p.fleets, 4);
        assert_eq!(p.logo, 3);
        assert!(!p.full_data);
        assert!(p.race.is_none());
    }

    #[test]
    fn fulldata_flag_requires_relations_region() {
        // fullData set but block ends right after the header => error.
        let mut d = vec![0u8; HEADER_LEN];
        d[6] = 0x04; // fullData flag
        assert!(PlayerRecord::from_payload(&d).is_err());
    }
}
