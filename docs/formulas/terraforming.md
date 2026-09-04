# Subsystem: Terraforming

- **Status:** reach verified (96%); step count transcribed but over-counts
- **Ghidra routine(s):** `FCanTerraformLppl` (read via `PctPlanetOptValue`
  `1048:6b88`), the `hstTerra` part table
- **Manual reference:** `MANUAL.PDF` pp. 6-14..6-15
- **Uses RNG:** no
- **Implemented in:** `crates/stars-core/src/terraform.rs`

Terraforming moves a planet's gravity, temperature and radiation toward the
race's ideal. Two rules carry the whole model.

## 1. Reach is measured from the *original* environment

`FCanTerraformLppl` computes the band as `rgEnvVarOrig ± reach`, clamped to
`1..=99` — **not** from the planet's current values. Terraforming ten clicks
and then researching a wider module does not let you go ten further. This is
why the planet record keeps `rgEnvVarOrig` beside the current environment, and
why `stars-core`'s `Planet` now carries `env_orig`.

## 2. Reach is the best single module that applies

The `hstTerra` part table is laid out exactly as the binary indexes it: eight
Total Terraform modules (items 0-7), then four each of Gravity (8-11),
Temperature (12-15) and Radiation (16-19). For each variable the reach is the
widest Total Terraform the player can build, or the widest variable-specific
module, whichever goes further. A race immune to a variable never terraforms
it.

```
reach[v] = max(widest buildable Total Terraform,
               widest buildable module for v)          # 0 if immune to v
band[v]  = clamp(envOrig[v] - reach[v], 1, 99) .. clamp(envOrig[v] + reach[v], 1, 99)
optimal[v] = env[v] moved toward the race ideal, stopping at the ideal
             or the edge of band[v], whichever comes first
```

`optimal` is what `PctPlanetOptValue` measures habitability against, which is
the value the AI's colonisation gate uses — see `ai.md`.

## Steps available

`MANUAL.PDF` p. 6-14 gives the rule outright: "If you can improve Gravity by
3%, Temperature by 5% and Radiation by 2% the Production dialog will let you
add 10% Terraforming to the queue. Each 1% Terraforming task executed will
modify one of the environmental factors by 1%." So the count is the sum, over
the three variables, of the improvement still available.

## Measured

Against `fixtures/games/all-computer-players`:

| check | result |
|-------|--------|
| environment moved no further than `reach` allows | 9760 of 10,165 planet-turns (96%) |
| AI auto-terraform order equals `min(steps, 4)` | 207 of 865 (23%) |

The reach model holds. The 405 planet-turns that exceed it are worth chasing:
Claim Adjuster races terraform from orbit, and a planet that changed hands
carries terraforming done by a previous owner with different technology, so
both would show as overshoot against the *current* owner's reach.

The step count does not hold, and fails in one direction only — it never
predicts fewer steps than the game used, and the shortfall is 1, 2 or 3.
Following a single planet across turns, the recorded count drops by exactly one
each time a variable moves one click, so the game is tracking the same
quantity, but from a band one click narrower than the one computed here. On
planet 86 at 2424, with environment `[45, 73, 4]`, original `[45, 73, 3]`,
ideal `[35, 0, 50]` and reach `[5, 0, 5]`, the radiation band computes as
`1..=8` and the recorded counts behave as though it were `1..=7`.

Whether that is an off-by-one in the band, a resource limit `InitProduction`
has already applied when it builds the production catalogue, or something else
is unresolved. Because of that the step count is **not** wired into the AI's
terraform decision, which still takes the count from its caller.

## Open questions

- The one-click discrepancy above.
- The direction-selection arm of `FCanTerraformLppl`, which picks which way to
  terraform for the UI's environment graph. It is not needed for the value or
  the step count, and the decompilation of that branch is not yet trustworthy.
- Claim Adjuster orbital terraforming (`MANUAL.PDF` p. 19-3), which is a
  separate mechanism.
