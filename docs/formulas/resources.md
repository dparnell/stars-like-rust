# Subsystem: Resources, Mines & Factories

- **Status:** verified
- **Ghidra routine(s):** `1048:788e` `CResourcesAtPlanet`, `1048:7248` `CMaxMines`, `1048:7304` `CMaxOperableMines`, `1048:755c` `CMaxFactories`, `1048:7618` `CMaxOperableFactories`, `1048:74be` `CFactoriesOperating`
- **Manual reference:** `MANUAL.PDF` pp. 6-2..6-3
- **Uses RNG:** no
- **Implemented in:** `crates/stars-core/src/resources.rs`

Resources are the currency of the production queue: they come from colonists
and from factories, and both are capped by how many people the planet holds.

## Formula

```
# --- what the planet could ever operate (set by capacity, not by today's pop)
maxMines     = max(maxPop * rsMineOperate / 100, 10)
maxFactories = max(maxPop * rsFactOperate / 100, 10)

# --- what today's population can actually staff
operableMines     = clamp(pop * rsMineOperate / 100, 1, maxMines)
operableFactories = clamp(pop * rsFactOperate / 100, 1, maxFactories)

minesOperating     = min(planet.cMines, operableMines)
factoriesOperating = min(planet.cFactories, operableFactories)

# --- resources
workingPop = pop
if workingPop > maxPop:                       # overcrowding
    workingPop = maxPop + (workingPop - maxPop) / 2
    workingPop = min(workingPop, 2 * maxPop)

resources  = workingPop / rsResGen
resources += (factoriesOperating * rsFactProd + 9) / 10     # rounds up
resources  = max(resources, 1)
```

## Units

Population is stored in units of 100 colonists, and the race attribute bytes
are scaled to match, which is why the divisors look odd until they are lined
up:

| attribute | stored meaning | stored value (Humanoid) |
|-----------|----------------|--------------------------|
| `rsResGen` | colonists per resource, in thousands | 10 → 1 resource per 1000 colonists |
| `rsFactProd` | resources per 10 factories | 10 → 1 each |
| `rsFactOperate` | factories per 10,000 colonists | 10 |
| `rsMineProd` | kT per 10 mines at concentration 100 | 10 |
| `rsMineOperate` | mines per 10,000 colonists | 10 |

So 1,000,000 colonists (`pop = 10000`) with 1000 factories produce
`10000/10 = 1000` resources from people plus `1000*10/10 = 1000` from
factories: 2000 in total.

## Overcrowding

The manual (p. 6-3) states it as a rule about workers: "the number of people
between 100% and 300% population capacity work at 50% efficiency. Any
population in excess of 300% capacity can perform no useful work whatsoever."

The code expresses the same thing as a cap on effective population:
`maxPop + excess/2`, itself capped at `2 * maxPop`. At exactly 300% of capacity
those coincide — `maxPop + (2*maxPop)/2 = 2*maxPop` — which is why the second
clamp is `2 *` and not `3 *`.

## Edge cases & clamps

- Both "operable" figures have a floor of 1 and both "max" figures a floor of
  10, so a tiny colony can still run a mine.
- The factory contribution rounds **up** (`+9` before dividing by 10); the
  colonist contribution truncates.
- An inhabited planet always produces at least 1 resource.
- Alternate Reality races use an entirely different expression based on the
  starbase and their Energy tech level; it is specified in the binary at
  `1048:788e` but not implemented here, because it needs ship designs.

## Worked example (becomes a test vector)

`pop = 20000` (200% of a 10000 capacity), no factories, `rsResGen = 10`:

- overcrowded: `workingPop = 10000 + (20000-10000)/2 = 15000`
- `resources = 15000 / 10 = 1500`

Captured at: `../vectors/planetary-economy.json` (`resources`).

## Open questions

- The Alternate Reality resource formula, and the `rgTech[iEnergy]` term it
  uses, are unimplemented (Step 5).
