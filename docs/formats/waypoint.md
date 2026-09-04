# Waypoint blocks (types 19 and 20)

Status: **decoded & verified** — implemented in `stars-formats::waypoint`.

A fleet's ordered waypoint list is stored as a run of type-19 and type-20
blocks placed
immediately after the fleet block. The owning fleet's `waypoint_count`
(full-fleet field, see `fleet.md`) says how many type-20 blocks belong to it.
The first waypoint of a fleet is a "waypoint zero" holding the fleet's current
position.

## Two block types, and why it matters

An order is the NB09 `ORDER` struct, `sizeof(ORDER) == 18`: an 8-byte header
and a 10-byte union of task-specific data (`TASKXPORT`, `TASKLAYMINES`,
`TASKPATROL`, `TASKSELL`). The file writes it as one of two block types:

| Type | Name | Size | Contents |
|------|------|------|----------|
| 20 | `rtOrderB` | 8 | the header alone — **always task 0** |
| 19 | `rtOrderA` | 18 | header *and* the task union — **always a real task** |

A waypoint with no task is written short. Reading only type 20 therefore finds
task 0 everywhere and suggests, wrongly, that the fixtures contain no orders at
all — which is exactly the conclusion an earlier revision of `cargo.md` drew.
Counted over both types across `all-computer-players` and Exodus:

| block type | count | tasks |
|---|---:|---|
| 20 | 48,368 | task 0 only |
| 19 | 11,773 | Transport 4,331 · Colonize 4,935 · Remote Mining 741 · Lay Minefield 1,726 · Patrol 40 |

Every type-19 task has `fValidTask` set.

## Byte 7 is three fields, not one

The NB09 `ORDER` bitfield settles a value this document previously carried as
an opaque "object type":

```c
uint16_t grTask : 4;      /* +0x0006 bits 0-3  */
uint16_t iWarp : 4;       /*         bits 4-7  */
uint16_t grobj : 4;       /*         bits 8-11 */
uint16_t fValidTask : 1;  /*         bit 12    */
uint16_t fNoAutoTrack : 1;/*         bit 13    */
```

So the familiar `object_type = 17` is `0x11`: `grobj = 1` (a planet target)
with the task-valid bit set. `WaypointRecord` now exposes `object_class`,
`valid_task` and `no_auto_track` alongside the raw byte.

**`fValidTask` is the gate.** The task nibble keeps whatever was last chosen
even after the task has been carried out or cancelled, so a reader that ignores
this bit overstates how many fleets have live orders.

## Layout (8-byte header + optional task data)

| Offset | Size | Field           | Notes                                        |
|--------|------|-----------------|----------------------------------------------|
| 0–1    | 2    | x               | galaxy x position                            |
| 2–3    | 2    | y               | galaxy y position                            |
| 4–5    | 2    | object id       | target object; `0xFFFF` = bare coordinate    |
| 6      | 1    | `grTask`/`iWarp`| low nibble = task, high nibble = warp speed  |
| 7      | 1    | `grobj`/flags   | low nibble = target class; bit 4 `fValidTask`; bit 5 `fNoAutoTrack` |
| 7      | 1    | object type     | e.g. 17 = orbiting a planet                  |
| 8..    | var  | task data       | present when task > 0 (usually ~10 bytes)     |

Task ids (from `Structure19.xml`): 0 = none, 1 = Transport, 2 = Colonize,
3 = Remote Mining, 4 = Merge with Fleet, 5 = Scrap Fleet, 6 = Lay Minefield,
7 = Patrol, 8 = Route, 9 = Transfer Fleet.

## Evidence

`fixtures/incoming/turn0/Game.hst` has 14 waypoints (one per starting fleet).
Each decodes to `object_type = 17`, warp 0, task 0, and its `object_id` is the
orbited homeworld — the three distinct targets are planet ids 32, 69 and 112,
matching the three homeworlds.

## Source

- stars-4x `decompiled`: `Structures/Structure20.xml` (and `Structure19.xml`
  for the task lookup).
- stars-4x `starsapi`: `WaypointBlock.java` (warp = high nibble, task = low
  nibble; positionObjectType byte; trailing bytes preserved).

## Open questions

- The full `object type` enumeration (17 confirmed = planet).
- The Transport task's payload is now decoded (see below); the other tasks'
  payloads (`TASKLAYMINES`, `TASKPATROL`, `TASKSELL`) are named by the NB09
  structures but not yet decoded, and are preserved verbatim.

## The Transport task's payload

The ten bytes after a Transport waypoint's header are the `ORDER` union's
`TASKXPORT` arm: `ITEMACTION rgia[5]`, one entry per cargo kind in the usual
order — ironium, boranium, germanium, colonists, fuel. Each entry is a single
16-bit word:

| Bits | Width | Field |
|------|-------|-------|
| 0-11 | 12 | `cQuan`, the quantity the action refers to |
| 12-15 | 4 | `iAction`, an `XferActionType` |

and the actions are

| code | action |
|-----:|--------|
| 0 | none |
| 1 | load all available |
| 2 | unload all |
| 3 | load exactly `cQuan` |
| 4 | unload exactly `cQuan` |
| 5 | fill up to `cQuan` percent |
| 6 | wait for `cQuan` percent |
| 7 | load dunnage |
| 8 | set amount to `cQuan` |
| 9 | set waypoint to `cQuan` |

### Checked against the fixtures

All **15,526** Transport waypoints in this repository decode, and the result
corroborates the split three ways rather than merely not crashing:

- every action code is a **valid** one — `LoadAll` 10,218, `UnloadAll` 25,339,
  `FillPercent` 2,622, and no unknown code anywhere in the four-bit space, which
  a wrong offset or bit boundary would certainly produce;
- `LoadAll` and `UnloadAll` carry a quantity of **0** in every case, which is
  right for actions that take none;
- `FillPercent` carries only **33 and 66** — percentages, exactly as the action
  name implies.

`stars-core` performs `LoadAll`, `UnloadAll`, `LoadExact`, `UnloadExact` and
`FillPercent` on arrival. `LoadDunnage`, `WaitPercent`, `SetAmount` and
`SetWaypoint` are decoded but **not performed**: their behaviour depends on
parts of `SatisfyOrders` that could not be read confidently, and none of them
occurs in these fixtures.
