//! `.xy` universe file: header + game-info + the packed planet array.
//!
//! Unlike the other formats, a `.xy` file is **not** a clean sequence of blocks
//! to EOF. It is:
//!
//! ```text
//! [ FileHeaderBlock (type 8, plaintext, 16-byte payload) ]
//! [ game-info block  (type 7, encrypted, 64-byte payload) ]
//! [ planet region: N * 4-byte planet records + trailer ]  <- to EOF
//! ```
//!
//! The number of planets **N** is not derived from the file length; it is read
//! from the game-info block (a little-endian `u16` at offset 10, verified
//! against `24/128/160/360/540`-planet real files). After the `N` records an
//! optional **trailer** follows: an **in-game** `.xy` carries a 2-byte trailer
//! (`00 00`), while a **standalone** universe-definition file carries a 4-byte
//! trailer (`02 00 <players> 00`). The trailer is preserved verbatim so every
//! file round-trips.
//!
//! The planet region is **not** block-framed and — unlike every other payload —
//! is stored *without* the stream cipher: parsing the raw on-disk bytes as
//! 4-byte records yields sane, in-bounds, non-overlapping coordinates.
//!
//! Each 4-byte record is a little-endian `u32` partitioned as (confirmed
//! against six real universes and matching the community `struct position`):
//!
//! ```text
//! bits  0..9   x offset   (10 bits) — delta added to the running x
//! bits 10..21  y          (12 bits) — absolute y coordinate
//! bits 22..31  name id     (10 bits) — index into the master planet-name table
//! ```
//!
//! `x` is **not** stored absolutely: each record's `xoffset` is added to a
//! running total, so a planet's absolute x is the sum of all `xoffset`s up to
//! and including it. This is why planets are stored in non-decreasing x order
//! (the original tools require `x` to never decrease planet-to-planet). With
//! this reconstruction every planet in every sample universe has a unique
//! position, its `y` lies in a band whose width matches the universe size
//! class, and its `name id` resolves to a unique entry in the master name table
//! (see `docs/formats/xy.md` and [`crate::names`]).

use crate::block::{join_blocks, Block, FILE_HEADER_BLOCK};
use crate::header::FileHeader;
use crate::names::planet_name;
use crate::{FormatError, Result};

/// Type id of the `.xy` game-info block (a "planets" block per the registry).
pub const GAME_INFO_BLOCK: u8 = 7;

/// Size in bytes of one packed planet record in the `.xy` planet region.
pub const PLANET_RECORD_LEN: usize = 4;

/// Offset of the planet-count `u16` within the decrypted game-info payload.
pub const GAME_INFO_PLANET_COUNT_OFFSET: usize = 10;

/// Offset of the player-count byte within the decrypted game-info payload.
///
/// The player count is the low 5 bits of this byte.
pub const GAME_INFO_PLAYER_COUNT_OFFSET: usize = 8;

/// A single planet's packed record, unpacked from a 4-byte `.xy` word.
///
/// The fields are stored **exactly as on disk**, so [`PlanetPosition::to_word`]
/// is a byte-exact inverse of [`PlanetPosition::from_word`]. Note that
/// [`PlanetPosition::x_offset`] is a *delta*, not an absolute coordinate — use
/// [`Universe::planets_resolved`] to obtain absolute positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetPosition {
    /// X offset (low 10 bits): the amount added to the running x total.
    pub x_offset: u16,
    /// Y coordinate (next 12 bits): absolute.
    pub y: u16,
    /// Planet name index (high 10 bits): an index into the master name table.
    pub name_index: u16,
}

impl PlanetPosition {
    /// Unpack a 4-byte record word into its fields.
    #[must_use]
    pub fn from_word(word: u32) -> Self {
        Self {
            x_offset: (word & 0x03FF) as u16,
            y: ((word >> 10) & 0x0FFF) as u16,
            name_index: ((word >> 22) & 0x03FF) as u16,
        }
    }

    /// Re-pack these fields into their 4-byte record word.
    #[must_use]
    pub fn to_word(self) -> u32 {
        (u32::from(self.x_offset) & 0x03FF)
            | ((u32::from(self.y) & 0x0FFF) << 10)
            | ((u32::from(self.name_index) & 0x03FF) << 22)
    }
}

/// A planet with its **absolute** position and resolved name.
///
/// Produced by [`Universe::planets_resolved`] after summing the per-record
/// `x_offset`s and looking each `name_index` up in the master name table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Planet {
    /// Planet id (0-based index within the universe).
    pub id: u16,
    /// Absolute x coordinate (running sum of `x_offset`s up to this planet).
    pub x: u32,
    /// Absolute y coordinate.
    pub y: u16,
    /// The 10-bit name index stored in the record.
    pub name_index: u16,
    /// The resolved planet name, or `None` if the index is outside the table.
    pub name: Option<&'static str>,
}

/// The base every planet's x coordinate is measured from.
///
/// The `.xy` stores x as a chain of 10-bit deltas, and the chain starts at 1000
/// rather than 0. Confirmed against the engine's own coordinates: a fleet the
/// game records as orbiting a planet must sit exactly on it, and across
/// **43,769 orbiting-fleet readings** in the two sixteen-player games the
/// difference `fleet - planet` was `(1000, 0)` every single time — never any
/// other value, and never any offset in y.
///
/// This could not be caught by the round-trip tests, which re-emit the same
/// packed deltas whatever base they are read against.
pub const X_BASE: u32 = 1000;

/// The `.xy` game-info block (the engine's 64-byte `GAME` struct).
///
/// Field offsets are the NB09 `GAME` layout, confirmed by the static asserts
/// the reconstructed sources carry: `lid` at 0, `mdSize` at 4, `mdDensity` at
/// 6, `cPlayer` at 8, `cPlanMax` at 10, `mdStartDist` at 12, `fDirty` at 14,
/// the flag word at 16, `turn` at 18, the twelve victory-condition bytes at
/// 20 and a 32-byte name at 32.
///
/// Only the fields a reader needs are broken out; everything else is kept in
/// [`GameInfo::raw`] so the block re-encodes byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameInfo {
    /// Per-game id (`GAME.lid`).
    pub id: u32,
    /// Universe size class, 0 (tiny) to 4 (huge) — `GAME.mdSize`.
    pub size: i16,
    /// Planet density class — `GAME.mdDensity`.
    pub density: i16,
    /// How many players — `GAME.cPlayer`.
    pub players: i16,
    /// How many planets — `GAME.cPlanMax`.
    pub planets: i16,
    /// Starting-distance class — `GAME.mdStartDist`.
    pub start_distance: i16,
    /// The packed option flags at offset 16 (`fExtraFuel` … `fClumping`).
    pub flags: u16,
    /// Turn counter; the year is `2400 + turn`.
    pub turn: u16,
    /// The game's name, as stored (NUL-padded to 32 bytes).
    pub name: String,
    /// The whole 64-byte payload, kept so unmodelled fields survive a
    /// round-trip.
    pub raw: Vec<u8>,
}

/// Bit positions within [`GameInfo::flags`] (`GAME.wCrap`).
pub mod game_flag {
    /// Unlimited minerals ("extra fuel" in the original's field name).
    pub const EXTRA_FUEL: u16 = 1 << 0;
    /// Slower tech advances: research costs double.
    pub const SLOW_TECH: u16 = 1 << 1;
    /// Exactly one human player, so the game plays without a separate host.
    pub const SINGLE_PLAYER: u16 = 1 << 2;
    /// The tutorial universe.
    pub const TUTORIAL: u16 = 1 << 3;
    /// Computer players form alliances.
    ///
    /// The NB09 field is `fAisBand`, which reads like a handicap band and was
    /// taken that way here; the Advanced Game dialog settles it. Checkbox
    /// `0x3fc` on resource 390 is captioned
    /// `&Computer Players Form Alliances`, and `NewGameDlg` (`1078:7fb6`)
    /// checks it from **bit 4** of the flags word.
    pub const AIS_BAND: u16 = 1 << 4;
    /// Public-player (BBS) game.
    pub const BBS_PLAY: u16 = 1 << 5;
    /// Scores are visible to everyone.
    pub const VIS_SCORES: u16 = 1 << 6;
    /// No random events: no wormholes, no artifacts, no Mystery Trader.
    pub const NO_RANDOM: u16 = 1 << 7;
    /// Clump the planets rather than scattering them evenly.
    pub const CLUMPING: u16 = 1 << 8;
}

/// The ten victory conditions, in the order they are stored in `GAME.rgvc`.
///
/// Each is one byte: bit 7 says the game is using it, and the low seven bits
/// are a **setting**, not the threshold — [`GameInfo::victory_value`] turns one
/// into the other.
pub mod victory {
    /// Owns a percentage of all planets.
    pub const PLANET_CONTROL: usize = 0;
    /// Attains a tech level, in [`TECH_FIELDS`] fields.
    pub const TECH_LEVEL: usize = 1;
    /// How many fields [`TECH_LEVEL`] must be reached in.
    pub const TECH_FIELDS: usize = 2;
    /// Exceeds a score.
    pub const SCORE: usize = 3;
    /// Exceeds the second player's score by a percentage.
    pub const SCORE_EXCESS: usize = 4;
    /// Produces a number of thousands of resources a year.
    pub const PRODUCTION: usize = 5;
    /// Owns a number of capital ships.
    pub const CAPITAL_SHIPS: usize = 6;
    /// Holds the highest score after a number of years.
    pub const HIGH_SCORE_AT: usize = 7;
    /// How many of the others a player must meet to win.
    pub const MUST_MEET: usize = 8;
    /// The earliest year a win counts.
    pub const LEAST_YEARS: usize = 9;
    /// How many conditions there are room for.
    pub const COUNT: usize = 12;
    /// Where they start in the game-info payload.
    pub const OFFSET: usize = 20;
}

impl GameInfo {
    /// Size in bytes of the game-info payload.
    pub const LEN: usize = 64;

    /// Decode a 64-byte game-info payload.
    ///
    /// # Errors
    ///
    /// [`FormatError::Malformed`] if the payload is shorter than
    /// [`GameInfo::LEN`].
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < Self::LEN {
            return Err(FormatError::Malformed(format!(
                ".xy game-info block is {} bytes, need {}",
                data.len(),
                Self::LEN
            )));
        }
        let w = |o: usize| i16::from_le_bytes([data[o], data[o + 1]]);
        let name_end = data[32..64].iter().position(|b| *b == 0).unwrap_or(32) + 32;
        Ok(Self {
            id: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            size: w(4),
            density: w(6),
            players: w(8),
            planets: w(10),
            start_distance: w(12),
            flags: u16::from_le_bytes([data[16], data[17]]),
            turn: u16::from_le_bytes([data[18], data[19]]),
            name: String::from_utf8_lossy(&data[32..name_end]).into_owned(),
            raw: data[..Self::LEN].to_vec(),
        })
    }

    /// The raw victory-condition bytes (`GAME.rgvc`).
    #[must_use]
    pub fn victory_bytes(&self) -> [u8; victory::COUNT] {
        let mut out = [0u8; victory::COUNT];
        if let Some(bytes) = self
            .raw
            .get(victory::OFFSET..victory::OFFSET + victory::COUNT)
        {
            out.copy_from_slice(bytes);
        }
        out
    }

    /// Whether the game is using a victory condition (`GetVCCheck`,
    /// `1078:b60c`): bit 7 of its byte.
    #[must_use]
    pub fn victory_active(&self, condition: usize) -> bool {
        self.victory_bytes()
            .get(condition)
            .is_some_and(|b| b & 0x80 != 0)
    }

    /// The threshold a victory condition is set to (`GetVCVal`, `1078:b710`).
    ///
    /// The stored seven bits are a position on the dialog's slider, not the
    /// number itself; each condition scales its own way, which is why the
    /// mapping lives here rather than in the caller.
    #[must_use]
    pub fn victory_value(&self, condition: usize) -> i32 {
        let bytes = self.victory_bytes();
        let raw = i32::from(bytes.get(condition).copied().unwrap_or(0) & 0x7F);
        match condition {
            victory::PLANET_CONTROL => raw * 5 + 20,
            victory::TECH_LEVEL => raw + 8,
            victory::TECH_FIELDS => raw + 2,
            victory::SCORE => raw * 1000 + 1000,
            victory::SCORE_EXCESS => raw * 10 + 20,
            victory::PRODUCTION | victory::CAPITAL_SHIPS => raw * 10 + 10,
            victory::HIGH_SCORE_AT | victory::LEAST_YEARS => raw * 10 + 30,
            // "How many must be met" cannot exceed how many are in use. The
            // original counts the active conditions, skipping the tech-fields
            // byte because it belongs to the tech-level one.
            victory::MUST_MEET => {
                let active = (0..8)
                    .filter(|c| *c != victory::TECH_FIELDS)
                    .filter(|c| bytes.get(*c).is_some_and(|b| b & 0x80 != 0))
                    .count();
                raw.min(i32::try_from(active).unwrap_or(raw))
            }
            _ => raw,
        }
    }

    /// Re-encode this game info, preserving every byte the struct does not
    /// model.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.raw.clone();
        out.resize(Self::LEN, 0);
        out[0..4].copy_from_slice(&self.id.to_le_bytes());
        out[4..6].copy_from_slice(&self.size.to_le_bytes());
        out[6..8].copy_from_slice(&self.density.to_le_bytes());
        out[8..10].copy_from_slice(&self.players.to_le_bytes());
        out[10..12].copy_from_slice(&self.planets.to_le_bytes());
        out[12..14].copy_from_slice(&self.start_distance.to_le_bytes());
        out[16..18].copy_from_slice(&self.flags.to_le_bytes());
        out[18..20].copy_from_slice(&self.turn.to_le_bytes());
        let name = self.name.as_bytes();
        let n = name.len().min(31);
        out[32..64].fill(0);
        out[32..32 + n].copy_from_slice(&name[..n]);
        out
    }
}

/// A parsed `.xy` universe file.
///
/// [`Universe::decode`] and [`Universe::encode`] are byte-exact inverses for
/// real files, so `.xy` round-trips like the fully-framed formats.
#[derive(Debug, Clone)]
pub struct Universe {
    /// The parsed file header.
    pub header: FileHeader,
    /// Raw 16-byte header payload, kept verbatim to re-emit the plaintext block.
    header_payload: Vec<u8>,
    /// Decrypted game-info (type-7) payload.
    pub game_info: Vec<u8>,
    /// The packed planet records, in planet-id order.
    pub planets: Vec<PlanetPosition>,
    /// Any bytes following the planet records, kept verbatim.
    ///
    /// A 2-byte `00 00` for an in-game `.xy`; a 4-byte `02 00 <players> 00` for
    /// a standalone universe-definition file (see the module docs).
    pub trailer: Vec<u8>,
}

impl Universe {
    /// Decode a `.xy` file into its header, game-info and planet array.
    ///
    /// # Errors
    ///
    /// - [`FormatError::UnexpectedEof`] if the file is too short to contain the
    ///   header and game-info blocks.
    /// - [`FormatError::Malformed`] if the leading block is not a header block,
    ///   the game-info block is not type [`GAME_INFO_BLOCK`], the game-info
    ///   payload is too short to hold the planet count, or the planet region is
    ///   too short for the declared number of 4-byte records.
    /// - Propagates header-parse errors (e.g. [`FormatError::BadMagic`]).
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        // --- header block (type 8, plaintext) ---
        let (hdr_type, hdr_payload, after_hdr) = read_block(bytes, 0)?;
        if hdr_type != FILE_HEADER_BLOCK {
            return Err(FormatError::Malformed(format!(
                ".xy does not start with a header block (type {FILE_HEADER_BLOCK}), found {hdr_type}"
            )));
        }
        let header = FileHeader::parse(hdr_payload)?;

        // --- game-info block (type 7, encrypted) ---
        let (gi_type, gi_payload, after_gi) = read_block(bytes, after_hdr)?;
        if gi_type != GAME_INFO_BLOCK {
            return Err(FormatError::Malformed(format!(
                ".xy second block is not the game-info block (type {GAME_INFO_BLOCK}), found {gi_type}"
            )));
        }
        let mut rng = header.init_rng();
        let game_info = rng.apply(gi_payload);

        // The planet count is authoritative from the game-info block, not the
        // file length (the region carries a trailer).
        if game_info.len() < GAME_INFO_PLANET_COUNT_OFFSET + 2 {
            return Err(FormatError::Malformed(format!(
                ".xy game-info block ({} bytes) too short for the planet count",
                game_info.len()
            )));
        }
        let planet_count = u16::from_le_bytes([
            game_info[GAME_INFO_PLANET_COUNT_OFFSET],
            game_info[GAME_INFO_PLANET_COUNT_OFFSET + 1],
        ]) as usize;

        // --- planet region: N * 4-byte records + trailer (no leading header) ---
        let body = &bytes[after_gi..];
        let records_len = planet_count * PLANET_RECORD_LEN;
        if body.len() < records_len {
            return Err(FormatError::UnexpectedEof {
                offset: after_gi,
                needed: records_len - body.len(),
            });
        }
        let planets = (0..planet_count)
            .map(|i| {
                let o = i * PLANET_RECORD_LEN;
                PlanetPosition::from_word(u32::from_le_bytes([
                    body[o],
                    body[o + 1],
                    body[o + 2],
                    body[o + 3],
                ]))
            })
            .collect();
        let trailer = body[records_len..].to_vec();

        Ok(Self {
            header,
            header_payload: hdr_payload.to_vec(),
            game_info,
            planets,
            trailer,
        })
    }

    /// Re-encode this universe back to raw `.xy` bytes.
    ///
    /// Byte-for-byte inverse of [`Universe::decode`] for unmodified real files.
    ///
    /// # Errors
    ///
    /// [`FormatError::Malformed`] if a block no longer fits the header word.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut rng = self.header.init_rng();
        let header_block = Block::new(FILE_HEADER_BLOCK, self.header_payload.clone())?;
        let game_info_block = Block::new(GAME_INFO_BLOCK, rng.apply(&self.game_info))?;
        let mut out = join_blocks(&[header_block, game_info_block])?;

        for planet in &self.planets {
            out.extend_from_slice(&planet.to_word().to_le_bytes());
        }
        out.extend_from_slice(&self.trailer);
        Ok(out)
    }

    /// Build a brand-new universe file from generated planet positions.
    ///
    /// This is the writer side of `GenerateWorld` (`create.c`): the planet
    /// records are the same chain of 10-bit x deltas the original emits,
    /// starting from [`X_BASE`], and the file ends with the same 4-byte
    /// player-count block (`WriteRt(0, 2, &cPlayer)`) that a standalone
    /// universe-definition file carries.
    ///
    /// `positions` must be sorted by ascending x — the delta encoding cannot
    /// represent a decrease — and must be the same length as `name_ids`.
    ///
    /// # Errors
    ///
    /// [`FormatError::Malformed`] if the two slices differ in length, if x
    /// ever decreases, or if a delta or coordinate does not fit its field.
    pub fn create(
        header: FileHeader,
        info: &GameInfo,
        positions: &[(u16, u16)],
        name_ids: &[u16],
    ) -> Result<Self> {
        if positions.len() != name_ids.len() {
            return Err(FormatError::Malformed(format!(
                ".xy needs one name per planet: {} positions, {} names",
                positions.len(),
                name_ids.len()
            )));
        }
        let mut planets = Vec::with_capacity(positions.len());
        let mut x_prev = X_BASE;
        for (i, ((x, y), name)) in positions.iter().zip(name_ids).enumerate() {
            let x = u32::from(*x);
            if x < x_prev {
                return Err(FormatError::Malformed(format!(
                    ".xy planet {i} moves backwards in x ({x} after {x_prev})"
                )));
            }
            let dx = x - x_prev;
            if dx > 0x3FF || u32::from(*y) > 0x0FFF || u32::from(*name) > 0x3FF {
                return Err(FormatError::Malformed(format!(
                    ".xy planet {i} does not fit its record: dx={dx} y={y} name={name}"
                )));
            }
            planets.push(PlanetPosition {
                x_offset: dx as u16,
                y: *y,
                name_index: *name,
            });
            x_prev = x;
        }

        let players = u8::try_from(info.players.max(0)).unwrap_or(u8::MAX);
        Ok(Self {
            header_payload: header.to_payload().to_vec(),
            header,
            game_info: info.encode(),
            planets,
            // `WriteRt(0, 2, &cPlayer)`: a type-0 block holding the player
            // count, written outside the cipher like the planet region.
            trailer: vec![0x02, 0x00, players, 0x00],
        })
    }

    /// Decode the game-info block into its fields.
    ///
    /// # Errors
    ///
    /// [`FormatError::Malformed`] if the block is shorter than
    /// [`GameInfo::LEN`].
    pub fn game(&self) -> Result<GameInfo> {
        GameInfo::decode(&self.game_info)
    }

    /// The number of planets in the universe.
    #[must_use]
    pub fn planet_count(&self) -> usize {
        self.planets.len()
    }

    /// The number of players declared in the game-info block.
    ///
    /// Read from the low 5 bits of game-info offset
    /// [`GAME_INFO_PLAYER_COUNT_OFFSET`]. Verified against the sample game
    /// (3 players) and self-consistent with the standalone universe files'
    /// trailer.
    #[must_use]
    pub fn player_count(&self) -> u8 {
        self.game_info
            .get(GAME_INFO_PLAYER_COUNT_OFFSET)
            .map_or(0, |b| b & 0x1F)
    }

    /// Resolve every planet to its **absolute** position and name.
    ///
    /// Absolute x is [`X_BASE`] plus the running sum of the per-record
    /// `x_offset`s; `y` and the name index are taken verbatim, and the name is
    /// looked up in the master planet-name table
    /// ([`crate::names::planet_name`]).
    #[must_use]
    pub fn planets_resolved(&self) -> Vec<Planet> {
        let mut x = X_BASE;
        self.planets
            .iter()
            .enumerate()
            .map(|(i, p)| {
                x += u32::from(p.x_offset);
                Planet {
                    id: i as u16,
                    x,
                    y: p.y,
                    name_index: p.name_index,
                    name: planet_name(p.name_index),
                }
            })
            .collect()
    }
}

/// Read one framed block starting at `offset`, returning `(type_id, payload,
/// next_offset)`.
fn read_block(bytes: &[u8], offset: usize) -> Result<(u8, &[u8], usize)> {
    if offset + 2 > bytes.len() {
        return Err(FormatError::UnexpectedEof {
            offset,
            needed: (offset + 2) - bytes.len(),
        });
    }
    let header = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
    let size = (header & 0x03FF) as usize;
    let type_id = (header >> 10) as u8;
    let start = offset + 2;
    if start + size > bytes.len() {
        return Err(FormatError::UnexpectedEof {
            offset: start,
            needed: (start + size) - bytes.len(),
        });
    }
    Ok((type_id, &bytes[start..start + size], start + size))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The victory bytes of `fixtures/games/all-computer-players/*.xy`, which
    /// is a game set up with four conditions and a first-past-the-post rule.
    const REAL: [u8; 12] = [
        0x88, 0x8e, 0x82, 0x0a, 0x88, 0x09, 0x09, 0x07, 0x01, 0x04, 0x00, 0x00,
    ];

    fn game(bytes: [u8; 12]) -> GameInfo {
        let mut raw = vec![0u8; GameInfo::LEN];
        raw[victory::OFFSET..victory::OFFSET + 12].copy_from_slice(&bytes);
        GameInfo::decode(&raw).expect("decodes")
    }

    #[test]
    fn victory_sliders_become_thresholds() {
        let g = game(REAL);
        // Bit 7 says the game is playing for it.
        assert!(g.victory_active(victory::PLANET_CONTROL));
        assert!(g.victory_active(victory::TECH_LEVEL));
        assert!(g.victory_active(victory::SCORE_EXCESS));
        assert!(!g.victory_active(victory::SCORE));
        assert!(!g.victory_active(victory::PRODUCTION));

        // ... and the low seven bits are a slider position, not the number.
        assert_eq!(g.victory_value(victory::PLANET_CONTROL), 60, "8 * 5 + 20 %");
        assert_eq!(g.victory_value(victory::TECH_LEVEL), 22, "14 + 8");
        assert_eq!(g.victory_value(victory::TECH_FIELDS), 4, "2 + 2");
        assert_eq!(g.victory_value(victory::SCORE), 11_000);
        assert_eq!(g.victory_value(victory::SCORE_EXCESS), 100, "8 * 10 + 20 %");
        assert_eq!(g.victory_value(victory::PRODUCTION), 100, "thousands");
        assert_eq!(g.victory_value(victory::CAPITAL_SHIPS), 100);
        assert_eq!(g.victory_value(victory::HIGH_SCORE_AT), 100, "years");
        assert_eq!(g.victory_value(victory::LEAST_YEARS), 70);
        // Four conditions are switched on, and this game asks for one of them.
        assert_eq!(g.victory_value(victory::MUST_MEET), 1);
    }

    /// "How many must be met" can never exceed how many are being played for.
    #[test]
    fn must_meet_is_capped_by_what_is_switched_on() {
        let mut bytes = [0u8; 12];
        bytes[victory::MUST_MEET] = 5;
        assert_eq!(game(bytes).victory_value(victory::MUST_MEET), 0);
        bytes[victory::PLANET_CONTROL] = 0x80;
        bytes[victory::SCORE] = 0x80;
        assert_eq!(game(bytes).victory_value(victory::MUST_MEET), 2);
    }

    /// A slider left at zero still has a threshold: the bottom of its range.
    #[test]
    fn a_slider_at_zero_is_the_bottom_of_the_range() {
        let g = game([0; 12]);
        assert_eq!(g.victory_value(victory::PLANET_CONTROL), 20);
        assert_eq!(g.victory_value(victory::TECH_LEVEL), 8);
        assert_eq!(g.victory_value(victory::TECH_FIELDS), 2);
        assert_eq!(g.victory_value(victory::HIGH_SCORE_AT), 30);
    }

    #[test]
    fn planet_position_word_round_trips() {
        for word in [0u32, 0x0000_0001, 0x8c05_5555, 0xFFFF_FFFF, 0x1c05_8552] {
            // 10 + 12 + 10 = 32, so the full 32-bit word round-trips.
            assert_eq!(
                PlanetPosition::from_word(word).to_word(),
                word,
                "{word:#010x}"
            );
        }
    }

    #[test]
    fn planet_position_field_split() {
        // x_offset=5, y=10, name=3  ->  5 | (10<<10) | (3<<22)
        let p = PlanetPosition {
            x_offset: 5,
            y: 10,
            name_index: 3,
        };
        let w = p.to_word();
        assert_eq!(w, 5 | (10 << 10) | (3 << 22));
        assert_eq!(PlanetPosition::from_word(w), p);
    }

    #[test]
    fn field_widths_are_masked() {
        // y is 12 bits and name is 10 bits; oversized inputs are truncated.
        let p = PlanetPosition {
            x_offset: 0x3FF,
            y: 0x0FFF,
            name_index: 0x03FF,
        };
        assert_eq!(PlanetPosition::from_word(p.to_word()), p);
        // The top y bit (0x1000) and top name bits are dropped.
        let clipped = PlanetPosition {
            x_offset: 0,
            y: 0x1000,
            name_index: 0x0400,
        };
        assert_eq!(clipped.to_word(), 0);
    }
}
