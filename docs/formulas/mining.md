# Subsystem: Mining & Mineral Concentration Decay

- **Status:** verified
- **Ghidra routine(s):** `1028:5362` `EstMineralsMined`, `10b8:55a4` `MineMinerals`, `1048:73fc` `CMinesOperating`
- **Manual reference:** `MANUAL.PDF` pp. 13-2..13-3, p. 6-5
- **Uses RNG:** **yes** — the fractional kiloton is resolved by `Random(100)` (see `../rng/prng.md`)
- **Implemented in:** `crates/stars-core/src/mining.rs`

## The operating mine count, verified without the RNG

`CMinesOperating` is `min(planet.cMines, CMaxOperableMines(planet, iplr, 0))`,
or `sqrt(population)` for an Alternate Reality race. The third argument is a
literal zero (`MOV AX,0x0; PUSH AX` at `1048:7485`, before the call at
`1048:7496`), so the cap is measured against the population the planet has
**now**, not after this year's growth — unlike the AI's own use of
`CMaxOperableMines`, which shares that flag but is a different call site.

Verifying the count is harder than it looks, because the headline mining figure
allows a kilotonne either way and that is almost exactly the width an off-by-one
mine count moves the result. Two measurements avoid the roll entirely:

| check | result |
|-------|--------|
| exact where every remainder is zero, so no roll happens | **4,606 of 4,629 (99.5%)** |
| our count among those consistent with a quiet planet's three gains | 98% |
| ...where the gains pin down a single count | 99% of 1,020 |

The first is asserted by
`economy_against_ai_game::the_operating_mine_count_is_exact_where_no_roll_is_involved`;
the others come from `examples/mines_check`, which inverts the observation and
asks which mine counts could have produced the gains.


## Measured accuracy

Scored in isolation against `fixtures/games/all-computer-players` — 101 turns,
sixteen players — by looking only at planet-year pairs where the planet built
nothing and had an empty queue, so the change in surface minerals is exactly
what was mined. 5715 such pairs give 17,145 mineral readings.

| result | readings |
|--------|----------|
| exactly the truncated estimate | 11,898 |
| one above it (the RNG rolled the remainder) | 2,083 |
| off by 2 to 4 | 211 |
| off by 5 or more | 2,953 |

The first two are correct: the original rolls the leftover hundredths through
the RNG, so either value is right. The large-error bucket is the other things
that move surface minerals and are invisible here — cargo transfers, mineral
packets, mineral alchemy — not mining. Of the 14,192 readings this test can
attribute to mining, 13,981 (98.5%) agree.

This was measured while looking for the source of the AI's production
inaccuracy, on the assumption that mining was a likely culprit. It is not.


## Inputs

| Name | Type | Range / units | Source |
|------|------|---------------|--------|
| operating mines | `int32_t` | count | `CMinesOperating`, see `resources.md` |
| `planet.rgMinConc[3]` | `uint8_t` | concentration, `1..=100+` | `PLANET` +0x09 |
| `planet.rgpctMinLevel[3]` | `uint8_t` | decay sub-level in 1/256ths; `0` means full (256) | `PLANET` +0x06 |
| `race.rgAttr[rsMineProd]` | `char` | kT per 10 mines at concentration 100 | `PLAYER` +0x3E+4 |

## Outputs

| Name | Type | Range / units |
|------|------|---------------|
| minerals mined | `int32_t[3]` | kT, added to `planet.rgwtMin[0..3]` |
| new concentration | `uint8_t[3]` | decremented as the deposit depletes |

## Formula

```
raw = mines * concentration
if remote:  quantity100 = raw                       # robot miners are always 10
else:       quantity100 = raw * rsMineProd / 10

kt        = quantity100 / 100
remainder = quantity100 % 100
if remainder != 0 and generating_turn:
    if Random(100) < remainder: kt += 1             # probabilistic rounding

planet.rgwtMin[i] += kt
```

The manual's worked example (p. 13-2) is exactly this: "10 mines produce up to
10 kT per year" (`rsMineProd = 10`) and "Germanium concentration 50" means
"100 mines will produce 50 kT" — `100 * 50 * 10 / 10 / 100 = 50`.

The probabilistic rounding is the **only** RNG the planetary economy consumes.
It is applied per mineral per planet per year, in planet order, so reproducing
a turn exactly requires drawing in the same order as the original.

## Concentration decay

Depletion is driven by *mine-years*, not by how much was actually extracted, so
an inefficient miner depletes a deposit exactly as fast as an efficient one
(the manual makes this point explicitly on p. 13-3).

```
decay = mines * concentration / 100        # mine-years, note: not scaled by rsMineProd

while decay >= 1 and rgMinConc[i] >= 2:
    level = rgpctMinLevel[i] or 256        # stored 0 means a full 256/256ths
    conc  = clamp_for_decay(rgMinConc[i])
    threshold = 12500 * level / 256 / conc

    if decay < threshold:                  # bank what is left of the point
        perPoint  = 12500 / conc
        newLevel  = max((threshold - decay) * 256 / perPoint, 1)   # 1028:5840
        if newLevel >= level: newLevel = level - 1
        rgpctMinLevel[i] = newLevel
        if newLevel == 0: rgMinConc[i] -= 1
        break

    decay -= threshold
    rgMinConc[i] -= 1
    rgpctMinLevel[i] = 0
```

where `clamp_for_decay` flattens the curve at the extremes:

| concentration | used for decay |
|---------------|----------------|
| `>= 101` | 100 |
| `25..=100` | unchanged |
| `5..=24` | 25 |
| `< 5` | 10 |

The banked level is the **remainder** of the point, not the progress
through it: `rgpctMinLevel` counts down. (This project had it as the
progress until the tutorial's Stove Top lost a point of every
concentration every year, where `tutorial.m1` shows none lost in three.)

`12500 / concentration` is precisely the manual's "to calculate approximately
how many Mine years must pass to reduce a mineral's concentration by one,
divide 12,500 by the current mineral concentration" (p. 13-2). The
`rgpctMinLevel` byte is the sub-point accumulator that makes that arithmetic
exact across years.

## Edge cases & clamps

- Concentration never falls below 1 (the loop stops at `>= 2`).
- A **home world** mines as if its concentration were at least 30, however
  depleted it is (`MANUAL.PDF` p. 6-5). The floor applies to local mining and
  to Alternate Reality orbital mining, but not to another player's robot
  miners.
- Alternate Reality planets operate `floor(sqrt(population))` mines and always
  mine at efficiency 10; their fleets' robot miners are added on top.
- An unowned or empty planet mines nothing and reports `-1` per mineral in the
  original's estimate path.

## Worked example (becomes a test vector)

100 mines, concentration 50, `rsMineProd = 10`:

- `raw = 100 * 50 = 5000`; `quantity100 = 5000 * 10 / 10 = 5000`
- `kt = 50`, remainder `0` — no RNG draw needed
- decay: `100 * 50 / 100 = 50` mine-years against a threshold of
  `12500 * 256/256 / 50 = 250`, so a quarter of the way to losing a point:
  `rgpctMinLevel` becomes `50 * 256 / 250 = 51`.

Captured at: `../vectors/planetary-economy.json` (`mining`).

## Open questions

- ~~The remote-mining path is specified here but not yet exercised.~~ The
  Remote Mining task runs it (`waypoint-tasks.md`); no fleet in the corpus
  carries a mining robot, so it rests on the transcription.


## Remote mining

A fleet orbiting a planet with the Remote Mining waypoint task (task 3) mines
it from orbit. Two pieces:

**How many mines the fleet is worth** — `CMineFromLpfl` (`1080:2600`),
implemented as `remote_mines`. Each design contributes the sum of its mining
slots, the number fitted times that part's rating, multiplied by the ships of
that design present. A Robo-Midget Miner rates 5, so a ship carrying two mines
as ten planetary mines would. **The total is capped at 4000** once it passes
3999.

**What those mines extract** — [`minerals_mined`] with an explicit count, which
this crate already had. Remote miners always work at efficiency 10 whatever the
race's mining skill, and they do not get the homeworld concentration floor,
both of which fall out of the `fRemote` flag through `EstMineralsMined`.

### Alternate Reality mines its own planets this way

`EstMineralsMined` has a second path, taken only when the planet's owner is an
Alternate Reality race and the call is not already a remote one. It walks the
fleet list for that player's fleets orbiting the planet with a mining order and
adds each one's `CMineFromLpfl` contribution. An AR race has no planetary mines
at all — `CMaxOperableMines` returns 0 for it — so this is how it mines
anything.

### Not verified

The waypoint task that triggers it reads 0 on all 50,173 waypoints in the
fixtures, because a task is consumed when it executes (see
`../formats/cargo.md` for the same effect on transport orders). Nothing in the
corpus records a fleet with a live mining order, so the mine count is
transcribed and unit-tested but not scored against a recording.
