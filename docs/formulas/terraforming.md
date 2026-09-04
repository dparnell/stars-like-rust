# Subsystem: Terraforming

- **Status:** verified — reach 99.9%, step count 100%
- **Ghidra routine(s):** `AutoTerraform` (`10b8:48f6`),
  `FCanTerraformLppl` (`1048:8022`, read via
  `PctPlanetOptValue` `1048:6b88`), `IpctCanTerraformLppl` (`1048:7f56`),
  `InitProduction` (`10d0:015e`), `FQueueAiTerraforming` (`1090:8d28`),
  `FLookupPart` (the `hstTerra` arm), the `hstTerra` part table
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
| AI auto-terraform order equals `min(steps, 4)`, fresh orders | **196 of 196 (100%)** |

The 31 reach stragglers overshoot by one to three clicks. The order count has no
residual — see below for what the earlier 70% figure was measuring.

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

## The count: where it comes from, and the two-stage running balance

The AI's terraform order is not computed by the AI. `FQueueAiTerraforming`
(`1090:8d28`) reads the count straight out of the production catalogue and
clamps it:

```c
if (((uint)(pProdGlob + i)->cItem & 0x3ff) < 5) uVar4 = cItem & 0x3ff;
else                                            uVar4 = 4;
AddItemToQueue(item, uVar4, grobjPlanet, mdAddItem);
```

and `InitProduction` (`10d0:015e`) fills that catalogue entry — item
`0x3000 >> 10 = 0xc` — from `IpctCanTerraformLppl` (`1048:7f56`):

```c
count = 0;
if (FCanTerraformLppl(lppl, low, high, items, 1)) {
  for (v = 0; v < 3; v++) {
    if (low[v]  != -1) count += lppl->rgEnvVar[v] - low[v];
    if (high[v] != -1) count += high[v] - lppl->rgEnvVar[v];
  }
}
```

The fifth argument matters and the decompiler loses it — it merges the fourth
parameter (an output array of part ids) with the flag. The disassembly at
`1048:7f5f` is unambiguous:

```asm
MOV AX,0x1
PUSH AX                  ; arg5 = 1
LEA AX,[BP + -0x16]      ; arg4 = part-id array
...
CALLF 0x1048:8022        ; FCanTerraformLppl
```

With that flag set, `FCanTerraformLppl` keeps only the direction that moves
toward the race's ideal and clamps that bound at the ideal, so only one of the
two terms above is ever non-`-1` per variable. The count is therefore the sum
over the three variables of the improvement still available — exactly the rule
`MANUAL.PDF` p. 6-14 states, and exactly what `terraform_steps` computes.

### The observable, twice over

Two earlier revisions of this document reported this count as wrong — first at
23%, then, after restricting to fresh orders, as a 70% match with an unexplained
30% of over-predictions. Both figures were artifacts of the same thing, applied
at two different time scales.

**A queue entry is a running balance, not a record of what was chosen.** It
counts down as production builds it. That is why scoring every planet-turn that
carries an order compares a fresh decision against the remains of an older one,
and why restricting to planets with no order the turn before is necessary.

It is not sufficient. `Produce` runs **later in the same turn** the AI queued the
item, so even a fresh order has already been drawn down by whatever the planet
built that year before the file was written. The clicks spent are visible as
environment movement, so the decision can be reconstructed:

```
decision = recorded count + |env(Y) - env(Y-1)| summed over the three variables
```

Scored that way, `min(terraform_steps, 4)` is right for **182 of 182** fresh
orders in `all-computer-players` and **14 of 14** in `no-random-events` —
100%, with no residual at all. The three inputs are independent: the prediction
comes from the previous year's environment, reach and race ideal; the recorded
count is read from the file; the clicks built are measured from the environment
delta.

The two explanations tested against the old 30% residual — capping the count by
what the planet can afford that year (7% exact, badly under-predicting) and
counting habitability gain rather than clicks (66%) — were both correctly
rejected, and neither was needed.

## The Claim Adjuster terraforms for free

`AutoTerraform` (`10b8:48f6`) is step 16 of the turn pipeline. It scans every
player for `GetRaceStat(plr, rsMajorAdv) == 3` and returns immediately if none
matches, so it is a Claim Adjuster routine and nothing else. For each planet
such a player owns it does two separate things.

**A permanent drift of the planet's baseline.** One variable, chosen with
`Random(3)`, is nudged a single click toward the race's ideal — applied to
`rgEnvVarOrig`, not to the current environment:

```text
v = Random(3)
skip if the race is immune to v, or orig[v] is already the ideal
skip unless Random(10) == 0
skip unless pop >= 1000, or Random(1000) < pop
orig[v] += 1 toward the ideal
```

Moving the *original* is what makes this the Claim Adjuster's permanent
improvement: it shifts the whole reachable band rather than being overwritten by
the terraforming that follows two lines later. The corpus confirms the target.
Over planet-turns under unbroken ownership, `env_orig` moves **36 times for
Claim Adjusters and 36 times is also how often their `env` moves**, out of 962
planet-turns — the two counts are identical because the drift moves the baseline
and the jump below then re-seats the environment on it. For every other race
`env_orig` moves 22 times in 20,497 planet-turns (0.1%), which is planets
changing hands rather than drift.

**Then the environment jumps to the reachable bound** — not one click, the whole
way:

```c
if (FCanTerraformLppl(planet, low, high, items, 1)) {
  for (v = 0; v < 3; v++)
    if (low[v] == -1) { if (high[v] != -1) env[v] = high[v]; }
    else                                   env[v] = low[v];
}
```

The fifth argument is `PUSH 0x1` at `10b8:4b5f`, the same flag
`IpctCanTerraformLppl` passes, so only the direction moving toward the ideal
survives and each bound is clamped at the ideal. The result is exactly
`optimal_env`.

Implemented as `terraform::auto_terraform` and run by `generate_turn`; it is a
no-op, consuming no RNG draw, for every non-Claim-Adjuster race.

### Why this matters beyond the Claim Adjuster

It is the reason **Rototill queues no terraforming**, which `ai.md` had recorded
as unexplained. The AI's order count comes from the production catalogue, and
`InitProduction` adds a terraform item only when `IpctCanTerraformLppl` exceeds
zero. A Claim Adjuster's planets are already at their optimum every turn, so the
count is zero and no item is offered. Measured: 0 available steps on all 718
Rototill planet-turns past the population gate, despite 633 of them sitting off
the race's ideal — off the ideal, but already as far as the technology reaches.

The drift does **not** push a planet past its band: Claim Adjuster planets
account for 0 reach violations in 2010 non-immune axis-readings.

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

- (resolved) The step count's apparent 30% over-prediction: there was none. See
  "The count" above.
- The direction-selection arm of `FCanTerraformLppl`, which picks which way to
  terraform for the UI's environment graph. It is not needed for the value or
  the step count, and the decompilation of that branch is not yet trustworthy.
- Claim Adjuster orbital terraforming (`MANUAL.PDF` p. 19-3), a separate
  mechanism and a likely source of the reach overshoots.
