# Subsystem: Mining & Mineral Concentration Decay

- **Status:** verified
- **Ghidra routine(s):** `1028:5362` `EstMineralsMined`, `10b8:55a4` `MineMinerals`, `1048:73fc` `CMinesOperating`
- **Manual reference:** `MANUAL.PDF` pp. 13-2..13-3, p. 6-5
- **Uses RNG:** **yes** — the fractional kiloton is resolved by `Random(100)` (see `../rng/prng.md`)
- **Implemented in:** `crates/stars-core/src/mining.rs`

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

    if decay < threshold:                  # bank partial progress
        perPoint  = 12500 / conc
        newLevel  = max(decay * 256 / perPoint, 1)
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

- The remote-mining path is specified here but not yet exercised: it needs
  fleets with robot mining modules (Step 4).
