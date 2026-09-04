//! Typed decoder for the **fleet** blocks (`rtFleetA`/`B`/`C`, type ids 16, 17
//! and 18) that appear in `.hst`, `.mN` and `.hN` files.
//!
//! A fleet block has a fixed 14-byte header (id/owner, detail+flags, orbit
//! planet, position, and a ship-design bitmask) followed by presence-gated
//! sections: a per-design ship count list, an optional variable-length cargo
//! hold, and then either full-fleet data (per-ship damage, battle plan, waypoint
//! count) for `rtFleetA`, or a direction/mass estimate for the partial forms.
//!
//! The layout was recovered from TotalHost's `StarsFleet.pl` (Rick Steeves,
//! derived from `starsapi`) and cross-checked against
//! `fixtures/incoming/turn0/Game.hst`: its 14 starting fleets each orbit their
//! owner's homeworld (owner 0 → planet 69, owner 1 → 112, owner 2 → 32) and
//! carry a single ship.
//!
//! [`FleetRecord::encode`] is an exact inverse of [`FleetRecord::decode`]:
//! every fleet block in the fixtures re-encodes byte for byte. Three details
//! make that non-trivial. The **ship-count width** is a flag in the block
//! (`fByteCsh`), not something to be inferred from the values, so it is kept.
//! The **ship bitmask** can name a design slot whose count is zero, so the mask
//! is kept rather than rebuilt from the stacks. And the **cargo hold** is
//! length-prefixed the same way a planet's surface minerals are: the shortest
//! width that holds each value.

use crate::block::BlockType;
use crate::file::StarsFile;

/// A stack of identical ships within a fleet: how many of a given design slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipStack {
    /// Ship-design slot (0..=15) this stack refers to.
    pub design_slot: u8,
    /// Number of ships of that design in the fleet.
    pub count: u16,
}

/// A fleet's cargo hold (minerals in kilotons, population in colonists, fuel in
/// millifuel units as stored).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cargo {
    /// Ironium, in kilotons.
    pub ironium: u32,
    /// Boranium, in kilotons.
    pub boranium: u32,
    /// Germanium, in kilotons.
    pub germanium: u32,
    /// Colonists aboard (stored ÷ 100; multiplied out here).
    pub population: u32,
    /// Fuel aboard.
    pub fuel: u32,
}

/// Per-design battle damage on a full fleet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipDamage {
    /// Ship-design slot (0..=15).
    pub design_slot: u8,
    /// Percent of ships in the stack that are damaged (`pctSh`).
    pub ships_pct: u8,
    /// Accumulated armor damage percent (`pctDp`).
    pub armor_pct: u16,
}

/// A decoded fleet record.
///
/// The optional sections mirror the detail level: cargo appears at detail >= 4,
/// while full fleets (`rtFleetA`, detail 7) add damage, battle plan and waypoint
/// count, and the partial forms instead carry a movement estimate
/// ([`delta_x`](Self::delta_x)/[`delta_y`](Self::delta_y)/[`warp`](Self::warp)/[`mass`](Self::mass)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetRecord {
    /// Fleet number (0-based, per owning player), from the low 9 bits of the id
    /// word.
    pub id: u16,
    /// Owning player (0-based), from bits 9..=12 of the id word.
    pub owner: u8,
    /// Detail level (`det`): 3 = some, 4 = more (adds cargo), 7 = all (full).
    pub detail: u8,
    /// `fInclude` — the fleet is included in reports / turn processing.
    pub include: bool,
    /// `fRepeatOrders` — the fleet's waypoint orders repeat.
    pub repeat_orders: bool,
    /// `fDead` — the fleet was destroyed this turn.
    pub dead: bool,
    /// `fByteCsh` — ship counts are one byte each rather than two.
    pub byte_counts: bool,
    /// Bits 4..=7 of the flags byte, which this module does not interpret.
    pub flags_high: u8,
    /// Planet id this fleet is orbiting (0-based), or `None` if it is in deep
    /// space (stored id `65535`).
    pub orbiting: Option<u16>,
    /// Galaxy x position.
    pub x: u16,
    /// Galaxy y position.
    pub y: u16,
    /// Which design slots the block stores a count for.
    ///
    /// Kept alongside [`Self::ships`] because a block may name a slot whose
    /// count is zero, and [`Self::ships`] holds only the non-empty ones.
    pub ship_slots: u16,
    /// Ship stacks present in the fleet (only non-empty design slots).
    pub ships: Vec<ShipStack>,
    /// Cargo hold, present at detail >= 4.
    pub cargo: Option<Cargo>,
    /// Battle-plan index (0-based), full fleets only.
    pub battle_plan: Option<u8>,
    /// Number of waypoint (order) blocks that follow this fleet, full fleets
    /// only.
    pub waypoint_count: Option<u8>,
    /// Per-design damage, full fleets only (empty when undamaged).
    pub damage: Vec<ShipDamage>,
    /// Movement direction x component (partial fleets only), two's-complement.
    pub delta_x: Option<i8>,
    /// Movement direction y component (partial fleets only), two's-complement.
    pub delta_y: Option<i8>,
    /// Warp speed estimate (partial fleets only).
    pub warp: Option<u8>,
    /// Total mass estimate, in kilotons (partial fleets only).
    pub mass: Option<u32>,
    /// Bits 4..=7 of the partial form's warp byte, uninterpreted.
    pub warp_high: u8,
    /// The byte after the warp byte in the partial form, uninterpreted.
    pub partial_unused: u8,
    /// Any bytes after the last field this module understands, kept so the
    /// block re-encodes exactly.
    pub trailing: Vec<u8>,
}

/// Orbit-planet field value that marks a fleet in deep space.
const ORBIT_NONE: u16 = 0xFFFF;

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

fn read_n(d: &[u8], o: usize, n: usize) -> Option<u32> {
    let mut v = 0u32;
    for i in 0..n {
        v |= (*d.get(o + i)? as u32) << (8 * i);
    }
    Some(v)
}

/// Decode a 2-bit content-length selector into a byte width via `[0,1,2,4]`.
fn seg_len(bits: u16) -> usize {
    [0usize, 1, 2, 4][(bits & 0x03) as usize]
}

impl FleetRecord {
    /// Decode a fleet block payload. `type_id` is the block's type id (16, 17 or
    /// 18); 16 is a full fleet, 17/18 are partial (enemy) fleets.
    ///
    /// Returns `None` if the payload is truncated.
    #[must_use]
    pub fn decode(data: &[u8], type_id: u8) -> Option<Self> {
        let id_word = read16(data, 0)?;
        let id = id_word & 0x01FF;
        let owner = ((id_word >> 9) & 0x0F) as u8;

        let detail = *data.get(4)?;
        let byte5 = *data.get(5)?;
        let f_byte_csh = (byte5 >> 3) & 1;
        let dead = (byte5 >> 2) & 1 != 0;
        let repeat_orders = (byte5 >> 1) & 1 != 0;
        let include = byte5 & 1 != 0;

        let orbit_raw = read16(data, 6)?;
        let orbiting = if orbit_raw == ORBIT_NONE {
            None
        } else {
            Some(orbit_raw)
        };
        let x = read16(data, 8)?;
        let y = read16(data, 10)?;
        let ship_bitmask = read16(data, 12)?;
        let mut index = 14usize;

        // Ship counts: 1 byte each when fByteCsh is set, otherwise 2 bytes each.
        let two_byte_counts = f_byte_csh == 0;
        let mut ships = Vec::new();
        for bit in 0..16u8 {
            if ship_bitmask & (1 << bit) != 0 {
                let count = if two_byte_counts {
                    let v = read16(data, index)?;
                    index += 2;
                    v
                } else {
                    let v = u16::from(*data.get(index)?);
                    index += 1;
                    v
                };
                if count > 0 {
                    ships.push(ShipStack {
                        design_slot: bit,
                        count,
                    });
                }
            }
        }

        // Cargo hold: present at detail >= 4 (detMore/detAll).
        let mut cargo = None;
        if detail >= 4 {
            let cl = read16(data, index)?;
            index += 2;
            let i_len = seg_len(cl);
            let b_len = seg_len(cl >> 2);
            let g_len = seg_len(cl >> 4);
            let pop_len = seg_len(cl >> 6);
            let fuel_len = seg_len(cl >> 8);

            let ironium = read_n(data, index, i_len)?;
            index += i_len;
            let boranium = read_n(data, index, b_len)?;
            index += b_len;
            let germanium = read_n(data, index, g_len)?;
            index += g_len;
            let population = read_n(data, index, pop_len)?;
            index += pop_len;
            let fuel = read_n(data, index, fuel_len)?;
            index += fuel_len;

            cargo = Some(Cargo {
                ironium,
                boranium,
                germanium,
                population: population * 100,
                fuel,
            });
        }

        let mut record = Self {
            id,
            owner,
            detail,
            include,
            repeat_orders,
            dead,
            byte_counts: f_byte_csh == 1,
            flags_high: byte5 >> 4,
            orbiting,
            x,
            y,
            ship_slots: ship_bitmask,
            ships,
            cargo,
            battle_plan: None,
            waypoint_count: None,
            damage: Vec::new(),
            delta_x: None,
            delta_y: None,
            warp: None,
            mass: None,
            warp_high: 0,
            partial_unused: 0,
            trailing: Vec::new(),
        };

        if detail == 7 {
            // Full fleet: damage bitmask + values, battle plan, waypoint count.
            let damage_bitmask = read16(data, index)?;
            index += 2;
            for bit in 0..16u8 {
                if damage_bitmask & (1 << bit) != 0 {
                    let v = read16(data, index)?;
                    index += 2;
                    record.damage.push(ShipDamage {
                        design_slot: bit,
                        ships_pct: (v & 0x7F) as u8,
                        armor_pct: (v >> 7) & 0x01FF,
                    });
                }
            }
            record.battle_plan = Some(*data.get(index)?);
            index += 1;
            record.waypoint_count = Some(*data.get(index)?);
            index += 1;
        } else if type_id != 16 {
            // Partial fleet (rtFleetB/C): direction + mass estimate.
            let dx = *data.get(index)? as i8;
            index += 1;
            let dy = *data.get(index)? as i8;
            index += 1;
            let warp_byte = *data.get(index)?;
            index += 1; // warp/flags byte
            record.partial_unused = *data.get(index)?;
            index += 1; // unused byte
            let mass = read32(data, index)?;
            index += 4;
            record.delta_x = Some(dx);
            record.delta_y = Some(dy);
            record.warp = Some(warp_byte & 0x0F);
            record.warp_high = warp_byte >> 4;
            record.mass = Some(mass);
        }

        record.trailing = data.get(index..).unwrap_or_default().to_vec();
        Some(record)
    }

    /// Re-encode this record as a fleet block payload.
    ///
    /// Exact inverse of [`FleetRecord::decode`] for every fleet block in the
    /// fixtures — see `tests/round_trip.rs`.
    #[must_use]
    pub fn encode(&self, type_id: u8) -> Vec<u8> {
        let mut out = Vec::with_capacity(32);
        let id_word = (self.id & 0x01FF) | ((u16::from(self.owner) & 0x0F) << 9);
        out.extend_from_slice(&id_word.to_le_bytes());
        // `FLEET.iPlayer`: the owner repeated. Bits 13..=15 of the id word are
        // `junk` in the NB09 struct. Both are as written here in all 459,430
        // fleet blocks in the fixtures.
        out.extend_from_slice(&u16::from(self.owner).to_le_bytes());
        out.push(self.detail);
        out.push(
            u8::from(self.include)
                | (u8::from(self.repeat_orders) << 1)
                | (u8::from(self.dead) << 2)
                | (u8::from(self.byte_counts) << 3)
                | (self.flags_high << 4),
        );
        out.extend_from_slice(&self.orbiting.unwrap_or(ORBIT_NONE).to_le_bytes());
        out.extend_from_slice(&self.x.to_le_bytes());
        out.extend_from_slice(&self.y.to_le_bytes());
        out.extend_from_slice(&self.ship_slots.to_le_bytes());

        for bit in 0..16u8 {
            if self.ship_slots & (1 << bit) == 0 {
                continue;
            }
            let count = self
                .ships
                .iter()
                .find(|s| s.design_slot == bit)
                .map_or(0, |s| s.count);
            if self.byte_counts {
                out.push((count & 0xFF) as u8);
            } else {
                out.extend_from_slice(&count.to_le_bytes());
            }
        }

        if self.detail >= 4 {
            let c = self.cargo.unwrap_or_default();
            let values = [
                c.ironium,
                c.boranium,
                c.germanium,
                c.population / 100,
                c.fuel,
            ];
            let mut lengths = 0u16;
            for (i, value) in values.iter().enumerate() {
                lengths |= u16::from(seg_code(*value)) << (2 * i);
            }
            out.extend_from_slice(&lengths.to_le_bytes());
            for value in values {
                let n = seg_len(u16::from(seg_code(value)));
                for byte in 0..n {
                    out.push(((value >> (8 * byte)) & 0xFF) as u8);
                }
            }
        }

        if self.detail == 7 {
            let mut mask = 0u16;
            for d in &self.damage {
                mask |= 1 << d.design_slot;
            }
            out.extend_from_slice(&mask.to_le_bytes());
            for bit in 0..16u8 {
                if mask & (1 << bit) == 0 {
                    continue;
                }
                let d = self
                    .damage
                    .iter()
                    .find(|d| d.design_slot == bit)
                    .copied()
                    .unwrap_or(ShipDamage {
                        design_slot: bit,
                        ships_pct: 0,
                        armor_pct: 0,
                    });
                let v = u16::from(d.ships_pct) & 0x7F | ((d.armor_pct & 0x01FF) << 7);
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.push(self.battle_plan.unwrap_or(0));
            out.push(self.waypoint_count.unwrap_or(0));
        } else if type_id != 16 {
            out.push(self.delta_x.unwrap_or(0) as u8);
            out.push(self.delta_y.unwrap_or(0) as u8);
            out.push((self.warp.unwrap_or(0) & 0x0F) | (self.warp_high << 4));
            out.push(self.partial_unused);
            out.extend_from_slice(&self.mass.unwrap_or(0).to_le_bytes());
        }

        out.extend_from_slice(&self.trailing);
        out
    }
}

/// The two-bit code for the shortest field that holds `value`.
fn seg_code(value: u32) -> u8 {
    if value == 0 {
        0
    } else if value <= 0xFF {
        1
    } else if value <= 0xFFFF {
        2
    } else {
        3
    }
}

/// Decode every fleet block (types 16/17/18) in a decrypted [`StarsFile`], in
/// file order.
#[must_use]
pub fn fleet_records(file: &StarsFile) -> Vec<FleetRecord> {
    file.blocks
        .iter()
        .filter_map(|b| {
            let type_id = match b.block_type() {
                BlockType::Fleet => 16u8,
                BlockType::PartialFleet => 17,
                BlockType::Other(18) => 18,
                _ => return None,
            };
            FleetRecord::decode(&b.data, type_id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a full-fleet (type 16) payload: fleet 3 of player 2, orbiting
    /// planet 69, at (1444, 1564), with a single ship in slot 0, detail 7 and
    /// no cargo/damage.
    fn full_fleet_payload() -> Vec<u8> {
        let id_word: u16 = 3 | (2 << 9);
        let mut v = Vec::new();
        v.extend_from_slice(&id_word.to_le_bytes()); // 0-1 id/owner
        v.extend_from_slice(&0u16.to_le_bytes()); // 2-3 iPlayer (ignored)
        v.push(7); // 4 det
        v.push(0b0000_1001); // 5 flags: fByteCsh(bit3) + fInclude(bit0)
        v.extend_from_slice(&69u16.to_le_bytes()); // 6-7 orbit planet
        v.extend_from_slice(&1444u16.to_le_bytes()); // 8-9 x
        v.extend_from_slice(&1564u16.to_le_bytes()); // 10-11 y
        v.extend_from_slice(&0x0001u16.to_le_bytes()); // 12-13 ship bitmask: slot 0
        v.push(1); // ship count (1 byte, fByteCsh set)
                   // detail >= 4: cargo content-length word (0 = empty hold)
        v.extend_from_slice(&0u16.to_le_bytes()); // cargo content lengths
                                                  // detail==7: damage bitmask (none), battle plan, waypoint count
        v.extend_from_slice(&0u16.to_le_bytes()); // damage bitmask
        v.push(0); // battle plan
        v.push(1); // waypoint count
        v
    }

    #[test]
    fn decodes_full_fleet() {
        let f = FleetRecord::decode(&full_fleet_payload(), 16).unwrap();
        assert_eq!(f.id, 3);
        assert_eq!(f.owner, 2);
        assert_eq!(f.detail, 7);
        assert!(f.include);
        assert!(!f.dead);
        assert_eq!(f.orbiting, Some(69));
        assert_eq!((f.x, f.y), (1444, 1564));
        assert_eq!(
            f.ships,
            vec![ShipStack {
                design_slot: 0,
                count: 1
            }]
        );
        assert_eq!(f.battle_plan, Some(0));
        assert_eq!(f.waypoint_count, Some(1));
        assert!(f.damage.is_empty());
        // A full fleet always carries a cargo section; here it is empty.
        assert_eq!(f.cargo, Some(Cargo::default()));
    }

    #[test]
    fn rejects_truncated_payload() {
        assert!(FleetRecord::decode(&[0x00], 16).is_none());
        assert!(FleetRecord::decode(&[], 16).is_none());
    }
}
