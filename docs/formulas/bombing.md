# Subsystem: Bombing

- **Status:** in progress — the gates, defence coverage and damage application
  are recovered and implemented; the bomb totals are transcribed with one step
  inferred, and none of it is verified against a recording
- **Ghidra routine(s):** `DoBombing` (`battle.c`), `CalcPctSurvive`
  (`util.c`), `FCalcFleetBombDamage` (`1038:145c`), `CMaxDefenses`,
  `CMaxOperableDefenses` (`1048:77ae`)
- **Manual reference:** `MANUAL.PDF` — not consulted; the file is image-only in
  this repository and no text could be extracted from it
- **Uses RNG:** yes — the building split and the population remainder
- **Implemented in:** `crates/stars-core/src/bombing.rs`

Bombing is separate from ship combat: it runs after the battle, over every
fleet sitting at a planet.

## When a fleet may bomb

```c
if (!lpfl->fDead && lpfl->idPlanet != -1 && !lpfl->fBombed)
    if (lppl->iPlayer != lpfl->iPlayer && lppl->iPlayer != -1)
        if (FAttackPlayer(lpfl, lppl->iPlayer) && !lppl->fStarbase)
```

The planet must be **owned by someone else and inhabited**, the bomber must be
at war with them, and — the rule that is easy to miss — the planet must have
**no starbase**. A starbase stops bombing outright, however small it is.

## Defence coverage

`CalcPctSurvive` decides how much of a run gets through:

```c
if (planet unowned || cDefenses == 0) { *ppct = 1.0; return; }
cDefenses = min(cDefenses, CMaxOperableDefenses(lppl, owner, false));
pct      = pow(1 - dDmgCol / 1000.0, cDefenses);
pctSmart = pow(1 - dDmgCol / 2000.0, cDefenses);
```

`dDmgCol` is the coverage rating of the best defence the **owner** can build —
SDI 10, Missile Battery 20, Laser Battery 24, Planetary Shield 30, Neutron
Shield 38. Each defence multiplies the survival, so coverage approaches but
never reaches complete. **Smart bombs are stopped at half the rate**, which is
what keeps them useful against a defended world.

Defences are counted only up to what the planet can staff, and two caps recovered
alongside it:

| | rule |
|-|------|
| `CMaxDefenses` | `clamp(habitability * 4, 10, 100)`, and 0 for Alternate Reality |
| `CMaxOperableDefenses` | `min(CMaxDefenses, (pop + 24) / 25)`, itself capped at 1000 |

## What the bombs add up to

`FCalcFleetBombDamage` walks **every fleet at the planet**, not just one, and
sorts each bomb into one of three buckets. It is a stub in the reconstructed
sources, and Ghidra's decompilation shifts every parameter by one because the
leading far `FLEET *` occupies two slots; reading it takes that into account.

- **Retro Bomb** (`iItem & 0xFF == 9`, tested before anything else) counts
  toward reversing terraforming and kills nobody.
- **Smart bombs** — those that do no building damage: the Smart, Neutron,
  Enriched Neutron, Peerless and Annihilator — **do not sum**. The routine keeps
  a running product of `(1 - damage/1000)`, one factor per bomb. Four Smart
  Bombs at 1.3% each come to 5.0995%, not 5.2%.
- **Everything else** sums outright into the people and building totals, and
  the five basic bombs (indices below 5) each add **3** to a floor on the kill.

### The one inferred step

The product is converted back to a tenths-of-a-percent figure at the end, and
that conversion is the single step the decompilation hides — the value is left
on the FPU stack, so no expression is visible. `bomb_load` uses
`(1 - product) * 1000`, clamped to 1000, which is the natural reading of a
survival product and matches the units everything else uses. **It is a
reading, not a transcription**, and is marked as such in the code.

Two further contributors were read but are **not** implemented, because what
they are could not be established: a beam-slot item and an Alternate Reality
mechanical special each add fixed amounts to the people and floor totals.
Neither appears in this repository's fixtures.

## Applying it

Whatever the defences stop, stops: each bucket is scaled by the survival
fraction with a half added before truncating.

**Buildings** are shared over the three installation kinds in proportion to how
many of each the planet has, with the remainder resolved by a roll, and the
mines taking whatever is left after factories and defences:

```c
cPPE = cFactories + cMines + cDefenses;
q = count * dmgBombBldg / cPPE;   r = count * dmgBombBldg % cPPE;
if (r > 0 && Random(cPPE) < r) q++;
```

**Population** is killed in two stages. The smart bombs take their share of the
whole population first, capped one short of wiping it out — so a planet is never
emptied by smart bombs alone, however many arrive. The ordinary bombs then take
their share of **what is left**:

```c
cKillPeopleS = min(pop * dmgPeopleSmart / 1000, pop - 1);
popRem       = pop - cKillPeopleS;
cKillPeople  = popRem * dmgBombPeople / 1000;
if (remainder > 0 && Random(1000) <= remainder) cKillPeople++;   /* note <= */
cKillPeople += cKillPeopleS;
if (dmgBombPeople > 0 && cKillPeople < 1) cKillPeople = 1;
if (cKillPeople < dmgBombFloor) cKillPeople = dmgBombFloor;
```

The remainder test is `<=`, not the `<` used for the building split and for
mining — a small asymmetry worth preserving.

## Verification

**None.** The fixtures contain plenty of bombs — 2069 M-70 slots, 1756 Cherry,
507 Smart, 450 Neutron, 166 M-80, 50 Black Cat across all games — so bombing
certainly happened in them. But a bombing run leaves no record of its own: it is
not written to the battle recording, and a planet that lost population between
two turns lost it to bombing, ground combat, growth, cargo transfers or a
change of owner, with nothing in the files to separate them.

So this subsystem is transcribed and unit-tested but unconfirmed, and the
population formula in particular has an untested rounding rule and an inferred
smart-bomb conversion. Separating a bombing loss from the other things that move
population would need a corpus with a known, isolated bombing event.
