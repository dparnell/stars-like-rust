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
13. DoOrders(1)                   DoBattles, DoBombing, then second-pass orders
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

## Before the year: replaying what the players submitted

A host does not start from its own state alone. Each player's client has
already carried their orders out against its copy of the game and written a
`.xN` recording what it did; the host replays those logs first, so that its
state matches every client's before the year runs.

`stars_core::replay` does that. Everything a log carries is applied
**immediately** — waypoints, production queues, research settings, planet
routing, ship designs — because that is where the client had already applied
it. The one exception is cargo transfers, which are collected into a
`TurnOrders` and handed to the generator, because `DoOrders(0)` is a step *of*
the year and a transfer applied there feeds the same year's growth.

An order log arrives from the player's machine, so every operation is checked
against the player it came from: a waypoint must name one of their fleets, a
queue or routing change one of their planets, a design change their own list.
A cargo transfer may legitimately touch someone else's planet — that is how
colonists are landed — so only its fleet end is checked. Anything else is
counted as rejected and dropped. See `../formats/orders-x.md`.

## When a planet changes hands

A planet that becomes a player's — settled or taken — does not start with an
empty production queue. The player keeps a **default queue** (`PLAYER.zpq1`),
and the game hands it to the planet along with its "no research" flag, dropping
the entries the race cannot build: an Alternate Reality race queues no planetary
installation, a Claim Adjuster no terraforming. See `../formats/player.md`.

## What `generate_turn` currently performs

`crates/stars-core/src/turn.rs` runs the recorded **cargo transfers**
(`DoOrders(0)`) and the colonist landings they cause (`DropColonists`), mining,
the per-planet resource split,
**the production queue**, population growth, the research advance and
**`AutoTerraform`** (step 16, the Claim Adjuster's free terraforming) and
**`RemoteTerraforming`** (step 17, Orbital Adjusters acting from orbit), plus
**remote mining** from `SatisfyOrders(3)` — the
steps whose formulas are recovered. Every other step is returned in
`TurnReport::skipped` rather than quietly omitted, so a partial turn cannot be
mistaken for a complete one.

The queue builds the planetary installations; ship designs are recognised but
skipped, because turning a completed design into a fleet needs a fleet model
the pipeline does not have yet.

## How much of a turn is right

`crates/stars-core/tests/differential_turn.rs` loads a year of the Exodus game,
generates a turn, and compares every planet field against the file the original
engine wrote for the following year. Over 28 year-pairs and 438 planet-years:

| field | agrees |
|-------|-------:|
| population | **87%** |
| mineral concentrations | **86%** |
| mines | 65% |
| factories | 53% |
| surface minerals | 19% |

The top two are the subsystems the pipeline models end to end, and they match
their individual differential tests. The rest fall away for understood reasons:
mines and factories depend on a build queue whose ship items cannot be built
yet, and surface minerals move with cargo the pipeline does not carry and are
spent on ships it does not build.

The test asserts the top two and reports the rest, so a regression in what is
modelled shows up without pretending the rest is finished.

## Open questions

- Steps 1–9 and 11–24 need fleets, orders and ship designs.
- The player shuffle at step 1 consumes RNG draws before anything else does, so
  reproducing a turn bit-for-bit will require it even though it only decides
  the order order-files are replayed in.


## The whole-turn replay's surface-mineral figure

The replay reports "surface minerals 27%", which reads much worse than the
model is. That figure demands **all three minerals match at once**, which
compounds a per-mineral error cubically. Per reading, over 1314 readings across
438 planet-years:

| result | readings |
|--------|----------|
| exact | 804 (61%) |
| within one kilotonne | 1097 (83%) |
| off by 2 to 5 | ~110 |
| off by 6 or more | 107 (8%) |

The error distribution is dominated by 0, -1 and +1 — 804, 212 and 81 readings.
That single kilotonne is the **mining remainder roll**: `EstMineralsMined`
resolves the leftover hundredths with `Random(100)`, and the replay starts the
RNG fresh rather than in the state the original had reached by the time it
mined. The rolls are therefore independent of the original's, and disagree
about a third of the time by exactly the amount one roll is worth.

So the surface-mineral figure is mostly **RNG misalignment, not missing
consumption**. Aligning the stream would take per-mineral agreement to
something near 83% and the all-three figure to roughly 57%.

The residual that is not rounding is the 8% off by six or more. That is where a
genuinely unmodelled effect lives, and it is a much smaller target than 73%.

This also ties two open items together: torpedo combat resolution is blocked on
the same thing, "needs the RNG in the right state". Recovering the RNG's
position through a turn would move both.
