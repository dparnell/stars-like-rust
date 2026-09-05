//! Typed decoder for the **`.xN` player-orders file** — the "order log"
//! (`dtLog`) a player submits to the host each turn.
//!
//! Unlike `.hst`/`.mN` state files (a flat set of one-record-per-type blocks),
//! an orders file is a **log**: after the plaintext file header it carries a
//! single log header (`RTLOGHDR`, type id 9) followed by a sequence of
//! *operation* records whose block ids come from the `rtLog*` family. When the
//! host loads the file it replays each operation against its in-memory state
//! (insert a waypoint, transfer cargo, change a production queue, …).
//!
//! Layout recovered from the NB09 debug symbols of `Stars! 2.7j`
//! (sirgwain's `stars-asm` — `structs.h`/`enums.h`/`log.c`; see
//! `docs/formats/orders-x.md` and `docs/formats/nb09-structs.md`) and verified
//! against the 40-turn `fixtures/games/exodus/*/EXODUS.X6` order sequence.
//!
//! Like the other record decoders this is a read-only *interpreted view*;
//! byte-exact write-back still goes through the container in [`crate::file`].

use crate::battleplan::{BattlePlanRecord, PLAN_DELETED};
use crate::design::DesignRecord;
use crate::file::StarsFile;
use crate::header::FileHeader;
use crate::production::ProductionQueueRecord;
use crate::strings::{decode_user_string, encode_user_string};
use crate::{FormatError, Result};

/// The block type id of the order-log header record (`RTLOGHDR`).
pub const LOG_HEADER_BLOCK: u8 = 9;

/// Size in bytes of the order-log header (`cbRTLOGHDR`).
pub const LOG_HEADER_LEN: usize = 17;

/// The record type of one operation in an order log.
///
/// The ids come from the `rtLog*` family in the game's `RecordType` enum. In a
/// `.xN` file these ids mean the **log** operation, which differs from the
/// same id in a state file (e.g. id 27 is a *ship-design change* here, not a
/// plain design block).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogRecordType {
    /// Terminator (type id 0, `rtEOF`).
    Eof,
    /// Order-log header (type id 9, `RTLOGHDR`).
    Header,
    /// Cargo transfer, int8 quantities (`rtLogCargoXfer8`, 1).
    CargoXfer8,
    /// Cargo transfer, int16 quantities (`rtLogCargoXfer16`, 2).
    CargoXfer16,
    /// Delete one or two fleet waypoint orders (`rtLogFleetOrderDelete`, 3).
    FleetOrderDelete,
    /// Insert a fleet waypoint order (`rtLogFleetOrderInsert`, 4).
    FleetOrderInsert,
    /// Overwrite a fleet waypoint order (`rtLogFleetOrderUpdate`, 5).
    FleetOrderUpdate,
    /// Set/clear a fleet flag bit (`rtLogFleetFlagBit9`, 10).
    FleetFlagBit,
    /// Set a fleet order's attribute nibble (`rtLogFleetOrderAttrNib`, 11).
    FleetOrderAttrNib,
    /// Fleet-to-fleet cargo transfer (`rtLogFleetCargoXfer`, 23).
    FleetCargoXfer,
    /// Split a fleet (`rtLogFleetSplit`, 24).
    FleetSplit,
    /// Cargo transfer, int32 quantities (`rtLogCargoXfer32`, 25).
    CargoXfer32,
    /// Create/update/delete a ship design (`rtLogShDef`, 27).
    ShipDesign,
    /// Set/clear a planet's production queue (`rtLogPlanetProdQ`, 29).
    PlanetProdQueue,
    /// Define or delete one of the player's battle plans (`rtBtlPlan`, 30).
    BattlePlan,
    /// Change the player's turn password (`rtChgPassword`, 36).
    ChangePassword,
    /// Research settings (`rtLogResearch`, 34).
    Research,
    /// Planet routing / starbase / infrastructure bits (`rtLogPlanetRouting`, 35).
    PlanetRouting,
    /// Merge fleets (`rtLogFleetMerge`, 37).
    FleetMerge,
    /// Player-relations table (`rtLogRelations`, 38).
    Relations,
    /// Set a fleet's battle plan (`rtLogFleetPlan`, 42).
    FleetPlan,
    /// Set one byte inside a `THING` union (`rtLogThingByteParam`, 43).
    ThingByteParam,
    /// Fleet rename (`rtLogFleetName`, 44).
    FleetName,
    /// Host-only opaque blob (`rtLogPlayerZpq1`, 46).
    PlayerZpq1,
    /// Any other id, kept verbatim.
    Other(u8),
}

impl LogRecordType {
    /// Classify a block type id as an order-log record type.
    #[must_use]
    pub fn from_id(id: u8) -> Self {
        match id {
            0 => Self::Eof,
            9 => Self::Header,
            1 => Self::CargoXfer8,
            2 => Self::CargoXfer16,
            3 => Self::FleetOrderDelete,
            4 => Self::FleetOrderInsert,
            5 => Self::FleetOrderUpdate,
            10 => Self::FleetFlagBit,
            11 => Self::FleetOrderAttrNib,
            23 => Self::FleetCargoXfer,
            24 => Self::FleetSplit,
            25 => Self::CargoXfer32,
            27 => Self::ShipDesign,
            29 => Self::PlanetProdQueue,
            30 => Self::BattlePlan,
            36 => Self::ChangePassword,
            34 => Self::Research,
            35 => Self::PlanetRouting,
            37 => Self::FleetMerge,
            38 => Self::Relations,
            42 => Self::FleetPlan,
            43 => Self::ThingByteParam,
            44 => Self::FleetName,
            46 => Self::PlayerZpq1,
            other => Self::Other(other),
        }
    }

    /// The numeric block type id for this record type.
    #[must_use]
    pub fn id(self) -> u8 {
        match self {
            Self::Eof => 0,
            Self::Header => 9,
            Self::CargoXfer8 => 1,
            Self::CargoXfer16 => 2,
            Self::FleetOrderDelete => 3,
            Self::FleetOrderInsert => 4,
            Self::FleetOrderUpdate => 5,
            Self::FleetFlagBit => 10,
            Self::FleetOrderAttrNib => 11,
            Self::FleetCargoXfer => 23,
            Self::FleetSplit => 24,
            Self::CargoXfer32 => 25,
            Self::ShipDesign => 27,
            Self::PlanetProdQueue => 29,
            Self::BattlePlan => 30,
            Self::ChangePassword => 36,
            Self::Research => 34,
            Self::PlanetRouting => 35,
            Self::FleetMerge => 37,
            Self::Relations => 38,
            Self::FleetPlan => 42,
            Self::ThingByteParam => 43,
            Self::FleetName => 44,
            Self::PlayerZpq1 => 46,
            Self::Other(id) => id,
        }
    }
}

/// The **owning player** encoded in an object id (`THING.iplr`, bits 9..=12).
///
/// Stars! object ids pack `id:9 | iplr:4 | ith:3`; for a fleet the low 9 bits
/// are the fleet number and bits 9..=12 are the owner's 0-based player index.
#[must_use]
pub fn object_owner(id: u16) -> u8 {
    ((id >> 9) & 0xF) as u8
}

/// The **object index** encoded in an object id (`THING.id`, low 9 bits).
#[must_use]
pub fn object_index(id: u16) -> u16 {
    id & 0x1FF
}

/// A decoded order-log header (`RTLOGHDR`, type id 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogHeader {
    /// `cbLog` — total byte length (framed) of all operation records that
    /// follow this header, up to but excluding the file footer.
    pub log_byte_count: u16,
    /// `lSerialNumber` — a per-game/per-player serial (constant across a game's
    /// turns), used by the host to validate the submitted orders.
    pub serial_number: i32,
    /// `rgbConfig[11]` — 11 config/verification bytes recorded when the orders
    /// were written; opaque and preserved verbatim.
    pub config: [u8; 11],
}

impl LogHeader {
    /// Decode a **decrypted** type-9 `RTLOGHDR` payload.
    ///
    /// Returns `None` if the payload is shorter than the fixed 17-byte record.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 17 {
            return None;
        }
        let mut config = [0u8; 11];
        config.copy_from_slice(&data[6..17]);
        Some(Self {
            log_byte_count: u16::from_le_bytes([data[0], data[1]]),
            serial_number: i32::from_le_bytes([data[2], data[3], data[4], data[5]]),
            config,
        })
    }

    /// Re-encode this header as a `RTLOGHDR` payload.
    #[must_use]
    pub fn encode(&self) -> [u8; LOG_HEADER_LEN] {
        let mut out = [0u8; LOG_HEADER_LEN];
        out[0..2].copy_from_slice(&self.log_byte_count.to_le_bytes());
        out[2..6].copy_from_slice(&self.serial_number.to_le_bytes());
        out[6..17].copy_from_slice(&self.config);
        out
    }
}

/// A decoded fleet-waypoint order (`RTWAYPT`), the payload of an insert
/// (type id 4) or update (type id 5) log operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaypointOrder {
    /// The fleet the order applies to (raw object id; see [`object_owner`] /
    /// [`object_index`]).
    pub fleet_id: u16,
    /// The waypoint slot index within the fleet's order list.
    pub waypoint_index: u16,
    /// Destination X (universe coordinate).
    pub x: i16,
    /// Destination Y (universe coordinate).
    pub y: i16,
    /// The target object id at the waypoint (0 when it is a bare point).
    pub target_id: i16,
    /// The waypoint task (`ORDER.grTask`, low nibble of the flags word).
    pub task: u8,
    /// The warp factor to travel at (`ORDER.iWarp`).
    pub warp: u8,
    /// The target object class (`ORDER.grobj`).
    pub grobj: u8,
    /// Whether the task is valid (`ORDER.fValidTask`).
    pub valid_task: bool,
    /// Bits 13..=15 of the flags word, which this module does not interpret.
    /// Bit 13 is `fNoAutoTrack` in the state file's own waypoint record.
    pub flags_high: u8,
    /// Any trailing task-specific union bytes (`TASKXPORT`/`TASKLAYMINES`/…),
    /// preserved verbatim; empty for tasks that carry no extra data.
    pub task_data: Vec<u8>,
}

impl WaypointOrder {
    /// Decode a **decrypted** `RTWAYPT` payload (an insert/update operation).
    ///
    /// Returns `None` if the payload is shorter than the 12-byte fixed part.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 12 {
            return None;
        }
        let flags = u16::from_le_bytes([data[10], data[11]]);
        Some(Self {
            fleet_id: u16::from_le_bytes([data[0], data[1]]),
            waypoint_index: u16::from_le_bytes([data[2], data[3]]),
            x: i16::from_le_bytes([data[4], data[5]]),
            y: i16::from_le_bytes([data[6], data[7]]),
            target_id: i16::from_le_bytes([data[8], data[9]]),
            task: (flags & 0xF) as u8,
            warp: ((flags >> 4) & 0xF) as u8,
            grobj: ((flags >> 8) & 0xF) as u8,
            valid_task: (flags & 0x1000) != 0,
            flags_high: ((flags >> 13) & 0x7) as u8,
            task_data: data[12..].to_vec(),
        })
    }

    /// Re-encode this order as an insert/update payload (`RTWAYPT`).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let flags = u16::from(self.task & 0xF)
            | (u16::from(self.warp & 0xF) << 4)
            | (u16::from(self.grobj & 0xF) << 8)
            | (u16::from(self.valid_task) << 12)
            | (u16::from(self.flags_high & 0x7) << 13);
        let mut out = Vec::with_capacity(12 + self.task_data.len());
        out.extend_from_slice(&self.fleet_id.to_le_bytes());
        out.extend_from_slice(&self.waypoint_index.to_le_bytes());
        out.extend_from_slice(&self.x.to_le_bytes());
        out.extend_from_slice(&self.y.to_le_bytes());
        out.extend_from_slice(&self.target_id.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&self.task_data);
        out
    }
}

/// A decoded fleet-order-delete operation (`RTSHIPINT`, type id 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetOrderDelete {
    /// The fleet whose waypoint order(s) are removed.
    pub fleet_id: u16,
    /// The index of the order to delete.
    pub order_index: u16,
    /// Whether an *extra* trailing order is also deleted (the high bit of the
    /// index word).
    pub delete_extra: bool,
}

impl FleetOrderDelete {
    /// Decode a **decrypted** type-3 payload.
    ///
    /// Returns `None` if the payload is shorter than 4 bytes.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let raw = u16::from_le_bytes([data[2], data[3]]);
        Some(Self {
            fleet_id: u16::from_le_bytes([data[0], data[1]]),
            order_index: raw & 0x7FFF,
            delete_extra: (raw & 0x8000) != 0,
        })
    }

    /// Re-encode this operation as a type-3 payload.
    #[must_use]
    pub fn encode(&self) -> [u8; 4] {
        let index = (self.order_index & 0x7FFF) | (u16::from(self.delete_extra) << 15);
        let mut out = [0u8; 4];
        out[0..2].copy_from_slice(&self.fleet_id.to_le_bytes());
        out[2..4].copy_from_slice(&index.to_le_bytes());
        out
    }
}

/// A decoded research-settings operation (type id 34).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchOrder {
    /// Percentage of the player's resources devoted to research (`pctResearch`).
    pub pct_resources: u8,
    /// The tech field currently being researched (low nibble of the second
    /// byte): 0=Energy, 1=Weapons, 2=Propulsion, 3=Construction,
    /// 4=Electronics, 5=Biotech.
    pub current_field: u8,
    /// The "next field to research" selector (high nibble of the second byte).
    pub next_field: u8,
}

impl ResearchOrder {
    /// Decode a **decrypted** type-34 payload.
    ///
    /// Returns `None` if the payload is shorter than 2 bytes.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        Some(Self {
            pct_resources: data[0],
            current_field: data[1] & 0xF,
            next_field: (data[1] >> 4) & 0xF,
        })
    }

    /// Re-encode this operation as a type-34 payload.
    #[must_use]
    pub fn encode(&self) -> [u8; 2] {
        [
            self.pct_resources,
            (self.current_field & 0xF) | (self.next_field << 4),
        ]
    }
}

/// A decoded planet-routing operation (`RTCHGPLANETLONG`, type id 35).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanetRoutingOrder {
    /// The planet the order applies to.
    pub planet_id: u16,
    /// Whether research is disabled on this planet (`fNoResearch`).
    pub no_research: bool,
    /// The mass-driver "fling" target planet id (`idFling`, 10 bits).
    pub fling_target: u16,
    /// The fling warp factor (`iWarpFling`, 4 bits).
    pub fling_warp: u8,
    /// The fleet-route destination planet id (`idRoute`, 10 bits).
    pub route_target: u16,
    /// The NB09 `unused:7` bitfield (bits 25..=31), preserved so the record
    /// re-encodes exactly.
    pub reserved: u8,
}

impl PlanetRoutingOrder {
    /// Decode a **decrypted** type-35 payload.
    ///
    /// Returns `None` if the payload is shorter than 6 bytes.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 6 {
            return None;
        }
        let planet_id = u16::from_le_bytes([data[0], data[1]]);
        let bits = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
        Some(Self {
            planet_id,
            no_research: (bits & 0x1) != 0,
            fling_target: ((bits >> 1) & 0x3FF) as u16,
            fling_warp: ((bits >> 11) & 0xF) as u8,
            route_target: ((bits >> 15) & 0x3FF) as u16,
            reserved: ((bits >> 25) & 0x7F) as u8,
        })
    }

    /// Re-encode this operation as a type-35 payload.
    #[must_use]
    pub fn encode(&self) -> [u8; 6] {
        let bits = u32::from(self.no_research)
            | ((u32::from(self.fling_target) & 0x3FF) << 1)
            | ((u32::from(self.fling_warp) & 0xF) << 11)
            | ((u32::from(self.route_target) & 0x3FF) << 15)
            | ((u32::from(self.reserved) & 0x7F) << 25);
        let mut out = [0u8; 6];
        out[0..2].copy_from_slice(&self.planet_id.to_le_bytes());
        out[2..6].copy_from_slice(&bits.to_le_bytes());
        out
    }
}

/// A decoded transfer operation (`RTXFER` family, type ids 1/2/23/25).
///
/// A transfer moves something between two objects (`id1`/`id2`, classes
/// `grobj1`/`grobj2`). A `grbitItems` bitmask selects what, one signed quantity
/// follows per set bit, and a **positive** quantity means the first object
/// gains. The four variants differ in the width of the mask and of each
/// quantity:
///
/// | op (type id)                | mask width | quantity width |
/// |-----------------------------|-----------:|---------------:|
/// | `rtLogCargoXfer8` (1)        | `u8`       | `i8`  (`RTXFER`)  |
/// | `rtLogCargoXfer16` (2)       | `u8`       | `i16` (`RTXFERX`) |
/// | `rtLogFleetCargoXfer` (23)   | `u16`      | `i16` (`RTXFERF`) |
/// | `rtLogCargoXfer32` (25)      | `u8`       | `i32` (`RTXFERL`) |
///
/// # What the mask selects
///
/// For the three narrow variants it is the five **cargo kinds** — ironium,
/// boranium, germanium, colonists, fuel — and the quantities are amounts.
///
/// For `rtLogFleetCargoXfer` (23), the fleet-to-fleet form, it is the sixteen
/// **ship design slots**, and the quantities are ship counts. That is what the
/// wider mask is for, and it is what makes a fleet split: the client writes a
/// `rtLogFleetSplit` and then moves ships into the new fleet with one of these.
///
/// Verified across all 45 such records in the fixtures: every mask names a
/// design slot the source fleet actually holds ships of — fleet 21 of the
/// exodus game carries only design 3 and its transfers mask `0x0008`, fleet 12
/// carries only design 0 and masks `0x0001`, and a sixteen-player game's fleet
/// 10 carries only design 12 and masks `0x1000` — and every quantity is a small
/// count rather than a cargo amount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoTransfer {
    /// The first object id involved in the transfer (`id1`).
    pub id1: u16,
    /// The second object id involved in the transfer (`id2`).
    pub id2: u16,
    /// The class of the first object (`grobj1`, low nibble of byte 4).
    pub grobj1: u8,
    /// The class of the second object (`grobj2`, high nibble of byte 4).
    pub grobj2: u8,
    /// The `grbitItems` bitmask selecting which cargo categories are present
    /// (8-bit for most variants, 16-bit for `rtLogFleetCargoXfer`).
    pub items_mask: u16,
    /// One signed quantity per set bit in [`Self::items_mask`], in ascending
    /// bit order, widened to `i32`.
    pub quantities: Vec<i32>,
    /// The raw quantity region (everything after the mask), preserved verbatim.
    pub quantity_bytes: Vec<u8>,
}

impl CargoTransfer {
    /// The mask width (`true` = 16-bit) and quantity width (bytes) for an op.
    fn params(record_type: LogRecordType) -> Option<(bool, usize)> {
        match record_type {
            LogRecordType::CargoXfer8 => Some((false, 1)),
            LogRecordType::CargoXfer16 => Some((false, 2)),
            LogRecordType::CargoXfer32 => Some((false, 4)),
            LogRecordType::FleetCargoXfer => Some((true, 2)),
            _ => None,
        }
    }

    /// Decode a **decrypted** cargo-transfer payload for the given op variant.
    ///
    /// Returns `None` if `record_type` is not a cargo-transfer op or the
    /// payload is too short for the fixed prefix + mask.
    #[must_use]
    pub fn decode(data: &[u8], record_type: LogRecordType) -> Option<Self> {
        let (mask_u16, width) = Self::params(record_type)?;
        if data.len() < 5 {
            return None;
        }
        let (mask, quantity_start) = if mask_u16 {
            if data.len() < 7 {
                return None;
            }
            (u16::from_le_bytes([data[5], data[6]]), 7usize)
        } else {
            (u16::from(data[5]), 6usize)
        };
        let count = mask.count_ones() as usize;
        let mut quantities = Vec::with_capacity(count);
        let mut off = quantity_start;
        for _ in 0..count {
            if off + width > data.len() {
                break;
            }
            let q: i32 = match width {
                1 => i32::from(data[off] as i8),
                2 => i32::from(i16::from_le_bytes([data[off], data[off + 1]])),
                _ => i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]),
            };
            quantities.push(q);
            off += width;
        }
        Some(Self {
            id1: u16::from_le_bytes([data[0], data[1]]),
            id2: u16::from_le_bytes([data[2], data[3]]),
            grobj1: data[4] & 0xF,
            grobj2: (data[4] >> 4) & 0xF,
            items_mask: mask,
            quantities,
            quantity_bytes: data[quantity_start..].to_vec(),
        })
    }

    /// Re-encode this transfer as the payload of `record_type`.
    ///
    /// The quantities are written at the width the variant calls for; the mask
    /// decides how many are written, so a quantity list shorter than the mask
    /// pads with zeros.
    ///
    /// Returns `None` if `record_type` is not a cargo-transfer op.
    #[must_use]
    pub fn encode(&self, record_type: LogRecordType) -> Option<Vec<u8>> {
        let (mask_u16, width) = Self::params(record_type)?;
        let mut out = Vec::with_capacity(8 + self.quantities.len() * width);
        out.extend_from_slice(&self.id1.to_le_bytes());
        out.extend_from_slice(&self.id2.to_le_bytes());
        out.push((self.grobj1 & 0xF) | ((self.grobj2 & 0xF) << 4));
        if mask_u16 {
            out.extend_from_slice(&self.items_mask.to_le_bytes());
        } else {
            #[allow(clippy::cast_possible_truncation)]
            out.push(self.items_mask as u8);
        }
        for i in 0..self.items_mask.count_ones() as usize {
            let q = self.quantities.get(i).copied().unwrap_or(0);
            match width {
                1 => {
                    #[allow(clippy::cast_possible_truncation)]
                    out.push(q as i8 as u8);
                }
                2 => {
                    #[allow(clippy::cast_possible_truncation)]
                    out.extend_from_slice(&(q as i16).to_le_bytes());
                }
                _ => out.extend_from_slice(&q.to_le_bytes()),
            }
        }
        Some(out)
    }

    /// The narrowest **cargo** variant that can carry these quantities, which
    /// is what keeps the log small.
    ///
    /// Not for a fleet-to-fleet ship transfer, which is always
    /// [`LogRecordType::FleetCargoXfer`] because it needs the sixteen-bit mask.
    #[must_use]
    pub fn narrowest_cargo(&self) -> LogRecordType {
        if self.quantities.iter().all(|q| i8::try_from(*q).is_ok()) {
            LogRecordType::CargoXfer8
        } else if self.quantities.iter().all(|q| i16::try_from(*q).is_ok()) {
            LogRecordType::CargoXfer16
        } else {
            LogRecordType::CargoXfer32
        }
    }
}

/// A decoded repeat-orders operation (`rtLogFleetFlagBit9`, type id 10).
///
/// `{ int16_t id; int16_t value; }` — the fleet, and whether its waypoint
/// orders repeat. The replay takes `value & 1` into `FLEET.fRepOrders`, which
/// is bit 9 of the fleet's flag word and where the operation's name comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetRepeatOrders {
    /// The fleet (raw object id).
    pub fleet_id: u16,
    /// Whether its orders repeat once the last waypoint is reached.
    pub repeat: bool,
}

impl FleetRepeatOrders {
    /// Decode a **decrypted** type-10 payload.
    ///
    /// Returns `None` if the payload is shorter than 4 bytes.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        Some(Self {
            fleet_id: u16::from_le_bytes([data[0], data[1]]),
            repeat: u16::from_le_bytes([data[2], data[3]]) & 1 != 0,
        })
    }

    /// Re-encode this operation as a type-10 payload.
    #[must_use]
    pub fn encode(&self) -> [u8; 4] {
        let mut out = [0u8; 4];
        out[0..2].copy_from_slice(&self.fleet_id.to_le_bytes());
        out[2..4].copy_from_slice(&u16::from(self.repeat).to_le_bytes());
        out
    }
}

/// A decoded waypoint-task operation (`rtLogFleetOrderAttrNib`, type id 11).
///
/// `{ int16_t id; int16_t iOrder; int16_t value; }` — it sets the low nibble of
/// one waypoint's flag word, which is `ORDER.grTask`, and leaves the other
/// twelve bits alone.
///
/// The layout is read from the replay arm at `1048:c3f0`, which types 10 and 11
/// share: it looks the fleet up by the id at `+0`, takes the order index from
/// `+2`, rejects an index the fleet does not have (`FLEET.cord <= iOrder`) and a
/// **raw** `value` of 10 or more, then merges `value & 0x0F` into the waypoint's
/// flag word. Nothing in `stars.2.7j.exe` writes this record — see
/// `docs/formats/orders-x.md` — so `value` is kept whole rather than masked:
/// that keeps a record written by some other producer byte-exact through a
/// round trip, and lets the bound be tested the way the original tests it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetOrderTask {
    /// The fleet (raw object id).
    pub fleet_id: u16,
    /// Which of its waypoints.
    pub order_index: u16,
    /// The value word, raw. Only its low nibble reaches the waypoint, and the
    /// original refuses the record outright when the whole word is 10 or more.
    pub value: u16,
}

impl FleetOrderTask {
    /// Build one that sets `task`.
    #[must_use]
    pub fn new(fleet_id: u16, order_index: u16, task: u8) -> Self {
        Self {
            fleet_id,
            order_index,
            value: u16::from(task),
        }
    }

    /// The task id this record would apply — the low nibble of [`Self::value`].
    #[must_use]
    pub fn task(&self) -> u8 {
        (self.value & 0x0F) as u8
    }

    /// Whether the original's replay would accept the value at all.
    ///
    /// `1048:c46e`: `if (value >= 10) return 0`, against the whole word, so a
    /// value whose low nibble is a legal task is still refused when any higher
    /// bit is set.
    #[must_use]
    pub fn value_in_range(&self) -> bool {
        self.value < 10
    }

    /// Decode a **decrypted** type-11 payload.
    ///
    /// Returns `None` if the payload is shorter than 6 bytes.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 6 {
            return None;
        }
        Some(Self {
            fleet_id: u16::from_le_bytes([data[0], data[1]]),
            order_index: u16::from_le_bytes([data[2], data[3]]),
            value: u16::from_le_bytes([data[4], data[5]]),
        })
    }

    /// Re-encode this operation as a type-11 payload.
    #[must_use]
    pub fn encode(&self) -> [u8; 6] {
        let mut out = [0u8; 6];
        out[0..2].copy_from_slice(&self.fleet_id.to_le_bytes());
        out[2..4].copy_from_slice(&self.order_index.to_le_bytes());
        out[4..6].copy_from_slice(&self.value.to_le_bytes());
        out
    }
}

/// A decoded battle-plan operation (`rtLogFleetPlan`, type id 42).
///
/// `{ int16_t id; int16_t iplan; }` — which of the player's battle plans the
/// fleet fights under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetPlan {
    /// The fleet (raw object id).
    pub fleet_id: u16,
    /// The battle-plan slot.
    pub plan: u8,
}

impl FleetPlan {
    /// Decode a **decrypted** type-42 payload.
    ///
    /// Returns `None` if the payload is shorter than 4 bytes.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        Some(Self {
            fleet_id: u16::from_le_bytes([data[0], data[1]]),
            plan: (u16::from_le_bytes([data[2], data[3]]) & 0xFF) as u8,
        })
    }

    /// Re-encode this operation as a type-42 payload.
    #[must_use]
    pub fn encode(&self) -> [u8; 4] {
        let mut out = [0u8; 4];
        out[0..2].copy_from_slice(&self.fleet_id.to_le_bytes());
        out[2..4].copy_from_slice(&u16::from(self.plan).to_le_bytes());
        out
    }
}

/// A decoded battle-plan definition (`rtBtlPlan`, type id 30).
///
/// This is the operation that **writes a plan**: its name, tactic, target
/// preferences and who it will attack. The one that says which plan a fleet
/// fights under is [`FleetPlan`] (42).
///
/// The payload is byte-for-byte the type-30 **block** of a state file:
/// `WriteBattlePlan` (`1070:89b8`) fills one buffer and hands it either to
/// `WriteMemRt` for the log or to the block writer for the file, so
/// [`BattlePlanRecord`] decodes both. See `docs/formats/battleplan.md`.
///
/// A **delete** is written short: when byte 1 has [`PLAN_DELETED`] set,
/// `WriteBattlePlan` stops after the first word and the record is two bytes,
/// with no targets and no name (`1070:89f0`). The replay tests the same bit
/// before it reads any further, so the short form is not a truncation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattlePlanChange {
    /// The plan as written. On a delete only `race_id`, `plan_id` and the
    /// `tactic` byte carrying the flag are meaningful; the rest is zero.
    pub plan: BattlePlanRecord,
    /// Whether this deletes the plan rather than defining it.
    pub delete: bool,
}

impl BattlePlanChange {
    /// Decode a **decrypted** type-30 payload, in either form.
    ///
    /// Returns `None` if the payload is shorter than the two bytes a delete
    /// needs, or if a definition is malformed.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        if data[1] & PLAN_DELETED != 0 {
            return Some(Self {
                plan: BattlePlanRecord {
                    race_id: data[0] & 0x0F,
                    plan_id: data[0] >> 4,
                    tactic: data[1],
                    primary_target: 0,
                    secondary_target: 0,
                    attack_who: 0,
                    name: String::new(),
                    trailing: Vec::new(),
                },
                delete: true,
            });
        }
        Some(Self {
            plan: BattlePlanRecord::from_payload(data).ok()?,
            delete: false,
        })
    }

    /// Re-encode this operation as a type-30 payload.
    ///
    /// The [`PLAN_DELETED`] bit is forced to agree with [`Self::delete`], so a
    /// record built by hand cannot disagree with itself.
    ///
    /// # Errors
    /// Propagates [`BattlePlanRecord::encode`] on a definition whose name does
    /// not fit its length byte.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let byte0 = (self.plan.race_id & 0x0F) | (self.plan.plan_id << 4);
        if self.delete {
            return Ok(vec![byte0, self.plan.tactic | PLAN_DELETED]);
        }
        let mut plan = self.plan.clone();
        plan.tactic &= !PLAN_DELETED;
        plan.encode()
    }
}

/// A decoded password change (`rtChgPassword`, type id 36).
///
/// `{ int32_t lSalt; }` — four bytes, and nothing else: Stars! stores a
/// checksum of the typed password rather than the password, and this record
/// carries the same value the player block holds at
/// [`PASSWORD_OFFSET`](crate::PASSWORD_OFFSET). `0` means the password was
/// cleared. See [`crate::password`] for how the value is derived and what it
/// is worth.
///
/// Written by `NewPasswordDlg` (`1040:5ec7`) — but only when a player's game is
/// open. Asked the same question with no game loaded, the dialog is setting the
/// **host's** password and stores it in a global instead of logging anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordChange {
    /// The salt of the new password; `0` for none.
    pub salt: u32,
}

impl PasswordChange {
    /// Decode a **decrypted** type-36 payload.
    ///
    /// Returns `None` if the payload is shorter than 4 bytes.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        Some(Self {
            salt: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
        })
    }

    /// Re-encode this operation as a type-36 payload.
    #[must_use]
    pub fn encode(&self) -> [u8; 4] {
        self.salt.to_le_bytes()
    }
}

/// A decoded player-relations operation (`rtLogRelations`, type id 38).
///
/// One byte per player in the game — `0` neutral, `1` friend, `2` enemy — which
/// is why the record is as long as the player count. The whole table is written
/// each time, and the client **replaces** a relations record it has already
/// written this turn rather than appending a second, so only the last one in a
/// log matters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relations {
    /// How the submitting player regards each player, indexed by player number.
    pub toward: Vec<u8>,
}

impl Relations {
    /// Decode a **decrypted** type-38 payload.
    #[must_use]
    pub fn decode(data: &[u8]) -> Self {
        Self {
            toward: data.to_vec(),
        }
    }

    /// Re-encode this operation as a type-38 payload.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.toward.clone()
    }
}

/// A decoded fleet-split operation (`rtLogFleetSplit`, type id 24).
///
/// Two bytes: the object id of the fleet being split. It says nothing about
/// what leaves, because the client writes a fleet-to-fleet ship transfer
/// straight afterwards naming the new fleet and the ships that move into it —
/// see [`CargoTransfer`]. All 44 split records in the fixtures are exactly two
/// bytes and every one is followed by such a transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetSplit {
    /// The fleet being split (raw object id).
    pub fleet_id: u16,
}

impl FleetSplit {
    /// Decode a **decrypted** type-24 payload.
    ///
    /// Returns `None` if the payload is shorter than 2 bytes.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        Some(Self {
            fleet_id: u16::from_le_bytes([data[0], data[1]]),
        })
    }

    /// Re-encode this operation as a type-24 payload.
    #[must_use]
    pub fn encode(&self) -> [u8; 2] {
        self.fleet_id.to_le_bytes()
    }
}

/// A decoded fleet-merge operation (`rtLogFleetMerge`, type id 37).
///
/// A list of fleet object ids, two bytes each. **The first survives and the
/// rest are absorbed into it.**
///
/// The direction is read off the corpus rather than the struct: of the nine
/// merges in the exodus game, seven have the first fleet present in the next
/// year's state file and every other one gone. The two exceptions are
/// explicable — one merge is followed by the surviving fleet being destroyed,
/// and fleet numbers are reused once a fleet dies — and no case has the first
/// fleet vanish while a later one survives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetMerge {
    /// The fleets, survivor first.
    pub fleets: Vec<u16>,
}

impl FleetMerge {
    /// Decode a **decrypted** type-37 payload.
    ///
    /// Returns `None` if the payload does not name at least two fleets.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        Some(Self {
            fleets: data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect(),
        })
    }

    /// The fleet everything merges into.
    #[must_use]
    pub fn survivor(&self) -> Option<u16> {
        self.fleets.first().copied()
    }

    /// The fleets absorbed into [`Self::survivor`].
    #[must_use]
    pub fn absorbed(&self) -> &[u16] {
        self.fleets.get(1..).unwrap_or_default()
    }

    /// Re-encode this operation as a type-37 payload.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.fleets.len() * 2);
        for fleet in &self.fleets {
            out.extend_from_slice(&fleet.to_le_bytes());
        }
        out
    }
}

/// A decoded fleet-rename operation (`RTCHGNAME`, type id 44).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetName {
    /// The object being renamed (raw object id; see [`object_owner`] /
    /// [`object_index`]).
    pub id: u16,
    /// The object class (`grobj`).
    pub grobj: u16,
    /// The new name, decoded from the trailing packed-string field. A leading
    /// length byte of `0` means the name was stored as a literal C string
    /// rather than the packed encoding; both are handled by the decoder.
    pub name: String,
}

impl FleetName {
    /// Decode a **decrypted** `RTCHGNAME` payload (type id 44).
    ///
    /// Returns `None` if the payload is shorter than the 4-byte fixed part.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        Some(Self {
            id: u16::from_le_bytes([data[0], data[1]]),
            grobj: u16::from_le_bytes([data[2], data[3]]),
            name: decode_user_string(&data[4..]),
        })
    }

    /// Re-encode this rename as a `RTCHGNAME` payload (type id 44).
    ///
    /// The name goes through the user-string codec, the same one the fleet's
    /// own name block uses — see [`crate::strings::encode_user_string`].
    /// Nothing in the fixtures renames a fleet, so this is derived from the
    /// struct rather than fixture-verified.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.name.len());
        out.extend_from_slice(&self.id.to_le_bytes());
        out.extend_from_slice(&self.grobj.to_le_bytes());
        out.extend_from_slice(&encode_user_string(&self.name));
        out
    }
}

/// A decoded ship-design-change operation (`RTCHGSHDEF`, type id 27).
///
/// The op carries a packed header word (`mdChg`/`iPlr`/`ishdef`) followed by an
/// embedded ship-design record (`RTSHDEF`, the same layout as a type-26 design
/// block). A pure *delete* carries only the header word and no design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipDesignChange {
    /// The change mode (`mdChg`, low nibble): add / update / delete.
    pub mode: u8,
    /// The owning player index (`iPlr`, next nibble).
    pub player: u8,
    /// The design slot index (`ishdef`, 5 bits at bit 8).
    pub design_index: u8,
    /// The NB09 `junk:3` field (bits 13..=15 of the header word), preserved so
    /// the record re-encodes exactly.
    pub header_high: u8,
    /// The embedded design (`RTSHDEF`); `None` for a delete (header only).
    pub design: Option<DesignRecord>,
}

impl ShipDesignChange {
    /// Decode a **decrypted** `RTCHGSHDEF` payload (type id 27).
    ///
    /// Returns `None` if the payload is shorter than the 2-byte header word.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        let hdr = u16::from_le_bytes([data[0], data[1]]);
        let design = if data.len() > 2 {
            DesignRecord::from_payload(&data[2..]).ok()
        } else {
            None
        };
        Some(Self {
            mode: (hdr & 0xF) as u8,
            player: ((hdr >> 4) & 0xF) as u8,
            design_index: ((hdr >> 8) & 0x1F) as u8,
            header_high: ((hdr >> 13) & 0x7) as u8,
            design,
        })
    }

    /// Re-encode this change as a `RTCHGSHDEF` payload (type id 27).
    ///
    /// # Errors
    /// Propagates [`DesignRecord::encode`]'s error when the embedded design
    /// does not fit its block.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let hdr = u16::from(self.mode & 0xF)
            | (u16::from(self.player & 0xF) << 4)
            | (u16::from(self.design_index & 0x1F) << 8)
            | (u16::from(self.header_high & 0x7) << 13);
        let mut out = hdr.to_le_bytes().to_vec();
        if let Some(design) = &self.design {
            out.extend_from_slice(&design.encode()?);
        }
        Ok(out)
    }
}

/// A decoded `THING`-parameter operation (`RTLOGTHING`, type id 43).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThingParam {
    /// The full object id of the `THING` (`idFull`).
    pub id_full: u16,
    /// The parameter written into the `THING` (`fDetonate` — e.g. arm/disarm a
    /// mine field or detonation flag).
    pub param: i16,
}

impl ThingParam {
    /// Decode a **decrypted** `RTLOGTHING` payload (type id 43).
    ///
    /// Returns `None` if the payload is shorter than 4 bytes.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        Some(Self {
            id_full: u16::from_le_bytes([data[0], data[1]]),
            param: i16::from_le_bytes([data[2], data[3]]),
        })
    }

    /// Re-encode this operation as a `RTLOGTHING` payload (type id 43).
    #[must_use]
    pub fn encode(&self) -> [u8; 4] {
        let mut out = [0u8; 4];
        out[0..2].copy_from_slice(&self.id_full.to_le_bytes());
        out[2..4].copy_from_slice(&self.param.to_le_bytes());
        out
    }
}

/// One classified record in an order log: its type plus the raw decrypted
/// payload. Typed views are decoded on demand via the accessor methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord {
    /// The classified record type.
    pub record_type: LogRecordType,
    /// The raw decrypted payload bytes of the block.
    pub data: Vec<u8>,
}

impl LogRecord {
    /// Decode this record as a fleet-waypoint order (insert/update).
    #[must_use]
    pub fn as_waypoint(&self) -> Option<WaypointOrder> {
        matches!(
            self.record_type,
            LogRecordType::FleetOrderInsert | LogRecordType::FleetOrderUpdate
        )
        .then(|| WaypointOrder::decode(&self.data))
        .flatten()
    }

    /// Decode this record as a fleet-order-delete operation.
    #[must_use]
    pub fn as_fleet_order_delete(&self) -> Option<FleetOrderDelete> {
        (self.record_type == LogRecordType::FleetOrderDelete)
            .then(|| FleetOrderDelete::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a research-settings operation.
    #[must_use]
    pub fn as_research(&self) -> Option<ResearchOrder> {
        (self.record_type == LogRecordType::Research)
            .then(|| ResearchOrder::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a planet-routing operation.
    #[must_use]
    pub fn as_planet_routing(&self) -> Option<PlanetRoutingOrder> {
        (self.record_type == LogRecordType::PlanetRouting)
            .then(|| PlanetRoutingOrder::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a cargo-transfer operation, including the
    /// per-item quantities (their width is taken from the op variant).
    #[must_use]
    pub fn as_cargo_transfer(&self) -> Option<CargoTransfer> {
        CargoTransfer::decode(&self.data, self.record_type)
    }

    /// Decode this record as a fleet-rename operation.
    #[must_use]
    pub fn as_fleet_name(&self) -> Option<FleetName> {
        (self.record_type == LogRecordType::FleetName)
            .then(|| FleetName::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a ship-design-change operation.
    #[must_use]
    pub fn as_ship_design_change(&self) -> Option<ShipDesignChange> {
        (self.record_type == LogRecordType::ShipDesign)
            .then(|| ShipDesignChange::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a production-queue-change operation (planet id +
    /// queued items).
    #[must_use]
    pub fn as_production_queue(&self) -> Option<ProductionQueueRecord> {
        (self.record_type == LogRecordType::PlanetProdQueue)
            .then(|| ProductionQueueRecord::decode_change(&self.data))
            .flatten()
    }

    /// Decode this record as a repeat-orders change.
    #[must_use]
    pub fn as_repeat_orders(&self) -> Option<FleetRepeatOrders> {
        (self.record_type == LogRecordType::FleetFlagBit)
            .then(|| FleetRepeatOrders::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a waypoint-task change.
    #[must_use]
    pub fn as_order_task(&self) -> Option<FleetOrderTask> {
        (self.record_type == LogRecordType::FleetOrderAttrNib)
            .then(|| FleetOrderTask::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a battle-plan change.
    #[must_use]
    pub fn as_fleet_plan(&self) -> Option<FleetPlan> {
        (self.record_type == LogRecordType::FleetPlan)
            .then(|| FleetPlan::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a battle-plan definition.
    #[must_use]
    pub fn as_battle_plan(&self) -> Option<BattlePlanChange> {
        (self.record_type == LogRecordType::BattlePlan)
            .then(|| BattlePlanChange::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a password change.
    #[must_use]
    pub fn as_password_change(&self) -> Option<PasswordChange> {
        (self.record_type == LogRecordType::ChangePassword)
            .then(|| PasswordChange::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a player-relations change.
    #[must_use]
    pub fn as_relations(&self) -> Option<Relations> {
        (self.record_type == LogRecordType::Relations).then(|| Relations::decode(&self.data))
    }

    /// Decode this record as a fleet split.
    #[must_use]
    pub fn as_fleet_split(&self) -> Option<FleetSplit> {
        (self.record_type == LogRecordType::FleetSplit)
            .then(|| FleetSplit::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a fleet merge.
    #[must_use]
    pub fn as_fleet_merge(&self) -> Option<FleetMerge> {
        (self.record_type == LogRecordType::FleetMerge)
            .then(|| FleetMerge::decode(&self.data))
            .flatten()
    }

    /// Decode this record as a `THING`-parameter operation.
    #[must_use]
    pub fn as_thing_param(&self) -> Option<ThingParam> {
        (self.record_type == LogRecordType::ThingByteParam)
            .then(|| ThingParam::decode(&self.data))
            .flatten()
    }
}

impl LogRecord {
    /// A record holding an already-encoded payload.
    #[must_use]
    pub fn raw(record_type: LogRecordType, data: Vec<u8>) -> Self {
        Self { record_type, data }
    }

    /// Insert (`insert`) or overwrite a fleet's waypoint order.
    #[must_use]
    pub fn waypoint(order: &WaypointOrder, insert: bool) -> Self {
        Self::raw(
            if insert {
                LogRecordType::FleetOrderInsert
            } else {
                LogRecordType::FleetOrderUpdate
            },
            order.encode(),
        )
    }

    /// Delete a fleet's waypoint order.
    #[must_use]
    pub fn delete_waypoint(order: FleetOrderDelete) -> Self {
        Self::raw(LogRecordType::FleetOrderDelete, order.encode().to_vec())
    }

    /// Change the research settings.
    #[must_use]
    pub fn research_settings(order: ResearchOrder) -> Self {
        Self::raw(LogRecordType::Research, order.encode().to_vec())
    }

    /// Change a planet's routing, fling and no-research settings.
    #[must_use]
    pub fn planet_routing(order: PlanetRoutingOrder) -> Self {
        Self::raw(LogRecordType::PlanetRouting, order.encode().to_vec())
    }

    /// Move cargo, in the narrowest variant that carries the quantities.
    #[must_use]
    pub fn cargo(transfer: &CargoTransfer) -> Self {
        let record_type = transfer.narrowest_cargo();
        let data = transfer.encode(record_type).unwrap_or_default();
        Self::raw(record_type, data)
    }

    /// Move **ships** between two fleets: the mask names design slots.
    #[must_use]
    pub fn ships(transfer: &CargoTransfer) -> Self {
        let data = transfer
            .encode(LogRecordType::FleetCargoXfer)
            .unwrap_or_default();
        Self::raw(LogRecordType::FleetCargoXfer, data)
    }

    /// Split a fleet, which the client follows with a [`Self::ships`] transfer
    /// naming the new fleet.
    #[must_use]
    pub fn split_fleet(fleet: FleetSplit) -> Self {
        Self::raw(LogRecordType::FleetSplit, fleet.encode().to_vec())
    }

    /// Set whether a fleet's waypoint orders repeat.
    #[must_use]
    pub fn repeat_orders(order: FleetRepeatOrders) -> Self {
        Self::raw(LogRecordType::FleetFlagBit, order.encode().to_vec())
    }

    /// Set the task on one of a fleet's waypoints.
    #[must_use]
    pub fn order_task(order: FleetOrderTask) -> Self {
        Self::raw(LogRecordType::FleetOrderAttrNib, order.encode().to_vec())
    }

    /// Set which battle plan a fleet fights under.
    #[must_use]
    pub fn fleet_plan(order: FleetPlan) -> Self {
        Self::raw(LogRecordType::FleetPlan, order.encode().to_vec())
    }

    /// Define, overwrite or delete one of the player's battle plans.
    ///
    /// # Errors
    /// Propagates [`BattlePlanChange::encode`].
    pub fn battle_plan(change: &BattlePlanChange) -> Result<Self> {
        Ok(Self::raw(LogRecordType::BattlePlan, change.encode()?))
    }

    /// Change the player's turn password.
    #[must_use]
    pub fn change_password(change: PasswordChange) -> Self {
        Self::raw(LogRecordType::ChangePassword, change.encode().to_vec())
    }

    /// Set how the player regards everyone.
    #[must_use]
    pub fn relations(order: &Relations) -> Self {
        Self::raw(LogRecordType::Relations, order.encode())
    }

    /// Merge fleets into the first of them.
    #[must_use]
    pub fn merge_fleets(merge: &FleetMerge) -> Self {
        Self::raw(LogRecordType::FleetMerge, merge.encode())
    }

    /// Replace a planet's production queue.
    #[must_use]
    pub fn production_queue(queue: &ProductionQueueRecord) -> Self {
        Self::raw(LogRecordType::PlanetProdQueue, queue.encode_change())
    }

    /// Create, change or delete a ship design.
    ///
    /// # Errors
    /// Propagates the design encoder's error.
    pub fn ship_design(change: &ShipDesignChange) -> Result<Self> {
        Ok(Self::raw(LogRecordType::ShipDesign, change.encode()?))
    }

    /// Rename a fleet.
    #[must_use]
    pub fn fleet_name(rename: &FleetName) -> Self {
        Self::raw(LogRecordType::FleetName, rename.encode())
    }

    /// Set a byte inside a space object (arm a minefield, say).
    #[must_use]
    pub fn thing_param(param: ThingParam) -> Self {
        Self::raw(LogRecordType::ThingByteParam, param.encode().to_vec())
    }

    /// The framed size of this record: its payload plus the two-byte block
    /// header, which is the unit `RTLOGHDR.cbLog` counts in.
    #[must_use]
    pub fn framed_len(&self) -> usize {
        self.data.len() + 2
    }
}

/// A fully-parsed `.xN` order log: the header plus the operation records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderLog {
    /// The order-log header (`RTLOGHDR`), if present.
    pub header: Option<LogHeader>,
    /// The operation records, in file (replay) order. The file header (type 8)
    /// and the log header (type 9) are excluded; the trailing footer (type 0)
    /// is excluded too.
    pub records: Vec<LogRecord>,
}

impl OrderLog {
    /// An empty log, with a header carrying the given identity.
    ///
    /// `serial_number` is the **registration serial of the copy of Stars! that
    /// wrote the file** (`vSerialNumber`), and `config` an eleven-byte
    /// fingerprint of the machine it was written on (`vrgbEnvCur`). The host
    /// uses the pair to notice two players submitting from one registration.
    /// An unregistered copy carries zero, which is what this project writes
    /// unless a caller passes something it copied from a file the real client
    /// produced.
    #[must_use]
    pub fn new(serial_number: i32, config: [u8; 11]) -> Self {
        Self {
            header: Some(LogHeader {
                log_byte_count: 0,
                serial_number,
                config,
            }),
            records: Vec::new(),
        }
    }

    /// The framed size of every record, which is what `cbLog` counts.
    #[must_use]
    pub fn log_byte_count(&self) -> usize {
        self.records.iter().map(LogRecord::framed_len).sum()
    }

    /// Assemble this log into a complete `.xN` file.
    ///
    /// `cbLog` is recomputed from the records, so a caller need not maintain
    /// it. The file header must name [`crate::FileType::Orders`] and the player
    /// whose orders these are.
    ///
    /// # Errors
    /// [`FormatError::Malformed`] if the log is longer than `cbLog` can count,
    /// or a record does not fit its block.
    pub fn to_file(&self, header: &FileHeader) -> Result<Vec<u8>> {
        let count = u16::try_from(self.log_byte_count()).map_err(|_| {
            FormatError::Malformed(format!(
                "an order log of {} bytes is longer than cbLog can count",
                self.log_byte_count()
            ))
        })?;
        let log_header = LogHeader {
            log_byte_count: count,
            ..self.header.unwrap_or(LogHeader {
                log_byte_count: 0,
                serial_number: 0,
                config: [0; 11],
            })
        };

        let mut body = vec![crate::block::Block::new(
            LOG_HEADER_BLOCK,
            log_header.encode().to_vec(),
        )?];
        for record in &self.records {
            body.push(crate::block::Block::new(
                record.record_type.id(),
                record.data.clone(),
            )?);
        }
        StarsFile::build(header, &body, Vec::new())
    }
}

/// Parse a decoded [`StarsFile`] as an order log.
///
/// The file/plaintext header block and the log header (`RTLOGHDR`) are lifted
/// out; every remaining non-footer block becomes a classified [`LogRecord`].
#[must_use]
pub fn order_log(file: &StarsFile) -> OrderLog {
    let mut header = None;
    let mut records = Vec::new();
    for block in &file.blocks {
        match block.type_id {
            crate::file::FILE_FOOTER_BLOCK => {}
            8 => {} // plaintext file header (RTBOF)
            LOG_HEADER_BLOCK => header = LogHeader::decode(&block.data),
            id => records.push(LogRecord {
                record_type: LogRecordType::from_id(id),
                data: block.data.clone(),
            }),
        }
    }
    OrderLog { header, records }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_type_id_round_trips() {
        for id in 0u8..=63 {
            assert_eq!(LogRecordType::from_id(id).id(), id);
        }
    }

    #[test]
    fn decodes_log_header() {
        let mut d = Vec::new();
        d.extend_from_slice(&168u16.to_le_bytes());
        d.extend_from_slice(&0x006d7776i32.to_le_bytes());
        d.extend_from_slice(&[
            0xa5, 0xdc, 0xa6, 0x59, 0x7a, 0xc5, 0xa5, 0xdc, 0xa6, 0x7a, 0xa0,
        ]);
        let h = LogHeader::decode(&d).unwrap();
        assert_eq!(h.log_byte_count, 168);
        assert_eq!(h.serial_number, 0x006d7776);
        assert_eq!(h.config[0], 0xa5);
        assert_eq!(h.config[10], 0xa0);
    }

    #[test]
    fn rejects_truncated_log_header() {
        assert!(LogHeader::decode(&[0u8; 16]).is_none());
    }

    #[test]
    fn object_id_owner_and_index() {
        // 0x0a18 = fleet 24 owned by player index 5 (i.e. player 6).
        assert_eq!(object_owner(0x0a18), 5);
        assert_eq!(object_index(0x0a18), 24);
    }

    #[test]
    fn decodes_waypoint_update() {
        // fleet 0x0a18, waypoint 1, (1432,1504) -> target 187, warp 7, task 1.
        let d = [
            0x18, 0x0a, 0x01, 0x00, 0x98, 0x05, 0xe0, 0x05, 0xbb, 0x00, 0x71, 0x11, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x20,
        ];
        let w = WaypointOrder::decode(&d).unwrap();
        assert_eq!(w.fleet_id, 0x0a18);
        assert_eq!(object_owner(w.fleet_id), 5);
        assert_eq!(w.waypoint_index, 1);
        assert_eq!(w.x, 1432);
        assert_eq!(w.y, 1504);
        assert_eq!(w.target_id, 187);
        assert_eq!(w.task, 1);
        assert_eq!(w.warp, 7);
        assert_eq!(w.grobj, 1);
        assert!(w.valid_task);
        assert_eq!(w.task_data.len(), 8);
    }

    #[test]
    fn decodes_fleet_order_delete() {
        let d = [0x12, 0x0a, 0x05, 0x00];
        let del = FleetOrderDelete::decode(&d).unwrap();
        assert_eq!(del.fleet_id, 0x0a12);
        assert_eq!(del.order_index, 5);
        assert!(!del.delete_extra);
    }

    #[test]
    fn decodes_research() {
        let d = [0x1e, 0x62];
        let r = ResearchOrder::decode(&d).unwrap();
        assert_eq!(r.pct_resources, 30);
        assert_eq!(r.current_field, 2);
        assert_eq!(r.next_field, 6);
    }

    #[test]
    fn decodes_planet_routing() {
        let d = [0xe1, 0x00, 0x01, 0x00, 0x00, 0x00];
        let r = PlanetRoutingOrder::decode(&d).unwrap();
        assert_eq!(r.planet_id, 225);
        assert!(r.no_research);
        assert_eq!(r.fling_target, 0);
    }

    #[test]
    fn decodes_cargo_transfer_xfer8() {
        // id1=0x0a10, id2=0x0020, classes 2/1, mask 0x08 (one item), qty i8 -1.
        let d = [0x10, 0x0a, 0x20, 0x00, 0x12, 0x08, 0xff];
        let x = CargoTransfer::decode(&d, LogRecordType::CargoXfer8).unwrap();
        assert_eq!(x.id1, 0x0a10);
        assert_eq!(x.id2, 0x0020);
        assert_eq!(x.grobj1, 2);
        assert_eq!(x.grobj2, 1);
        assert_eq!(x.items_mask, 0x08);
        assert_eq!(x.quantities, vec![-1]);
        assert_eq!(x.quantity_bytes, vec![0xff]);
    }

    #[test]
    fn decodes_cargo_transfer_fleet_u16_mask_i16_qty() {
        // FleetCargoXfer: 16-bit mask 0x0001 (one item), one i16 quantity 300.
        let d = [0x10, 0x0a, 0x12, 0x0a, 0x00, 0x01, 0x00, 0x2c, 0x01];
        let x = CargoTransfer::decode(&d, LogRecordType::FleetCargoXfer).unwrap();
        assert_eq!(x.items_mask, 0x0001);
        assert_eq!(x.quantities, vec![300]);
    }

    #[test]
    fn non_cargo_type_yields_no_transfer() {
        assert!(CargoTransfer::decode(&[0u8; 8], LogRecordType::Research).is_none());
    }

    #[test]
    fn decodes_fleet_name() {
        // id, grobj, then a packed-string field: len=1, one nibble byte 0x12
        // -> "ae" (single-nibble table).
        let d = [0x10, 0x0a, 0x01, 0x00, 0x01, 0x12];
        let n = FleetName::decode(&d).unwrap();
        assert_eq!(n.id, 0x0a10);
        assert_eq!(n.grobj, 1);
        assert_eq!(n.name, "ae");
    }

    #[test]
    fn decodes_ship_design_change_delete_has_no_design() {
        // Header only (delete): word 0x0450 -> mdChg=0, iPlr=5, ishdef=4.
        let d = [0x50, 0x04];
        let c = ShipDesignChange::decode(&d).unwrap();
        assert_eq!(c.mode, 0);
        assert_eq!(c.player, 5);
        assert_eq!(c.design_index, 4);
        assert!(c.design.is_none());
    }

    /// The first type-30 block of `fixtures/incoming/turn0/Game.hst`: player 0's
    /// "Default" plan. A log record carries exactly these bytes, because
    /// `WriteBattlePlan` fills one buffer for both sinks.
    const DEFAULT_PLAN: [u8; 10] = [0x00, 0x04, 0x13, 0x02, 0x05, 0xb3, 0x2d, 0x71, 0xde, 0x5a];

    #[test]
    fn decodes_a_battle_plan_definition() {
        let change = BattlePlanChange::decode(&DEFAULT_PLAN).unwrap();
        assert!(!change.delete);
        assert_eq!(change.plan.race_id, 0);
        assert_eq!(change.plan.plan_id, 0);
        assert_eq!(change.plan.tactic, 4);
        assert_eq!(change.plan.primary_target, 3);
        assert_eq!(change.plan.secondary_target, 1);
        assert_eq!(change.plan.attack_who, 2);
        assert_eq!(change.plan.name, "Default");
        assert_eq!(change.encode().unwrap(), DEFAULT_PLAN);
    }

    #[test]
    fn a_deleted_battle_plan_is_two_bytes() {
        // Player 2's plan 3, deleted: the flag is bit 6 of byte 1, which is
        // bit 14 of the word `WriteBattlePlan` tests before it stops.
        let d = [0x32, 0x40];
        let change = BattlePlanChange::decode(&d).unwrap();
        assert!(change.delete);
        assert_eq!(change.plan.race_id, 2);
        assert_eq!(change.plan.plan_id, 3);
        assert_eq!(change.encode().unwrap(), d);
    }

    #[test]
    fn the_delete_flag_and_the_form_cannot_disagree() {
        // A definition built from a record whose flag bit is set loses the bit
        // rather than writing a full record the host would read as a delete.
        let mut change = BattlePlanChange::decode(&DEFAULT_PLAN).unwrap();
        change.plan.tactic |= PLAN_DELETED;
        let encoded = change.encode().unwrap();
        assert_eq!(encoded, DEFAULT_PLAN);
        // And a delete ignores everything but the first two bytes.
        change.delete = true;
        assert_eq!(change.encode().unwrap(), [0x00, 0x44]);
    }

    #[test]
    fn decodes_thing_param() {
        let d = [0x2a, 0x00, 0x01, 0x00];
        let t = ThingParam::decode(&d).unwrap();
        assert_eq!(t.id_full, 0x2a);
        assert_eq!(t.param, 1);
    }
}
