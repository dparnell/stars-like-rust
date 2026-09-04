# Cargo transfer records (block types 1, 2, 23, 25)

Status: **ids and quantity decoded; the two mode bytes not pinned down** —
implemented in `stars-formats::cargo`.

## Not the waypoint task

The waypoint's Transport task (task id 1, see `waypoint.md`) is not where a
transfer is recorded. **Every waypoint in both fixture games carries task 0** —
2884 in Exodus and 47,289 in the sixteen-AI game, without exception. A task is
consumed when it executes, exactly as production queue entries and fleet orders
are, so the surviving record of what moved is elsewhere.

It is in the `.x` order file, as one block per transfer. The block *type*
chooses the quantity width:

| Type | Name | Length | Quantity |
|------|------|--------|----------|
| 1  | `rtLogCargoXfer8`     | 7 | one byte |
| 2  | `rtLogCargoXfer16`    | 8 | two bytes |
| 23 | `rtLogFleetCargoXfer` | 9 | two bytes, fleet to fleet |
| 25 | `rtLogCargoXfer32`    | — | four bytes; none in the fixtures |

## Layout

| Offset | Field | Notes |
|--------|-------|-------|
| 0-1 | fleet | the fleet doing the transferring |
| 2-3 | target | planet id, or the other fleet for type 23 |
| 4 | `mode` | `0x12` for every planet transfer, `0x22` for every fleet one |
| 5 | `selector` | `0x08` in every planet transfer; absent in type 23 |
| 6.. | quantity | signed, width by block type |

## What the fixtures cannot settle

The ids and the quantity are clear: they vary across samples and take sensible
values — fleet ids in the `0x0a00` range, planet 225 (the player's homeworld),
quantities of 25, 250 and 500.

The two middle bytes are not. All 43 planet transfers in the Exodus orders
carry `mode = 0x12` and `selector = 0x08`, and all 44 fleet transfers
`mode = 0x22`. With no variation there is nothing to read them against, so both
are exposed raw.

The `0x10` between the two `mode` values, and the fact that every planet
transfer moves what look like colonists, suggest the high nibble picks planet
or fleet and `selector` is a cargo bitmask with bit 3 for colonists. That is a
reading of two constants, not evidence, and is not implemented.

## This corrects an earlier expectation

Cargo transfer was recorded in the delivery plan as the likeliest cause of the
whole-turn replay's 27% surface-mineral figure — freighters moving minerals to
and from planets being the one large unmodelled consumer left.

The fixtures say otherwise. **Every recorded planet transfer moves colonists,
not minerals**, so processing these records would not touch surface minerals at
all. Whatever holds that figure down is something else, and the search should
start over.

## Source

- `docs/formats/record-types.md`, entries 1, 2, 23 and 25.
- `fixtures/games/exodus/*/EXODUS.X6` — 17 type-1, 26 type-2 and 44 type-23
  blocks across the forty turns.
