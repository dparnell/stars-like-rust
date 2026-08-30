//! Typed decoder for the **race record** — the encrypted type-6 block that
//! carries a race definition in a `.rN` file (and, in fuller form, a player in
//! `.mN`/`.hst`).
//!
//! Only the fields **verified against real files** are exposed here (see
//! `docs/formats/race-r.md`): the player marker, the habitability ranges,
//! growth rate, per-field research cost, the primary racial trait (PRT) and the
//! lesser-racial-trait (LRT) bitfield. The remaining bytes of the record
//! (early flags, economy split, packed name strings) are not yet decoded, so
//! this is a *read-only view* of the proven subset rather than a full model —
//! it is deliberately **not** used for re-encoding, which still goes through the
//! byte-exact container in [`crate::file`].
//!
//! The PRT byte (offset 76) and the LRT bitfield (offset 78) were confirmed by
//! the seven built-in default races (all `LRT = 0`) plus the seven shipped AI
//! races in `fixtures/games/exodus/Races/`, whose primary traits match their
//! file names (`OFFENDER` → WM, `DEFENDER` → SD, `SNEAK` → SS, `JUMPERS` → IT,
//! `FLEXIBLE` → JOAT, …) and whose LRT bits all fall inside the documented
//! 14-bit range with no stray bits.

use crate::file::StarsFile;
use crate::strings;
use crate::{FormatError, Result};

/// Offset of the player-id byte in the race record (`0xFF` for a race-only
/// block, a 0-based index for a full player block).
pub const PLAYER_ID_OFFSET: usize = 0;
/// Offset of the three habitability *center* clicks (gravity, temperature,
/// radiation). `0xFF` in any axis marks the race immune on that axis.
pub const HAB_CENTER_OFFSET: usize = 16;
/// Offset of the three habitability *low* bounds.
pub const HAB_LOW_OFFSET: usize = 19;
/// Offset of the three habitability *high* bounds.
pub const HAB_HIGH_OFFSET: usize = 22;
/// Offset of the maximum population growth rate (percent).
pub const GROWTH_RATE_OFFSET: usize = 25;
/// Offset of the six per-field research-cost bytes (Energy, Weapons,
/// Propulsion, Construction, Electronics, Biotechnology).
pub const RESEARCH_COST_OFFSET: usize = 70;
/// Offset of the primary-racial-trait byte.
pub const PRT_OFFSET: usize = 76;
/// Offset of the lesser-racial-trait bitfield (little-endian `u16`).
pub const LRT_OFFSET: usize = 78;
/// Offset of the flags byte that carries the `fullData` marker (bit 2). It is
/// set for `.rN` race files and full player blocks, where the names live at the
/// end of the record; when clear, the names start at [`SHORT_NAMES_OFFSET`].
pub const FLAGS_OFFSET: usize = 6;
/// `data[FLAGS_OFFSET] & FULL_DATA_FLAG` selects the record's name framing.
pub const FULL_DATA_FLAG: u8 = 0x04;
/// Where the singular/plural name fields start in a **short** (non-fullData)
/// player block.
pub const SHORT_NAMES_OFFSET: usize = 8;
/// In a **fullData** record the player-relations table starts here; the names
/// follow it. `data[PLAYER_RELATIONS_OFFSET]` is the relations length.
pub const PLAYER_RELATIONS_OFFSET: usize = 112;

/// The smallest race record we can fully interpret (must cover the LRT `u16`).
const MIN_RECORD_LEN: usize = LRT_OFFSET + 2;

/// Primary Racial Trait — the single defining trait every race picks exactly
/// one of (race-record offset 76).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prt {
    /// Hyper Expansion.
    HE,
    /// Super Stealth.
    SS,
    /// War Monger.
    WM,
    /// Claim Adjuster.
    CA,
    /// Inner Strength.
    IS,
    /// Space Demolition.
    SD,
    /// Packet Physics.
    PP,
    /// Interstellar Traveler.
    IT,
    /// Alternate Reality.
    AR,
    /// Jack of All Trades.
    JOAT,
    /// An unrecognised trait id.
    Unknown(u8),
}

impl Prt {
    /// Decode the PRT id stored at offset 76.
    #[must_use]
    pub fn from_id(id: u8) -> Self {
        match id {
            0 => Self::HE,
            1 => Self::SS,
            2 => Self::WM,
            3 => Self::CA,
            4 => Self::IS,
            5 => Self::SD,
            6 => Self::PP,
            7 => Self::IT,
            8 => Self::AR,
            9 => Self::JOAT,
            other => Self::Unknown(other),
        }
    }

    /// The two/four-letter abbreviation used in the game and manual.
    #[must_use]
    pub fn abbrev(self) -> &'static str {
        match self {
            Self::HE => "HE",
            Self::SS => "SS",
            Self::WM => "WM",
            Self::CA => "CA",
            Self::IS => "IS",
            Self::SD => "SD",
            Self::PP => "PP",
            Self::IT => "IT",
            Self::AR => "AR",
            Self::JOAT => "JOAT",
            Self::Unknown(_) => "?",
        }
    }
}

/// A Lesser Racial Trait, identified by its bit position in the LRT bitfield
/// (race-record offset 78). Bit order is the community-documented Stars! order,
/// confirmed here against the shipped AI races (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lrt {
    /// bit 0 — Improved Fuel Efficiency.
    IFE,
    /// bit 1 — Total Terraforming.
    TT,
    /// bit 2 — Advanced Remote Mining.
    ARM,
    /// bit 3 — Improved Starbases.
    ISB,
    /// bit 4 — Generalized Research.
    GR,
    /// bit 5 — Ultimate Recycling.
    UR,
    /// bit 6 — No Ram Scoop Engines.
    NRSE,
    /// bit 7 — Only Basic Remote Mining.
    OBRM,
    /// bit 8 — No Advanced Scanners.
    NAS,
    /// bit 9 — Low Starting Population.
    LSP,
    /// bit 10 — Bleeding Edge Technology.
    BET,
    /// bit 11 — Regenerating Shields.
    RS,
    /// bit 12 — Cheap Engines.
    CE,
    /// bit 13 — Mineral Alchemy.
    MA,
}

impl Lrt {
    /// All fourteen traits, in bit-position order.
    pub const ALL: [Lrt; 14] = [
        Lrt::IFE,
        Lrt::TT,
        Lrt::ARM,
        Lrt::ISB,
        Lrt::GR,
        Lrt::UR,
        Lrt::NRSE,
        Lrt::OBRM,
        Lrt::NAS,
        Lrt::LSP,
        Lrt::BET,
        Lrt::RS,
        Lrt::CE,
        Lrt::MA,
    ];

    /// The single-bit mask this trait occupies in the LRT bitfield.
    #[must_use]
    pub fn bit(self) -> u16 {
        let idx = Self::ALL.iter().position(|&t| t == self).unwrap();
        1 << idx
    }

    /// The trait abbreviation used in the game and manual.
    #[must_use]
    pub fn abbrev(self) -> &'static str {
        match self {
            Self::IFE => "IFE",
            Self::TT => "TT",
            Self::ARM => "ARM",
            Self::ISB => "ISB",
            Self::GR => "GR",
            Self::UR => "UR",
            Self::NRSE => "NRSE",
            Self::OBRM => "OBRM",
            Self::NAS => "NAS",
            Self::LSP => "LSP",
            Self::BET => "BET",
            Self::RS => "RS",
            Self::CE => "CE",
            Self::MA => "MA",
        }
    }
}

/// The habitability range for one environment axis (gravity, temperature or
/// radiation), in the game's 0..=100 "click" units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HabRange {
    /// The center click, or `None` when the race is **immune** on this axis
    /// (stored as `0xFF`).
    pub center: Option<u8>,
    /// The lower habitable bound (`None` when immune).
    pub low: Option<u8>,
    /// The upper habitable bound (`None` when immune).
    pub high: Option<u8>,
}

impl HabRange {
    /// Whether the race is immune on this axis (any bound is the `0xFF` marker).
    #[must_use]
    pub fn is_immune(self) -> bool {
        self.center.is_none()
    }
}

fn hab(byte: u8) -> Option<u8> {
    if byte == 0xFF {
        None
    } else {
        Some(byte)
    }
}

/// The verified subset of a race record, decoded from a decrypted type-6 block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaceRecord {
    /// Player marker: `0xFF` in a race-only (`.rN`) block, else a 0-based id.
    pub player_id: u8,
    /// Gravity habitability range.
    pub gravity: HabRange,
    /// Temperature habitability range.
    pub temperature: HabRange,
    /// Radiation habitability range.
    pub radiation: HabRange,
    /// Maximum population growth rate (percent).
    pub growth_rate: u8,
    /// Per-field research cost in field order (Energy, Weapons, Propulsion,
    /// Construction, Electronics, Biotechnology); `0`/`1`/`2` = costs-less /
    /// normal / costs-more.
    pub research_cost: [u8; 6],
    /// Primary racial trait.
    pub prt: Prt,
    /// Raw lesser-racial-trait bitfield (offset 78).
    pub lrt_bits: u16,
    /// Singular race name (e.g. `"Humanoid"`), decoded from the packed
    /// [`strings`] field at the end of the record. Empty if it could not be
    /// located (e.g. a truncated record).
    pub singular_name: String,
    /// Plural race name (e.g. `"Humanoids"`).
    pub plural_name: String,
}

impl RaceRecord {
    /// Decode the verified fields from a **decrypted** type-6 block payload.
    ///
    /// # Errors
    /// Returns [`FormatError::Malformed`] if the payload is shorter than the
    /// portion of the record this decoder reads.
    pub fn from_payload(data: &[u8]) -> Result<Self> {
        if data.len() < MIN_RECORD_LEN {
            return Err(FormatError::Malformed(format!(
                "race record ({} bytes) too short (need at least {MIN_RECORD_LEN})",
                data.len()
            )));
        }
        let axis = |off: usize| (data[off], data[off + 1], data[off + 2]);
        let (gc, tc, rc) = axis(HAB_CENTER_OFFSET);
        let (gl, tl, rl) = axis(HAB_LOW_OFFSET);
        let (gh, th, rh) = axis(HAB_HIGH_OFFSET);
        let mut research_cost = [0u8; 6];
        research_cost.copy_from_slice(&data[RESEARCH_COST_OFFSET..RESEARCH_COST_OFFSET + 6]);
        let (singular, plural) = decode_race_names(data);
        Ok(Self {
            player_id: data[PLAYER_ID_OFFSET],
            gravity: HabRange {
                center: hab(gc),
                low: hab(gl),
                high: hab(gh),
            },
            temperature: HabRange {
                center: hab(tc),
                low: hab(tl),
                high: hab(th),
            },
            radiation: HabRange {
                center: hab(rc),
                low: hab(rl),
                high: hab(rh),
            },
            growth_rate: data[GROWTH_RATE_OFFSET],
            research_cost,
            prt: Prt::from_id(data[PRT_OFFSET]),
            lrt_bits: u16::from_le_bytes([data[LRT_OFFSET], data[LRT_OFFSET + 1]]),
            singular_name: singular,
            plural_name: plural,
        })
    }

    /// Decode the race record from the type-6 block of a decoded file.
    ///
    /// # Errors
    /// Returns [`FormatError::Malformed`] if the file has no type-6 block or the
    /// record is too short.
    pub fn from_file(file: &StarsFile) -> Result<Self> {
        let block = file.blocks.iter().find(|b| b.type_id == 6).ok_or_else(|| {
            FormatError::Malformed("file has no race/player (type-6) block".into())
        })?;
        Self::from_payload(&block.data)
    }

    /// Whether the given lesser racial trait is enabled.
    #[must_use]
    pub fn has_lrt(&self, lrt: Lrt) -> bool {
        self.lrt_bits & lrt.bit() != 0
    }

    /// The enabled lesser racial traits, in bit-position order.
    #[must_use]
    pub fn lrts(&self) -> Vec<Lrt> {
        Lrt::ALL
            .iter()
            .copied()
            .filter(|&l| self.has_lrt(l))
            .collect()
    }

    /// Whether any bit outside the documented 14-bit LRT range is set (should
    /// never happen for a well-formed record; a useful decode sanity check).
    #[must_use]
    pub fn has_unknown_lrt_bits(&self) -> bool {
        self.lrt_bits & !0x3FFF != 0
    }
}

/// Locate and decode the singular/plural race names from a decrypted type-6
/// record. Returns empty strings for any field whose framing runs past the end
/// of the record (a truncated/unknown layout) rather than panicking.
///
/// Framing (from TotalHost `StarsBlock.pm`): a `fullData` record (bit
/// [`FULL_DATA_FLAG`] set in `data[FLAGS_OFFSET]`, always the case for `.rN`
/// files) stores the names after the player-relations table —
/// `index = PLAYER_RELATIONS_OFFSET + data[PLAYER_RELATIONS_OFFSET] + 1`;
/// otherwise they start at [`SHORT_NAMES_OFFSET`]. From `index`, each name is a
/// length-prefixed packed [`strings`] field: the singular field is
/// `data[index ..= index + data[index]]` and the plural field runs from just
/// after it to the end of the record.
fn decode_race_names(data: &[u8]) -> (String, String) {
    let full_data = data
        .get(FLAGS_OFFSET)
        .is_some_and(|&b| b & FULL_DATA_FLAG != 0);
    let index = if full_data {
        match data.get(PLAYER_RELATIONS_OFFSET) {
            Some(&relations_len) => PLAYER_RELATIONS_OFFSET + relations_len as usize + 1,
            None => return (String::new(), String::new()),
        }
    } else {
        SHORT_NAMES_OFFSET
    };

    let Some(&singular_len) = data.get(index) else {
        return (String::new(), String::new());
    };
    // Singular field is [len][len bytes]; ends at index + singular_len inclusive.
    let singular_end = index + singular_len as usize;
    if singular_end >= data.len() {
        return (String::new(), String::new());
    }
    let singular = strings::decode_field(&data[index..=singular_end]);
    // Plural field is the rest of the record, again [len][packed...].
    let plural = strings::decode_field(&data[singular_end + 1..]);
    (singular, plural)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prt_round_trips_all_ids() {
        for id in 0..10u8 {
            assert!(!matches!(Prt::from_id(id), Prt::Unknown(_)));
        }
        assert_eq!(Prt::from_id(10), Prt::Unknown(10));
    }

    #[test]
    fn lrt_bits_are_distinct_and_ordered() {
        for (i, l) in Lrt::ALL.iter().enumerate() {
            assert_eq!(l.bit(), 1 << i, "{l:?} bit position");
        }
    }

    #[test]
    fn decodes_a_synthetic_humanoid_like_record() {
        // A perfectly-centered, JOAT, no-LRT record (like the Humanoid default).
        let mut d = vec![0u8; 90];
        d[PLAYER_ID_OFFSET] = 0xFF;
        for i in 0..3 {
            d[HAB_CENTER_OFFSET + i] = 50;
            d[HAB_LOW_OFFSET + i] = 15;
            d[HAB_HIGH_OFFSET + i] = 85;
        }
        d[GROWTH_RATE_OFFSET] = 15;
        for i in 0..6 {
            d[RESEARCH_COST_OFFSET + i] = 1;
        }
        d[PRT_OFFSET] = 9; // JOAT
                           // LRT = 0.
        let r = RaceRecord::from_payload(&d).unwrap();
        assert_eq!(r.player_id, 0xFF);
        assert_eq!(r.prt, Prt::JOAT);
        assert_eq!(r.growth_rate, 15);
        assert_eq!(r.research_cost, [1; 6]);
        assert_eq!(r.gravity.center, Some(50));
        assert_eq!(r.gravity.low, Some(15));
        assert_eq!(r.gravity.high, Some(85));
        assert!(!r.gravity.is_immune());
        assert!(r.lrts().is_empty());
        assert!(!r.has_unknown_lrt_bits());
    }

    #[test]
    fn immune_axis_is_none() {
        let mut d = vec![0u8; 90];
        d[HAB_CENTER_OFFSET] = 0xFF; // gravity immune
        d[HAB_LOW_OFFSET] = 0xFF;
        d[HAB_HIGH_OFFSET] = 0xFF;
        d[PRT_OFFSET] = 0;
        let r = RaceRecord::from_payload(&d).unwrap();
        assert!(r.gravity.is_immune());
        assert_eq!(r.gravity.center, None);
    }

    #[test]
    fn too_short_is_rejected() {
        assert!(RaceRecord::from_payload(&[0u8; 10]).is_err());
    }

    #[test]
    fn lrt_bitfield_decodes_named_traits() {
        // 0x0a81 = IFE | OBRM | LSP | RS (the exodus AI base set).
        let mut d = vec![0u8; 90];
        d[LRT_OFFSET] = 0x81;
        d[LRT_OFFSET + 1] = 0x0a;
        let r = RaceRecord::from_payload(&d).unwrap();
        assert_eq!(r.lrts(), vec![Lrt::IFE, Lrt::OBRM, Lrt::LSP, Lrt::RS]);
        assert!(r.has_lrt(Lrt::IFE));
        assert!(!r.has_lrt(Lrt::TT));
        assert!(!r.has_unknown_lrt_bits());
    }
}
