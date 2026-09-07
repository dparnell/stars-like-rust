//! Typed decoder for the **race record** — the encrypted type-6 block that
//! carries a race definition in a `.rN` file (and, in fuller form, a player in
//! `.mN`/`.hst`).
//!
//! Only the fields **verified against real files** are exposed here (see
//! `docs/formats/race-r.md`): the player marker, the habitability ranges,
//! growth rate, the `fullData` marker, the research percentage, the seven-byte
//! economy block, the per-field research cost, the primary racial trait (PRT),
//! the lesser-racial-trait (LRT) bitfield, the two known checkbox flags, and the
//! packed singular/plural names. The field offsets follow TotalHost's
//! `StarsRace.pl` and were cross-checked against the fixtures. The
//! player-block-only bytes (homeworld, password, tech levels, MT items, player
//! relations — all zero in a `.rN` file) are left undecoded. This is a
//! *read-only view*, not a full model — it is deliberately **not** used for
//! re-encoding, which still goes through the byte-exact container in
//! [`crate::file`].
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
/// Offset of the research-percentage byte (share of resources spent on
/// research; defaults to `15`). Only meaningful in a `fullData` record.
pub const RESEARCH_PERCENTAGE_OFFSET: usize = 56;
/// Offset of the seven-byte economy block (see [`Economy`]). Only meaningful in
/// a `fullData` record.
pub const ECONOMY_OFFSET: usize = 62;
/// Offset of the "spend leftover advantage points on" selector. Only meaningful
/// in a `fullData` record.
pub const SPEND_LEFTOVER_OFFSET: usize = 69;
/// Offset of the six per-field research-cost bytes (Energy, Weapons,
/// Propulsion, Construction, Electronics, Biotechnology).
pub const RESEARCH_COST_OFFSET: usize = 70;
/// Offset of the primary-racial-trait byte.
pub const PRT_OFFSET: usize = 76;
/// Offset of the lesser-racial-trait bitfield (little-endian `u16`).
///
/// It is the low half of `PLAYER.grbitAttr`, a `uint32_t` at `+0x4e` — so the
/// "checkbox" byte at [`CHECKBOX_OFFSET`] is the same field's high byte and
/// the flags there are simply bits 24 to 31 of it.
pub const LRT_OFFSET: usize = 78;
/// Offset of the checkbox-flags byte (offset 81) — the high byte of
/// `PLAYER.grbitAttr`. Only meaningful in a `fullData` record.
pub const CHECKBOX_OFFSET: usize = 81;
/// Checkbox bit 5 — *expensive tech starts at level 3* (`ibitRaceTech3`, 29).
pub const CHECKBOX_EXPENSIVE_TECH_AT_3: u8 = 1 << 5;
/// Checkbox bit 6 — the race is played by the computer (`ibitRaceAIPlayer`,
/// 30). Not a wizard setting: it marks the templates the AI opponents are
/// built from, and is the one bit set in the shipped `random.r1`.
pub const CHECKBOX_AI_PLAYER: u8 = 1 << 6;
/// Checkbox bit 7 — *factories cost 1 less germanium to build*
/// (`ibitRaceCheapFact`, 31).
pub const CHECKBOX_FACTORIES_COST_1_LESS_GERM: u8 = 1 << 7;
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

    /// The PRT id stored at offset 76.
    #[must_use]
    pub fn id(self) -> u8 {
        match self {
            Self::HE => 0,
            Self::SS => 1,
            Self::WM => 2,
            Self::CA => 3,
            Self::IS => 4,
            Self::SD => 5,
            Self::PP => 6,
            Self::IT => 7,
            Self::AR => 8,
            Self::JOAT => 9,
            Self::Unknown(other) => other,
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

/// The seven-byte economy block (race-record offsets 62–68). These are the
/// player-adjustable production settings from the race wizard's "Production"
/// panel; the values are the raw stored bytes (e.g. Humanoid's default
/// `10,10,10,10,10,5,10`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Economy {
    /// Offset 62 — colonists needed per resource, in thousands (`10` = one
    /// resource per 1000 colonists for the Humanoid default).
    pub resource_per_colonist: u8,
    /// Offset 63 — resources produced per 10 factories.
    pub produce_per_factory: u8,
    /// Offset 64 — resources to build one factory.
    pub factory_build_cost: u8,
    /// Offset 65 — factories that can be operated per 10,000 colonists.
    pub factories_operated: u8,
    /// Offset 66 — mineral output per 10 mines.
    pub produce_per_mine: u8,
    /// Offset 67 — resources to build one mine.
    pub mine_build_cost: u8,
    /// Offset 68 — mines that can be operated per 10,000 colonists.
    pub mines_operated: u8,
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
    /// Whether the record carries the full player/race payload (the `fullData`
    /// flag, bit 2 of the flags byte at offset 6). Always `true` for `.rN`
    /// files; the [`research_percentage`](Self::research_percentage),
    /// [`economy`](Self::economy) and checkbox fields below are only meaningful
    /// when this is set.
    pub full_data: bool,
    /// Share of resources spent on research, in percent (offset 56; defaults to
    /// `15`).
    pub research_percentage: u8,
    /// Production/economy settings (offsets 62–68).
    pub economy: Economy,
    /// Where surplus advantage points are spent (offset 69): a small selector
    /// (e.g. `3` = on factories); exposed as the raw stored byte.
    pub spend_leftover_points: u8,
    /// Per-field research cost in field order (Energy, Weapons, Propulsion,
    /// Construction, Electronics, Biotechnology); `0`/`1`/`2` = costs-less /
    /// normal / costs-more.
    pub research_cost: [u8; 6],
    /// Primary racial trait.
    pub prt: Prt,
    /// Raw lesser-racial-trait bitfield (offset 78).
    pub lrt_bits: u16,
    /// Checkbox: *expensive tech starts at level 3* (offset 81, bit 5).
    pub expensive_tech_starts_at_level_3: bool,
    /// `ibitRaceAIPlayer` (offset 81, bit 6): the race is one the computer
    /// plays. The wizard does not offer it; the shipped `random.r1` — the
    /// template the AI opponents are drawn from — is the one fixture that
    /// carries it.
    pub ai_player: bool,
    /// Checkbox: *factories cost 1 less germanium to build* (offset 81, bit 7).
    pub factories_cost_one_less_germanium: bool,
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
        // `fullData`-only fields (economy, research %, checkboxes). Read with
        // bounds-checked accessors so a short/non-fullData record decodes to
        // sane defaults rather than panicking.
        let byte = |off: usize| data.get(off).copied().unwrap_or(0);
        let full_data = byte(FLAGS_OFFSET) & FULL_DATA_FLAG != 0;
        let checkbox = byte(CHECKBOX_OFFSET);
        let economy = Economy {
            resource_per_colonist: byte(ECONOMY_OFFSET),
            produce_per_factory: byte(ECONOMY_OFFSET + 1),
            factory_build_cost: byte(ECONOMY_OFFSET + 2),
            factories_operated: byte(ECONOMY_OFFSET + 3),
            produce_per_mine: byte(ECONOMY_OFFSET + 4),
            mine_build_cost: byte(ECONOMY_OFFSET + 5),
            mines_operated: byte(ECONOMY_OFFSET + 6),
        };
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
            full_data,
            research_percentage: byte(RESEARCH_PERCENTAGE_OFFSET),
            economy,
            spend_leftover_points: byte(SPEND_LEFTOVER_OFFSET),
            research_cost,
            prt: Prt::from_id(data[PRT_OFFSET]),
            lrt_bits: u16::from_le_bytes([data[LRT_OFFSET], data[LRT_OFFSET + 1]]),
            expensive_tech_starts_at_level_3: checkbox & CHECKBOX_EXPENSIVE_TECH_AT_3 != 0,
            ai_player: checkbox & CHECKBOX_AI_PLAYER != 0,
            factories_cost_one_less_germanium: checkbox & CHECKBOX_FACTORIES_COST_1_LESS_GERM != 0,
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

/// Write the modelled race fields back into a type-6 payload.
///
/// The caller owns the buffer, which must already be at least
/// [`CHECKBOX_OFFSET`] + 1 bytes: this writes the fields it models over
/// whatever is there and leaves everything else — the player header, the
/// research state, the bytes nothing has identified — untouched. That is what
/// makes [`crate::PlayerRecord::encode`] an exact inverse of its decoder:
/// re-encoding a record nobody edited writes the same bytes back.
///
/// The names are **not** written here; they live after the player-relations
/// table and are the caller's to place.
pub fn write_race_fields(race: &RaceRecord, data: &mut [u8]) {
    if data.len() <= CHECKBOX_OFFSET {
        return;
    }
    data[PLAYER_ID_OFFSET] = race.player_id;

    for (i, range) in [race.gravity, race.temperature, race.radiation]
        .into_iter()
        .enumerate()
    {
        data[HAB_CENTER_OFFSET + i] = range.center.unwrap_or(0xFF);
        data[HAB_LOW_OFFSET + i] = range.low.unwrap_or(0xFF);
        data[HAB_HIGH_OFFSET + i] = range.high.unwrap_or(0xFF);
    }
    data[GROWTH_RATE_OFFSET] = race.growth_rate;

    if race.full_data {
        data[FLAGS_OFFSET] |= FULL_DATA_FLAG;
    } else {
        data[FLAGS_OFFSET] &= !FULL_DATA_FLAG;
    }
    data[RESEARCH_PERCENTAGE_OFFSET] = race.research_percentage;

    let e = race.economy;
    data[ECONOMY_OFFSET] = e.resource_per_colonist;
    data[ECONOMY_OFFSET + 1] = e.produce_per_factory;
    data[ECONOMY_OFFSET + 2] = e.factory_build_cost;
    data[ECONOMY_OFFSET + 3] = e.factories_operated;
    data[ECONOMY_OFFSET + 4] = e.produce_per_mine;
    data[ECONOMY_OFFSET + 5] = e.mine_build_cost;
    data[ECONOMY_OFFSET + 6] = e.mines_operated;
    data[SPEND_LEFTOVER_OFFSET] = race.spend_leftover_points;
    data[RESEARCH_COST_OFFSET..RESEARCH_COST_OFFSET + 6].copy_from_slice(&race.research_cost);

    data[PRT_OFFSET] = race.prt.id();
    data[LRT_OFFSET..LRT_OFFSET + 2].copy_from_slice(&race.lrt_bits.to_le_bytes());

    let mut checkbox = data[CHECKBOX_OFFSET]
        & !(CHECKBOX_EXPENSIVE_TECH_AT_3
            | CHECKBOX_AI_PLAYER
            | CHECKBOX_FACTORIES_COST_1_LESS_GERM);
    if race.expensive_tech_starts_at_level_3 {
        checkbox |= CHECKBOX_EXPENSIVE_TECH_AT_3;
    }
    if race.ai_player {
        checkbox |= CHECKBOX_AI_PLAYER;
    }
    if race.factories_cost_one_less_germanium {
        checkbox |= CHECKBOX_FACTORIES_COST_1_LESS_GERM;
    }
    data[CHECKBOX_OFFSET] = checkbox;
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
    // The plural field is framed the same way, and its own length byte says
    // where it ends: a record can carry padding after it, and reading to the
    // end of the block would decode that padding as part of the name.
    let plural_at = singular_end + 1;
    let plural = match data.get(plural_at) {
        Some(&len) => {
            let end = plural_at + len as usize;
            if end < data.len() {
                strings::decode_field(&data[plural_at..=end])
            } else {
                String::new()
            }
        }
        None => String::new(),
    };
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
