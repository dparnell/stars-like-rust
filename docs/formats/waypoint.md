# Waypoint block (type 20)

Status: **decoded & verified** — implemented in `stars-formats::waypoint`.

A fleet's ordered waypoint list is stored as a run of type-20 blocks placed
immediately after the fleet block. The owning fleet's `waypoint_count`
(full-fleet field, see `fleet.md`) says how many type-20 blocks belong to it.
The first waypoint of a fleet is a "waypoint zero" holding the fleet's current
position.

## Layout (8-byte header + optional task data)

| Offset | Size | Field           | Notes                                        |
|--------|------|-----------------|----------------------------------------------|
| 0–1    | 2    | x               | galaxy x position                            |
| 2–3    | 2    | y               | galaxy y position                            |
| 4–5    | 2    | object id       | target object; `0xFFFF` = bare coordinate    |
| 6      | 1    | task / warp     | low nibble = task, high nibble = warp speed  |
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
- The internal layout of task-specific extra bytes (transport load/unload
  orders, minefield parameters, etc.) — preserved verbatim for now.
