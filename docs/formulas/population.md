# Subsystem: Population Growth & Death

- **Status:** verified
- **Ghidra routine(s):** `1038:7082` `ChgPopFromPlanet`, `10e0:65d4` `PctTrueMaxGrowth`, `10b8:50a0` `UpdatePopulations`
- **Manual reference:** `MANUAL.PDF` pp. 6-3..6-4 ("Growth Rate", "Overcrowding")
- **Uses RNG:** no
- **Implemented in:** `crates/stars-core/src/population.rs`

## Inputs

| Name | Type | Range / units | Source |
|------|------|---------------|--------|
| `planet.rgwtMin[3]` | `int32_t` | population, in units of 100 colonists | `PLANET` +0x28 |
| `planet.rgbImp[0]` | `uint8_t` | fractional-population accumulator, `0..99` | `PLANET` +0x14 (`iDeltaPop:8`) |
| habitability | `int16_t` | percent | `habitability.md` |
| maximum population | `int32_t` | units of 100 colonists | `habitability.md` |
| `race.pctIdealGrowth` | `char` | max growth rate, `1..=20` percent | `PLAYER` +0x19 |

## Outputs

| Name | Type | Range / units |
|------|------|---------------|
| population change | `int32_t` | units of 100 colonists, signed |
| new accumulator | `uint8_t` | `0..99` |

## The accumulator

Growth is computed at 1/100th of the stored population unit and the leftover is
banked in `iDeltaPop`, so a colony growing by less than one stored unit a year
still grows. This is why the accumulator is stored in the save file, and why a
prediction that ignores it drifts. It is also what makes differential testing
sharp: the accumulator is touched *only* by this formula, so matching it is
strong evidence the arithmetic is right.

## Formula

```
if planet.iPlayer == -1 or pop == 0: return 0

hab = PctPlanetDesirability(planet, owner)

if hab < 0:
    # --- hostile planet: colonists die ---
    deaths100 = max(pop * (-hab) / 10, 1)
    whole = deaths100 / 100
    frac  = deaths100 % 100
    if whole == 0 and frac == 0: frac = 1
    accum -= frac
    if accum < 0: whole += 1; accum += 100
    return -whole

# --- growth ---
maxPop   = CalcPlanetMaxPop(planet, owner)
pctGrow  = PctTrueMaxGrowth(owner) * hab       # hundredths of a percent

if pop > maxPop / 4:                            # growth plateaus past 25%
    pctFull = pop * 1000 / maxPop               # permille of capacity

    if pop >= maxPop:
        if pop < maxPop + 10: return 0          # dead band at capacity
        pctRetard = max(99 - pctFull / 10, -300)
        pctGrow   = pctRetard << 2
    else:
        retard  = 1000 - pctFull
        if pctGrow < 1000:
            pctGrow = pctGrow * retard^2 / 562500
        else:
            pctGrow = ((pctGrow / 10) * retard^2 / 562500) * 10

coarse   = pop * (pctGrow / 100)
growth100 = (pop * pctGrow / 100) if coarse < 10000000 else coarse

whole = growth100 / 100
frac  = growth100 % 100
if whole == 0 and frac == 0: frac = 1
accum += frac
if accum >= 100: whole += 1; accum -= 100
elif accum < 0:  whole -= 1; accum += 100
return whole
```

`PctTrueMaxGrowth` is simply `race.pctIdealGrowth`, doubled for Hyper
Expansion.

### Why 562500

The crowding factor is `((1000 - pctFull) / 750)^2`, written with the division
folded into one constant: `750^2 = 562500`. At 25% of capacity `pctFull = 250`,
the numerator is `750^2` and the factor is exactly 1 — which is precisely the
manual's "population growth begins to plateau after the planet reaches 25%
capacity". At capacity it is 0.

The two branches around `pctGrow < 1000` are the original avoiding 32-bit
overflow by dividing before multiplying; they are not different rules, and the
`/10 … *10` costs a little precision at high growth rates.

### Why the clamp is -300

Over capacity the loss rate is `(99 - pctFull/10) * 4` hundredths of a percent.
At 400% of capacity `pctFull = 4000`, so `99 - 400 = -301`, clamped to `-300`,
giving `-1200` hundredths — exactly the **12% annual maximum at 400% capacity**
the manual states (p. 6-3). That the clamp constant and the manual's figure
agree to the digit is the strongest single confirmation in this spec.

## Edge cases & clamps

- **A planet with nobody left is given up.** `UpdatePopulations`
  (`10b8:50a0`), after the year's change: a planet with an owner and a
  count of zero sends `idmColonistsHaveDiedOffLongerControlPlanet` (`0x23`)
  when the change was negative and `…HaveJumpedShip…` (`0x40`) when there
  was nobody to begin with — one more for an Alternate Reality race — and
  then `UninhabitPlanet` clears it: owner, queue, defences, scanner,
  starbase, routes, the lot. The tutorial's Wallaby, whose people die off
  faster than the freighters can bring them, is the worked case; the
  engine does the same in `generate_turn` after `update_population`.
- Colonists put down by a **freighter** on a planet that is not the
  fleet's owner's — an empty one, or another player's — are a landing,
  settled with the year's colony drops (`resolve_colonist_drops`), not an
  addition to the count; a Transport's Unload All at a planet that has
  died is how the tutorial's Wallaby is settled again.
- A planet within 10 units of capacity neither grows nor shrinks, and the
  accumulator is left untouched.
- A death or growth computation that rounds to zero is forced to a minimum of
  one hundredth, so a hostile planet always loses *something*.
- The manual's example "a planet with a value of -9% will kill 0.9% of the
  colonists each year" is exactly `pop * 9 / 10 / 100`.

## Turn ordering

`FGenerateTurn` (`10b0:0000`) runs the year in this order:

```
DoOrders(0) → UnmarkMineFields → MoveThings(0) → MoveFleets →
ThingDecay → BreedColonistsInTransit → Produce → MoveThings(1) →
FuelFleets → DoOrders(1) → SweepForMines → HealShips →
AutoTerraform → RemoteTerraforming → SpankTheCheaters →
ValidateWaypoints → UpdateGuesses → turn++ → UpdatePlayerScores
```

Population is updated inside `Produce`. Empirically, **terraforming applies
before the growth it affects**: predicting the Exodus fixture with each
planet's pre-terraform environment matches 344 of 426 planet-years, while using
the environment recorded at the end of the step matches 372. Colonists are also
loaded and unloaded by freighters in both `DoOrders(0)` and `DoOrders(1)`, i.e.
both before and after growth.

## Worked example (becomes a test vector)

A 1,000,000-colonist planet (`pop = 10000`) at habitability `-9`:

- `deaths100 = 10000 * 9 / 10 = 9000`
- `whole = 90`, `frac = 0`
- change = `-90` units = 9,000 colonists = 0.9% of the population

Captured at: `../vectors/planetary-economy.json` (`population_change`).

## Verification

`crates/stars-core/tests/differential_growth.rs` replays the 40-turn Exodus
game and the three-player sample game: 372 of 426 planet-years reproduce both
the population and the accumulator exactly, another 15 reproduce the
accumulator (population moved by freighter after growth), and 4 more are
explained by a starting population reachable by loading colonists before
growth. The residual 35 are concentrated on a handful of heavily-trafficked
planets whose colonists move both before and after the growth step.

## Open questions

- The Alternate Reality growth path is not implemented (see `habitability.md`).
- The remaining 35 unexplained planet-years should resolve once order
  processing lands in Step 4; they are worth re-checking then.
