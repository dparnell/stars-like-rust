# Production-queue blocks (types 28 & 29)

Status: **decoded & verified** — implemented in `stars-formats::production`,
field layout read out of `AddItemToQueue` (`1090:407b`).

A production queue is a planet's build list. Type 28 (`PRODUCTION_QUEUE`) is the
form stored in `.hst`/`.mN`; type 29 (`PRODUCTION_QUEUE_CHANGE`) is the `.xN`
order form, which prefixes a 2-byte planet id.

## Which planet a queue belongs to

A type-28 block carries **no planet id**. It belongs to the planet block it
immediately follows, and only planets that have a queue get a block at all — so
queues cannot be matched to planets by index. `production_queues_by_planet`
walks the blocks in order and pairs them up.

In `fixtures/incoming/turn1/Game.hst` the two queues belong to planets 32 and
112, while planet 69 — which lies between them in the file — has none. Zipping
the queue list against the planet list therefore hands planet 112's queue to
planet 69.

## Item layout

Each item is one 32-bit little-endian word. `AddItemToQueue` writes the fields
one at a time, each with an explicit shift (in `CX`, passed to the shift helper
at `1118:0d50`) and mask, which pins the layout exactly:

| Bits    | Width | Field        | Written as                          |
|---------|-------|--------------|-------------------------------------|
| 0–9     | 10    | `count`      | `count & 0x3ff`, shift 0            |
| 10–16   | 7     | `item`       | `iItem & 0x7f`, shift `0xa`         |
| 17–19   | 3     | `class`      | `grobj & 0x7`, shift `0x11`         |
| 20–26   | 7     | `completion` | zeroed on insert; a percentage 0–99 |
| 27–31   | 5     | unknown      | zeroed on insert; 0 in every fixture |

The block holds `len / 4` items; a partial trailing word (if any) is ignored.

Type 29 (`.xN`) — identical, but with a leading `u16` planet id before the item
list.

### `class` — what the entry builds

`class` is the game's `GrobjClass`. Only two values occur in a queue:

| Value | Name          | `item` means                             |
|-------|---------------|-------------------------------------------|
| 1     | `grobjPlanet` | a `ProdItemType` id (the table below)     |
| 2     | `grobjFleet`  | a ship or starbase design slot            |

### `item` — the `ProdItemType` ids

From the binary's own debug symbols:

| Id | Name                        | Id | Name                          |
|----|-----------------------------|----|-------------------------------|
| 0  | `iobjMine`                  | 9  | `mdIdleDefense`               |
| 1  | `iobjFactory`               | 11 | `mdIdleAlchemy`               |
| 2  | `iobjDefense`               | 12 | `mdIdleTerraform`             |
| 3  | `iobjAlchemy`               | 13 | `iobjGenesis`                 |
| 4  | `iobjMinTerraform`          | 14–17 | `iobjPacket{Iron,Bor,Germ,Mixed}` |
| 5  | `iobjMaxTerraform`          | 18–26 | planetary scanners (`Viewer50` … `Snooper620X`) |
| 6  | `iobjPacket`                | 27 | `iobjPlanetaryScanner`        |
| 7  | `mdIdleFactory`             | 31 | `iobjUnknown`                 |
| 8  | `mdIdleMine`                |    |                               |

The `mdIdle*` ids are the **auto-build** form of an item: they keep building as
the planet grows instead of counting down. Auto-build is thus a distinct set of
item ids, not a flag applied to an ordinary one.

## Evidence

Decoding all 2162 queue entries across the 40-turn Exodus game and the
`incoming` fixtures yields, for `class = 1`, only the ids 0, 1, 2, 7, 8, 9 and
12 — every one a valid `ProdItemType`, and no value outside the enum. For
`class = 2` it yields only the small design slots 0, 3, 6 and 18. `completion`
spans 0–99, and the top 5 bits are zero everywhere.

The two AI queues in `fixtures/incoming/turn1/Game.hst` decode as five ships of
design 0 plus (for one player) one of design 18 — the first with 81% of its cost
already paid, the rest untouched, which is how Stars! applies resources to the
front of the queue.

## A correction worth recording

This project previously read `item` as a **10-bit** field spanning bits 10–19,
which silently folds `class` into the top of the item id: `class = 1` appeared
as `item + 128` and `class = 2` as `item + 256`. The `+ 256` range was then read
as "auto build", which is wrong twice over — those entries build *ships*, and
the real auto-build items are ids 7/8/9/11/12.

The cost of the error was invisible in a round-trip test, because re-encoding
the misread fields reproduces the same bytes. It showed up only in simulation:
`planetary_item_cost` returned `None` for every id in the 128–140 range, so the
turn generator silently skipped every genuine planetary build in the Exodus
save. Correcting the layout moved the whole-turn replay from 53% to 71% on
factories, 65% to 67% on mines and 19% to 27% on surface minerals.

The lesson is the same one `blocks.md` records about the multi-segment cipher: a
format bug that round-trips cleanly can only be caught by checking the decoded
values against what the game does with them.

## Source

- `AddItemToQueue` (`1090:3e50`, field writes at `1090:407b`) — the shifts and
  masks above.
- `ProdItemType` and `GrobjClass` enums, from the NB09 debug symbols.
- stars-4x `decompiled`: `Structures/Structure28.xml` and `Structure29.xml`
  (block framing only; its field split is the 10-bit one corrected above).

## Open questions

- The meaning of the unknown 5-bit field at bits 27–31, which is zero in every
  fixture available.
- Whether a `class = 2` entry distinguishes a ship design from a starbase design
  by slot range; design 18 in the `incoming` fixture sits above the 16 ship
  design slots.
