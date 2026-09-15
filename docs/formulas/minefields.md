# Minefields

Status: **in progress** — laying, growth, traversal, damage, decay, sweeping
and detonation are simulated. What is left is the merging of overlapping
intervals of one kind, and the visibility a Space Demolition owner gains.

A minefield is a circle in the galaxy and a `THING` in the file
([`thing.md`](../formats/thing.md), `ith = 0`). It stores a centre and a mine
count, and nothing else, because **its radius in light years is the square root
of its mine count**. The game never writes the radius down: the laying code
asks whether a fleet is inside a field by comparing the *squared* distance
against the count (`10b0:9c2b`), which is the same test.

Model: [`stars_core::minefield`](../../crates/stars-core/src/minefield.rs).
There are 24,193 minefield objects across the fixtures, and 6,892 waypoints
carrying the order that made them.

## The three kinds

Every number that separates them is a table in the executable:

| kind | safe warp | chance a light year | damage a ship | least damage |
|------|----------:|--------------------:|--------------:|-------------:|
| 0 standard | 4 | 0.3% | 100 (125) | 500 (600) |
| 1 heavy | 6 | 1.0% | 500 (600) | 2000 (2500) |
| 2 speed bump | 5 | 3.5% | 0 | 0 |

Bracketed figures apply to a fleet with a ram scoop. The chance is **per light
year travelled, per warp factor over the safe speed**.

- `rgiWarpSafe` `10b0:4f5a` — `4, 6, 5`
- `rgpctMineHit` `10b0:4f54` — `3, 10, 35` (per mille)
- `rgrgdmgMine` `10b0:4f3c` — `100 125 | 500 600 | 0 0`
- `rgrgdmgMinMine` `10b0:4f48` — `500 600 | 2000 2500 | 0 0`

## Laying

The Lay Minefield waypoint task, run in the same pass as remote mining. The arm
is `10b0:999e` and the placement loop `10b0:9aa9`.

**Who may lay.** A fleet must have been *here all turn*. The exception is a
Space Demolition player (`rsMajorAdv == raMines`), who lays while moving — and
a fleet that moved lays **half** as many mines.

**How many.** `CLayMinesFromLpfl` (`1080:2886`):

```
mines = 10 × Σ over designs: ships × Σ over slots:
            (slot is a mine layer of this kind ? count × part.ability : 0)
```

The ten is what turns the stored `ability` of 4 on a "Mine Dispenser 40" into
the forty mines its name promises. The ten mine-layer parts are split by kind by
the item-index bounds the routine takes: items 0–3 are the Mine Dispensers
(standard), 4–6 the Heavy Dispensers, 7–9 the Speed Traps. A fleet lays each
kind it carries, into a separate field.

Two cases in that routine are **not** modelled: one beam-slot item that also
counts toward a standard field, and hulls 27 and 28, whose per-ship total is
*replaced* rather than added to.

**Where they go.** Of the player's own fields of that kind that **contain** the
fleet, the nearest takes the mines, unless it already holds more than
`1,000,000`, in which case a new field starts. Growing a field moves its centre
toward the fleet, weighted by the two mine counts:

```
centre = (fleet × laid + centre × held) / (laid + held)
```

Otherwise a new field starts where the fleet is. Because each kind is laid
separately, one fleet can leave a standard field and a speed bump on the same
spot — which is exactly what the fixtures show, for instance player 13's 681
standard mines and 271 speed-bump mines sharing `(1079, 1844)`.

**The countdown.** The task's payload is a number of years: `5` means
*indefinitely* and is never spent, `0` clears the order, and anything else
counts down by one each year (`10b0:9a46`). All 6,836 well-formed lay-mines
waypoints in the fixtures hold `(5, 5)` — everyone chose "indefinitely".

## Flying through one

`FTravelThroughMineFields` (`10b0:4f60`), run on every leg as it is flown.

The speed that matters is **recovered from the distance travelled**, not read
from the order: the original searches for the first `iWarp` in `3..10` whose
square reaches `travelled - 1`. A fleet at or under the safe warp is never at
risk, and a race's mine expertise counts as extra safe warp factors — Space
Demolition two, Super Stealth one (`10b0:4f94`).

Only fields belonging to **another player who is not a friend** are tested; your
own and your friends' are flown through freely. Each field the leg crosses gives
an interval of light years (`FIntersectCircleLine`), which goes into a list
kept **per kind** — three lists of up to eight, sorted by start
(`10b0:5049`–`10b0:52a0`). An interval that overlaps or touches one already
there (its end no earlier than that one's start less one) is merged into it,
widening it and swallowing whatever later intervals it now reaches; a ninth
disjoint interval of a kind is dropped. So two fields of a kind on top of
each other are rolled once. The lists are then walked together, always the
earliest start next (`10b0:56b6`), and every light year inside an interval
is a separate roll:

```
hit if Random(1000) < (warp - safe warp - expertise) × chance
```

The first hit stops the fleet where it happened — `from + (to − from) × hit /
length` (`10b0:5a2e`) — and the field it hit is the **deepest** enemy field
of that kind at that point, the least `distance² − mines` (`10b0:5cc0`):
a fleet caught in the overlap of two fields is charged to the one whose
centre it is nearest, radius for radius. `minefield::traverse` and
`merge_span`, with unit tests for the merging and for two overlapping
fields rolling as one.

A hit costs the **field** as well: `cMines / 20`, or `cMines / 100` once the
first would pass fifty, with floors of ten and fifty — about 5% of a small field
and 1% of a large one (`10b0:2097`). The fleet's owner also learns the field is
there, which is the one piece of minefield visibility this engine maintains.

### The damage

`FTravelThroughMineFields` from `10b0:57f1`, implemented in
[`minefield::apply_damage`](../../crates/stars-core/src/minefield.rs) and
wired up by `turn::mine_hit`.

The tables are the kind's **damage a ship** and **minimum total**
(`rgrgdmgMine` `10b0:4f3c`, `rgrgdmgMinMine` `10b0:4f48`), each with a
plain column and a *ram scoop* column. The ram-scoop column applies when any
design in the fleet has an engine whose `rgcFuelUsed[4]` is zero
(`10b0:5613`) — one that runs free at warp 4, which is every scoop and also
the Settler's Delight, the Fuel Mizer and the Enigma Pulsar. A fleet of
**four or fewer** whose `damage a ship × ships` falls short of the minimum is
topped up to it, once, on the first design with ships.

Then, design by design (`10b0:5a19`–`10b0:5c60`):

```
damage   = (damage a ship × ships + top-up) × engines on the hull
absorbed = min(damage, shields a ship × ships)
damage   = damage − absorbed
         + armour × damaged ships × damage‰ / 500     (what they carried)
share    = damage / ships
if armour < share:  every ship of the design is destroyed
else:               100% damaged, damage‰ = share × 500 / armour, at least 1
```

The engine count is the design's engine slot count, so a hull with two engines
takes twice the blow. Shields are `DpShieldOfShdef`'s figure — the battle
value, Regenerating Shields' 40% included — pooled across the ships of the
design; armour is the design's stored `dp`. The damage a stack already carried
is converted back to points and added, and the whole is spread evenly, which
is why a stack is either wholly destroyed or wholly marked. The total dealt,
before shields, is what the messages report, capped at `0x7ff8`.

**What the dead take with them.** The destroyed ships are made into a fleet
of their own and `FleetTransferCargoBalance` (`10b0:5d3a`) moves them their
share of the cargo — the part the survivors' holds can no longer carry —
which `DropSalvage` (`10b0:5f2c`) leaves as a stationary packet where the
fleet was stopped. A fleet left with nothing is gone.

**Detonation** (`ThingDecay`, `10b8:70c6`) runs the same routine with no roll
over **every** fleet inside the field — the owner's own included, all but its
ships on the Mini Mine Layer and Super Mine Layer hulls (`10b0:58f4`) — once
a year however many fields go off (`det` bit 12), and a detonation that does
no damage is not a hit at all (`10b0:5cf0`). It costs the field nothing extra
and grants no visibility.

**Messages** (`10b0:6174` on): the fleet's owner gets `0xc5` (no damage),
`0xc6` (damage), `0xc7` (ships lost), `0x15f` (destroyed) or `0xc8`
(destroyed, salvage left — object `−6`, the salvage's id first); the field's
owner `0xc9`, `0xca`, `0xcb` or `0xcc` (object `−6`, the salvage's or the
field's id), and `0x162` when the fleet was its own. A detonation says
`0x160`/`0x161` and `0x163`/`0x164` instead of the `0xc6`/`0xc7` and
`0xca`/`0xcb` pairs. The wordings here are this project's.

Not modelled: a Space Demolition field owner learns the designs its field hit
(`10b0:6174` sets a seen-by bit on each), for which the state has no place.

### Worked example

Ten scouts of 400 armour and one engine each, no shields, in a standard field:
`100 × 10 = 1,000`, no top-up (more than four ships), no shields, share 100 a
ship — under 400, so all ten are marked 100% damaged at `100 × 500 / 400 =
125‰`. Hit again the same way, they carry `400 × 10 × 125 / 500 = 1,000` back
into the sum: 2,000 over ten is 200 a ship, `250‰`. Ten scouts of 20 armour
are destroyed outright by the first hit. `minefield::tests` pins each of
these, the shield and engine cases, and the spared mine layers;
`tests/minefields.rs` runs a fleet through a heavy field in a whole turn,
both surviving and destroyed with its cargo left behind.

## Decay

`ThingDecay` (`10b8:70c6`), run straight after movement — step 8 of
[the turn order](turn-order.md).

```
pct = 2 + (Space Demolition ? 1 : 4) × planets inside the field
pct = min(pct, 50)
if armed to detonate: pct += 25
lost = max(cMines × pct / 100, pct)
if kind != speed bump: lost = max(lost, 10)
```

Two percent a year, in other words, and four more for every planet inside the
circle — one for a Space Demolition player, whose fields last four times as
long against the same planets. A field that loses everything is gone.

The floors matter more than they look: a small standard field loses at least ten
mines a year whatever the percentage says, so a field of a few dozen mines
evaporates in a couple of years unless it is fed. Speed bumps have no such
floor.

## Sweeping

`SweepForMines` (`10b8:76a4`), run late — step 14, after the second pass of
orders. Everything armed with beams clears the fields it is sitting in that
belong to a player it is not friendly with: first every **fleet**, then every
**planet with a starbase**.

What one design sweeps a year is `CMineSweepFromLphul` (`1080:2bfa`):

```
sweep = Σ over beam slots: range² × count × damage
```

with three adjustments — a **starbase** reaches one square further, a
**gattling** sweeps as though its range were 4 whatever it really is, and a
**sapper** sweeps nothing at all. A fleet sweeps that per ship.

A **speed bump** field gives up only a third of what the sweeper could manage,
and every sweep clears at least two mines. The last rule is the interesting one:
if the sweep would take the field below the sweeper's own distance from the
centre, it takes exactly enough to leave the field just short of the sweeper
instead. A fleet can therefore shrink a field until it is standing outside it,
but only a fleet at the very centre — or one whose sweep exceeds the whole field
— clears it away entirely.

## Detonation

A field with `fDetonate` set goes off under everyone standing in it, at the top
of `ThingDecay`: every fleet inside that does not belong to the field's owner
takes the damage it would have taken flying into it, with no roll. The field
then decays 25 percentage points faster for the privilege.

## Not modelled

- **Visibility.** `grbitPlr` and `grbitPlrNow` say who has seen a field. They
  are preserved, and set for a player who hits or sweeps one, but not otherwise
  maintained — nothing here recomputes what each player's scanners can see.
- The merging of overlapping intervals of the same kind in the traversal
  roll, and the Space Demolition owner's view of the designs its field hit.

## Source

- `SatisfyOrders` lay-mines arm `10b0:999e`, placement loop `10b0:9aa9`.
- `CLayMinesFromLpfl` `1080:2886`; the component table in
  `stars_core::components::MINE_LAYERS`.
- `FTravelThroughMineFields` `10b0:4f60` and the four tables above.
- `ThingDecay` `10b8:70c6`, `SweepForMines` `10b8:76a4`,
  `CMineSweepFromLphul` `1080:2bfa`.
- `docs/formats/thing.md` for the record, and `docs/formulas/waypoint-tasks.md`
  for the task that starts it all.
