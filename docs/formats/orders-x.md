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
| 30 | `rtBtlPlan`             | define one of the player's five battle plans |
| 34 | `rtLogResearch`         | research settings                            |
| 35 | `rtLogPlanetRouting`    | planet routing / starbase / infra bits       |
| 36 | `rtChgPassword`         | change the player's password                 |
| 37 | `rtLogFleetMerge`       | merge fleets                                 |
| 38 | `rtLogRelations`        | player-relations table                       |
| 42 | `rtLogFleetPlan`        | set a fleet's battle plan                    |
| 43 | `rtLogThingByteParam`   | set one byte inside a `THING` union          |
| 44 | `rtLogFleetName`        | fleet rename (packed/compressed string)      |
| 46 | `rtLogPlayerZpq1`       | host-only opaque blob                        |

All operation ids seen across the 40 exodus files classify to a known type (no
`Other(_)`). Three of the ids above appear in **no** fixture: 30 and 36, because
nobody in the sample redefined a battle plan or changed a password mid-game, and
11, because nothing writes it at all — see below. All three are decoded and
replayed all the same, and for 30 and 36 the payload is verified from the state
file instead: a type-30 record is byte-for-byte a battle-plan block, of which
the fixtures carry 15, and a type-36 record is the four bytes at offset 12 of a
player block, of which they carry 7,040.

### Which routine writes each operation

`WriteMemRt` (`1048:a130`) is the only routine that appends to the log buffer:
it is what formats the two-byte `(rt << 10) | cb` header, copies the payload
after it and advances the length at `[0x9a4]`. So the operations a 2.7j client
can emit are exactly the record types its **24** call sites push, and those are:

| id(s)      | written by                | call site(s)                          |
|------------|---------------------------|---------------------------------------|
| 1 / 2 / 25 | `LogMakeValidXfer`        | `1048:9f98`, width picked at `9e21` / `9ea1` / `9f16` |
| 3, 4, 5    | `LogChangeFleet`          | `1048:91d3`, `9260`, `932f`           |
| 10, 42     | `LogChangeFleet`          | `1048:90dc`, `908b`                   |
| 23         | `LogMakeValidXferf`       | `1048:a0f9`                           |
| 24         | `LogSplitFleet`           | `1048:8b5e`                           |
| 27         | `LogChangeShDef`          | `1048:8cc1`, `8d1d`                   |
| 29         | `LogChangePlanet`         | `1048:95c0`, `96fd`                   |
| 30         | `WriteBattlePlan`, from `LogChangeBtlplan` | `1070:8aae`          |
| 34         | `ResearchDlg`, `IroEnsureAi` | `10d8:0808`; `1090:430e`, `448c`, `45b5` |
| 35         | `LogChangePlanet`         | `1048:98f9`                           |
| 36         | `NewPasswordDlg`          | `1040:5ec7`                           |
| 37         | `LogMergeFleet`           | `1048:8c0e`                           |
| 38         | `LogChangeRelations`      | `1048:9394`                           |
| 43         | `MineWndProc`             | `1028:0368`                           |
| 44         | `LogChangeName`           | `1048:8e98`                           |
| 46         | `FWriteLogFile`           | `1048:ce89`                           |

Twenty-three sites push a constant type; the one computed type, in
`LogMakeValidXfer`, is chosen from `{1, 2, 25}` — the three cargo-transfer
widths — so the table is the whole of it. The `rtBOF` header (id 8) is not in it
because `FWriteLogFile` lays that out itself rather than through `WriteMemRt`.

`IroEnsureAi` is the only writer that is not driven by a person: an AI player's
turn is logged the same way a human's is, and all it ever writes is research.

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
`LogChangeShDef` (`log.c`) confirms the two modes: **1** when a design follows,
**0** for a delete.

**`ishdef` is the whole slot.** `SHDEF.det` packs it as
`det:8, fInclude:1, fFree:1, ishdef:5, fGift:1`, and a starbase design lives at
16..=25 — so its top bit is set. `LogChangeShDef` writes that entire five-bit
field into the header word, while the embedded record's `design_number` carries
only the low four and what the record calls its "starbase" flag *is* the fifth
bit. So the slot is read from the header alone; adding the starbase offset to
it a second time put every edited starbase design in slot 32 and up, which no
ship fixture could reveal because ships only ever use 0..=15.

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

`{ int16_t id; int16_t iOrder; int16_t value; }` — the fleet, one of its
waypoints, and a value whose low nibble becomes `ORDER.grTask`. The rest of the
waypoint's flag word is left alone, which is what the name says: the operation
writes a nibble, not a record.

**No fixture contains one, and no fixture ever will: nothing in
`stars.2.7j.exe` writes this operation.** Every one of the 24 `WriteMemRt` call
sites pushes some other type (see the writer table above), so the client cannot
produce a type-11 record however it is driven — a waypoint's task reaches the
host inside a whole `rtLogFleetOrderUpdate` (5) instead, the same path that
carries every other waypoint edit. The absence is a property of the format, not
a hole in the sample; the earlier note that this was "the only operation with no
example in the corpus" understated it.

What survives is the **replay** side, which the host still runs, and which is
where the layout comes from. The arm is at `1048:c3f0`, shared with
`rtLogFleetFlagBit9` (10) — the two ids are adjacent entries in the dispatch
table at `1048:c738` pointing at the same code:

```
lpfl = LpflFromId(*(int16_t *)lpb);          // 1038:2078
if (lpfl == NULL) return 0;                  // 1048:c406
ifl = *(int16_t *)(lpb + 2);
if (lpfl->cord <= ifl) return 0;             // 1048:c462, signed
if (*(int16_t *)(lpb + 4) >= 10) return 0;   // 1048:c46e, the whole word
ord = lpfl->lpplord->rgord[ifl];             // base + 4, stride 0x12
ord.grTask = (ord.grTask & 0xfff0)           // 1048:c4c4, the word at ord+6
           | (*(uint16_t *)(lpb + 4) & 0x0f);
```

The `+ 6` the arm masks is `ORDER.grTask` at `ORDER+0x06` in the `RTWAYPT`
layout above, and the `0x12` stride is the in-memory `ORD` — the eight-byte
header plus its ten-byte task union — so the arm is unambiguously writing a
waypoint's task and nothing else.

Two details the bound checks give away. The value is tested **before** it is
masked, so `0x10` is refused even though its nibble is a legal task — which is
why [`FleetOrderTask`] keeps the raw `value` word and offers `task()` and
`value_in_range()` over it, rather than storing a masked nibble that could not
reproduce the bytes it came from. And `cord <= ifl` is a signed comparison with
no lower bound, so the original accepts a negative index and writes in front of
the order array; this project cannot express that and refuses it.

Replayed by `set_order_task`. Because the corpus cannot supply a case, the
worked examples live in [`docs/vectors/order-attr-nib.json`](../vectors/order-attr-nib.json)
— eight records read out of the arm above, each with the verdict it produces —
and a test in `replay.rs` runs every one of them.

### Turn password (`rtChgPassword`, id 36)

`{ int32_t lSalt; }` — four bytes, and nothing else. Stars! does not store a
password: it stores a 32-bit checksum of the typed text, which the game calls a
*salt*, and compares that against the salt of whatever is typed next time
(`FCheckPassword`, `1040:58d8`). This record carries the same value the player
block holds at offset 12, and `0` means the password was cleared. Decoded by
[`PasswordChange`].

`NewPasswordDlg` writes it at `1040:5ec7` — but only when a player's game is
open (`iplrMe != -1`). Asked the same question with no game loaded, the dialog
is setting the **host's** password and stores it in a global instead of logging
anything, which is why a `.hst` never needs this record.

The replay arm is `1048:c65c`:

```
if (!((gd >> 1) & 1)) return 1;              // DS:0x7ca, the runtime mode word
*(int32_t *)(&rgplr[iplrMe] + 0x0c) = *(int32_t *)lpb;
```

Bit 1 of `gd` is the flag that says a game is open with its orders live — every
`Log*` writer opens with the same test (`LogSplitFleet` at `1048:8b17`, and so
on). This engine has no runtime mode word; it is always applying to a state it
holds, so `replay` runs the arm unconditionally. `PLAYER + 0x0c` is offset 12 of
the player block, which [`race-r.md`](race-r.md) had already identified as the
password field from an independent source.

**The salt.** `LSaltFromSz` (`1040:59ce`) folds the string's bytes in pairs —
the first added, the second multiplied — into a 32-bit accumulator that wraps,
with each byte sign-extended. An empty password is `0`; a non-empty one that
happens to fold to `0` is bumped to `1`, so `0` unambiguously means *no
password*. The dialog reads at most seventeen characters
(`GetWindowText(..., 0x12)`). Transcribed in [`stars_formats::password`].

This is a checksum, not a password hash, and it was never more than a way to
stop the other players in a play-by-mail game opening each other's turns by
accident. This project stores exactly what the game stores — the salt, never the
text — and offers no way to go back the other way, because the game itself only
ever compares salts.

The corpus pins the field but not the fold: 6,969 of the 7,040 full player
blocks in the fixtures carry one and the same non-zero salt (the exodus games
were set up with a single password) and the other 71 carry `0`. No fixture
supplies a *password and its salt*, so the transcription of `LSaltFromSz` rests
on the disassembly alone, and nobody should try to recover the string behind
that value — it is a real person's, and it is not needed for anything.

### Battle plan definition (`rtBtlPlan`, id 30)

The operation that **writes a plan**: its name, tactic, target preferences and
who it attacks. The one that says which plan a fleet fights under is
[`rtLogFleetPlan`](#battle-plan-rtlogfleetplan-id-42) (42) — different record,
different id, easily confused.

The payload is exactly a state file's type-30 block, documented in
[`battleplan.md`](battleplan.md): `WriteBattlePlan` (`1070:89b8`) fills one
buffer and hands it either to `WriteMemRt` for the log or to the block writer
for the file, so one decoder reads both. `LogChangeBtlplan` (`1048:93d0`) is the
wrapper that logs it, called from five places in `BattlePlansDlg` (`10f0:0652`)
— adding, editing and deleting each log one. Unlike the relations record it does
**not** replace an earlier record of its own kind, so a log can carry several
and they apply in order.

A **delete** is written short. When byte 1 has bit 6 set (`PLAN_DELETED`, bit 14
of the first word), `WriteBattlePlan` stops after two bytes — no targets, no
name — and the replay tests the same bit before reading any further, so the
short form is not a truncation. Decoded by [`BattlePlanChange`], which keeps the
two forms apart and refuses to let the flag and the length disagree.

The host's arm is at `1048:c287`:

```
w0 = *(uint16_t *)lpb;
iplan = (w0 >> 4) & 0x0f;                      // the slot
if ((w0 & 0x0f) != iplrMe || iplan > cplan[iplrMe])
    return (w0 >> 14) & 1;                     // a stray delete is shrugged off,
                                               // a stray definition refused
if ((w0 >> 14) & 1) { DeleteBattlePlan(iplan, 0); return 1; }
if (((w0 >> 8) & 0x0f) > 6) return 0;          // tactic
w1 = *(uint16_t *)(lpb + 2);
if ((w1 & 0x0f) > 8) return 0;                 // primary target
if (((w1 >> 4) & 0x0f) > 8) return 0;          // secondary target
if (iplan == cplan[iplrMe]) {                  // appending a new plan
    if (iplan >= 16) return 0;
    cplan[iplrMe]++;
}
ReadBattlePlan(lpb, &rgbtlplan[iplrMe][iplan], iplan);
```

Four things that matter to a host:

- **A record only ever changes its own author's plans.** The owner nibble must
  equal the player whose log it is.
- **Writing one past the end is how a plan is made.** `iplan == cplan` appends,
  up to sixteen; anything further is refused.
- **The slot comes from the record, and is stamped back.** `ReadBattlePlan`
  overwrites the stored plan's id nibble with the slot it was routed to. That
  matters because the two default plans *Sniper* and *Chicken* both ship with id
  3 — see `battleplan.md`.
- **A delete is not just a removal.** It shifts the following plans up, restamps
  their ids, and walks the player's fleets moving every plan index at or past
  the deleted slot down one.

Replayed by `set_battle_plan`, which keeps all four and reports an out-of-range
delete as neither applied nor rejected, because that is what the original makes
of it. No fixture carries one — nobody in the sample edited a battle plan — but
the payload is fixture-verified through the 15 type-30 blocks of
`fixtures/incoming/turn0/Game.hst`, one of which is the worked example in the
`BattlePlanChange` tests.

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
| defines, retunes or deletes a battle plan | one type-30 record, with the slot stamped into it; a run of edits to one plan collapses to the last |
| sets or clears the turn password | one type-36 record carrying its salt, replacing any earlier one |
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
| battle plan definition (30) | the plan itself is written, appended or deleted; a delete shifts the rest up and moves every fleet index at or past it down |
| turn password (36) | the salt goes into the player's own record; nothing in the engine reads it back |
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
- `rtLogFleetOrderAttrNib` (11) has no writer in `stars.2.7j.exe` at all, so no
  fixture can contain one; its layout is read from the replay arm at
  `1048:c3f0` and pinned by `docs/vectors/order-attr-nib.json`. Whether some
  other build of the client emits it is untested — the host would accept it, so
  this project keeps decoding and replaying it.
- `rtChgPassword` (36) appears in no fixture — nobody in the sample changed a
  password mid-game — so the record's four bytes are verified through the player
  block's own field rather than through a log. The fold that turns a password
  into those four bytes is transcribed from `LSaltFromSz` and is not
  differentially verified: no fixture pairs a password with its salt.
- `rtBtlPlan` (30) is decoded, written and replayed, but no fixture carries one
  as an *order*; the payload is verified through the state-file blocks instead.
  What the `tactic` and target values mean is still undecoded — only their
  ranges are known — so the editor shows them as numbers.
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
[`BattlePlanChange`]: ../../crates/stars-formats/src/orders.rs
[`PasswordChange`]: ../../crates/stars-formats/src/orders.rs
[`FleetOrderTask`]: ../../crates/stars-formats/src/orders.rs
[`FleetPlan`]: ../../crates/stars-formats/src/orders.rs
[`Relations`]: ../../crates/stars-formats/src/orders.rs
[`FleetMerge`]: ../../crates/stars-formats/src/orders.rs
[`decode_stars_string`]: ../../crates/stars-formats/src/strings.rs
