# Cargo and ship transfer records (block types 1, 2, 23, 25)

Status: **fully decoded** — implemented in `stars-formats::cargo`, applied by
`stars-core::orders`.

## Not the waypoint task

The waypoint's Transport task (task id 1, see `waypoint.md`) is not where a
transfer is recorded. **Every waypoint in both fixture games carries task 0** —
2884 in Exodus and 47,289 in the sixteen-AI game, without exception. A task is
consumed when it executes, exactly as production queue entries and fleet orders
are, so the surviving record of what moved is elsewhere.

It is in the `.x` order file, as one block per transfer. The block *type*
chooses the quantity width:

| Type | Name | Quantity width | What it moves |
|------|------|----------------|---------------|
| 1  | `rtLogCargoXfer8`     | `i8`  | cargo |
| 2  | `rtLogCargoXfer16`    | `i16` | cargo |
| 25 | `rtLogCargoXfer32`    | `i32` | cargo (none in the fixtures) |
| 23 | `rtLogFleetCargoXfer` | `i16` | **ships**, not cargo — see below |

## Cargo transfers (types 1, 2, 25)

| Offset | Field | Notes |
|--------|-------|-------|
| 0-1 | source id | the object named first |
| 2-3 | destination id | the object named second |
| 4 | `srcdst` | low nibble = source class, high nibble = destination class |
| 5 | `grbit` | which cargo kinds follow, one bit each |
| 6.. | quantities | one per set bit, packed consecutively, width by block type |

**Byte 4 is two `GrobjClass` nibbles**, not an opaque mode: `grobjPlanet` 1,
`grobjFleet` 2, `grobjOther` 4, `grobjThing` 8. The replay reads them as

```c
GrobjClass srcClass = (GrobjClass)(srcdst & (grobjThing|grobjOther|grobjFleet|grobjPlanet));
GrobjClass dstClass = (GrobjClass)(srcdst >> 4);
```

so the `0x12` on every Exodus planet transfer is *fleet → planet*, and `0x22`
is *fleet → fleet*. A destination of `grobjOther` means there isn't one: the
cargo is jettisoned.

**Byte 5 is a five-bit cargo mask, and the quantities are an array.** This is
the part an earlier revision of this document got wrong. It recorded byte 5 as
an unexplained constant and read a single quantity at byte 6, because every
sample in the fixtures has the same value. The replay loop settles it:

```c
grbit = (uint16_t)lpb[5];
iLook = 0;
for (i = 0; i < 5; i++) {
    if ((grbit & 1u) == 0)      rgcXfer[i] = 0;
    else                        rgcXfer[i] = <quantity at lpb[6 + iLook++ * width]>;
    grbit >>= 1;
}
```

One quantity per **set bit**, packed with no gaps, so a block's length depends
on how many kinds moved. The kinds are ironium, boranium, germanium, colonists,
fuel — fixed by the replay singling out `i == 3` for the colonist-drop
bookkeeping that turns an unload over a foreign planet into an invasion. Every
Exodus transfer carries `grbit = 0x08`, bit 3 alone, which is why all 43 of them
move colonists and nothing else.

### Sign, and the two passes

The replay applies `ChgCargo(source, +q)` and `ChgCargo(destination, -q)`, so a
**positive quantity means the source gains**: a fleet named as the source with
`+25` colonists has *loaded* 25 from the planet.

Quantities are applied in two passes over the five kinds — every negative first,
then every positive — with each side handled on the opposite pass from the
other, so that room freed by unloading is available to whatever loads next.

`ChgCargo` clamps twice and returns what actually moved: an object can never
give more than it holds, and a fleet can never take more than it has room for
(`GetCargoFree`, or `GetFuelFree` for fuel). If one side moves less than asked,
the other is held to the smaller amount. A planet has no capacity limit and no
fuel at all — `ChgCargo` returns 0 outright for fuel on a planet.

## Ship transfers (type 23)

`rtLogFleetCargoXfer` is a misleading name. Its replay arm moves `fl.rgcsh[i]`,
the count of ships of design `i`, between two fleets — it is a split or merge,
not a cargo movement. It refuses outright when the two fleets have different
owners, and calls `FleetTransferCargoBalance` afterwards to redistribute the
cargo across the new ship counts.

The layout differs from the cargo forms in both mask width and offset:

| Offset | Field | Notes |
|--------|-------|-------|
| 0-1 | first fleet | |
| 2-3 | second fleet | must have the same owner |
| 4 | `srcdst` | present but unused by this arm |
| 5-6 | `grbit` | **16 bits**, one per design slot |
| 7.. | counts | `i16` per set bit |

Sign matches the cargo form: the replay does `first += delta` and
`second -= delta`, so a negative count moves ships **from** the first fleet to
the second. Counts are clamped into `0..=0x7FFD` on both fleets.

Example, from `fixtures/games/exodus/2402/EXODUS.X6`:
`04 0a 06 0a 22 01 00 ff ff` — fleet `0x0a04`, fleet `0x0a06`, mask `0x0001`
(design slot 0), count `-1`: one ship of design 0 leaves the first fleet for the
second.

## Applying them

`stars-core::orders::apply_cargo_transfer` implements the two-pass loop and
`ChgCargo`'s clamping, and `generate_turn_with_orders` runs them as `DoOrders(0)`
— before movement and production, so a transfer feeds the same year's growth.

Verification is thin, and honestly so: the only `.x` files in this repository
are Exodus's, which record 43 transfers in forty files, all of them one fleet
loading or unloading colonists at one planet. Scored on the 27 planet-years
those transfers name, population agreement goes from **0 with the orders
skipped to 3 with them applied**, and the whole-turn replay from 382 to 384
planet-years. The direction is right and nothing regresses, but a corpus with
mineral transfers, jettisons, or a `grbit` with more than one bit set would test
far more of this than the fixtures can.
