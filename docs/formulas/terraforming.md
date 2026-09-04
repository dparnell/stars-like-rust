# Subsystem: Terraforming

- **Status:** reach verified (99.9%); step count transcribed but over-counts
- **Ghidra routine(s):** `FCanTerraformLppl` (read via `PctPlanetOptValue`
  `1048:6b88`), `FLookupPart` (the `hstTerra` arm), the `hstTerra` part table
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

"Can build" is not a pure technology test. `FLookupPart` (`hstTerra` arm) gates
the eight Total Terraform modules behind the **Total Terraforming** lesser
racial trait:

```c
else if (HVar1 == hstTerra) {
  if (0x13 < iItem) return 0;
  ppart->pcom = (COMPART *)(iItem * 0x36 + 0x19e2);
  if (idPlayer != -1 && iItem < 8 &&
      GetRaceGrbit(rgplr + idPlayer, ibitRaceTT) == 0)
    return -1;
}
```

This is load-bearing because Total Terraform 3 costs no research at all: without
the gate every race would begin the game able to move all three variables three
clicks. A race without the trait can do nothing until it researches a
variable-specific module, the cheapest of which (Gravity Terraform 3) needs
Propulsion 1 and Biotechnology 1. Note also that `FCanTerraformLppl` searches
each group **downward** (`for (i = 7; i >= 0; i--)`), taking the first module
that passes, so a group contributes nothing at all when none is buildable.

```
reach[v] = max(widest buildable Total Terraform,       # requires the TT trait
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

Across both sixteen-player AI games, `all-computer-players` and
`no-random-events`:

| check | result |
|-------|--------|
| environment moved no further than `reach` allows | **37,712 of 37,743 axis-readings (99.9%)** |
| AI auto-terraform order equals `min(steps, 4)`, fresh orders | 133 of 187 (71%) |

The 31 stragglers overshoot by one to three clicks.

### Why this is scored per axis, and why immune axes are excluded

An earlier revision of this document scored the reach **per planet-turn over
every axis**, reported 96% on the first corpus and 90% once the second was
added, and recorded the gap as unexplained — guessing at "a per-race term the
reach is missing". The reach was not missing a term worth six points. The
**observable was wrong**.

`env - env_orig` is not a record of what a planet's current owner did. It is a
record of what *any* actor ever did to that planet, and two mechanisms write to
it that no reach can account for:

- **A previous owner.** Planet 260 of `all-computer-players` is the clean case.
  Player 7, a Hyper Expansion race immune to all three variables, holds it
  untouched from 2407 and loses it in 2428. Player 14, centred on 50/50/50,
  terraforms it from `[47, 33, 49]` to `[50, 44, 50]` between 2429 and 2441.
  Player 7 retakes it in 2454 and carries that offset for the rest of the game.
- **Hostile action.** Planet 25 loses two clicks of temperature between 2447 and
  2448, and one of radiation between 2457 and 2458 — each in the same year its
  population drops sharply, while an owner immune to all three holds it
  throughout.

Both land overwhelmingly on axes their owner is **immune** to, and the reason is
mechanical: an immune race never terraforms, so it never overwrites the marks a
previous owner or an attacker left. Every violation in both corpora belonged to
a race immune on the violating axis, and the count of violations exceeded the
count of planet-turns held — nearly every axis of nearly every planet those
races held was "violating".

That also explains the corpus split that prompted the investigation:
`no-random-events` has **four** all-immune Hyper Expansion races to
`all-computer-players`' two, so the same inherited marks are spread over a much
smaller pool of owned planets.

Scored where the formula actually governs — an axis the current owner can
terraform — the model is right 99.9% of the time. Three earlier hypotheses were
tested against the old framing and correctly rejected: Claim Adjuster orbital
terraforming (`no-random-events` contains no Claim Adjuster at all), planets
changing hands as an event count (81 conquests against 56 — but the right unit
is planet-*turns*, since a conquered planet keeps the offset for every remaining
year), and the Total Terraforming trait (both games carry it in similar numbers,
6 players against 7). The trait gate was genuinely missing and has been added;
on these fixtures it moves the score by a fraction of a point, because every
race that terraforms at all also has adequate variable-specific modules.

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

The most likely remaining explanation was once that `terraform_reach` is too
generous for some players: several residual cases land exactly right with a
reach two smaller. It does not fit all of them, and narrowing the reach to make
it fit
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

- The residual 30% above. It is **not** in `terraform_reach`, which is now
  measured at 99.9% on the axes it governs; the over-prediction is in the step
  count or in what caps the order.
- The direction-selection arm of `FCanTerraformLppl`, which picks which way to
  terraform for the UI's environment graph. It is not needed for the value or
  the step count, and the decompilation of that branch is not yet trustworthy.
- Claim Adjuster orbital terraforming (`MANUAL.PDF` p. 19-3), a separate
  mechanism and a likely source of the reach overshoots.
