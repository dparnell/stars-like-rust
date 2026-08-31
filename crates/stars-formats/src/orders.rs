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

use crate::design::DesignRecord;
use crate::file::StarsFile;
use crate::production::ProductionQueueRecord;
use crate::strings::decode_field;

/// The block type id of the order-log header record (`RTLOGHDR`).
pub const LOG_HEADER_BLOCK: u8 = 9;

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
            task_data: data[12..].to_vec(),
        })
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
        })
    }
}

/// A decoded cargo-transfer operation (`RTXFER` family, type ids 1/2/23/25).
///
/// A transfer moves cargo between two objects (`id1`/`id2`, classes
/// `grobj1`/`grobj2`). A `grbitItems` bitmask selects which cargo categories
/// are present, and one signed quantity follows per set bit. The four op
/// variants differ only in the width of the mask and of each quantity:
///
/// | op (type id)                | mask width | quantity width |
/// |-----------------------------|-----------:|---------------:|
/// | `rtLogCargoXfer8` (1)        | `u8`       | `i8`  (`RTXFER`)  |
/// | `rtLogCargoXfer16` (2)       | `u8`       | `i16` (`RTXFERX`) |
/// | `rtLogFleetCargoXfer` (23)   | `u16`      | `i16` (`RTXFERF`) |
/// | `rtLogCargoXfer32` (25)      | `u8`       | `i32` (`RTXFERL`) |
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
            name: decode_field(&data[4..]),
        })
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
            design,
        })
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

    /// Decode this record as a `THING`-parameter operation.
    #[must_use]
    pub fn as_thing_param(&self) -> Option<ThingParam> {
        (self.record_type == LogRecordType::ThingByteParam)
            .then(|| ThingParam::decode(&self.data))
            .flatten()
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

    #[test]
    fn decodes_thing_param() {
        let d = [0x2a, 0x00, 0x01, 0x00];
        let t = ThingParam::decode(&d).unwrap();
        assert_eq!(t.id_full, 0x2a);
        assert_eq!(t.param, 1);
    }
}
