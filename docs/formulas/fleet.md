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

## Open questions

- **Orders and waypoints** are decoded by the format layer but not modelled, so
  fleets do not move, transfer cargo, or colonise. This is the largest single
  gap in the turn pipeline and is what holds the whole-turn replay's surface
  mineral figure down to 19%.
- Ship building: the production queue recognises ship designs but cannot turn a
  completed one into a fleet.
- Fuel consumption is implemented (`movement.md`) but nothing calls it, because
  nothing moves yet.
