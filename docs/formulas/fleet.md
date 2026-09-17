# Subsystem: Fleets

- **Status:** model and loading done; orders, movement and cargo transfer are not
- **Implemented in:** `crates/stars-core/src/fleet.rs`, loaded by `crates/stars-core/src/load.rs`
- **Records decoded by:** `stars_formats::FleetRecord`, see `../formats/fleet.md`

A fleet is a group of one player's ships, made of **stacks** — some number of
one design each. Everything the simulation needs about it derives from those
stacks and the designs they name.

## Derived values

| Value | Rule |
|-------|------|
| Ships | sum of the stacks' counts |
| Mass | Σ (design mass × count) + cargo mass |
| Cargo capacity | Σ (design cargo capacity × count) |
| Fuel capacity | Σ (design fuel capacity × count) |
| Armed | any stack whose design carries a weapon |

Cargo mass counts minerals and colonists; **fuel is massless**, which is why a
fleet's range does not shrink as it burns.

## What can and cannot be checked

This is worth stating because it is easy to imagine a check that does not
exist. A fleet record carries a `mass` field **only when it is an enemy fleet
seen at a distance** (record types 17 and 18) — and our designs do not describe
another player's ships, so that number cannot be compared against a computed
one. A player's *own* fleets carry no mass at all, because the game recomputes
it from the designs.

So there is no case in the fixtures where a recorded fleet mass can be checked
against a computed fleet mass. What `crates/stars-core/tests/load_real_games.rs`
does instead is verify the invariants that *are* falsifiable across 97 loaded
fleets, 66 of which are built entirely from designs the file also carries:

- every live fleet in the file is loaded, cross-checked against the format
  layer's own count;
- no fleet carries negative cargo, or masses less than what it carries;
- **no fleet carries more fuel than its designs' tanks hold** — which is a real
  check on the design fuel model, since the fuel figure comes from the file and
  the capacity from our component tables.

## Movement

Waypoints are associated with their fleet by position in the block stream: a
fleet's waypoint blocks are written immediately after it, which is how the
loader pairs them.

Each turn a fleet covers `warp^2` light years toward its next waypoint,
stopping exactly on it if that would overshoot, using the geometry in
`movement.md`. On arrival the waypoint is consumed and the fleet is left
orbiting whatever it named.

**378 of 438 moving fleets land exactly where the engine put them** — checked
against the recorded positions in the next year's file, with no allowance made,
since a fleet's position depends only on its own orders. The remainder are
fleets whose orders changed, that merged or split, or that ran out of fuel.

Fuel **is** deducted. The subtlety, from `EstFuelUse`, is the cargo: it is
assigned to the **most fuel-efficient designs first**, filling each to its
capacity, so a fleet carries its load in whatever burns least to move it. Each
design then burns `mass * engine figure * distance / 2000`, and the total is
divided by ten, rounding up; Improved Fuel Efficiency cuts each engine's figure
by 15% first. A fleet that cannot afford its whole leg travels only as far as
its fuel allows.

Adding fuel did not change the 86%, which is the reassuring outcome: it means
the model is not cutting fleets short that the engine let through.

## Damage repair (`HealShips`, `10b8:444c`)

A stack's damage is two fields of one word (`DV`): `pctSh`, the percent of its
ships that are damaged (7 bits), and `pctDp`, how much each of those has lost,
in **500ths of the design's armour** (9 bits). `HealShips` runs late in the
turn, after `SweepForMines`, and for every fleet that is not dead and not
`fNoHeal` — set on a fleet that fought (`DoBattles`), was caught in a minefield
(`FTravelThroughMineFields`) or jumped a gate (`FStargateJump`), and cleared
for everybody at the top of `FGenerateTurn` — takes this much off every damaged
design's `pctDp`, clearing the whole word when the damage is no more than it:

| Where the fleet is | 500ths | guide's figure |
|---|---:|---:|
| moved this year (`!fHereAllTurn`) | 5 | 1% |
| stopped in space | 10 | 2% |
| over somebody else's planet, or nobody's | 15 | 3% |
| over its own planet: no starbase, or the starbase was attacked this year (`PLANET.fNoHeal`) | 25 | 5% |
| over its own starbase without a dock (`wtCargoMax == 0`) | 40 | 8% |
| over its own dock | 100 | 20% |

The rate is **doubled for Inner Strength**, and then a bonus is added for a
tanker in the fleet: 50 (10%) if any design is a Super Fuel Xport (hull 26),
else 25 (5%) if any is a Fuel Transport (hull 25) — the better one only, not
both, and not doubled.

Then every starbase whose planet was not attacked this year mends 50 (10%) of
its own `pctDp`, 75 (15%) for Inner Strength.

The guide's *Damage Repair* topic (`STARS!.HLP` context `0x410661`) states the
same table in percent and adds that a fleet bombing a planet does not heal;
`DoBombing` (`10f0:aefa`) sets no such flag in this build, so a bombing fleet
over an enemy planet heals at the 3% rate unless it also fought.

Worked examples: `crates/stars-core/tests/healing.rs`.

## Open questions

- ~~Orders beyond movement are decoded by the format layer but not
  modelled.~~ Every waypoint task is simulated now — see
  `waypoint-tasks.md`.
- ~~Ramscoops gain fuel in flight; not modelled.~~ `LCalcFuelGainFromRamScoops`
  is in `movement.md` under *Ramscoop fuel*.
- ~~When a fleet cannot afford its ordered warp the original searches
  downward.~~ Done: `movement.md`, *Running dry*.
- ~~Ship building: the production queue cannot turn a completed ship into a
  fleet.~~ Done: `production.md`.
- ~~Fuel consumption is implemented but nothing calls it.~~ Movement calls it.
- Stargate jumps are `stargates.md`; repair is *Damage repair* above.
