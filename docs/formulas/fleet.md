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

Fuel is deliberately *not* deducted yet: that needs each design's engine and
the cargo assignment across stacks, so a fleet currently never runs dry. That
is the one place this is knowingly generous, and it is the likeliest cause of
some of the 60 that do not match.

## Open questions

- **Orders** beyond movement — cargo transfer, colonisation, remote mining —
  are decoded by the format layer but not modelled. This is what holds the
  whole-turn replay's surface mineral figure down to 19%.
- Fuel consumption during movement, as above.
- Ship building: the production queue recognises ship designs but cannot turn a
  completed one into a fleet.
- Fuel consumption is implemented (`movement.md`) but nothing calls it, because
  nothing moves yet.
