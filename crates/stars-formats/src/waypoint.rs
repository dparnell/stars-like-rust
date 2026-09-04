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
//! Like the other record decoders this is an *interpreted, read-only view*;
//! byte-exact write-back still goes through the container in [`crate::file`].

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
    /// `task == 0`).
    pub task_data: Vec<u8>,
}

/// Object-id value that marks a bare-coordinate waypoint (no target object).
const OBJECT_NONE: u16 = 0xFFFF;

/// The fixed 8-byte waypoint header length.
const HEADER_LEN: usize = 8;

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
}

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
