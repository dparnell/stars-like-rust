# Waypoint tasks

Status: **verified in outline** — every one of the ten tasks is simulated, and
Transport is transcribed whole. What is left is detail inside two of the
others, noted where it applies.

A waypoint carries a task in the low nibble of its flags word (`ORDER.grTask`,
see [`waypoint.md`](../formats/waypoint.md)), performed when the fleet reaches
it. `SatisfyOrders` (`10b0:6798`) runs the whole set, several times a year:
the turn pipeline calls it with a **pass** number and each task acts on the
pass it belongs to.

| id | task | pass | state |
|---:|------|------|-------|
| 0 | none | — | — |
| 1 | Transport | every pass: odd unload, even load | transcribed (`transport.rs`, below) |
| 2 | Colonize | the first it is seen on | simulated |
| 3 | Remote Mining | 3 | simulated, in its own pass (`mining.rs`) |
| 4 | Merge | 2 and 4 | simulated |
| 5 | Scrap | 1 | simulated |
| 6 | Lay Minefield | 3 | simulated (`minefields.md`) |
| 7 | Patrol | end of turn | simulated |
| 8 | Route | 4 | simulated |
| 9 | Give | 4 | simulated |

The four passes are `DoOrders(0)`'s 1 and 2 **before** the fleets move and
`DoOrders(1)`'s 3 and 4 after, `DropColonists` settling each pass's landings
before the next (`crate::orders::execute_tasks_pass`). So a task set on the
waypoint a fleet already stands at runs before it flies — a Scrap set at
home scraps in pass 1, and a Scrap on a waypoint reached this year waits for
next year's pass 1 — and `MoveFleets` holds a fleet whose current order is a
Transport still in progress or a Lay Minefield (`10b0:33c5`).

The task is **consumed** when it runs, which is why every waypoint in a saved
game that has already been reached reads `0`.

## Transport (1)

`10b0:686a`–`80f5`, transcribed in `crates/stars-core/src/transport.rs`. The
far side is the waypoint's object: the planet the fleet orbits, a fleet, a
mineral packet (minerals only, and no fuel), or deep space (fuel and nothing
else — and nothing at all can be *loaded* there). What may be **taken** is
`iSteal`: 3 at the player's own planet, 1 for their own fleet, else the
scanners' allowance — a Pick Pocket (scanner 5) lets a fleet be robbed, a
Robber Baron (14) a planet too. A load from a side that may not be taken from
waits, and on pass 4 is given up with `0x11f`/`0x120`/`0x123`.

Two stand-ins: at an **unowned** planet with another fleet of the player's
here all turn and carrying mining robots, the load is that fleet's dig (the
planet's surface, reported as `0x7d`); a **fleet of the player's own** with no
hold, here all turn at the player's planet, is a tanker, and minerals asked of
it come off the planet.

Each item is one of the five kinds with an action and a quantity:

| action | unload pass (1, 3) | load pass (2, 4) |
|---|---|---|
| Load All | — | all the far side has |
| Unload All | all aboard | — |
| Load Exact | — | the quantity |
| Unload Exact | the quantity | — |
| Fill Up To % | — | to the percentage of capacity |
| Wait For % | — | the same, and the task waits while short |
| Load Dunnage | — | after everything else, whatever room is left (a second round) |
| Set Amount To | down to the quantity | up to it, saying so (`0x121`/`0x122`) when the far side falls short |
| Set Waypoint To | give the far side up to the quantity | take its excess |

Fuel's Load Dunnage is **load optimal fuel**: once the rest is aboard, the next
leg's need is found (`EstFuelUse`, asked again until the lighter tank settles)
and the excess given back; with no next leg all the fuel goes back; short of
the need the task waits, and says why on pass 2 (`0x3c`/`0x3d`) or gives up on
pass 4 (`0x126`).

Unloading **colonists** onto a planet not the player's is a landing
(`FQueueColonistDrop`, `ground.md`) — refused with a message onto an empty
planet (`0x55`: it must be colonised), by an Alternate Reality captain
(`0x56`), or under a starbase (`0x135`); colonists cannot go to another
player's fleet (`0x155`) or into space (`0x165`); a fleet whose owner counts
the player an enemy takes nothing at all.

An unload pass keeps the task with each item done set to nothing; a load pass
consumes it (`idmHasCompletedAssignedOrders` when nothing lies beyond) unless
something is still waited for — a "Wait For" not met, a refused load before
pass 4, fuel short — when the fleet stays put for it. Tests:
`crates/stars-core/tests/transport.rs`.

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
not over the sum. Scrapped in **deep space**, each mineral less a quarter
(`recovered -= recovered >> 2`, `10f0:814b`) is dropped as a **salvage**
object where the fleet stood (`DropSalvage`), for anyone to load. Fuel and
colonists aboard are not recovered either way. A **Bleeding Edge Technology**
race's designs are re-costed at its current levels first (`UpdateShdefCost`
on a copy, `10f0:7f4e`) — `ShipDesign::true_cost`.

## Route (8)

`AutoRouteFleet` (`1080:1e52`). A fleet sitting at one of its owner's planets
that has a route destination set is given a waypoint to that planet. The
destination is `PLANET.idRoute & 0x3ff`, stored **one-based** so that zero can
mean "no route", which is why `stars_core::Planet::route_dest` subtracts the one
on the way in and adds it back on the way out.

The speed is `IFindIdealWarp`'s, or "warp 11" — the gate — when both planets
are the owner's with a gate on the starbase, the fleet carries nothing and its
heaviest design would jump undamaged (`stargates.md`); below warp 9 with a dock
at the far end, the fastest warp up to 9 whose range covers the leg; then
walked down while it costs no more years, and lower while the tank will not
stretch. The new leg carries the Route task on, so the chain continues, and
the player is told (`0x127`, or `0x128` when the warp came out 0 for want of
fuel). `crate::turn::auto_route_fleet`.

The Route arm of `SatisfyOrders` (`10b0:908f`) runs this on **any** pass for
a fleet with nothing beyond its waypoint at an owned planet with a route; on
the last pass a fleet that cannot be routed on is given work instead
(`AutoFleetOrder`, `ship2.c`): at a planet nobody owns — or the owner's own,
for an Alternate Reality race — a fleet with mining robots merges into
another of the owner's fleets on the spot that mines under 4,000 kT a year,
or, failing one, is set to Remote Mining where it is.

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

## Patrol (7)

Forty waypoints in the corpus, every one of them with an **all-zero payload**,
which turns out to be the answer rather than a dead end: the payload is the
patrol's warp and range, and zero means the defaults.

The task is not run by `SatisfyOrders` at all. It is run while each player's
turn file is **written**, because what a patrol does is decided from that
player's own view of the galaxy; this engine runs it at the end of a generated
year, which is the same moment.

`FCheckPatrolWP` (`10f8:71ac`) — a tutorial check, not the implementation —
gives away where the range lives: `ORDER + 0x0a`, the second word of the task
union. The first word is the patrol's warp. The rules are in `save.c`:

- **A fleet inherits the patrol.** If the first waypoint has no task but the
  second patrols, the first takes the task and both its settings.
- **A fleet already chasing a fleet is left alone.**
- **The search.** Every other player's fleet **on the patroller's map**
  (`lpflTarget->fInclude`, the year's view — `scanning.md`) is considered
  from where the patroller is — or, for a fleet in deep space with repeat
  orders, from where it is *going*. A candidate must be one the fleet's
  battle plan will attack
  (`FAttackPlayer`, `10f0:ae06`) and must match the plan's **primary target
  class** (`FMatchTarget`, `1038:6612`).
- **One patroller per target.** A target nobody has claimed this pass beats a
  nearer one that somebody has, and the chosen target is claimed.
- **The range** is `iDist × 50 + 50` light years, except that the tenth
  setting, which would be 550, means *as far as it takes*. Every patrol in the
  fixtures is set to `0`, so fifty light years.
- If the best target is inside the range, a waypoint naming that **fleet** is
  inserted after the first, and the player is told.

Two rules worth having on their own, because combat wants them too:

**Who a fleet will attack** (`FAttackPlayer`): the "attack who" byte of its
battle plan decides — `0` nobody, `1` enemies, `2` anyone who is not a friend,
`3` everyone, `4 + n` only player `n` — against the fleet owner's own relations
table.

**What counts as a target** (`FMatchTarget`): the **hull's** category, not what
is fitted to it. `1` is a freighter, `2..=4` the armed hulls, `5` a bomber and
`7` a fuel transport; "unarmed ships" means a fleet with no armed hull in it.
The original also takes an "exact" flag, and with it clear — which is how the
patrol search calls it — anything matches None, Any or Starbase.

## Give (9)

Hand a fleet to another player. The arm is at `10b0:932b`, on pass 4, and the
block is long enough that the community reconstruction gives up on it — but
what it does comes apart into four rules.

**Who gets it** is the waypoint's `id`, counted among the **other** players: an
index at or above the giver's own is shifted up by one, so the stored number is
a position in the list of everybody else. It has to land on a player who is in
the game.

**A fleet carrying colonists cannot be given** (`10b0:9436`, testing
`FLEET.rgwtMin[3]`). People are not a gift.

**The receiving player must have room for the designs.** Each design the fleet
uses is looked for in the recipient's own list first — an identical design is
reused rather than copied — and only the rest need a free slot, which the
original finds by walking their design array for one whose in-use bit is clear.
If any design has nowhere to go, the whole gift is refused.

**Then it changes hands**: the original builds a fleet for the recipient and
copies the position and orders across; this engine renumbers the fleet into the
recipient's numbering, remaps its stacks onto their design slots and stops it
where it stands.

No fixture contains one — nobody in the sample gave a fleet away — so this rests
on the binary alone. The messages the original sends, and the salvage-value
bookkeeping around a gift, are not modelled.

## Source

- `SatisfyOrders` `10b0:6798`; the arms cited above.
- `Merge2Fleets` `1050:c9f6`, `CreateSalvage` `10f0:7ee8`,
  `AutoRouteFleet` `1080:1e52`, `CLayMinesFromLpfl` `1080:2886`,
  `FCheckPatrolWP` `10f8:71ac`.
- Task ids: `grTask*` in the reconstructed `enums.h`, matching
  `stars_formats::task`.
