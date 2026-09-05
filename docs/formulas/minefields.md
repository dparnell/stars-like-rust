# Minefields

Status: **in progress** — laying, growth and traversal are simulated; decay,
sweeping and detonation are not.

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
an interval of light years, and every light year inside one is a separate roll:

```
hit if Random(1000) < (warp - safe warp - expertise) × chance
```

The first hit stops the fleet where it happened. Damage is `damage a ship ×
ships`, except that a fleet of **four or fewer** takes the kind's minimum
instead — which is why a lone scout is such an expensive way to find a
minefield.

Three things the original does that this does not yet: it merges overlapping
intervals of the same kind so two fields on top of each other are rolled once,
it scales damage by the engine count, and it lets shields absorb before armour.
Damage here is applied to the fleet as a total.

## Not modelled

- **Decay.** Fields shrink each year, faster near planets; nothing here shrinks
  them.
- **Sweeping.** Beam weapons clear mines from a field a fleet sits in
  (`CMineSweepFromLpfl`, `1080:2b26`).
- **Detonation.** `fDetonate` is carried through the file but never acted on.
- **Visibility.** `grbitPlr` and `grbitPlrNow` say who has seen a field; they
  are preserved, not maintained.

## Source

- `SatisfyOrders` lay-mines arm `10b0:999e`, placement loop `10b0:9aa9`.
- `CLayMinesFromLpfl` `1080:2886`; the component table in
  `stars_core::components::MINE_LAYERS`.
- `FTravelThroughMineFields` `10b0:4f60` and the four tables above.
- `docs/formats/thing.md` for the record, and `docs/formulas/waypoint-tasks.md`
  for the task that starts it all.
