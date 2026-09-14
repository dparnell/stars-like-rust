//! Typed decoder for the **battle recording** (VCR) block, type id 31
//! (`rtBtlData`), and its continuations (type 39, `rtContinue`).
//!
//! A battle recording is what the in-game VCR plays back: the tokens that took
//! part, where each stood, and then a stream of action records saying who moved
//! where, who shot whom and what died. It is the only place the game writes
//! down what combat actually did, which makes it the reference a combat
//! implementation can be checked against.
//!
//! ## Layout
//!
//! The record is a `BTLDATA` header, then `ctok` fixed-size tokens, then a
//! variable-length stream of action records that runs to `cbData`:
//!
//! ```text
//! BTLDATA        14 bytes
//! TOK[ctok]      29 bytes each
//! BTLREC ...     6 bytes + 8 per kill, repeated until cbData is reached
//! ```
//!
//! ### `BTLDATA` (14 bytes)
//!
//! | Offset | Size | Field | Meaning |
//! |-------:|-----:|-------|---------|
//! | 0 | 2 | `id` | battle id; the high byte counts up per battle in a turn |
//! | 2 | 1 | `cplr` | number of players involved |
//! | 3 | 1 | `ctok` | number of tokens |
//! | 4 | 2 | `grfPlr` | bitmask of the players involved |
//! | 6 | 2 | `cbData` | **total** size of the whole record, header included |
//! | 8 | 2 | `idPlanet` | planet the battle happened at |
//! | 10 | 4 | `pt` | location, two `int16` |
//!
//! ### `TOK` (29 bytes)
//!
//! One per participating stack. Carries the owner, the design, the starting
//! square, initiative, the cloaking/jamming/beam-defence percentages, mass,
//! shields, ship count, and the battle-plan tactics.
//!
//! ### `BTLREC` (6 bytes, plus 8 per kill)
//!
//! | Offset | Size | Field | Meaning |
//! |-------:|-----:|-------|---------|
//! | 0 | 1 | `itok` | the token acting |
//! | 1 | 1 | `brcDest` | destination square, `y << 4 \| x`; `0xFF` means the token left the battle |
//! | 2 | 2 | `ctok` | **number of `KILL` records that follow** |
//! | 4 | 2 | `iRound:4, dzDis:4, itokAttack:8` | round, range, and target token |
//!
//! Records are walked by advancing `6 + ctok * 8` bytes at a time until
//! `cbData` is reached (`CBattleKills` in the original does exactly this).
//!
//! ### `KILL` (8 bytes)
//!
//! | Offset | Size | Field |
//! |-------:|-----:|-------|
//! | 0 | 1 | `itok` — the token damaged |
//! | 1 | 1 | `grfWeapon` — weapon class flags |
//! | 2 | 2 | `cshKill` — ships destroyed |
//! | 4 | 2 | `dpShield` — shield damage |
//! | 6 | 2 | `dv` — `pctSh:7, pctDp:9`, remaining shields and damage |
//!
//! ## Large battles
//!
//! A record of 1024 bytes or more is split: the first block (type 31) carries
//! the header and as many tokens as fit, and the rest follows in `rtContinue`
//! blocks (type 39). [`battle_records`] stitches those back together, so a
//! caller sees one recording either way.
//!
//! Sources: the NB09 `BTLDATA`, `TOK`, `BTLREC` and `KILL` structs; the writer
//! in `save.c`; and `CBattleKills` in `vcr.c` for the record walk.
//! Layout notes in `docs/formats/battle.md`.

use crate::block::Block;
use crate::file::StarsFile;

/// Size of the fixed `BTLDATA` header.
pub const HEADER_LEN: usize = 14;
/// Size of one `TOK` entry.
pub const TOKEN_LEN: usize = 29;
/// Size of one `BTLREC` action record, before its kills.
pub const ACTION_LEN: usize = 6;
/// Size of one `KILL` entry.
pub const KILL_LEN: usize = 8;

/// Block type id of a battle recording.
pub const BATTLE_BLOCK: u8 = 31;
/// Block type id of a battle-recording continuation.
pub const CONTINUE_BLOCK: u8 = 39;

/// A square on the 16x16 battle board (only 10x10 is used).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Square {
    /// Column, 0..=15.
    pub x: u8,
    /// Row, 0..=15.
    pub y: u8,
}

impl Square {
    /// Decode the packed `brc` byte: `y << 4 | x`.
    #[must_use]
    pub fn from_brc(brc: u8) -> Self {
        Self {
            x: brc & 0x0f,
            y: brc >> 4,
        }
    }

    /// Re-pack to the stored byte.
    #[must_use]
    pub fn to_brc(self) -> u8 {
        ((self.y & 0x0f) << 4) | (self.x & 0x0f)
    }
}

/// One stack of ships (or a starbase) taking part in a battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleToken {
    /// Fleet or planet id this token came from.
    pub id: u16,
    /// Owning player.
    pub player: u8,
    /// Object class (`1` planet, `2` fleet).
    pub object_class: u8,
    /// Design slot; `>= 16` marks a starbase.
    pub design: u8,
    /// Starting square.
    pub square: Square,
    /// Base initiative from the hull.
    pub initiative_base: u8,
    /// Lowest and highest initiative this token fires at.
    pub initiative_min: u8,
    /// Highest initiative this token fires at.
    pub initiative_max: u8,
    /// Token this one is targeting.
    pub target: u8,
    /// Cloaking, as a percentage.
    pub pct_cloak: u8,
    /// Torpedo jamming, as a percentage.
    pub pct_jam: u8,
    /// Battle-computer accuracy bonus, as a percentage.
    pub pct_computer: u8,
    /// Capacitor damage bonus, as a percentage.
    pub pct_capacitor: u8,
    /// Beam deflection, as a percentage.
    pub pct_beam_defence: u8,
    /// Mass of one ship, in kT.
    pub mass: u16,
    /// Shield points **per ship**; the stack's pool is this times [`ships`](Self::ships),
    /// which is how `FDamageTok` computes what beams have to strip.
    pub shields: u16,
    /// Number of ships in the stack.
    pub ships: u16,
    /// Packed damage value: `pctSh:7, pctDp:9`.
    pub damage: u16,
    /// Packed battle-plan tactics: `mdTarget1:4, mdTarget2:4, mdTactic:4, mdTarget0:4`.
    pub tactics: u16,
    /// Packed movement limits: `dxyLim:4, dxyMax:4, spd:4, cTarget:4`.
    pub movement: u16,
    /// Packed status flags: `fActive:1, fDetector:1, fTorp:1, fRegen:1, fMoved:1, dzDis:5, dwt:4, dMovesLeft:2`.
    pub flags: u16,
}

impl BattleToken {
    /// Whether this token is a starbase (design slots 16 and up).
    #[must_use]
    pub fn is_starbase(self) -> bool {
        self.design >= 16
    }

    /// Battle speed, in half-squares per round, from the packed movement word.
    #[must_use]
    pub fn speed(self) -> u8 {
        ((self.movement >> 8) & 0x0f) as u8
    }

    /// The battle-plan tactic, from bits 8..12 of the packed tactics word.
    #[must_use]
    pub fn tactic(self) -> u8 {
        ((self.tactics >> 8) & 0x0f) as u8
    }

    /// Primary target class, from the low nibble of the tactics word.
    #[must_use]
    pub fn primary_target(self) -> u8 {
        (self.tactics & 0x0f) as u8
    }

    /// Secondary target class, from the next nibble.
    #[must_use]
    pub fn secondary_target(self) -> u8 {
        ((self.tactics >> 4) & 0x0f) as u8
    }

    /// The token's weapon reach limit (`dxyLim`), from the low nibble of the
    /// packed movement word.
    #[must_use]
    pub fn weapon_reach(self) -> u8 {
        (self.movement & 0x0f) as u8
    }

    /// The reach the token prefers to fight at (`dxyMax`), from the next
    /// nibble.
    #[must_use]
    pub fn preferred_reach(self) -> u8 {
        ((self.movement >> 4) & 0x0f) as u8
    }

    /// The token's own class, from the top nibble of the tactics word, which
    /// is what other tokens' target filters match against.
    #[must_use]
    pub fn target_class(self) -> u8 {
        ((self.tactics >> 12) & 0x0f) as u8
    }

    /// Squares of movement still available this round (`dMovesLeft`).
    #[must_use]
    pub fn moves_left(self) -> u8 {
        ((self.flags >> 14) & 0x03) as u8
    }
}

/// Damage one token did to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kill {
    /// The token that took the damage.
    pub token: u8,
    /// Weapon-class flags.
    pub weapon: u8,
    /// Ships destroyed.
    pub ships_killed: u16,
    /// Shield damage done.
    pub shield_damage: u16,
    /// Packed remaining damage: `pctSh:7, pctDp:9`.
    pub damage: u16,
}

/// The `brcDest` value that marks a token leaving the battle rather than
/// moving to a square.
pub const BRC_DEPARTED: u8 = 0xff;

/// Which layout an action record uses.
///
/// The game has two, and the binary's own debug symbols name both: `BTLREC`
/// and `BTLREC26`. They are the same six bytes but divide the middle word
/// differently, so a record read with the wrong one parses into nonsense —
/// impossible target ids, shots at absurd ranges, tokens crossing the board in
/// a single round.
///
/// Which applies is decided by the **file version**, not by anything in the
/// record. `fixtures/games/exodus` is version 2.81 and uses [`Self::Modern`];
/// `fixtures/games/all-computer-players` is 2.66 and uses [`Self::V26`], which
/// is what the `26` in the symbol name refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionLayout {
    /// `BTLREC`: `itok`, `brcDest`, `ctok` as a 16-bit count, then the packed
    /// round/range/target word.
    Modern,
    /// `BTLREC26`: `itok`, `brcDest`, `itokAttack`, `ctok` as a *byte*, then
    /// the round word. The attacked token has its own byte here rather than
    /// riding in the top half of the packed word.
    V26,
}

impl ActionLayout {
    /// The layout a file of this version uses.
    ///
    /// The boundary is not pinned to a specific release: 2.66 uses [`Self::V26`]
    /// and 2.81 uses [`Self::Modern`], and no fixture sits between them.
    #[must_use]
    pub fn for_version(major: u16, minor: u16) -> Self {
        if (major, minor) < (2, 70) {
            Self::V26
        } else {
            Self::Modern
        }
    }
}

/// One action in a battle: a move, a shot, or both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleAction {
    /// The token acting.
    pub token: u8,
    /// Where it moved to, or `None` if it left the battle.
    ///
    /// A token that disengages is recorded with `brcDest` = `0xFF`, which is
    /// not a square: taken literally it would read as (15,15), off the 10x10
    /// board. Of the 941 actions in the Exodus recordings, 31 are this
    /// sentinel and no other off-board value occurs.
    pub destination: Option<Square>,
    /// Battle round, 0..=15.
    pub round: u8,
    /// Range to the target, in squares.
    pub range: u8,
    /// The token being attacked.
    pub target: u8,
    /// Damage dealt by this action.
    pub kills: Vec<Kill>,
}

/// A complete battle recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleRecord {
    /// Battle id.
    pub id: u16,
    /// Number of players involved.
    pub players: u8,
    /// Bitmask of the players involved.
    pub player_mask: u16,
    /// Planet the battle took place at.
    pub planet: u16,
    /// Location in the universe.
    pub position: (i16, i16),
    /// The participating stacks, in the order actions index them.
    pub tokens: Vec<BattleToken>,
    /// What happened, in order.
    pub actions: Vec<BattleAction>,
    /// Bytes the record declared (`cbData`).
    pub declared_len: u16,
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn i16_at(d: &[u8], o: usize) -> Option<i16> {
    Some(i16::from_le_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

impl BattleToken {
    fn decode(d: &[u8]) -> Option<Self> {
        Some(Self {
            id: u16_at(d, 0)?,
            player: *d.get(2)?,
            object_class: *d.get(3)?,
            design: *d.get(4)?,
            square: Square::from_brc(*d.get(5)?),
            initiative_base: *d.get(6)?,
            initiative_min: *d.get(7)?,
            initiative_max: *d.get(8)?,
            target: *d.get(9)?,
            pct_cloak: *d.get(10)?,
            pct_jam: *d.get(11)?,
            pct_computer: *d.get(12)?,
            pct_capacitor: *d.get(13)?,
            pct_beam_defence: *d.get(14)?,
            mass: u16_at(d, 15)?,
            shields: u16_at(d, 17)?,
            ships: u16_at(d, 19)?,
            damage: u16_at(d, 21)?,
            tactics: u16_at(d, 23)?,
            movement: u16_at(d, 25)?,
            flags: u16_at(d, 27)?,
        })
    }
}

impl BattleRecord {
    /// Decode one complete battle recording from its reassembled bytes.
    ///
    /// Returns `None` if the payload is truncated or internally inconsistent —
    /// for instance if an action record's kill count would run past the end.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        Self::decode_with(data, ActionLayout::Modern)
    }

    /// Decode a battle block using a named action layout.
    ///
    /// Prefer [`battle_records`], which picks the layout from the file's own
    /// version header.
    #[must_use]
    pub fn decode_with(data: &[u8], layout: ActionLayout) -> Option<Self> {
        if data.len() < HEADER_LEN {
            return None;
        }
        let token_count = usize::from(*data.get(3)?);
        let declared_len = u16_at(data, 6)?;

        // The record declares its own length; trust the shorter of that and
        // what we actually have, so a truncated tail cannot over-read.
        let end = usize::from(declared_len).min(data.len());
        let tokens_end = HEADER_LEN.checked_add(token_count.checked_mul(TOKEN_LEN)?)?;
        if tokens_end > end {
            return None;
        }

        let mut tokens = Vec::with_capacity(token_count);
        for i in 0..token_count {
            let at = HEADER_LEN + i * TOKEN_LEN;
            tokens.push(BattleToken::decode(&data[at..])?);
        }

        // Walk the action stream: 6 bytes, then 8 per kill, until the declared
        // end. This mirrors CBattleKills in the original.
        let mut actions = Vec::new();
        let mut at = tokens_end;
        while at + ACTION_LEN <= end {
            let (kill_count, packed_target) = match layout {
                ActionLayout::Modern => (usize::from(u16_at(data, at + 2)?), None),
                ActionLayout::V26 => (usize::from(*data.get(at + 3)?), Some(*data.get(at + 2)?)),
            };
            let record_len = ACTION_LEN.checked_add(kill_count.checked_mul(KILL_LEN)?)?;
            if at.checked_add(record_len)? > end {
                return None;
            }
            let packed = u16_at(data, at + 4)?;

            let mut kills = Vec::with_capacity(kill_count);
            for k in 0..kill_count {
                let o = at + ACTION_LEN + k * KILL_LEN;
                kills.push(Kill {
                    token: *data.get(o)?,
                    weapon: *data.get(o + 1)?,
                    ships_killed: u16_at(data, o + 2)?,
                    shield_damage: u16_at(data, o + 4)?,
                    damage: u16_at(data, o + 6)?,
                });
            }

            let brc_dest = *data.get(at + 1)?;
            actions.push(BattleAction {
                token: *data.get(at)?,
                destination: (brc_dest != BRC_DEPARTED).then(|| Square::from_brc(brc_dest)),
                round: (packed & 0x0f) as u8,
                range: ((packed >> 4) & 0x0f) as u8,
                target: packed_target.unwrap_or((packed >> 8) as u8),
                kills,
            });
            at += record_len;
        }

        Some(Self {
            id: u16_at(data, 0)?,
            players: *data.get(2)?,
            player_mask: u16_at(data, 4)?,
            planet: u16_at(data, 8)?,
            position: (i16_at(data, 10)?, i16_at(data, 12)?),
            tokens,
            actions,
            declared_len,
        })
    }

    /// Encode the recording in the [`ActionLayout::Modern`] layout, as one
    /// byte string: the fourteen-byte header, the tokens, then the actions
    /// with their kills. `declared_len` is written as the length of what
    /// is produced, whatever the field says.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            HEADER_LEN + self.tokens.len() * TOKEN_LEN + self.actions.len() * ACTION_LEN,
        );
        out.extend_from_slice(&self.id.to_le_bytes());
        out.push(self.players);
        out.push(u8::try_from(self.tokens.len()).unwrap_or(u8::MAX));
        out.extend_from_slice(&self.player_mask.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&self.planet.to_le_bytes());
        out.extend_from_slice(&self.position.0.to_le_bytes());
        out.extend_from_slice(&self.position.1.to_le_bytes());
        for t in &self.tokens {
            out.extend_from_slice(&t.id.to_le_bytes());
            out.push(t.player);
            out.push(t.object_class);
            out.push(t.design);
            out.push(t.square.to_brc());
            out.push(t.initiative_base);
            out.push(t.initiative_min);
            out.push(t.initiative_max);
            out.push(t.target);
            out.push(t.pct_cloak);
            out.push(t.pct_jam);
            out.push(t.pct_computer);
            out.push(t.pct_capacitor);
            out.push(t.pct_beam_defence);
            out.extend_from_slice(&t.mass.to_le_bytes());
            out.extend_from_slice(&t.shields.to_le_bytes());
            out.extend_from_slice(&t.ships.to_le_bytes());
            out.extend_from_slice(&t.damage.to_le_bytes());
            out.extend_from_slice(&t.tactics.to_le_bytes());
            out.extend_from_slice(&t.movement.to_le_bytes());
            out.extend_from_slice(&t.flags.to_le_bytes());
        }
        for a in &self.actions {
            out.push(a.token);
            out.push(a.destination.map_or(BRC_DEPARTED, Square::to_brc));
            out.extend_from_slice(
                &u16::try_from(a.kills.len())
                    .unwrap_or(u16::MAX)
                    .to_le_bytes(),
            );
            let packed = u16::from(a.round & 0x0f)
                | (u16::from(a.range & 0x0f) << 4)
                | (u16::from(a.target) << 8);
            out.extend_from_slice(&packed.to_le_bytes());
            for k in &a.kills {
                out.push(k.token);
                out.push(k.weapon);
                out.extend_from_slice(&k.ships_killed.to_le_bytes());
                out.extend_from_slice(&k.shield_damage.to_le_bytes());
                out.extend_from_slice(&k.damage.to_le_bytes());
            }
        }
        let len = u16::try_from(out.len()).unwrap_or(u16::MAX).to_le_bytes();
        out[6] = len[0];
        out[7] = len[1];
        out
    }

    /// Total ships destroyed in this battle.
    #[must_use]
    pub fn ships_destroyed(&self) -> u32 {
        self.actions
            .iter()
            .flat_map(|a| a.kills.iter())
            .map(|k| u32::from(k.ships_killed))
            .sum()
    }

    /// Ships destroyed belonging to one player.
    #[must_use]
    pub fn ships_destroyed_for(&self, player: u8) -> u32 {
        self.actions
            .iter()
            .flat_map(|a| a.kills.iter())
            .filter(|k| {
                self.tokens
                    .get(usize::from(k.token))
                    .is_some_and(|t| t.player == player)
            })
            .map(|k| u32::from(k.ships_killed))
            .sum()
    }

    /// The players that took part, from the bitmask.
    #[must_use]
    pub fn participants(&self) -> Vec<u8> {
        (0..16u8)
            .filter(|p| self.player_mask & (1 << p) != 0)
            .collect()
    }
}

/// Decode every battle recording in a slice of blocks.
///
/// A recording of 1024 bytes or more is split across one type-31 block and one
/// or more type-39 continuations; those are reassembled here, so each returned
/// record is complete.
#[must_use]
pub fn battle_records_in(blocks: &[Block]) -> Vec<BattleRecord> {
    battle_records_in_with(blocks, ActionLayout::Modern)
}

/// As [`battle_records_in`], but with the action layout named explicitly.
#[must_use]
pub fn battle_records_in_with(blocks: &[Block], layout: ActionLayout) -> Vec<BattleRecord> {
    let mut out = Vec::new();
    let mut pending: Option<Vec<u8>> = None;

    for block in blocks {
        match block.type_id {
            BATTLE_BLOCK => {
                if let Some(bytes) = pending.take() {
                    if let Some(record) = BattleRecord::decode_with(&bytes, layout) {
                        out.push(record);
                    }
                }
                pending = Some(block.data.clone());
            }
            CONTINUE_BLOCK => {
                if let Some(bytes) = pending.as_mut() {
                    bytes.extend_from_slice(&block.data);
                }
            }
            _ => {}
        }
    }
    if let Some(bytes) = pending {
        if let Some(record) = BattleRecord::decode_with(&bytes, layout) {
            out.push(record);
        }
    }
    out
}

/// Decode every battle recording in a file.
///
/// When a file holds several concatenated turns this reads all of them; use
/// [`battle_records_in`] with [`StarsFile::segment_blocks`] for one turn.
#[must_use]
pub fn battle_records(file: &StarsFile) -> Vec<BattleRecord> {
    let header = &file.latest_segment().header;
    let layout = ActionLayout::for_version(header.version_major, header.version_minor);
    battle_records_in_with(&file.blocks, layout)
}

#[cfg(test)]
mod encode_tests {
    use super::*;

    /// A recording survives the trip through its bytes.
    #[test]
    fn a_recording_round_trips() {
        let record = BattleRecord {
            id: 0x101,
            players: 2,
            player_mask: 0b11,
            planet: 10,
            position: (1149, 1135),
            tokens: vec![
                BattleToken {
                    id: 0x200,
                    player: 0,
                    object_class: 2,
                    design: 3,
                    square: Square { x: 1, y: 4 },
                    initiative_base: 5,
                    initiative_min: 5,
                    initiative_max: 9,
                    target: 0xff,
                    pct_cloak: 0,
                    pct_jam: 10,
                    pct_computer: 0,
                    pct_capacitor: 0,
                    pct_beam_defence: 100,
                    mass: 120,
                    shields: 40,
                    ships: 3,
                    damage: 0,
                    tactics: 0x3511,
                    movement: 0x0223,
                    flags: 0x0011,
                },
                BattleToken {
                    id: 0x201,
                    player: 1,
                    object_class: 2,
                    design: 0,
                    square: Square { x: 8, y: 5 },
                    initiative_base: 1,
                    initiative_min: 0xff,
                    initiative_max: 0xff,
                    target: 0xff,
                    pct_cloak: 0,
                    pct_jam: 0,
                    pct_computer: 0,
                    pct_capacitor: 0,
                    pct_beam_defence: 100,
                    mass: 20,
                    shields: 0,
                    ships: 1,
                    damage: 0,
                    tactics: 0x5000,
                    movement: 0x0300,
                    flags: 0x00e1,
                },
            ],
            actions: vec![
                BattleAction {
                    token: 0,
                    destination: Some(Square { x: 2, y: 4 }),
                    round: 0,
                    range: 0,
                    target: 0,
                    kills: Vec::new(),
                },
                BattleAction {
                    token: 0,
                    destination: Some(Square { x: 2, y: 4 }),
                    round: 1,
                    range: 3,
                    target: 1,
                    kills: vec![Kill {
                        token: 1,
                        weapon: 0,
                        ships_killed: 1,
                        shield_damage: 0,
                        damage: 0,
                    }],
                },
                BattleAction {
                    token: 1,
                    destination: None,
                    round: 2,
                    range: 0,
                    target: 0,
                    kills: Vec::new(),
                },
            ],
            declared_len: 0,
        };
        let bytes = record.encode();
        let back = BattleRecord::decode(&bytes).expect("decodes");
        assert_eq!(back.tokens, record.tokens);
        assert_eq!(back.actions, record.actions);
        assert_eq!(back.id, record.id);
        assert_eq!(back.player_mask, record.player_mask);
        assert_eq!(back.position, record.position);
        assert_eq!(usize::from(back.declared_len), bytes.len());
    }
}
