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
| AI auto-terraform order equals `min(steps, 4)`, fresh orders | 131 of 187 (70%) |

The reach model holds. The 405 planet-turns that exceed it are worth chasing:
Claim Adjuster races terraform from orbit, and a planet that changed hands
carries terraforming done by a previous owner with different technology, so
both would show as overshoot against the *current* owner's reach.

## The count: what the discrepancy turned out to be

An earlier revision of this document reported the step count matching only 23%
and described it as a one-click error in the band. That diagnosis was wrong.

A terraform entry **stays in the queue and counts down** as production builds
it. Scoring every planet-turn that carries one therefore compares a fresh
decision against the remains of an older one. Restricting to planets that had
no terraform order the turn before — the only planet-turns where a decision is
actually being made — moves the match from 23% to **70%**.

This is exactly the trap the mine and factory decision fell into, and the
general rule is worth stating plainly: **in this game a queue entry is a
running balance, not a record of what was chosen.** Any decision scored against
a queue has to be scored against a *fresh* entry.

The apparent "one click" was an artifact of the same thing. Following planet 86
across turns, the recorded count fell by one each time radiation rose by one —
not because the band was one narrower, but because production was spending the
order down.

### The residual 30%

The 56 fresh orders still wrong are all over-predictions, where this saturates
at the cap of four while the game queued one to three. Two explanations were
tested and rejected:

- **A resource limit.** Capping the count by what the planet can pay for that
  year swings it hard the other way — 7% exact, mostly under-predicting. The AI
  queues terraforming it cannot yet afford, which makes sense for an auto-build
  item that is paid off over several turns.
- **Habitability gain rather than clicks.** Counting the improvement in the
  planet's *value* instead of the number of one-percent steps scores 66%,
  slightly worse than clicks, and introduces under-predictions the click model
  does not have.

The most likely remaining explanation is that `terraform_reach` is too generous
for some players: several residual cases land exactly right with a reach two
smaller. It does not fit all of them, and narrowing the reach to make it fit
would be tuning to the data rather than reading the binary, so it is left open.

## Which factor a step moves

`IBestTerraform` (`1048:5dd2`), called from `FBuildObject` when a terraforming
item is built, decides. For each of the three variables it moves that variable
all the way to its reachable bound, measures how much the planet's habitability
changes, and scores it by the **gain per click**:

```
score[v] = |desirability(v at its bound) - desirability(now)| * 100 / clicks + 1
best     = the highest score, ties to the lowest index
```

The trailing `+ 1` matters: a variable that can move but gains nothing still
scores 1, and so beats one that cannot move at all, which scores 0.

**This is not what the manual says.** `MANUAL.PDF` p. 6-15 describes the task as
always working on "the factor that is the furthest out of range". The code does
not measure distance from the ideal at all — it measures efficiency. A variable
two clicks from a large habitability gain beats one ten clicks from a small
one. Implementing the manual's rule first cost five points of whole-turn
population accuracy, which is what sent this back to the binary.

## Measured

Terraforming is now applied during the turn, and the whole-turn replay no
longer needs the recorded environment fed into it. That allowance was hiding
how much the missing model cost:

| configuration | population |
|---------------|------------|
| recorded environment fed in, no terraforming | 87% |
| self-contained, no terraforming | 82% |
| self-contained, terraforming modelled | **85%** |

The old 87% was an oracle: the test copied each planet's *next* year
environment into the starting state, so habitability and growth were computed
from the answer. Removing that and modelling terraforming instead is worth
three points over not modelling it, and leaves the replay standing on its own.

The two points still separating it from the oracle are this model's own error —
the reach, the step count and the factor choice compounding — and are the
honest measure of what remains.

## Open questions

- The residual 30% above, most likely in `terraform_reach`.
- The direction-selection arm of `FCanTerraformLppl`, which picks which way to
  terraform for the UI's environment graph. It is not needed for the value or
  the step count, and the decompilation of that branch is not yet trustworthy.
- Claim Adjuster orbital terraforming (`MANUAL.PDF` p. 19-3), a separate
  mechanism and a likely source of the reach overshoots.
