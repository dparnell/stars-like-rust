//! Typed decoder for the space-object ("thing") records (`rtThing`, type id
//! 43) that appear in `.hst`, `.mN` and `.hN` files.
//!
//! A *thing* is any non-planet, non-fleet space object: a minefield, a mineral
//! packet in flight, a wormhole, or a mystery trader. They are written as a
//! small two-part section:
//!
//! 1. one `rtThing` record with `cb = 2` whose payload is a little-endian
//!    `u16` **count**, then
//! 2. that many `rtThing` records with `cb = 18` (`sizeof(THING)`), each a full
//!    [`THING`] struct.
//!
//! This layout was recovered from the reconstructed Stars! source
//! (`sirgwain/stars-decompile`, `file.c` "Load things" / `save.c` "Count and
//! write things") and the NB09 debug structs (`THING`, `THMINE`, `THPACK`,
//! `THWORM`, `THTRADER`, and the `ThingType` enum), then verified byte-for-byte
//! against the real sample games: every count record in the fixtures is
//! followed by exactly `count` 18-byte thing records (see `docs/formats/thing.md`).
//!
//! Like the other record decoders this is a read-only *interpreted view*;
//! byte-exact write-back still goes through the container in [`crate::file`].

use crate::block::BlockType;
use crate::file::StarsFile;

/// The block type id shared by the thing count record and the thing records
/// (`rtThing` / `rtLogThingByteParam`).
pub const THING_BLOCK: u8 = 43;

/// The on-disk size of a full `THING` record, in bytes (`sizeof(THING)`).
pub const THING_SIZE: usize = 18;

/// The object subtype (`ith`, the top 3 bits of `idFull`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThingType {
    /// A minefield (`ithMinefield = 0`).
    Minefield,
    /// A mineral packet in flight (`ithMineralPacket = 1`).
    MineralPacket,
    /// A wormhole (`ithWormhole = 2`).
    Wormhole,
    /// A mystery trader (`ithMysteryTrader = 3`).
    MysteryTrader,
    /// An unrecognised subtype id.
    Unknown(u8),
}

impl ThingType {
    /// Classify a raw 3-bit `ith` value.
    #[must_use]
    pub fn from_ith(ith: u8) -> Self {
        match ith {
            0 => Self::Minefield,
            1 => Self::MineralPacket,
            2 => Self::Wormhole,
            3 => Self::MysteryTrader,
            other => Self::Unknown(other),
        }
    }
}

/// A minefield (`THMINE`, the +0x06 union of a minefield [`Thing`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Minefield {
    /// Number of mines (`cMines`); the field's area scales with this.
    pub mines: i32,
    /// Bitmask of players who have detected the field (`grbitPlr`).
    pub players_seen: u16,
    /// Mine technology (`iType`): 0 = standard, 1 = heavy, 2 = speed-bump.
    pub kind: u8,
    /// Whether the field is set to detonate (`fDetonate`).
    pub detonate: bool,
    /// Bitmask of players who can currently see the field (`grbitPlrNow`).
    pub players_seen_now: u16,
}

/// A mineral packet in flight (`THPACK`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MineralPacket {
    /// Target planet id (`idPlanet`, 10 bits).
    pub target_planet: u16,
    /// Warp speed the packet is travelling at (`iWarp`, 4 bits).
    pub warp: u8,
    /// Whether the packet has moved this turn (`fMoved`).
    pub moved: bool,
    /// Whether the packet is included in this player's view (`fInclude`).
    pub include: bool,
    /// Cargo, in kilotons, as `[ironium, boranium, germanium]` (`rgwtMin`).
    pub minerals: [i16; 3],
    /// Maximum mass carried (`wtMax`, 14 bits) — used to derive decay/damage.
    pub mass_max: u16,
    /// Decay-rate class (`iDecayRate`, 2 bits).
    pub decay_rate: u8,
}

/// A wormhole (`THWORM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wormhole {
    /// Stability class (`iStable`, 2 bits).
    pub stability: u8,
    /// Turns since the wormhole last jumped (`cLastMove`, 10 bits).
    pub last_move: u16,
    /// Whether this player knows the destination endpoint (`fDestKnown`).
    pub dest_known: bool,
    /// Whether the wormhole is included in this player's view (`fInclude`).
    pub include: bool,
    /// Bitmask of players who have detected the wormhole (`grbitPlr`).
    pub players_seen: u16,
    /// Bitmask of players who have traversed it (`grbitPlrTrav`).
    pub players_traversed: u16,
    /// The `idFull` of the partner (far) endpoint (`idPartner`).
    pub partner_id: u16,
}

/// A mystery trader (`THTRADER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MysteryTrader {
    /// Destination x (`ptDest.x`).
    pub dest_x: i16,
    /// Destination y (`ptDest.y`).
    pub dest_y: i16,
    /// Warp speed (`iWarp`, 4 bits).
    pub warp: u8,
    /// Whether the trader is included in this player's view (`fInclude`).
    pub include: bool,
    /// Bitmask of players who have detected the trader (`grbitPlr`).
    pub players_seen: u16,
    /// Bitmask of players who have met/traded with the trader (`grbitTrader`).
    pub players_met: u16,
}

/// The decoded subtype payload of a [`Thing`], selected by its `ith`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThingKind {
    /// A minefield.
    Minefield(Minefield),
    /// A mineral packet.
    MineralPacket(MineralPacket),
    /// A wormhole.
    Wormhole(Wormhole),
    /// A mystery trader.
    MysteryTrader(MysteryTrader),
    /// An unrecognised subtype; the raw 10 union bytes are preserved.
    Unknown([u8; 10]),
}

/// A decoded space object (`THING`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thing {
    /// Object id within its subtype (`id`, low 9 bits of `idFull`).
    pub id: u16,
    /// Owning / originating player (`iplr`, 4 bits).
    pub player: u8,
    /// Raw subtype id (`ith`, 3 bits).
    pub ith: u8,
    /// The classified subtype.
    pub thing_type: ThingType,
    /// Current x position (`pt.x`).
    pub x: i16,
    /// Current y position (`pt.y`).
    pub y: i16,
    /// The subtype-specific payload.
    pub kind: ThingKind,
    /// Turn the object was last updated (`turn`).
    pub turn: u16,
}

fn u16le(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}

fn i16le(d: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([d[o], d[o + 1]])
}

fn i32le(d: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

impl Thing {
    /// Decode a **decrypted** full `THING` payload (18 bytes).
    ///
    /// Returns `None` if the payload is not exactly [`THING_SIZE`] bytes.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != THING_SIZE {
            return None;
        }
        let id_full = u16le(data, 0);
        let id = id_full & 0x01FF;
        let player = ((id_full >> 9) & 0x0F) as u8;
        let ith = ((id_full >> 13) & 0x07) as u8;
        let x = i16le(data, 2);
        let y = i16le(data, 4);
        // The 10-byte subtype union sits at +0x06.
        let u = &data[6..16];
        let kind = match ith {
            0 => ThingKind::Minefield(Minefield {
                mines: i32le(u, 0),
                players_seen: u16le(u, 4),
                kind: u[6],
                detonate: u[7] != 0,
                players_seen_now: u16le(u, 8),
            }),
            1 => {
                let w0 = u16le(u, 0);
                let w8 = u16le(u, 8);
                ThingKind::MineralPacket(MineralPacket {
                    target_planet: w0 & 0x03FF,
                    warp: ((w0 >> 10) & 0x0F) as u8,
                    moved: (w0 >> 14) & 1 != 0,
                    include: (w0 >> 15) & 1 != 0,
                    minerals: [i16le(u, 2), i16le(u, 4), i16le(u, 6)],
                    mass_max: w8 & 0x3FFF,
                    decay_rate: ((w8 >> 14) & 0x03) as u8,
                })
            }
            2 => {
                let w0 = u16le(u, 0);
                ThingKind::Wormhole(Wormhole {
                    stability: (w0 & 0x03) as u8,
                    last_move: (w0 >> 2) & 0x03FF,
                    dest_known: (w0 >> 12) & 1 != 0,
                    include: (w0 >> 13) & 1 != 0,
                    players_seen: u16le(u, 2),
                    players_traversed: u16le(u, 4),
                    partner_id: u16le(u, 6),
                })
            }
            3 => {
                let w4 = u16le(u, 4);
                ThingKind::MysteryTrader(MysteryTrader {
                    dest_x: i16le(u, 0),
                    dest_y: i16le(u, 2),
                    warp: (w4 & 0x0F) as u8,
                    include: (w4 >> 4) & 1 != 0,
                    players_seen: u16le(u, 6),
                    players_met: u16le(u, 8),
                })
            }
            _ => {
                let mut raw = [0u8; 10];
                raw.copy_from_slice(u);
                ThingKind::Unknown(raw)
            }
        };
        Some(Self {
            id,
            player,
            ith,
            thing_type: ThingType::from_ith(ith),
            x,
            y,
            kind,
            turn: u16le(data, 16),
        })
    }
}

/// The thing section of a file: the declared count and the decoded objects.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThingSection {
    /// The count declared by the leading 2-byte `rtThing` record.
    pub count: u16,
    /// The decoded 18-byte thing records that follow it.
    pub things: Vec<Thing>,
}

/// Decode the space-object section of a decrypted [`StarsFile`].
///
/// Scans for the 2-byte `rtThing` count record and decodes the `count` full
/// 18-byte `THING` records that follow it. Files with no objects (no count
/// record) yield an empty section.
#[must_use]
pub fn thing_section(file: &StarsFile) -> ThingSection {
    let blocks = &file.blocks;
    for (i, b) in blocks.iter().enumerate() {
        if b.block_type() == BlockType::Object && b.data.len() == 2 {
            let count = u16le(&b.data, 0);
            let things = blocks[i + 1..]
                .iter()
                .take_while(|b| b.block_type() == BlockType::Object && b.data.len() == THING_SIZE)
                .filter_map(|b| Thing::decode(&b.data))
                .collect();
            return ThingSection { count, things };
        }
    }
    ThingSection::default()
}

/// Decode every full 18-byte `THING` record in a decrypted [`StarsFile`], in
/// file order (ignoring the leading count record).
#[must_use]
pub fn thing_records(file: &StarsFile) -> Vec<Thing> {
    file.blocks
        .iter()
        .filter(|b| b.block_type() == BlockType::Object && b.data.len() == THING_SIZE)
        .filter_map(|b| Thing::decode(&b.data))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thing_bytes(id_full: u16, x: i16, y: i16, union: [u8; 10], turn: u16) -> Vec<u8> {
        let mut v = Vec::with_capacity(THING_SIZE);
        v.extend_from_slice(&id_full.to_le_bytes());
        v.extend_from_slice(&x.to_le_bytes());
        v.extend_from_slice(&y.to_le_bytes());
        v.extend_from_slice(&union);
        v.extend_from_slice(&turn.to_le_bytes());
        v
    }

    #[test]
    fn decodes_minefield() {
        // ith = 0 (minefield); id = 5, player = 1.
        let id_full = 5 | (1 << 9);
        let mut u = [0u8; 10];
        u[0..4].copy_from_slice(&1234i32.to_le_bytes()); // cMines
        u[4..6].copy_from_slice(&0x0006u16.to_le_bytes()); // grbitPlr
        u[6] = 1; // iType = heavy
        u[7] = 1; // fDetonate
        u[8..10].copy_from_slice(&0x0002u16.to_le_bytes()); // grbitPlrNow
        let t = Thing::decode(&thing_bytes(id_full, 100, 200, u, 2437)).unwrap();
        assert_eq!(t.id, 5);
        assert_eq!(t.player, 1);
        assert_eq!(t.thing_type, ThingType::Minefield);
        assert_eq!(t.x, 100);
        assert_eq!(t.y, 200);
        assert_eq!(t.turn, 2437);
        assert_eq!(
            t.kind,
            ThingKind::Minefield(Minefield {
                mines: 1234,
                players_seen: 6,
                kind: 1,
                detonate: true,
                players_seen_now: 2,
            })
        );
    }

    #[test]
    fn decodes_wormhole() {
        // ith = 2 (wormhole).
        let id_full = 3 | (2 << 13);
        let mut u = [0u8; 10];
        // w0: iStable=1, cLastMove=7, fDestKnown=1, fInclude=1
        let w0: u16 = 1 | (7 << 2) | (1 << 12) | (1 << 13);
        u[0..2].copy_from_slice(&w0.to_le_bytes());
        u[2..4].copy_from_slice(&0x00ffu16.to_le_bytes()); // grbitPlr
        u[4..6].copy_from_slice(&0x0001u16.to_le_bytes()); // grbitPlrTrav
        u[6..8].copy_from_slice(&42u16.to_le_bytes()); // idPartner
        let t = Thing::decode(&thing_bytes(id_full, -1, -1, u, 2440)).unwrap();
        assert_eq!(t.thing_type, ThingType::Wormhole);
        assert_eq!(
            t.kind,
            ThingKind::Wormhole(Wormhole {
                stability: 1,
                last_move: 7,
                dest_known: true,
                include: true,
                players_seen: 0x00ff,
                players_traversed: 1,
                partner_id: 42,
            })
        );
    }

    #[test]
    fn rejects_wrong_size() {
        assert!(Thing::decode(&[0u8; 17]).is_none());
        assert!(Thing::decode(&[0u8; 19]).is_none());
        assert!(Thing::decode(&[]).is_none());
    }
}
