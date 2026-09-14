# Subsystem: Fleet Movement & Fuel

- **Status:** in progress — geometry and the fuel expression verified; engine fuel tables need the parts data
- **Ghidra routine(s):** `10b0:32ce` `MoveFleets`, `1050:9fe4` `EstFuelUse`, `1038:3fe4` `DGetDistance`, `1050:a9f4` `LFuelUseToWaypoint`
- **Manual reference:** `MANUAL.PDF` ch. 16 (fleet movement), p. 23-8 (battle movement is a *different* formula)
- **Uses RNG:** no (mine-field encounters, which can stop a fleet, do)
- **Implemented in:** `crates/stars-core/src/movement.rs`

## Distance and speed

A fleet travels in a straight line toward its next waypoint at `warp^2` light
years per year — warp 9 covers 81 ly.

```
d       = sqrt((x2-x1)^2 + (y2-y1)^2)      # double precision, from 32-bit terms
dTravel = warp * warp
needed  = (int)(d + 0.9999)                # "close enough to arrive"
if dTravel >= needed: dTravel = needed     # stop exactly on the waypoint
if dTravel > fuelRange: dTravel = fuelRange
```

The `+0.9999` bias means a fleet 81.4 ly from its waypoint arrives this year at
warp 9 rather than stopping 0.4 ly short; the matching `-0.99999` bias on the
other side decides whether the fleet counts as having arrived.

## Position

```
if arrived:
    pt = waypoint
else:
    r  = dTravel / d
    pt.x = (int)((x2 - x1) * r + (0.5 if x2 > x1 else -0.5)) + x1
    pt.y = (int)((y2 - y1) * r + (0.5 if y2 > y1 else -0.5)) + y1
```

The `±0.5` is a round-half-away-from-zero applied per axis, so a fleet drifts
outward rather than toward the origin.

## Fuel

```
for each ship design in the fleet, cheapest engine first:
    cargo assigned to this design, up to its capacity
    mass = cargo + count * hullEmptyMass
    eff  = engine.rgcFuelUsed[warp]
    if ImprovedFuelEfficiency: eff -= eff * 15 / 100
    fuel += mass * eff * distance / 2000

fuel = (fuel + 9) / 10
```

`rgcFuelUsed[]` is a twelve-entry table per engine indexed by warp; a zero entry
means the engine runs free at that speed (which is how ramscoops and the low
warps of ordinary engines are expressed). The `/2000` and the final `/10` are
the fixed-point scaling of that table.

Range, rather than fuel, is obtained by running the same computation over a
nominal 1000 ly and dividing: `range = fuelOnBoard * 1000 / fuelPer1000`.

## Order of movement within a turn

From `FGenerateTurn` (`10b0:0000`): orders are applied, then `MoveThings(0)`,
then `MoveFleets`, and only afterwards `Produce` (which grows populations and
mines). Fuel is topped up by `FuelFleets` *after* production, so a fleet that
runs dry does so against last year's fuel.

## Running dry (`MoveFleets`, `10b0:42c3`)

Before a leg is flown the tank is measured against the **whole of the rest of
the leg** — `EstFuelUse` over the remaining distance:

* **enough**: the year's travel (`warp²`, or the rest of the way) is flown and
  paid for. The fuel range is not consulted, so a fleet with exactly enough
  arrives on its last drop;
* **not enough**: the fleet flies as far as the range allows this year and the
  tank is **set to zero**, not debited — the rounding in the range is simply
  forgiven.

Then, if that has left the fleet dry and still short of the waypoint, the leg
is slowed. The original counts warps up from 1, asking `EstFuelUse` what the
rest of the leg costs at each, and stops at the **first that costs anything**;
the one before it — the fastest warp the engines run free at — is written
onto the waypoint and the player is sent `idmHasRunFuelFleetsSpeedHasDecreased`
(`0x8b`, parameters `[fleet, warp]`). If warp 1 itself costs fuel there is
nothing to slow to: the fleet stays put and gets `idmHasRunFuel` (`0x27`). Every
stock engine is free at warp 1, so a stranded fleet in practice creeps on at
one light year a year.

Worked example — the tutorial's Teamster: a Medium Freighter on a Long Hump 6,
130 kT empty, 210 kT of ironium aboard, 306 mg in the tank and 49 ly to go at
warp 7. The leg costs `340 × 450 × 49 / 2000 / 10 = 375` mg, more than the
tank, so the fleet covers `306 × 1000 / 7650 = 40` ly, the tank is zeroed, and
the leg is rewritten to warp 1 (warp 2 would cost `340 × 20 × 9 / 20000 = 3`
mg). `crates/stars-core/tests/fuel.rs` checks the three cases.

## Chasing a fleet (`MoveFleets`, `10b0:426f`)

A waypoint aimed at a **fleet** is a moving target, and `MoveFleets` flies
it in passes. In the first pass every fleet whose next waypoint names a
fleet is skipped; the rest fly their year. Then, up to ten times over, the
chasers fly: each first has its waypoint moved to wherever its quarry now
stands, and then covers

* all of what it has left for the year (`warp²` less what it has flown)
  when the quarry has finished moving — a ship chasing a scout that has
  landed reaches it if it can;
* otherwise `(left + used + 4) / 5`, a fifth of the year's travel, so that
  two fleets chasing each other close in steps rather than one of them
  jumping the whole way at once.

A chaser that arrives, or runs out of allowance, or covers nothing, leaves
the chase; a chase that is still going after ten passes is left where it
is. Arriving on a fleet puts the chaser **in deep space** beside it, not in
orbit: only a waypoint aimed at a planet sets a fleet's orbit
(`KillUsedWaypoints` copies the waypoint's object over the fleet's, and a
fleet is not a planet). The tutorial's Armed Probe #9 catching the
Berserkers' scout over Hiho (page 37's battle) is this chase in the engine
(`turn.rs`, the `chase` list); the probe orbits nothing afterwards, which
is what the page's battle record says as well.

## Arriving, and Repeat Orders (`KillUsedWaypoints`, `1080:189a`)

When a fleet stands on its next waypoint, `KillUsedWaypoints` copies that
waypoint over the one it left and drops it with `DeleteWpFar(lpfl, 1,
fRepOrders)` (`1050:9e28`). With **Repeat Orders** on, the drop puts the
waypoint back at the **end** of the route instead — task and all, so the
route circles: `[A, B, A]` arriving at `B` becomes `[B, A, B]`. Three
things stop the recycling: a route of one leg (`cord == 2`, nothing to go
round), the last waypoint already standing where the reached one does,
and a Merge with a fleet (`1080:1bfb`). The engine does the same in
`move_fleet`; it is how the tutorial's Teamster keeps going back to Prune.

A fleet that reaches the last of its orders is told so (`0x4e`), unless
the waypoint carries a task that reports for itself.

## Ships that change fleets take their share (`FleetTransferCargoBalance`, `1050:ae7d`)

Every ship transfer — a split, a merge, Split All, a stargate jump, a
minefield's toll, `FRunLogRecord` replaying a ships record — ends with the
two fleets' cargo and fuel shared out by capacity. Each side gives up the
share of what it carries that the ships it **lost** made of its capacity:

```
fuel_out   = fuel  × lost_tank ÷ tank
cargo_out  = cargo × lost_hold ÷ hold        (the four kinds together)
```

with the cargo spread over the kinds in proportion to what is aboard and
any rounding shortfall made up a unit at a time from the first kind that
still has some. A side that gained ships gives nothing. So a split hands
the new fleet exactly its ships' share, and a merge pulls everything into
the survivor. `stars_core::fleet::balance_cargo`; the damage-percentage
rebalancing the same routine does is not modelled.

## Edge cases & clamps

- A fleet with a chase order (`grobj == 2`) re-runs the movement loop up to
  eight times as its target moves.
- Warp 10 and above is the stargate path and skips the mine-field check.
- Engines with id 10 (the radiating ram scoop) kill colonists in transit; the
  loss is `(86 - averageRadiation) / 2 * colonists / 100`, minimum 1, and only
  for races whose radiation range is not immune and averages below 85.
- A fleet that runs out of fuel mid-leg has its fuel zeroed and travels only
  as far as its range allowed — see *Running dry* above for what happens to
  its warp.

## Worked example (becomes a test vector)

A fleet at warp 9 travels `9 * 9 = 81` light years a year.

Captured at: `../vectors/planetary-economy.json` (`movement`).

## Open questions

- ~~Engine fuel-use tables live in the components data, which is not yet
  decoded.~~ **Resolved in Step 4:** `rgengine` is transcribed in
  `components.md` and `movement::engine_fuel_use` reads it. `fuel_used` is
  still parameterised by the caller, because assigning cargo to designs needs
  the hull table.
- Mine-field traversal (`FTravelThroughMineFields`, `10b0:4f60`), stargates and
  the chase loop are specified only in outline here; they belong with order
  processing in Step 4.
- Battle movement is an entirely separate formula (`MANUAL.PDF` p. 23-8) and is
  **not** covered by this spec.
