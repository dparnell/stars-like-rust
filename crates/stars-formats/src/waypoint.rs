//! Typed decoder for the **waypoint** block (type id 20, `WAYPOINT`) that
//! follows each full fleet block in `.mN`/`.hst`/`.hN` files.
//!
//! A fleet's ordered list of waypoints is stored as a run of type-20 blocks
//! immediately after the fleet, one per waypoint; the fleet's own
//! [`FleetRecord::waypoint_count`](crate::fleet::FleetRecord::waypoint_count)
//! says how many belong to it. The very first waypoint of a fleet is the
//! fleet's current position (a "waypoint zero").
//!
//! The 8-byte layout was recovered from the stars-4x `decompiled` project
//! (`Structures/Structure20.xml`) and the `starsapi` `WaypointBlock.java`, and
//! verified against `fixtures/incoming/turn0/Game.hst`, where every starting
//! fleet has a single waypoint sitting on its homeworld
//! (`object_type = 17` — a planet target with `fValidTask` set — `object_id` =
//! the orbited planet, `warp = 0`,
//! `task = 0`).
//!
//! ## Layout (8 bytes + optional task data)
//!
//! | Offset | Size | Field                                             |
//! |--------|------|---------------------------------------------------|
//! | 0–1    | 2    | x position                                         |
//! | 2–3    | 2    | y position                                         |
//! | 4–5    | 2    | target object id (`0xFFFF` = none / bare coords)  |
//! | 6      | 1    | low nibble = `grTask`, high nibble = `iWarp`       |
//! | 7      | 1    | low nibble = `grobj`; bit 4 `fValidTask`; bit 5 `fNoAutoTrack` |
//! | 7      | 1    | target object type                                |
//! | 8..    | var  | task-specific extra bytes (present when `task > 0`)|
//!
//! [`WaypointRecord::encode`] is an exact inverse of
//! [`WaypointRecord::decode`]: byte 7 is kept verbatim (the three fields
//! derived from it are views on it) and the task payload is kept as a tail, so
//! every waypoint block in the fixtures re-encodes byte for byte.

use crate::block::BlockType;
use crate::file::StarsFile;

/// A fleet waypoint order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaypointRecord {
    /// Galaxy x position of the waypoint.
    pub x: u16,
    /// Galaxy y position of the waypoint.
    pub y: u16,
    /// Id of the target object (planet/fleet/etc.), or `None` when the
    /// waypoint is a bare coordinate (stored id `0xFFFF`).
    pub object_id: Option<u16>,
    /// Byte 7 verbatim. It packs three fields — see [`Self::object_class`],
    /// [`Self::valid_task`] and [`Self::no_auto_track`]. The familiar value
    /// `17` is `0x11`: a planet target with the task-valid bit set.
    pub object_type: u8,
    /// What kind of object the waypoint targets (`grobj`, bits 8-11 of the
    /// word at offset 6): 1 planet, 2 fleet, 4 none, 8 thing.
    pub object_class: u8,
    /// `fValidTask` (bit 12). **A task only counts when this is set.** The
    /// task nibble keeps whatever was last chosen even after the task has been
    /// carried out or cancelled, so reading it alone overstates how many fleets
    /// have live orders.
    pub valid_task: bool,
    /// `fNoAutoTrack` (bit 13).
    pub no_auto_track: bool,
    /// Warp speed set for the leg reaching this waypoint (0..=15).
    pub warp: u8,
    /// Waypoint task id (0 = none, 1 = Transport, 2 = Colonize, 3 = Remote
    /// Mining, 4 = Merge, 5 = Scrap, 6 = Lay Minefield, 7 = Patrol, 8 = Route,
    /// 9 = Transfer). Exposed raw.
    pub task: u8,
    /// Task-specific extra bytes that follow the fixed header (empty when
    /// `task == 0`). Interpreted by [`Self::transport`] for a Transport task;
    /// the other tasks' payloads are kept verbatim.
    pub task_data: Vec<u8>,
}

/// What a Transport task does with one kind of cargo.
///
/// Source: the `XferActionType` enum and the `iAction` nibble of `ITEMACTION`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XferAction {
    /// Leave this cargo alone.
    None,
    /// Load everything the other side has.
    LoadAll,
    /// Unload everything the fleet carries.
    UnloadAll,
    /// Load exactly the stated quantity.
    LoadExact,
    /// Unload exactly the stated quantity.
    UnloadExact,
    /// Fill the hold to the stated percentage.
    FillPercent,
    /// Wait until the hold is the stated percentage full.
    WaitPercent,
    /// Load whatever is left over after the other kinds have loaded.
    LoadDunnage,
    /// Set the amount held to the stated quantity.
    SetAmount,
    /// Set the waypoint's amount to the stated quantity.
    SetWaypoint,
    /// A code this decoder does not know; kept so nothing is silently lost.
    Other(u8),
}

impl XferAction {
    /// Decode the 4-bit action code.
    #[must_use]
    pub fn from_raw(code: u8) -> Self {
        match code {
            0 => Self::None,
            1 => Self::LoadAll,
            2 => Self::UnloadAll,
            3 => Self::LoadExact,
            4 => Self::UnloadExact,
            5 => Self::FillPercent,
            6 => Self::WaitPercent,
            7 => Self::LoadDunnage,
            8 => Self::SetAmount,
            9 => Self::SetWaypoint,
            other => Self::Other(other),
        }
    }

    /// The four-bit `iAction` code this action is stored as.
    #[must_use]
    pub fn to_raw(self) -> u8 {
        match self {
            Self::None => 0,
            Self::LoadAll => 1,
            Self::UnloadAll => 2,
            Self::LoadExact => 3,
            Self::UnloadExact => 4,
            Self::FillPercent => 5,
            Self::WaitPercent => 6,
            Self::LoadDunnage => 7,
            Self::SetAmount => 8,
            Self::SetWaypoint => 9,
            Self::Other(other) => other & 0x0F,
        }
    }

    /// Whether this action moves cargo **into** the fleet.
    #[must_use]
    pub fn loads(self) -> bool {
        matches!(
            self,
            Self::LoadAll | Self::LoadExact | Self::LoadDunnage | Self::FillPercent
        )
    }
}

/// One cargo kind's instruction within a Transport task (`ITEMACTION`).
///
/// Packed into one 16-bit word as `cQuan:12, iAction:4`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemAction {
    /// The quantity the action refers to, where it takes one.
    pub quantity: u16,
    /// What to do.
    pub action: XferAction,
}

/// A Transport task's instructions, one per cargo kind (`TASKXPORT`).
///
/// The five kinds are the same as everywhere else in the game: ironium,
/// boranium, germanium, colonists, fuel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportTask {
    /// What to do with each cargo kind, in order.
    pub items: [ItemAction; 5],
}

/// Object-id value that marks a bare-coordinate waypoint (no target object).
const OBJECT_NONE: u16 = 0xFFFF;

/// The fixed 8-byte waypoint header length.
const HEADER_LEN: usize = 8;

/// Block type of a taskless waypoint (`rtOrderB`): the eight-byte header alone.
pub const WAYPOINT_BLOCK: u8 = 20;

/// Block type of a waypoint carrying a task (`rtOrderA`): header plus the
/// ten-byte `ORDER` task union.
pub const WAYPOINT_TASK_BLOCK: u8 = 19;

fn read16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

impl WaypointRecord {
    /// Decode a **decrypted** type-20 waypoint block payload.
    ///
    /// Returns `None` if the payload is shorter than the 8-byte header.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < HEADER_LEN {
            return None;
        }
        let x = read16(data, 0)?;
        let y = read16(data, 2)?;
        let object_raw = read16(data, 4)?;
        let object_id = if object_raw == OBJECT_NONE {
            None
        } else {
            Some(object_raw)
        };
        let task = data[6] & 0x0F;
        let warp = data[6] >> 4;
        let object_type = data[7];
        let object_class = data[7] & 0x0F;
        let valid_task = data[7] & 0x10 != 0;
        let no_auto_track = data[7] & 0x20 != 0;
        let task_data = data[HEADER_LEN..].to_vec();
        Some(Self {
            x,
            y,
            object_id,
            object_type,
            object_class,
            valid_task,
            no_auto_track,
            warp,
            task,
            task_data,
        })
    }

    /// Re-encode this waypoint as a block payload.
    ///
    /// Exact inverse of [`WaypointRecord::decode`] for every waypoint block in
    /// the fixtures. The block type follows from the payload length, which is
    /// what [`Self::block_type`] reports.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.task_data.len());
        out.extend_from_slice(&self.x.to_le_bytes());
        out.extend_from_slice(&self.y.to_le_bytes());
        out.extend_from_slice(&self.object_id.unwrap_or(OBJECT_NONE).to_le_bytes());
        out.push((self.task & 0x0F) | (self.warp << 4));
        out.push(self.object_type);
        out.extend_from_slice(&self.task_data);
        out
    }

    /// Which block type this waypoint is written as.
    ///
    /// A waypoint with no task payload is a type-20 block (`rtOrderB`, the
    /// eight-byte header alone); one with a payload is type 19 (`rtOrderA`,
    /// the header plus the ten-byte task union).
    #[must_use]
    pub fn block_type(&self) -> u8 {
        if self.task_data.is_empty() {
            WAYPOINT_BLOCK
        } else {
            WAYPOINT_TASK_BLOCK
        }
    }

    /// The Transport task's instructions, if this waypoint carries one.
    ///
    /// Source: the `ORDER` union's `TASKXPORT txp` arm — `ITEMACTION rgia[5]`,
    /// ten bytes immediately after the eight-byte header, each entry packed
    /// `cQuan:12, iAction:4`. An earlier revision of `docs/formats/waypoint.md`
    /// recorded this payload as an open question and kept it verbatim; the NB09
    /// structures name it exactly.
    ///
    /// Returns `None` when the waypoint is not a Transport task or the payload
    /// is short.
    #[must_use]
    pub fn transport(&self) -> Option<TransportTask> {
        if self.task != TASK_TRANSPORT || self.task_data.len() < 10 {
            return None;
        }
        let mut items = [ItemAction {
            quantity: 0,
            action: XferAction::None,
        }; 5];
        for (i, slot) in items.iter_mut().enumerate() {
            let word = u16::from_le_bytes([self.task_data[i * 2], self.task_data[i * 2 + 1]]);
            *slot = ItemAction {
                quantity: word & 0x0FFF,
                action: XferAction::from_raw((word >> 12) as u8),
            };
        }
        Some(TransportTask { items })
    }
}

/// Waypoint task ids (`grTask`).
pub mod task {
    /// No task.
    pub const NONE: u8 = 0;
    /// Transport: load and unload cargo.
    pub const TRANSPORT: u8 = 1;
    /// Colonize the planet.
    pub const COLONIZE: u8 = 2;
    /// Mine the planet from orbit.
    pub const REMOTE_MINING: u8 = 3;
    /// Merge into another fleet.
    pub const MERGE: u8 = 4;
    /// Scrap the fleet.
    pub const SCRAP: u8 = 5;
    /// Lay a minefield.
    pub const LAY_MINES: u8 = 6;
    /// Patrol.
    pub const PATROL: u8 = 7;
    /// Follow the planet's route.
    pub const ROUTE: u8 = 8;
    /// Give the fleet away.
    pub const TRANSFER: u8 = 9;

    /// The name the game shows for a task id.
    #[must_use]
    pub fn name(task: u8) -> &'static str {
        match task {
            NONE => "none",
            TRANSPORT => "transport",
            COLONIZE => "colonize",
            REMOTE_MINING => "remote mining",
            MERGE => "merge",
            SCRAP => "scrap",
            LAY_MINES => "lay minefield",
            PATROL => "patrol",
            ROUTE => "route",
            TRANSFER => "transfer fleet",
            _ => "unknown",
        }
    }
}

use task::TRANSPORT as TASK_TRANSPORT;

/// Decode every waypoint block (type 20) in a decoded [`StarsFile`], in file
/// order.
///
/// The result is a flat list in file order; callers that need per-fleet
/// grouping should walk the blocks together with the fleet records and use
/// each fleet's `waypoint_count`.
#[must_use]
pub fn waypoint_records(file: &StarsFile) -> Vec<WaypointRecord> {
    file.blocks
        .iter()
        .filter(|b| b.block_type() == BlockType::Waypoint)
        .filter_map(|b| WaypointRecord::decode(&b.data))
        .collect()
}

impl TransportTask {
    /// Re-encode the per-cargo instructions as the ten-byte `ORDER` task
    /// union, the inverse of [`WaypointRecord::transport`].
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(10);
        for item in &self.items {
            let word = (item.quantity & 0x0FFF) | (u16::from(item.action.to_raw()) << 12);
            out.extend_from_slice(&word.to_le_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_orbit_waypoint() {
        // x=1444, y=1564, object 69, task 0, warp 0, object type 17.
        let mut d = Vec::new();
        d.extend_from_slice(&1444u16.to_le_bytes());
        d.extend_from_slice(&1564u16.to_le_bytes());
        d.extend_from_slice(&69u16.to_le_bytes());
        d.push(0x00); // task 0, warp 0
        d.push(17); // object type
        let w = WaypointRecord::decode(&d).unwrap();
        assert_eq!((w.x, w.y), (1444, 1564));
        assert_eq!(w.object_id, Some(69));
        assert_eq!(w.object_type, 17);
        assert_eq!(w.warp, 0);
        assert_eq!(w.task, 0);
        assert!(w.task_data.is_empty());
    }

    #[test]
    fn splits_warp_and_task_nibbles() {
        let mut d = vec![0u8; HEADER_LEN];
        d[6] = 0x92; // task = 2 (Colonize), warp = 9
        let w = WaypointRecord::decode(&d).unwrap();
        assert_eq!(w.warp, 9);
        assert_eq!(w.task, 2);
    }

    #[test]
    fn bare_coordinate_has_no_object() {
        let mut d = vec![0u8; HEADER_LEN];
        d[4] = 0xFF;
        d[5] = 0xFF;
        let w = WaypointRecord::decode(&d).unwrap();
        assert_eq!(w.object_id, None);
    }

    #[test]
    fn rejects_truncated() {
        assert!(WaypointRecord::decode(&[0u8; 7]).is_none());
    }
}
