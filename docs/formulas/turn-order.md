# Subsystem: Turn Generation Order

- **Status:** verified (the ordering); the steps themselves are at varying stages
- **Ghidra routine(s):** `10b0:0000` `FGenerateTurn`, `10b8:0000` `Produce`
- **Manual reference:** —
- **Uses RNG:** yes, at several points
- **Implemented in:** `crates/stars-core/src/turn.rs`

The order steps happen in is itself a specification: get it wrong and results
diverge even when every individual formula is right.

## The pipeline

From `FGenerateTurn` (`10b0:0000`), after the host loads the game and replays
each player's order file:

```
 1. shuffle player order          Random(cPlayer - i) — a Fisher-Yates shuffle
 2. load and run each .xN order log
 3. validate waypoints
 4. DoOrders(0)                   first-pass orders (including cargo transfers)
 5. UnmarkMineFields
 6. MoveThings(0)                 packets, wormholes, Mystery Traders
 7. MoveFleets                    movement, fuel, mine-field traversal
 8. ThingDecay
 9. BreedColonistsInTransit
10. Produce                       mining, resources, building, growth, research
11. MoveThings(1)
12. FuelFleets
13. DoOrders(1)                   second-pass orders
14. SweepForMines
15. HealShips
16. AutoTerraform
17. RemoteTerraforming
18. SpankTheCheaters
19. ValidateWaypoints
20. UpdateGuesses
21. game.turn += 1
22. UpdatePlayerScores
23. recompute every design's scanner range and cloaking
24. write the .hst and each .mN
```

and `Produce` (`10b8:0000`) expands to:

```
MineMinerals → per-planet resource split and build queue →
UpdatePopulations → UpdateResearchStatus → RandomEvents
```

## Orderings that matter

- **Mining precedes production**, so minerals dug this year can be spent this
  year.
- **Production precedes population growth**, so a planet builds with the
  population it started the year with.
- **Population growth precedes research**, but both are inside `Produce`, and
  research is fed by resource accounting that happened *before* growth.
- **Terraforming happens after `Produce`** in the pipeline, yet the growth
  step of the *following* year sees the terraformed environment. Replaying the
  Exodus fixture confirms this: predicting growth from the pre-terraform
  environment matches 344 of 426 planet-years, from the post-terraform
  environment 372. See `population.md`.
- **Cargo moves twice**, in `DoOrders(0)` before growth and `DoOrders(1)`
  after, which is why a planet's recorded population can differ from
  growth alone in either direction.
- **Fuel is replenished after production**, so a fleet that runs dry does so
  against last year's fuel.

## What `generate_turn` currently performs

`crates/stars-core/src/turn.rs` runs mining, the per-planet resource split,
population growth and the research advance — the steps whose formulas are
recovered. Every other step is returned in `TurnReport::skipped` rather than
quietly omitted, so a partial turn cannot be mistaken for a complete one.

## Open questions

- Steps 1–9 and 11–24 need fleets, orders and ship designs.
- The player shuffle at step 1 consumes RNG draws before anything else does, so
  reproducing a turn bit-for-bit will require it even though it only decides
  the order order-files are replayed in.
