//! Typed decoder for the **planet** blocks (`rtPlanetA`/`B`/`C`, type ids 13,
//! 14 and 15) that appear in `.hst`, `.mN` and `.hN` files.
//!
//! Every galaxy planet is stored as one variable-length planet block. The
//! layout is *presence-driven*: a 2-byte id/owner word, a 2-byte flags/detail
//! word, and then a sequence of optional sections whose presence is controlled
//! by the detail level and the flag bits. This module decodes the sections that
//! are **verified against the real sample files** (see `docs/formats/planet.md`)
//! into a read-only [`PlanetRecord`].
//!
//! The field layout was recovered from TotalHost's `StarsPlanet.pl` (Rick
//! Steeves, in turn derived from the `starsapi` project) and cross-checked
//! against `fixtures/incoming/turn0/Game.hst`: its three homeworlds decode to
//! population 25,000 with 10 mines / 10 factories / 10 defenses and a starbase,
//! and the Humanoid player's homeworld reads the perfectly-centred environment
//! `50/50/50`.
//!
//! Like [`crate::race`], this is an *interpreted view* used for analysis and the
//! simulation core, **not** for re-encoding — write-back still goes through the
//! byte-exact container in [`crate::file`].

use crate::block::BlockType;
use crate::file::StarsFile;

/// The three Stars! minerals, in canonical order (ironium, boranium,
/// germanium).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Minerals {
    /// Ironium.
    pub ironium: u32,
    /// Boranium.
    pub boranium: u32,
    /// Germanium.
    pub germanium: u32,
}

/// A planet's environment reading (each 0..=255, mapping to the in-game
/// gravity/temperature/radiation scales).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Environment {
    /// Gravity click (0..=255).
    pub gravity: u8,
    /// Temperature click (0..=255).
    pub temperature: u8,
    /// Radiation click (0..=255).
    pub radiation: u8,
}

/// The mineral concentration of a planet's crust (each 0..=255).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Concentration {
    /// Ironium concentration.
    pub ironium: u8,
    /// Boranium concentration.
    pub boranium: u8,
    /// Germanium concentration.
    pub germanium: u8,
}

/// The installations on an owned, fully-visible planet (`rtPlanetA` only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Installations {
    /// Population change accumulator (`iDeltaPop`, low 8 bits of the block).
    pub delta_pop: u8,
    /// Number of mines built.
    pub mines: u16,
    /// Number of factories built.
    pub factories: u16,
    /// Number of defenses built.
    pub defenses: u16,
    /// Planetary scanner design index (`31` = no scanner).
    pub scanner: u8,
    /// Whether the planet holds an undiscovered Mystery-Trader artifact.
    pub artifact: bool,
    /// Whether the planet is flagged "don't contribute to research".
    pub no_research: bool,
}

/// A planet's starbase, when present and visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Starbase {
    /// Starbase design slot (0..=15).
    pub design: u8,
    /// Accumulated damage, in percent (0 when undamaged / unknown).
    pub damage_pct: u16,
    /// Mass-driver packet fling destination planet id (0 when none).
    pub fling_dest: u16,
    /// Mass-driver warp speed (0 when no fling configured).
    pub warp: u8,
    /// "Did not heal this year" flag (turn-generation bookkeeping).
    pub no_heal: bool,
}

/// A decoded planet record.
///
/// Fields that are absent from the on-disk record (because the detail level or
/// a flag bit did not select them) are `None`. In a host file (`.hst`) every
/// planet is present; uninhabited planets carry only concentrations and
/// environment, while inhabited planets add population, surface minerals,
/// installations and (if built) a starbase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetRecord {
    /// Planet number (0-based), from the low 11 bits of the first word.
    pub id: u16,
    /// Owning player (0-based), or `None` if the planet is unowned (owner field
    /// `31`).
    pub owner: Option<u8>,
    /// Detail level (`det`): `1` = minimal, `3` = some, `4` = more (adds surface
    /// minerals), `7` = all.
    pub detail: u8,
    /// Whether this is a player's homeworld.
    pub homeworld: bool,
    /// `fInclude` — the planet is included in reports / turn processing.
    pub include: bool,
    /// Whether a starbase is present.
    pub has_starbase: bool,
    /// `fIncEVO` — the planet has been terraformed (original environment stored).
    pub terraformed: bool,
    /// `fIncImp` — installations (mines/factories/defenses) are present.
    pub has_installations: bool,
    /// `fIsArtifact` — a Mystery-Trader artifact is present.
    pub artifact: bool,
    /// `fIncSurfMin` — surface minerals (and, at full detail, population) are
    /// stored.
    pub has_surface_minerals: bool,
    /// `fRouting` — a fleet route destination is stored.
    pub routing: bool,
    /// `fFirstYear` — first year this planet has been in the history file.
    pub first_year: bool,
    /// Mineral concentrations of the crust (detail >= 3).
    pub concentration: Option<Concentration>,
    /// Current environment (detail >= 3).
    pub environment: Option<Environment>,
    /// Original (pre-terraform) environment, when [`terraformed`](Self::terraformed).
    pub original_environment: Option<Environment>,
    /// The owner's own population estimate, in colonists (owned planets).
    ///
    /// Stored on disk as thousands of colonists; multiplied out here.
    pub pop_guess: Option<u32>,
    /// The owner's defense-coverage estimate index (0..=15), when owned.
    pub defense_guess: Option<u8>,
    /// Surface minerals available for mining/shipping, in kilotons (detail >= 4
    /// with surface minerals).
    pub surface_minerals: Option<Minerals>,
    /// Population, in colonists (full detail on an inhabited planet).
    ///
    /// Stored on disk as hundreds of colonists; multiplied out here.
    pub population: Option<u32>,
    /// Installations (mines/factories/defenses/scanner), for an owned
    /// fully-visible planet (`rtPlanetA`).
    pub installations: Option<Installations>,
    /// Starbase details, when a starbase is present and visible.
    pub starbase: Option<Starbase>,
    /// Fleet route destination planet id, when [`routing`](Self::routing) is set.
    pub route_dest: Option<u16>,
}

/// Owner field value that marks an unowned planet.
const OWNER_NONE: u16 = 31;

fn read16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn read32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *d.get(o)?,
        *d.get(o + 1)?,
        *d.get(o + 2)?,
        *d.get(o + 3)?,
    ]))
}

/// Read a little-endian value of `n` (0/1/2/4) bytes at `o`.
fn read_n(d: &[u8], o: usize, n: usize) -> Option<u32> {
    let mut v = 0u32;
    for i in 0..n {
        v |= (*d.get(o + i)? as u32) << (8 * i);
    }
    Some(v)
}

impl PlanetRecord {
    /// Decode a planet block payload. `type_id` is the block's type id (13, 14
    /// or 15), which selects a couple of format variations.
    ///
    /// Returns `None` if the payload is truncated. Optional sections that the
    /// record does not select are left as `None`.
    #[must_use]
    pub fn decode(data: &[u8], type_id: u8) -> Option<Self> {
        let field1 = read16(data, 0)?;
        let id = field1 & 0x07FF;
        let owner_raw = (field1 >> 11) & 0x1F;
        let owner = if owner_raw == OWNER_NONE {
            None
        } else {
            Some(owner_raw as u8)
        };

        let flags = read16(data, 2)?;
        let detail = (flags & 0x7F) as u8;
        let homeworld = (flags >> 7) & 1 != 0;
        let include = (flags >> 8) & 1 != 0;
        let has_starbase = (flags >> 9) & 1 != 0;
        let terraformed = (flags >> 10) & 1 != 0;
        let has_installations = (flags >> 11) & 1 != 0;
        let artifact = (flags >> 12) & 1 != 0;
        let has_surface_minerals = (flags >> 13) & 1 != 0;
        let routing = (flags >> 14) & 1 != 0;
        let first_year = (flags >> 15) & 1 != 0;

        let mut record = Self {
            id,
            owner,
            detail,
            homeworld,
            include,
            has_starbase,
            terraformed,
            has_installations,
            artifact,
            has_surface_minerals,
            routing,
            first_year,
            concentration: None,
            environment: None,
            original_environment: None,
            pop_guess: None,
            defense_guess: None,
            surface_minerals: None,
            population: None,
            installations: None,
            starbase: None,
            route_dest: None,
        };

        let mut index = 4usize;

        // `rtPlanetC` (type 15) carries only the header; everything else is
        // gated behind `det >= 3`.
        if type_id != 15 && detail >= 3 {
            // Concentration-decay bitmask: two bits per mineral. A value of `1`
            // means one countdown byte follows (before the concentrations). We
            // skip over those bytes; the concentration itself follows.
            let bitmask = *data.get(4)?;
            index = 5;
            if bitmask & 0x03 == 1 {
                index += 1;
            }
            if (bitmask >> 2) & 0x03 == 1 {
                index += 1;
            }
            if (bitmask >> 4) & 0x03 == 1 {
                index += 1;
            }

            record.concentration = Some(Concentration {
                ironium: *data.get(index)?,
                boranium: *data.get(index + 1)?,
                germanium: *data.get(index + 2)?,
            });
            index += 3;

            record.environment = Some(Environment {
                gravity: *data.get(index)?,
                temperature: *data.get(index + 1)?,
                radiation: *data.get(index + 2)?,
            });
            index += 3;

            if terraformed {
                record.original_environment = Some(Environment {
                    gravity: *data.get(index)?,
                    temperature: *data.get(index + 1)?,
                    radiation: *data.get(index + 2)?,
                });
                index += 3;
            }

            if owner.is_some() {
                let guess = read16(data, index)?;
                record.pop_guess = Some((u32::from(guess) & 0xFFF) * 1000);
                record.defense_guess = Some((guess >> 12) as u8);
                index += 2;
            }
        }

        if detail >= 4 && has_surface_minerals {
            let contents_lengths = *data.get(index)?;
            index += 1;
            let len_map = [0usize, 1, 2, 4];
            let i_len = len_map[(contents_lengths & 0x03) as usize];
            let b_len = len_map[((contents_lengths >> 2) & 0x03) as usize];
            let g_len = len_map[((contents_lengths >> 4) & 0x03) as usize];
            let pop_len = len_map[((contents_lengths >> 6) & 0x03) as usize];

            let ironium = read_n(data, index, i_len)?;
            index += i_len;
            let boranium = read_n(data, index, b_len)?;
            index += b_len;
            let germanium = read_n(data, index, g_len)?;
            index += g_len;
            record.surface_minerals = Some(Minerals {
                ironium,
                boranium,
                germanium,
            });

            if detail == 7 && pop_len > 0 {
                let pop = read_n(data, index, pop_len)?;
                index += pop_len;
                record.population = Some(pop * 100);
            }
        }

        if type_id == 13 && has_installations {
            let low = read32(data, index)?;
            let high = read32(data, index + 4)?;
            record.installations = Some(Installations {
                delta_pop: (low & 0xFF) as u8,
                mines: ((low >> 8) & 0xFFF) as u16,
                factories: ((low >> 20) & 0xFFF) as u16,
                defenses: (high & 0xFFF) as u16,
                scanner: ((high >> 12) & 0x1F) as u8,
                artifact: (high >> 22) & 1 != 0,
                no_research: (high >> 23) & 1 != 0,
            });
            index += 8;
        }

        if has_starbase && owner.is_some() {
            if type_id == 14 {
                let sb = *data.get(index)?;
                record.starbase = Some(Starbase {
                    design: sb & 0x0F,
                    damage_pct: 0,
                    fling_dest: 0,
                    warp: 0,
                    no_heal: false,
                });
                index += 1;
            } else {
                let field = read32(data, index)?;
                record.starbase = Some(Starbase {
                    design: (field & 0x0F) as u8,
                    damage_pct: ((field >> 4) & 0xFFF) as u16,
                    fling_dest: ((field >> 16) & 0x3FF) as u16,
                    warp: ((field >> 26) & 0x0F) as u8,
                    no_heal: (field >> 30) & 1 != 0,
                });
                index += 4;
            }
        }

        if owner.is_some() && routing {
            let route = read16(data, index)? & 0x03FF;
            record.route_dest = Some(route);
            // index += 2; // last field consumed
        }

        Some(record)
    }
}

/// Decode every planet block (types 13/14/15) in a decrypted [`StarsFile`], in
/// file order.
///
/// For a host file this yields one [`PlanetRecord`] per galaxy planet.
#[must_use]
pub fn planet_records(file: &StarsFile) -> Vec<PlanetRecord> {
    file.blocks
        .iter()
        .filter_map(|b| {
            let type_id = match b.block_type() {
                BlockType::Planet => 13u8,
                BlockType::PartialPlanet => 14,
                BlockType::MinimalPlanet => 15,
                _ => return None,
            };
            PlanetRecord::decode(&b.data, type_id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal (unowned, concentration+environment only) planet block
    /// payload for id `id`: header word, flags with `det=3`, a zero decay
    /// bitmask, then 3 concentration + 3 environment bytes.
    fn minimal_payload(id: u16) -> Vec<u8> {
        let field1 = id | (OWNER_NONE << 11);
        let flags: u16 = 3; // det=3, no flag bits
        let mut v = Vec::new();
        v.extend_from_slice(&field1.to_le_bytes());
        v.extend_from_slice(&flags.to_le_bytes());
        v.push(0); // decay bitmask: no countdown bytes
        v.extend_from_slice(&[30, 40, 50]); // concentrations
        v.extend_from_slice(&[60, 70, 80]); // environment
        v
    }

    #[test]
    fn decodes_unowned_minimal_planet() {
        let p = PlanetRecord::decode(&minimal_payload(5), 13).unwrap();
        assert_eq!(p.id, 5);
        assert_eq!(p.owner, None);
        assert_eq!(p.detail, 3);
        assert_eq!(
            p.concentration,
            Some(Concentration {
                ironium: 30,
                boranium: 40,
                germanium: 50
            })
        );
        assert_eq!(
            p.environment,
            Some(Environment {
                gravity: 60,
                temperature: 70,
                radiation: 80
            })
        );
        assert_eq!(p.population, None);
        assert_eq!(p.installations, None);
    }

    #[test]
    fn rejects_truncated_payload() {
        assert!(PlanetRecord::decode(&[0x00], 13).is_none());
        assert!(PlanetRecord::decode(&[], 13).is_none());
    }
}
