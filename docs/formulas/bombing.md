# Subsystem: Bombing

- **Status:** transcribed in full — the gates, defence coverage, the bomb
  totals and the damage application; none of it is verified against a
  recording, a bombing run leaving none
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

### The end of it

The product is turned back into a tenths-of-a-percent figure with `__ftol`
and capped at 1000 (`10381747`'s neighbour: `if (smart > 999) smart = 1000`).
The value is left on the FPU stack, so the expression is not visible in the
decompilation, but the cap fixes the units: `(1 − product) × 1000`.

Two things that are not bombs count too:

- a **Multi Contained Munition** in a beam slot (`hstBeam`, item `0x12`):
  20 to the people, 5 to the buildings and 3 to the floor, per munition;
- an **Orbital Construction Module** (`hstSpecialM`, item 1): 20 to the floor
  per module — how an Alternate Reality race clears a world it means to hang a
  starbase over.

`fMulti` is set when more than one fleet at the planet was walked. Neither
of the two extras appears in this repository's fixtures.

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

## The driver — `DoBombing` (`10f0:aefa`)

`DoBattles` runs it after the fighting (see `combat.md`). Every fleet not
dead and not yet marked `fBombed`, in orbit of a planet owned by somebody
else and inhabited, with **no starbase** there, whose battle plan attacks
that player (`FAttackPlayer`, `10f0:2ac6`: attack-who 1 enemies by
`rgmdRelation == 2`, 2 anyone not a friend, 3 everyone, 4+ the named
player) bombs it. `FCalcFleetBombDamage` adds up every fleet of the same
player at the planet (marking them bombed, `fMulti` when more than one),
`CalcPctSurvive` scales the buckets by what the defences let through, the
installations and people go as *Bombing a planet* above says, Retro Bombs
undo terraforming by `pctTerra` less half of what the defences stopped
(not written), the two players are told (`0x60`/`0x6a` and their
neighbours, two dozen wordings by what was destroyed; the transcription
sends one each), and a planet with nobody left is uninhabited
(`UninhabitPlanet`, `1048:8732`: owner, people, queue, defences, scanner
and starbase gone, mines and factories left standing, a Claim Adjuster's
environment restored). `crate::bombing::do_bombing` is the transcription.
