# Subsystem: Habitability & Maximum Population

- **Status:** verified
- **Ghidra routine(s):** `1048:6e1e` `PctPlanetDesirability`, `1048:7096` `CalcPlanetMaxPop`
- **Manual reference:** `MANUAL.PDF` pp. 6-2..6-3 ("Population", "Maximum Population")
- **Uses RNG:** no
- **Implemented in:** `crates/stars-core/src/hab.rs`

Habitability ("Value" in the UI) is the number every other planetary formula is
built on: it sets maximum population, scales the growth rate, and through
maximum population it caps operable mines and factories.

## Inputs

| Name | Type | Range / units | Source |
|------|------|---------------|--------|
| `planet.rgEnvVar[3]` | `char` | gravity/temperature/radiation clicks, `0..=100` | `PLANET` +0x0C |
| `race.rgEnvVar[3]` | `char` | the race's ideal for each variable | `PLAYER` +0x10 |
| `race.rgEnvVarMin[3]` | `char` | lower habitable bound | `PLAYER` +0x13 |
| `race.rgEnvVarMax[3]` | `char` | upper habitable bound; `< 0` = immune | `PLAYER` +0x16 |
| `race.rgAttr[rsMajorAdv]` | `char` | primary racial trait | `PLAYER` +0x3E |
| `race.grbitAttr` bit 9 | flag | Only Basic Remote Mining | `PLAYER` +0x4E |

## Outputs

| Name | Type | Range / units |
|------|------|---------------|
| habitability | `int16_t` | `-45..=100` percent; negative is hostile |
| maximum population | `int32_t` | units of 100 colonists (`10000` = 1,000,000) |

## Formula

Each of the three environment variables is scored independently, then combined.

```
pctPos = 0        # sum of squared percent-ideal, 0..30000
pctNeg = 0        # sum of out-of-range distances
pctMod = 10000    # scaling factor in 1/10000ths

for i in 0..3:
    value  = planet.rgEnvVar[i]
    center = race.rgEnvVar[i]
    min    = race.rgEnvVarMin[i]
    max    = race.rgEnvVarMax[i]

    if max < 0:                       # immune to this variable
        pctPos += 10000
        continue

    if value < min or value > max:    # hostile
        delta = (min - value) if value < min else (value - max)
        pctNeg += min(delta, 15)
        continue

    absdiff = abs(value - center)
    if value < center:
        d        = center - min             # half-width on the low side
        dPenalty = (center - value) * 2 - d
    else:
        d        = max - center             # half-width on the high side
        dPenalty = (value - center) * 2 - d

    pctIdeal = 100 - (absdiff * 100) / d    # integer division, truncating
    pctPos  += pctIdeal * pctIdeal

    if dPenalty > 0:
        pctMod = pctMod * (2*d - dPenalty) / (2*d)

if pctNeg != 0:
    return -pctNeg

return (int)(sqrt(pctPos / 3.0) + 0.9) * pctMod / 10000
```

The two floating-point constants are read straight out of the data segment:
`3.0` at `DS:0x1d2e` and `0.9` at `DS:0x1d36` (verified by reading those eight
bytes from our binary: `0000000000000840` and `cdccccccccccec3f`). The tail of
the routine at `1048:702a..7080` is
`FILD; FLD [1d2e]; FDIVP; sqrt; FLD [1d36]; FADDP; ftol`, so the bias is added
**after** the square root and the result is then truncated, not rounded.

Note that `d` is the half-width on the side the planet actually sits, so a
lopsided habitable range is measured against the nearer edge; and that `pctMod`
compounds across variables, because each off-centre variable multiplies it
again.

Maximum population is then:

```
maxPop = 500 if hab < 5 else hab * 100
if prt == HE:      maxPop -= maxPop / 2      # half
elif prt == JOAT:  maxPop += maxPop / 5      # +20%
if OBRM:           maxPop += maxPop / 10     # +10%, applied after the PRT
```

Alternate Reality races are the exception: their maximum population comes from
`rglPopMac[]` (`1120:08cc`) indexed by their starbase hull less `0x20`, so it
depends on the starbase rather than on the planet at all — and the planet
must be the player's own with a starbase up (`det` bit 9), else nobody:

| hull | `rglPopMac` | colonists |
|------|------------:|----------:|
| Orbital Fort | 2,500 | 250,000 |
| Space Dock | 5,000 | 500,000 |
| Space Station | 10,000 | 1,000,000 |
| Ultra Station | 20,000 | 2,000,000 |
| Death Star | 30,000 | 3,000,000 |

`MANUAL.PDF` p. 22-1 gives the two ends, 250,000 and 3,000,000. The OBRM
tenth is added on top as for everyone else; the PRT adjustments are not
reached. Implemented as `hab::calc_planet_max_pop_with_starbase`, which the
turn and the frontend call with the hull from the owner's designs; the
plain `calc_planet_max_pop` still answers `None` for AR, not knowing it.

## Edge cases & clamps

- A negative **upper** bound is the immunity marker (stored `0xFF`); the race
  scores a perfect 10000 on that variable and takes no off-centre penalty.
- The out-of-range penalty saturates at 15 clicks per variable, so the worst
  possible habitability is `-45`.
- Every positive habitability below 5% is treated as 5% for maximum population,
  giving the 500-unit (50,000 colonist) floor.
- `d == 0` would divide by zero. It is unreachable for a race the wizard can
  build (a zero-width half-range on the side the planet sits requires the
  planet to be simultaneously inside and outside the range), and our
  implementation treats it as ideal rather than trapping.

## Worked example (becomes a test vector)

Given a Jack-of-All-Trades race with Only Basic Remote Mining on a planet whose
gravity, temperature and radiation are all exactly the race's ideal:

- every variable scores `100^2 = 10000`, so `pctPos = 30000`, `pctMod = 10000`
- `sqrt(30000/3) + 0.9 = 100.9`, truncated to `100`
- `maxPop = 100 * 100 = 10000`, `+20%` = `12000`, `+10%` = `13200`

That is the manual's stated 1,320,000 colonists (`MANUAL.PDF` p. 6-3).

Captured at: `../vectors/planetary-economy.json` (`max_population`).

## Open questions

- The Alternate Reality maximum is unit-tested against the table
  (`golden_vectors::alternate_reality_lives_on_its_starbase`) but not
  differentially checked: no fixture game has an AR player.
