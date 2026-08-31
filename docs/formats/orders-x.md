# `.xN` player-orders file (the order log)

**Status:** container round-trips byte-for-byte; the log header and the common
operation records are **decoded & verified**. Implemented in
[`stars-formats::orders`](../../crates/stars-formats/src/orders.rs); verified by
[`tests/orders_files.rs`](../../crates/stars-formats/tests/orders_files.rs)
against the 40-turn `fixtures/games/exodus/*/EXODUS.X6` sequence.

**Reference:** the NB09 debug symbols of `Stars! 2.7j` (`structs.h`, `enums.h`,
`log.c`; see [`nb09-structs.md`](nb09-structs.md)), cross-checked field-by-field
against the exodus fixtures.

## What a `.xN` file is

A `.xN` file (`dt = dtLog = 1`) holds one player's **submitted orders** for a
turn. Unlike `.hst`/`.mN` state files — a flat set of one-record-per-type blocks
— an orders file is a **log**: a sequence of *operations* the host replays
against its in-memory state (insert a waypoint, transfer cargo, change a
production queue, set research, …).

The overall block layout is the standard Stars! container:

```
[ rtBOF (8)      ] plaintext file header (see header.rs / record-types.md)
[ RTLOGHDR (9)   ] the order-log header (see below)
[ op record ...  ] zero or more rtLog* operation records, in replay order
[ rtEOF (0)      ] footer / terminator
```

Every block except the header/footer is XOR-encrypted with the usual keystream,
so the container decodes/encodes byte-for-byte through
[`StarsFile`](../../crates/stars-formats/src/file.rs) unchanged.

## Order-log header (`RTLOGHDR`, type id 9)

The first record after the file header. **Note:** the recovered `RecordType`
enum jumps 8 → 10, so id 9 is not named there, but the debug symbols carry the
struct and `cbRTLOGHDR = 17`.

```c
typedef struct _rtloghdr {
    int16_t cbLog;         /* +0x00 (2) total framed bytes of the op records that follow */
    int32_t lSerialNumber; /* +0x02 (4) per-game/player serial (host validation) */
    uint8_t rgbConfig[11]; /* +0x06 (11) config/verification bytes */
} RTLOGHDR;                /* size=0x11 = 17 */
```

Verified on all 40 exodus files:

- `cbLog` exactly equals the sum of the framed sizes (`2 + payload`) of every
  operation record between this header and the footer — i.e. `filesize − 39`
  (18-byte file header block + 19-byte log-header block + 2-byte footer).
- `lSerialNumber` and the 11 `rgbConfig` bytes are **constant across the whole
  game** (all 40 turns), confirming they are per-game/player identity, not
  per-turn data.

Decoded by [`LogHeader`](../../crates/stars-formats/src/orders.rs).

## Operation record types (`rtLog*`)

Each operation is a block whose type id names the operation. In a `.xN` file
these ids mean the **log** variant, distinct from the same id in a state file
(e.g. id 27 = *ship-design change* here, id 29 = *production-queue change*).
[`LogRecordType`](../../crates/stars-formats/src/orders.rs) classifies them:

| id | `rtLog*` name           | operation                                    |
|---:|-------------------------|----------------------------------------------|
| 1  | `rtLogCargoXfer8`       | cargo transfer, int8 quantities              |
| 2  | `rtLogCargoXfer16`      | cargo transfer, int16 quantities             |
| 3  | `rtLogFleetOrderDelete` | delete a fleet waypoint order                |
| 4  | `rtLogFleetOrderInsert` | insert a fleet waypoint order                |
| 5  | `rtLogFleetOrderUpdate` | overwrite a fleet waypoint order             |
| 10 | `rtLogFleetFlagBit9`    | set/clear a fleet flag bit                   |
| 11 | `rtLogFleetOrderAttrNib`| set a fleet order attribute nibble           |
| 23 | `rtLogFleetCargoXfer`   | fleet-to-fleet cargo transfer                |
| 24 | `rtLogFleetSplit`       | split a fleet                                |
| 25 | `rtLogCargoXfer32`      | cargo transfer, int32 quantities             |
| 27 | `rtLogShDef`            | create/update/delete a ship design           |
| 29 | `rtLogPlanetProdQ`      | set/clear a planet's production queue         |
| 34 | `rtLogResearch`         | research settings                            |
| 35 | `rtLogPlanetRouting`    | planet routing / starbase / infra bits       |
| 37 | `rtLogFleetMerge`       | merge fleets                                 |
| 38 | `rtLogRelations`        | player-relations table                       |
| 42 | `rtLogFleetPlan`        | set a fleet's battle plan                    |
| 43 | `rtLogThingByteParam`   | set one byte inside a `THING` union          |
| 44 | `rtLogFleetName`        | fleet rename (packed/compressed string)      |
| 46 | `rtLogPlayerZpq1`       | host-only opaque blob                        |

All operation ids seen across the 40 exodus files classify to a known type (no
`Other(_)`).

### Object ids encode the owner

Fleet/planet ids in these operations are Stars! **object ids**, packed
`id:9 | iplr:4 | ith:3` (`THING`). For a fleet the low 9 bits are the fleet
number and bits 9..=12 are the 0-based owner index — every fleet referenced in
`EXODUS.X6` decodes to owner **5** (player 6), as expected. Helpers:
[`object_owner`] / [`object_index`].

## Decoded operations

The following operations have verified typed decoders. Others are exposed as a
classified [`LogRecord`] with the raw payload preserved.

### Waypoint insert/update (`RTWAYPT`, ids 4 / 5)

```c
typedef struct _rtwaypt {
    int16_t id;     /* +0x00 fleet object id       */
    int16_t iWaypt; /* +0x02 waypoint slot index   */
    ORDER   order;  /* +0x04 (variable-length ORDER) */
} RTWAYPT;

typedef struct _order {
    POINT    pt;         /* +0x00 x:i16, y:i16 (destination) */
    int16_t  id;         /* +0x04 target object id           */
    uint16_t grTask : 4, /* +0x06 waypoint task              */
        iWarp : 4,       /*       warp factor                */
        grobj : 4,       /*       target object class        */
        fValidTask : 1,  /*       task is valid              */
        ...;
    union { /* +0x08 task-specific data, 0..10 bytes */ } ;
} ORDER;
```

The log writes a **variable-length** `ORDER`: a bare point/target waypoint is 12
bytes total (`RTWAYPT` with no task union), while a transport-style task adds up
to 8 trailing bytes (seen as 20-byte records). [`WaypointOrder`] decodes the
fixed part (`fleet_id`, `waypoint_index`, `x`, `y`, `target_id`, `task`, `warp`,
`grobj`, `valid_task`) and preserves any task union as `task_data`.

### Fleet order delete (`RTSHIPINT`, id 3)

`{ int16_t id; int16_t i; }` — `id` is the fleet, `i` the order index; the high
bit of `i` (`0x8000`) means "also delete the extra trailing order". Decoded by
[`FleetOrderDelete`].

### Research settings (id 34)

A 2-byte record: `pctResearch` (byte 0) + packed nibbles in byte 1
(`current_field` low nibble = the tech field 0..5 = Energy/Weapons/Propulsion/
Construction/Electronics/Biotech; `next_field` high nibble = the "next field to
research" selector). Decoded by [`ResearchOrder`]. In exodus, byte 0 is `0x1e`
(30 %) throughout.

### Planet routing (`RTCHGPLANETLONG`, id 35)

```c
typedef struct _rtChgPlanetLong {
    int16_t  id;                 /* +0x00 planet id */
    uint32_t fNoResearch : 1,    /* +0x02 */
        idFling : 10, iWarpFling : 4, idRoute : 10, unused : 7;
} RTCHGPLANETLONG;               /* size=0x6 */
```

Decoded by [`PlanetRoutingOrder`] (`planet_id`, `no_research`, `fling_target`,
`fling_warp`, `route_target`).

### Cargo transfer (`RTXFER` family, ids 1 / 2 / 23 / 25)

The four variants differ only in the width of the item mask and of each
quantity; one signed quantity follows per set bit in `grbitItems`:

| op (id)                    | struct    | mask  | quantity |
|----------------------------|-----------|-------|----------|
| `rtLogCargoXfer8` (1)       | `RTXFER`  | `u8`  | `i8`     |
| `rtLogCargoXfer16` (2)      | `RTXFERX` | `u8`  | `i16`    |
| `rtLogFleetCargoXfer` (23)  | `RTXFERF` | `u16` | `i16`    |
| `rtLogCargoXfer32` (25)     | `RTXFERL` | `u8`  | `i32`    |

```c
typedef struct _rtxfer {
    uint16_t id1, id2;           /* +0x00 the two objects */
    uint8_t  grobj1 : 4, grobj2 : 4; /* +0x04 their classes */
    uint8_t  grbitItems;         /* +0x05 item bitmask (u16 in RTXFERF) */
    char     rgcQuan[1];         /* per-item quantities (width per variant) */
} RTXFER;
```

[`CargoTransfer`] decodes `id1`, `id2`, `grobj1`, `grobj2`, the `items_mask`,
and the per-item signed `quantities` (widened to `i32`), and still preserves the
raw quantity region as `quantity_bytes`. Verified on the exodus files: every
transfer has exactly `popcount(items_mask)` quantities and the raw bytes equal
`count × width`.

### Ship-design change (`RTCHGSHDEF`, id 27)

```c
typedef struct _rtchgshdef {
    uint16_t mdChg : 4, iPlr : 4, ishdef : 5, junk : 3; /* +0x00 header word */
    RTSHDEF  rtshdef;                                   /* +0x02 embedded design */
} RTCHGSHDEF;
```

[`ShipDesignChange`] decodes the header word (`mode`/`player`/`design_index`)
and, when present, the embedded design via
[`DesignRecord`](../../crates/stars-formats/src/design.rs) (a bare *delete* is
just the 2-byte header, no design). In exodus every design change is owned by
player 6 (`iPlr = 5`) and carries `mdChg = 1` when a design body is present.

### Production-queue change (`RTCHGPRODQ`, id 29)

`{ int16_t id; PROD rgprod[]; }` — a planet id followed by packed production
items. Decoded by
[`ProductionQueueRecord::decode_change`](../../crates/stars-formats/src/production.rs)
(exposed via `LogRecord::as_production_queue`); the `.xN` change form always
carries the target planet id.

### Fleet rename (`RTCHGNAME`, id 44)

```c
typedef struct _rtchgname {
    int16_t id;      /* +0x00 object id */
    int16_t grobj;   /* +0x02 object class */
    uint8_t rgb[33]; /* +0x04 packed-string field: [len][packed data] */
} RTCHGNAME;
```

[`FleetName`] decodes `id`, `grobj`, and the trailing name field via the
packed-string decoder ([`decode_stars_string`]); a leading length byte of `0`
means the name was written as a literal C string. (Not present in the exodus
capture, so decoded from the NB09 struct rather than fixture-verified.)

### `THING` byte parameter (`RTLOGTHING`, id 43)

`{ uint16_t idFull; int16_t fDetonate; }` — a full object id plus a parameter
(e.g. a minefield arm/detonate flag). Decoded by [`ThingParam`]. (Also absent
from exodus; decoded from the struct.)

## Open items

- The 11 `rgbConfig` bytes of `RTLOGHDR` (game-settings snapshot) are preserved
  but not field-split.
- `rtLogFleetSplit` (24), `rtLogFleetMerge` (37), `rtLogFleetFlagBit9` (10),
  `rtLogFleetOrderAttrNib` (11), `rtLogRelations` (38), `rtLogFleetPlan` (42)
  and `rtLogPlayerZpq1` (46) are classified but not yet field-decoded.
- `RTCHGNAME` (44) and `RTLOGTHING` (43) decoders are struct-derived; they need
  an orders fixture that renames a fleet / toggles a minefield to fixture-verify.

[`object_owner`]: ../../crates/stars-formats/src/orders.rs
[`object_index`]: ../../crates/stars-formats/src/orders.rs
[`LogRecord`]: ../../crates/stars-formats/src/orders.rs
[`WaypointOrder`]: ../../crates/stars-formats/src/orders.rs
[`FleetOrderDelete`]: ../../crates/stars-formats/src/orders.rs
[`ResearchOrder`]: ../../crates/stars-formats/src/orders.rs
[`PlanetRoutingOrder`]: ../../crates/stars-formats/src/orders.rs
[`CargoTransfer`]: ../../crates/stars-formats/src/orders.rs
[`ShipDesignChange`]: ../../crates/stars-formats/src/orders.rs
[`FleetName`]: ../../crates/stars-formats/src/orders.rs
[`ThingParam`]: ../../crates/stars-formats/src/orders.rs
[`decode_stars_string`]: ../../crates/stars-formats/src/strings.rs
