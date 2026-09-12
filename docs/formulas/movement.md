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
