# Waypoint tasks

Status: **in progress** — six of the ten tasks are simulated. Patrol and Give
are not; the other two are "none" and Transport.

A waypoint carries a task in the low nibble of its flags word (`ORDER.grTask`,
see [`waypoint.md`](../formats/waypoint.md)), performed when the fleet reaches
it. `SatisfyOrders` (`10b0:6798`) runs the whole set, several times a year:
the turn pipeline calls it with a **pass** number and each task acts on the
pass it belongs to.

| id | task | pass | state |
|---:|------|------|-------|
| 0 | none | — | — |
| 1 | Transport | 0 | simulated (`execute_arrival_tasks`) |
| 2 | Colonize | 1 | simulated |
| 3 | Remote Mining | 3 | simulated, in its own pass (`mining.rs`) |
| 4 | Merge | even | simulated |
| 5 | Scrap | 1 | simulated |
| 6 | Lay Minefield | 3 | simulated (`minefields.md`) |
| 7 | Patrol | — | not decoded |
| 8 | Route | 4 | simulated |
| 9 | Give | 4 | **not performed** |

The task is **consumed** when it runs, which is why every waypoint in a saved
game that has already been reached reads `0`.

## What the corpus contains

Counting every waypoint block in `fixtures/`:

| task | waypoints |
|------|----------:|
| none (0) | 201,913 |
| Colonize (2) | 21,198 |
| Transport (1) | 15,526 |
| **Lay Minefield (6)** | **6,892** |
| Remote Mining (3) | 1,557 |
| Patrol (7) | 40 |
| Merge (4), Scrap (5), Route (8), Give (9) | **0** |

So the four tasks with no example in the corpus are exactly the four that are
cheap to model, and the two that people actually use are the two that need a
subsystem. Laying minefields was the bigger of those two, and now has one — see
[`minefields.md`](minefields.md).

## Merge (4)

The waypoint must name a **fleet** — the class nibble decides, since a bare id
cannot tell planet 7 from fleet 7 — belonging to the same player, alive, and not
this fleet. Its ships and cargo move into that fleet and this one ceases to
exist: the original is `Merge2Fleets(dest, this, 1)`, so the fleet *carrying*
the order is the one that goes. Anything else (a planet target, another
player's fleet, itself) sends a message and cancels the order.

## Scrap (5)

Transcribed from `CreateSalvage` (`10f0:7ee8`), which is what the scrap arm
calls:

```
for each mineral kind:
    recovered = Σ over design slots: (ships × design.ore_cost[kind]) / 3
    recovered += whatever the hold carries of that kind
    if orbiting a planet:
        planet.surface[kind] += recovered × (planet has a starbase ? 8 : 5) / 10
```

A third of each ship's build cost, in other words, of which the planet keeps
**80%** with a starbase and **50%** without. The third is truncated per design,
not over the sum. Scrapped in deep space the original drops a **salvage**
object; this engine does not model salvage, so those minerals are lost. Fuel and
colonists aboard are not recovered either way, and the Bleeding Edge Tech
recosting `CreateSalvage` does first is not modelled.

## Route (8)

`AutoRouteFleet` (`1080:1e52`). A fleet sitting at one of its owner's planets
that has a route destination set is given a waypoint to that planet. The
destination is `PLANET.idRoute & 0x3ff`, stored **one-based** so that zero can
mean "no route", which is why `stars_core::Planet::route_dest` subtracts the one
on the way in and adds it back on the way out.

The original picks the speed with `IFindIdealWarp` and a stargate case — if both
planets have a stargate, the fleet carries no minerals, and the gate can take
its heaviest ship, the leg is flown at "warp 11", the gate — then walks the
speed down while the fuel does not stretch. This engine keeps the fleet's own
warp setting instead, so the leg is right and its speed may not be.

## Lay Minefield (6)

The most-used of these tasks by a wide margin, and the one with a subsystem
behind it: [`minefields.md`](minefields.md) covers laying, growth and what a
field does to a fleet that flies through it. The arm is at `10b0:999e`:

```
if (iPass != 3) skip
if (!fleet.fHereAllTurn && GetRaceStat(player, rsMajorAdv) != raMines) skip
cMines = CLayMinesFromLpfl(lpfl, -1, -1);
if (cMines == 0) { message "no mine layers"; cancel }
if (this waypoint's task is still 6) {
    if (ord.union.word0 == 0)      ord.grTask = 0;    // the years ran out
    else if (ord.union.word0 != 5) ord.union.word0 -= 1;
}
... lay cMines mines ...
```

The task's payload is a **countdown of years**, and `5` means *indefinitely* —
it is never decremented and the task is never cleared. All 6,836 well-formed
lay-mines waypoints in the fixtures hold `(5, 5)`: everybody chose
"indefinitely", and the second word looks like the setting the countdown started
from. Keeping that payload through a load and a save is why
`stars_core::fleet::Waypoint` carries the task's raw bytes.

## Patrol (7) — not decoded

Forty waypoints in the corpus, and their payload is **all zeros** in every one,
so the patrol range is not in the `ORDER` union where the transport
instructions live. `FCheckPatrolWP` (`10f8:71ac`) takes `(ifl, iord, id, iWarp,
iPlan, iDist)`, so a range does exist somewhere; the waypoint's own warp is set
(7 in most of the fixtures) and the target class varies between planet, fleet
and none. Performing it needs both that parameter and the interception search
that picks a target, neither of which is recovered yet.

## Give (9) — not performed

`SatisfyOrders` handles it on pass 4, in a block long enough that the
reconstruction in `tmp/stars-decompile` gives up on it (`"full give-away logic
is very large"`). The start of it (`10b0:9556`) loops over all sixteen players,
tests a per-player field, creates a fleet for the receiver and copies position
and orders across — a fleet changing hands has to bring its **designs** with it,
which is the expensive part. No fixture contains one. Nothing here models it.

## Source

- `SatisfyOrders` `10b0:6798`; the arms cited above.
- `Merge2Fleets` `1050:c9f6`, `CreateSalvage` `10f0:7ee8`,
  `AutoRouteFleet` `1080:1e52`, `CLayMinesFromLpfl` `1080:2886`,
  `FCheckPatrolWP` `10f8:71ac`.
- Task ids: `grTask*` in the reconstructed `enums.h`, matching
  `stars_formats::task`.
