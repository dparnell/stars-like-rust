# `.xN` player-orders file (the order log)

**Status:** **decoded, verified and writable.** The log header and the common
operation records each re-encode byte for byte, and all **58** `.xN` files in
the fixtures rebuild whole from their parsed logs — 878 records across eight
record types. Implemented in
[`stars-formats::orders`](../../crates/stars-formats/src/orders.rs); verified by
[`tests/orders_files.rs`](../../crates/stars-formats/tests/orders_files.rs) and
[`tests/round_trip.rs`](../../crates/stars-formats/tests/round_trip.rs) against
the 40-turn `fixtures/games/exodus/*/EXODUS.X6` sequence and the turn-1
`fixtures/incoming/turn1/Game.x1`.

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
  (18-byte file header block + 19-byte log-header block + 2-byte footer). It
  counts the operation records **only**: `FWriteLogFile` appends any outgoing
  player messages after the loop that emits them, and `cbLog` is set from
  `imemLogCur` before that.

### What the serial and config bytes actually are

`FWriteLogFile` (`log.c`) fills them from two globals:

```c
rtlh.lSerialNumber = vSerialNumber;
memcpy(rtlh.rgbConfig, vrgbEnvCur, 11);
```

`vSerialNumber` is the **registration serial of the copy of Stars! that wrote
the file** — the player's licence key, zero in an unregistered copy
(`globals.c` initialises it to 0, and `mdi.c` sets it from the registration
dialogue). `vrgbEnvCur[11]` is a **fingerprint of the machine** it was written
on: `mdi.c` compares it against a stored `vrgbMachineConfig` to notice the
program moving to a different computer.

The host reads both back out of each submitted `.xN` (`log.c` stores them into
`vrgts[idPlayer]`) and uses the pair to catch two players submitting from one
registration: `turn.c` rejects an invalid serial and flags two players that
share one. That is why the same eleven config bytes appear in two unrelated
games in the fixtures — the same person's machine wrote both — and why an
earlier revision of this note read them as per-game identity.

**We write zero.** A file this project produces carries no registration,
because it has none; inventing a serial would be forging a licence key.
[`stars_ui::App::save_orders`] does copy the serial and fingerprint out of a
`.xN` already sitting beside the save, so a file written for a player who has
real ones stays consistent with theirs.

Decoded by [`LogHeader`](../../crates/stars-formats/src/orders.rs) and written by
`LogHeader::encode`; `cbLog` is recomputed by
[`OrderLog::to_file`](../../crates/stars-formats/src/orders.rs) rather than
carried, so a caller assembling a log never has to maintain it.

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

### Transfers (`RTXFER` family, ids 1 / 2 / 23 / 25)

The four variants differ only in the width of the item mask and of each
quantity; one signed quantity follows per set bit in `grbitItems`, and a
**positive** quantity means the object named *first* gains:

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

#### What the mask selects, and why the fleet-to-fleet form is wider

For the three narrow variants the mask names the five **cargo kinds** —
ironium, boranium, germanium, colonists, fuel — and the quantities are amounts.

For `rtLogFleetCargoXfer` (23) it names the sixteen **ship design slots**, and
the quantities are ship counts. That is what the wider mask is for, and it is
how a fleet is split: the client writes a `rtLogFleetSplit` and then moves ships
into the new fleet with one of these.

The evidence is every one of the 45 such records in the fixtures. Each mask has
a single bit, and that bit is always a design slot the source fleet actually
holds ships of, checked against the same turn's state file:

| source fleet | its stacks in the `.mN` | mask |
|--------------|-------------------------|------|
| exodus 5#21 | design 3 × 3 | `0x0008` |
| exodus 5#13 | design 3 × 4 | `0x0008` |
| exodus 5#12 | design 0 × 5 | `0x0001` |
| a 16-player game's 6#10 | design 12 × 6 | `0x1000` |

Every quantity is a small count (−1, +3) rather than the hundreds a cargo
transfer moves, and bit 12 is not a cargo kind at all. An earlier revision of
this spec listed all four as cargo transfers.

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

[`FleetName`] decodes `id`, `grobj`, and the trailing name field as a **user
string** — the packed field with the literal escape, the same form a fleet's own
name block carries. See `strings.md`. (Not present in the exodus capture, so
decoded from the NB09 struct and the binary rather than fixture-verified.)

### Default production queue (`rtLogPlayerZpq1`, id 46)

`ZIPPRODQ1` truncated to the entries it uses — `2 * cpq + 2` bytes — which is
the queue a planet starts with when it becomes this player's. The single record
in the fixtures is eight bytes: three entries. See `player.md`, which decodes
the same structure out of the player block and gives what the game does with it.

The client writes it only when it differs from what the player's file holds and
when the previous record is not already one of these, so a log carries at most
one.

### Repeat orders (`rtLogFleetFlagBit9`, id 10)

`{ int16_t id; int16_t value; }` — the fleet, and `value & 1` into
`FLEET.fRepOrders`, which is bit 9 of the fleet's flag word and where the
operation's name comes from. A repeating fleet returns to its first waypoint
once it reaches its last. Decoded by [`FleetRepeatOrders`]; three records in the
fixtures.

### Waypoint task (`rtLogFleetOrderAttrNib`, id 11)

`{ int16_t id; int16_t iOrder; int16_t value; }` — it sets the low nibble of one
waypoint's flag word, which is `ORDER.grTask`. The replay refuses an order index
the fleet does not have and a task above 9, the highest the enumeration defines;
both checks are the original's. Decoded by [`FleetOrderTask`]. **No fixture
contains one**, so it is decoded from the replay arm rather than from data.

### Battle plan (`rtLogFleetPlan`, id 42)

`{ int16_t id; int16_t iplan; }` — which of the player's five battle plans the
fleet fights under. Decoded by [`FleetPlan`]; four records in the fixtures.

### Player relations (`rtLogRelations`, id 38)

One byte per player in the game — `0` neutral, `1` friend, `2` enemy — so the
record is as long as the player count; the single record in the fixtures is
eight bytes, from the eight-player exodus game. The **whole table** is written
each time, and `LogChangeRelations` rewinds the log when the record it is about
to write follows another of the same kind, so a log never carries two and only
the last state matters. Decoded by [`Relations`].

### Fleet split (`rtLogFleetSplit`, id 24)

Two bytes: the object id of the fleet being split. It says nothing about what
leaves, because the client writes a fleet-to-fleet **ship** transfer straight
afterwards naming the new fleet and the ships that move into it. All 44 split
records in the fixtures are exactly two bytes, and every one is followed by such
a transfer. Decoded by [`FleetSplit`].

### Fleet merge (`rtLogFleetMerge`, id 37)

A list of fleet object ids, two bytes each — eight of the nine in the fixtures
name two fleets and one names seven. **The first survives and the rest are
absorbed into it.**

That direction is read off the corpus rather than the struct. Of the nine merges
in the exodus game, seven have the first fleet present in the next year's state
file and every other one gone:

```text
2424  merge [4, 30]                        -> 4 present, 30 gone
2425  merge [12, 22, 23, 27, 28, 30, 36]   -> 12 present, the other six gone
2426  merge [26, 25]                       -> 26 present, 25 gone
2429  merge [31, 21]                       -> 31 present, 21 gone
2440  merge [17, 11]                       -> 17 present, 11 gone
2449  merge [16, 6]                        -> 16 present, 6 gone
```

The two exceptions are explicable and neither contradicts the direction: after
`2442 merge [34, 17]` *neither* fleet is in the next year's file, the survivor
having been lost that year, and the two cases where a later-named fleet is still
present are fleet numbers reused after a death. In no case does the first fleet
vanish while a later one survives.

Decoded by [`FleetMerge`], whose `survivor` and `absorbed` name the two halves.

### `THING` byte parameter (`RTLOGTHING`, id 43)

`{ uint16_t idFull; int16_t fDetonate; }` — a full object id plus a parameter
(e.g. a minefield arm/detonate flag). Decoded by [`ThingParam`]. (Also absent
from exodus; decoded from the struct.)

## Writing an order file

`OrderLog::to_file(&FileHeader)` assembles the whole file: the plaintext file
header, the `RTLOGHDR` with a recomputed `cbLog`, each operation record framed
and encrypted, and the empty footer. Every typed operation has an `encode` that
is the exact inverse of its decoder, and `LogRecord` has a constructor per
operation, so building a log is a matter of pushing records in the order the
player made the moves.

Every one of the 58 `.xN` files in the fixtures rebuilds byte for byte from its
parsed log. Record counts checked: 466 waypoint inserts/updates, 143 production
queues, 106 transfers, 58 log headers, 44 fleet splits, 43 ship-design changes,
29 research settings, 25 order deletes, 9 fleet merges, 8 planet routings, 4
battle plans, 3 repeat-orders flags and 1 relations table — 939 records over
thirteen types.

### What the frontend records

`stars_ui::App` keeps the log as the player acts, because an order file records
what the client **already did**, not what it intends:

| The player… | …is logged as |
|-------------|---------------|
| moves cargo between a fleet and the planet it orbits | a cargo transfer, in the narrowest of the four width variants that holds the quantities |
| sends a fleet somewhere | a delete of the old leg, if there was one, then an insert at waypoint 1 |
| gives it a task or transport instructions | an update of that waypoint |
| splits a fleet | a split naming it, then a ship transfer into the new fleet |
| changes a fleet's battle plan or repeat-orders flag | one record each |
| changes how they regard another player | one relations record, replacing any earlier one |
| changes what new colonies build | one default-queue record, likewise |
| merges fleets | one merge record, survivor first |
| renames a fleet | a rename record |
| edits a production queue | one queue record per planet, at the end |
| dials research | one research record, at the end |

Cargo transfers and fleet orders are **events** and go on as they happen; the
queue and the research setting are **state**, and their records replace whatever
the host holds, so one of each is enough. Generating a turn clears the log: it
covers one year.

The delete-then-insert idiom for replacing a leg, the insert-then-update idiom
for adding one and then giving it a task, and the split-then-transfer idiom are
all what the exodus logs do.

The frontend applies these orders by **replaying them** — the same function the
host runs on the submitted log — so what a session does to its own copy of the
game and what the host does to its copy cannot drift apart.

## Replaying an order log

`stars_core::replay::replay(state, player, log, orders)` applies one player's
log to a host's state, and `replay_logs` does several in the order the host
received them. What each operation does:

| Operation | Replayed as |
|-----------|-------------|
| cargo transfer (1/2/25) | collected into `TurnOrders` for `DoOrders(0)`, the first step of the year |
| fleet-to-fleet ship transfer (23) | ships move between two of the player's fleets; the destination is **created** if it does not exist, which is the second half of a split, and a fleet left with no ships ceases to exist |
| fleet split (24) | nothing on its own — the transfer that follows does the work |
| fleet merge (37) | the first fleet named takes the others' ships and cargo, and they cease to exist |
| fleet rename (44) | the name is set on the fleet, and saved with it |
| repeat orders (10) | the fleet's repeat-orders flag |
| waypoint task (11) | the task on one of the fleet's waypoints, bounds-checked |
| battle plan (42) | which battle plan the fleet fights under |
| player relations (38) | the player's whole relations table is replaced |
| default queue (46) | the queue the player's new colonies start with |
| waypoint insert / update (4/5) | inserted at or written over that slot of the fleet's order list |
| waypoint delete (3) | removed; with the high bit, everything from that slot on |
| production queue (29) | the planet's queue is replaced |
| research (34) | the player's percentage, current field and next-field policy |
| planet routing (35) | the planet's "no research" flag |
| ship design (27) | the design is created, replaced, or the slot freed |

**Every operation the format names is replayed.** A record type the format does
not define is named in `ReplayReport::unsupported` rather than silently
dropped.

A replayed rename is written back: the name goes into a type-21 block after the
fleet's waypoints — see `fleet.md`, which recovers that block from the binary,
since no captured game has a renamed fleet to check against.

Cargo transfers are the only operation that is *not* applied on the spot. The
client applied them when the player made them, but their effect belongs at
`DoOrders(0)` so that a transfer feeds the same year's growth — see
`../formulas/turn-order.md`.

### What a host must not take on trust

A log comes from the player's machine and can say anything. Every operation is
checked against the player whose file it was: a waypoint must name one of their
fleets, a production-queue or routing change one of their planets, a design
change their own design list. A cargo transfer may legitimately involve someone
else's planet — landing colonists, invading — so only the fleet end is checked.
Waypoint zero is where a fleet *is* rather than an order, so a delete naming it
is refused, and a log may extend a fleet's order list by at most one, since
anything further would mean the host and the client disagree about the fleet.
Everything refused is counted in `ReplayReport::rejected`.

### Using it

`stars <file.hst> --turn` replays every `.xN` beside the host file that names
that game and that year, reports what each carried, and then generates the
year. The graphical shell does the same when it generates a turn, skipping its
**own** player's log: this session applied those orders as they were made,
which is exactly what the log it wrote records, so replaying it would apply
every transfer twice.

## Open items

- The 11 `rgbConfig` bytes of `RTLOGHDR` are a machine fingerprint (see above);
  what they are a fingerprint *of* is not decoded, and they are preserved
  verbatim.
- The flags word of a logged `ORDER` carries bits 13-15 that this decoder does
  not interpret (bit 13 is `fNoAutoTrack` in the state file's own waypoint
  record). They are preserved, and a waypoint this project writes leaves them
  zero.
- `rtLogFleetOrderAttrNib` (11) appears in no fixture; its layout comes from the
  replay arm in `log.c` rather than from data. It is the only operation in the
  format with no example in the corpus.
- The type-21 fleet-name block appears in no fixture, so its layout is recovered
  from the binary rather than fixture-verified. See `fleet.md`.
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
[`FleetSplit`]: ../../crates/stars-formats/src/orders.rs
[`FleetRepeatOrders`]: ../../crates/stars-formats/src/orders.rs
[`FleetOrderTask`]: ../../crates/stars-formats/src/orders.rs
[`FleetPlan`]: ../../crates/stars-formats/src/orders.rs
[`Relations`]: ../../crates/stars-formats/src/orders.rs
[`FleetMerge`]: ../../crates/stars-formats/src/orders.rs
[`decode_stars_string`]: ../../crates/stars-formats/src/strings.rs
