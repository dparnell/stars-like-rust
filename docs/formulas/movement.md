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

When a fleet cannot afford its ordered warp, the original searches downward for
the fastest warp it *can* fuel, sets the waypoint's warp to that, and messages
the player; if even warp 1 is unaffordable the fleet stops.

## Edge cases & clamps

- A fleet with a chase order (`grobj == 2`) re-runs the movement loop up to
  eight times as its target moves.
- Warp 10 and above is the stargate path and skips the mine-field check.
- Engines with id 10 (the radiating ram scoop) kill colonists in transit; the
  loss is `(86 - averageRadiation) / 2 * colonists / 100`, minimum 1, and only
  for races whose radiation range is not immune and averages below 85.
- A fleet that runs out of fuel mid-leg has its fuel zeroed and travels only
  as far as its range allowed.

## Worked example (becomes a test vector)

A fleet at warp 9 travels `9 * 9 = 81` light years a year.

Captured at: `../vectors/planetary-economy.json` (`movement`).

## Open questions

- Engine fuel-use tables live in the components data, which is not yet decoded;
  the fuel function is therefore parameterised by the caller rather than
  looking parts up itself.
- Mine-field traversal (`FTravelThroughMineFields`, `10b0:4f60`), stargates and
  the chase loop are specified only in outline here; they belong with order
  processing in Step 4.
- Battle movement is an entirely separate formula (`MANUAL.PDF` p. 23-8) and is
  **not** covered by this spec.
