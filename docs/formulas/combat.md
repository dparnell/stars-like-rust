# Subsystem: Combat

- **Status:** in progress — board, movement (schedule, search, scoring, 94% against a 72% chance rate), targeting, the damage estimate and **both** firing loops implemented; the torpedo accuracy formula is transcribed but unverified, and replaying a recorded torpedo battle needs the RNG
- **Ghidra routine(s):** `battle.c` region — `DxyFromSpdRound`, `DzFromBrcBrc`, `CTorpHit`, `ScoreFromGiveAndTakeAndTactic`, `FAttack`, `FDamageTok`, `DxyMoveTokTo`, and the `rgbrcStart` table
- **Manual reference:** `MANUAL.PDF` pp. 23-2..23-10
- **Uses RNG:** **yes** — torpedo hits are rolled individually
- **Implemented in:** `crates/stars-core/src/battle.rs`
- **Recordings decoded by:** `stars-formats::battle`, see `../formats/battle.md`

Battles play out on a 10x10 board over at most 16 rounds. Each round every
token moves a fraction of a square, then weapons fire in initiative order.

Combat is the one subsystem with a **replay corpus**: the game writes a full
recording of every battle into the player's file, so an implementation can be
checked against what the original engine actually did. The 40-turn Exodus
fixture holds 47 of them.

## The board

Squares are stored one to a byte as `y << 4 | x`. Distance is **Chebyshev** —
the larger of the two axis differences — so a diagonal step costs the same as
an orthogonal one (`DzFromBrcBrc`).

## Starting positions

Participants are placed by a table indexed on the number of players:

```
base = players * (players - 1) / 2
square = rgbrcStart[base + side]
```

Two players start at (1,4) and (8,5), opposite sides of the board; three form a
triangle at (4,1), (8,8), (1,8); and so on. Everything a player brings starts
stacked on that one square.

## Movement

A token's speed is stored as a quarter-square index: `0` is half a square per
round, and each step adds a quarter, so `2` is exactly one square and `6` is
two. Fractional speeds are realised by moving different distances in different
rounds:

```
dxy = (speed + 2) / 4
switch speed & 3:
    case 0: dxy += (round & 1) == 0
    case 1: dxy += (round & 3) != 2
    case 3: dxy += (round & 3) == 0
    case 2: (an exact number of squares; no adjustment)
```

This reproduces the manual's "Movement in Squares per Round" table (p. 23-9)
for all nine listed speeds, and over any eight rounds the total is exactly
eight times the nominal speed.

Within a round the original moves everything a square at a time across three
phases — tokens able to make three squares move first, then those able to make
two, then everyone — heaviest first within each phase. That is why a token can
appear more than once per round in a recording.

## Disengaging

A token that leaves the battle is recorded with a destination of `0xFF` rather
than a square. See `../formats/battle.md`.

## Target selection

Each token scores candidate targets and takes the lowest score. What is being
minimised depends on the battle plan's tactic
(`ScoreFromGiveAndTakeAndTactic`):

| Tactic | Score |
|--------|-------|
| Disengage, Minimise damage to self | damage taken |
| Disengage if challenged, Maximise damage | `-damage given` |
| Maximise net damage, Maximise damage ratio | `-damage given * 100 / (damage taken + 1)`, capped at `-1` |

The cap matters: any damage dealt at all must beat dealing none, however much
is taken in return.

## Weapon accuracy

Jammers reduce a torpedo's accuracy; battle computers reduce its
**inaccuracy**, which is a different operation and yields less the more
accurate the torpedo already is. When both are present they cancel one for one
first, and only the remainder is applied (`MANUAL.PDF` p. 23-6):

```
if jam and computer: cancel one against the other
if computer == 0:  accuracy = base * (100 - jam) / 100
else:              accuracy = 100 - (100 - base) * (100 - computer) / 100
accuracy = max(accuracy, 1)
```

A 75% torpedo through a 50% battle computer is `100 - 25 * 50 / 100 = 88%` —
the manual's worked example exactly.

Each torpedo is then rolled separately with `Random(100)`, up to 200 of them;
beyond that the original takes the average instead, which caps the cost of a
huge salvo and removes its variance.

## Fire resolution

A weapon's damage, from `DpFromPtokBrcToBrc`:

```
dp = weapon.dp * launchers
if the attacker has a capacitor:  dp = dp * pctCap / 100
if range > 0:                     dp -= dp * range / 10 / weapon_range
if the target deflects beams:     dp = dp * pctBeamDef / 100
dp = dp * ships_firing
```

The middle line is the range falloff: a beam loses **a tenth of its damage at
maximum range**, scaled linearly in between. A starbase reaches one square
further than a ship carrying the same weapon.

Damage is then applied to a token by `FDamageTok`:

```
pool = shields_per_ship * ships          # shields pool across the stack
strip the pool first; a sapper stops here

# already-damaged ships die first, because they cost less to finish
cost_damaged = armour - damage_already_carried
kill damaged ships while cost_damaged fits
# then undamaged ships at full armour each
kill ships while armour fits

# whatever is left is spread over the survivors as fresh damage
```

Two consequences the manual calls out (p. 23-2) and which the tests check
directly: ten ships of 100 armour and 50 shields **stacked in one token** pool
500 shield points and lose nothing to a 500-damage volley, while the same ships
in ten separate tokens would lose three; and when a stack of ten 150-armour
ships takes 500, exactly three die and the surviving seven each carry under 5%
damage.

Damage in excess of what a token can absorb spills over to other tokens in the
same square, capped by the number of ships firing.

## The firing loop

`FDoCoolBattle` runs each round, and within a round drives `FAttack` once per
token per initiative level:

```
for initiative from highest down to lowest:
    for itok from the LAST token down to 0:        # note the direction
        for each weapon at this initiative:
            pick the best target in range
            fire; spill any overkill onto the next-best target
```

A weapon's firing initiative is **its own plus its hull's base**, capped at 63.

### The scan runs backwards, and it matters

The token loop is `itokScan = vctok - 1; while (itokScan >= 0) … itokScan--`.
Where two tokens fire at the same initiative, the one later in the array shoots
first. In a symmetric duel — two identical scouts, same initiative, both in
range — that single detail decides which one survives. Replaying the recordings
with the loop running forwards gets those battles exactly backwards; running it
the right way fixed five of them at once.

The array order is not arbitrary either: `RandomizeTokOrder` shuffles it at the
start of the battle with the game's PRNG, and a recording stores it
post-shuffle, so replaying from a recording inherits the real order.

### Target selection

Each weapon scores every enemy token in range that matches its primary target
class, falling back to the secondary class if none does, and takes the
**highest** score — value per point of work:

```
value = (design resource cost + boranium cost) * ships, then * 100
if the target deflects beams: value = value * pctBeamDef / 100
score = value * 100 / (armour left + shields left + 1)      # minimum 1
```

A sapper scores against shields alone and is worthless against a token with
none.

### Overkill

Damage beyond what the target can absorb does not vanish: the weapon re-picks a
target and fires again with the remainder, scaled down in proportion to what
got through, until nothing is left in range. That is what makes a big volley
sweep several small tokens.

## Movement AI

`DxyMoveTokTo` decides where a token goes each round:

1. score every square within a search radius of the current one, clamped to the
   board;
2. take the lowest score, preferring the **nearer** square on a tie and
   breaking exact ties with `Random`;
3. if that square is more than one step away, take a single step toward it,
   again choosing among the neighbours by score with `Random` breaking ties.

A disengaging token adds 2 to a square's score for each friendly token already
there and subtracts 1 from staying put, so it spreads out and prefers to run.

Every stage breaks ties randomly, so **a token's movement cannot be reproduced
without the generator in the same state** — the same constraint torpedoes have.

The selection and step-toward logic above are recovered in full and
implemented. The scoring is not.

### The scoring

`ScoreGuessBattleDamage` (`10f0:598c`) is a stub in the reconstructed sources
and its decompilation loses which token is which across the nested damage
estimates, so it was transcribed from the disassembly:

```
dpGivenBest = 0 ; dpTakenTotal = 0
for each active enemy the mover may attack:
    straight = distance(candidate square, enemy square)
    closes   = (enemy moves left >= our moves left) ? 1 : 0
    if closes == 0:
        near = far = straight
    else:
        near = max(straight - 1, 0)
        far  = the furthest of the four corners of the enemy's reachable box,
               clamped to the board, or `straight` if that is greater

    # the enemy will stand wherever suits them, so assume they do
    theirBest = 30000000
    for range in near ..= far:
        given = damage we would deal them at that range
        taken = damage they would deal us at that range
        theirs = score(give = taken, take = given, THEIR tactic)
        if theirs <= theirBest:
            theirBest = theirs ; takenAtBest = taken ; givenAtBest = given

    dpGivenBest   = max(dpGivenBest, givenAtBest)
    dpTakenTotal += takenAtBest

return score(give = dpGivenBest, take = dpTakenTotal, OUR tactic)
```

The **range band** is the crux, and it is what the earlier attempt was missing.
A square is not judged by the exchange as things stand, but by the exchange
after each enemy has moved to whatever range suits *them* — scored with *their*
tactic. That is what separates squares that would otherwise tie.

Two further details from the disassembly: a disengaging mover passes
`fProximity`, so threats that cannot quite reach it still count; and the damage
estimate is capped at what the target could actually absorb.

### The search radius, and the beeline

`DzMoveRangeToConsider` (`10f0:5312`) decides how far a token looks, and it has
two outcomes:

```
reach = weapon reach + our moves left
for each enemy of the class we hunt:
    dz = distance to it, plus one if it can close as fast as we can
    if dz <= reach:
        return (radius = our moves left, no beeline)   # something is engageable
    otherwise remember the nearest one we could actually hurt

return (radius = 1, beeline to that nearest enemy)
```

The second branch matters more than it looks: when nothing is in reach the
token **does not score squares at all**. `DxyMoveTokTo` overrides its chosen
destination with the remembered enemy square and simply steps toward it. That
is why fleets close across an empty board in a straight line instead of
dithering, and it is the single largest reason an earlier version of this
scoring looked flat — most opening moves never go through the scorer.

### The target-class filter

`FIsTargetOfMdTarget` matches a token against the class a battle plan hunts.
Two classes are broader than their names: "bombers and freighters" also matches
plain freighters, and "unarmed ships" matches freighters and fuel transports
too.

It gates only the damage a token **deals**. An enemy of the wrong class still
threatens it, and still counts toward the damage it would take — so a plan set
to hunt freighters does not walk blindly into a battleship.

The class to hunt is chosen once per movement decision, not per square: the
primary class if anything of that class is present (`FDoesPrimaryTargetTypeExist`),
otherwise the secondary.

### How well it does

Measured against the recordings, splitting the two paths a move can take:

| path | result |
|------|--------|
| beeline moves (nothing in reach) | **83 of 83 close on the target — 100%** |
| scored moves | **450 of 478 among our best-rated — 94%**, against a 72% chance rate |

The chance rate is what makes the second number mean anything: a scorer that
rated every square identically would hit 100% on the first figure and 100% on
the second.

For reference, as the pieces landed:

| | hit rate | chance rate | gap |
|-|---------:|------------:|----:|
| shape only, default tactics | 86% | 78% | 8 |
| scoring transcribed, real tactics | 86% | 71% | 15 |
| plus search radius and class filter | 92% | 81% | 11 |
| `DpFromPtokBrcToBrc` re-read from the disassembly | 84% | 72% | 12 |
| the replay kept the board current (see below) | **94%** | 72% | **22** |

That last row is the one to be careful about. Transcribing the damage estimate
faithfully from the disassembly, rather than from the decompilation, **lowered**
the hit rate from 92% to 84% — and lowered the chance rate with it, so the gap
barely moved. An earlier revision of this document reported the 92% row as the
current state; it is not, and the faithful version was kept deliberately
because a number that improves while the transcription gets less faithful is
measuring the wrong thing.

### What has been checked

`DxyMoveTokTo` (`10f0:5f2c`) has been read against the implementation line by
line, and the structure matches in every respect that could be compared:

- the score is a 32-bit quantity where **lower wins**, seeded at 30,000,000;
- the search box is the token's reachable range, clamped to the board;
- squares within one step have their scores cached in a 3×3 grid;
- when the best square is more than one step away the token does **not** jump
  to it — it takes a single step, choosing among the neighbours by their cached
  scores, with the axis-aligned and diagonal cases handled separately;
- ties are broken by reservoir sampling against `Random`;
- a Disengage token adds 2 to a square for each of its own tokens already
  there, and subtracts 1 for staying put.

One difference was found and fixed: the original's crowd count does not check
whether a token still has ships, and this crate was filtering the dead out. It
makes no difference to the measured rate on this corpus, so it is a
faithfulness fix rather than an improvement.

### Where the gap was: the replay, not the model

The residual was 6 beeline moves and 71 scored ones, and an earlier revision of
this document recorded it as unexplained after clearing both `DxyMoveTokTo` and
the damage estimate. It was in neither. **It was in the harness**, which fed the
scorer a battle that had stopped being the one the engine was fighting.

Two things were never updated as a recording was replayed:

- **Casualties.** Only actions carrying a destination were processed; firing
  records were skipped outright, so no token ever lost a ship, shields or
  armour. Every round after the first was scored against a board at full
  strength. The recording carries what is needed — `ships_killed`,
  `shield_damage` and the packed remaining damage — so it is applied as it
  happens. (`shields` is per ship while the recorded figure is the whole pool,
  so it has to go through the pool exactly as `apply_damage` does, recomputing
  the per-ship value against the ship count *before* the casualties.)
- **`moves_left`.** This is what a token has left to spend *in the current
  round*, and the scorer reads it twice: `DzMoveRangeToConsider` sizes the
  search box from the mover's, and `ScoreGuessBattleDamage` compares each
  enemy's against it to decide whether that enemy can close the distance. Read
  once from the token record it stayed at the starting allowance forever. It is
  now reset at the head of every round from `movement_this_round` and spent one
  per step.

The second was the larger error by far, and it was mis-scoring *which branch a
move took*: a stale allowance gives the wrong search radius, so moves fell into
the beeline path that the engine scored, and vice versa. Correcting it moved 29
moves out of the beeline path and into the scored one.

| | beeline | scored | chance | gap |
|-|--------:|-------:|-------:|----:|
| as recorded before | 105/111 (94%) | 379/450 (84%) | 72% | 12 |
| casualties applied | 103/112 (91%) | 395/449 (87%) | 75% | 12 |
| plus `moves_left` maintained | **83/83 (100%)** | **450/478 (94%)** | 72% | **22** |

Applying casualties alone barely moved the gap — the hit rate and the chance
rate rose together — and it was kept because it is what the engine does, not
because of the number. Maintaining `moves_left` is what actually separated the
scorer from chance.

What remains is 28 scored moves and no beeline ones. That residual is genuinely
unexplained, and it is now small enough that the RNG is a plausible share of it:
`DxyMoveTokTo` breaks ties by reservoir sampling, so where several squares tie
on score the engine's choice among them is not reproducible at all.

## Armour and shields

Shields **overlap across a whole token**: twenty scouts with 20 shield points
each present one 400-point pool that must be stripped before any armour is
touched. Beam weapons are stopped by that pool; torpedoes damage shields and
armour together. Shields are full at the start of every battle, and a
Regenerating Shields race recovers 10% of base at the start of each round
(`MANUAL.PDF` p. 23-2).

Beam damage beyond what is needed to destroy a token spills over to other
tokens in the same square, limited by the number of ships firing — which is why
stacking ships in one token is defensively better than spreading them across
several.

## Verification

`crates/stars-core/tests/combat_vectors.rs` checks the movement table, torpedo
accuracy, the starting-square table, Chebyshev distance, beam range falloff and
the damage model against `../vectors/combat.json` and the manual's worked
examples.

`crates/stars-core/tests/battle_replay.rs` replays the 47 Exodus recordings:

- **161 token starting positions** all land on a square the `rgbrcStart` table
  assigns for that battle's player count, and each player's tokens all start
  stacked on one square.
- **746 recorded moves** are each within the movement allowance for that
  token's stored speed and the round it happened in. This checks the movement
  schedule and the speed encoding together, against the original engine.
- 163 firing actions all occur at range 4 or less.
- 31 disengages are recognised rather than read as impossible moves.
- **22 of 31 first beam hits reproduce the recorded damage exactly** — both the
  shield points stripped and the ships destroyed — computed from the attacker's
  design, the recorded range and the target's state.
- **8 of 11 beam-only battles replay to the recorded casualties exactly.** This
  is the strongest check available: the movement is taken from the recording,
  but every shot, target choice and casualty is computed, and each token's
  state is carried forward through the whole battle rather than assumed.

`crates/stars-core/tests/combat_vectors.rs` checks the movement table, torpedo
accuracy, the starting-square table and Chebyshev distance against
`../vectors/combat.json`, whose cases are quoted from the manual.

## Open questions

- 28 scored moves are unexplained and no beeline ones. The RNG is a plausible
  share of that: `DxyMoveTokTo` breaks ties by reservoir sampling.
- Even complete, movement cannot be reproduced exactly without the RNG, because
  every tie-break draws from it.
- Three of the eleven replayable battles still come out inverted. All three are
  symmetric duels where both ships can destroy the other in one volley, so the
  result turns on exactly when in the round each closed to range; firing once
  at the end of the round is an approximation there.
- **The torpedo accuracy formula is unverified.** The firing loop is now
  implemented and the recording marks hits apart from misses, but only 8 of 100
  recorded volleys yield a clean count — 24 shots, which decides nothing. A
  corpus with more torpedo fire would settle it without needing the RNG.
- Gattling weapons, which hit every target in range rather than one, are
  described in `FAttack` but not implemented.
- `FIsTargetOfMdTarget` — the primary/secondary target-class filter — is not
  implemented, so target selection currently considers every enemy in range.
- `grfWeapon` is now mapped: `bitFBeamLow` 0x01, `bitFBeamHigh` 0x02,
  `bitFTorp` 0x04, `bitFMissile` 0x08, `bitFDeflected` 0x80, plus the unnamed
  `0xC0` `FDamageTok` adds to a torpedo record that stopped at the shields.
- The three-phase movement order and the heaviest-first rule within a phase are
  documented here from the manual but not yet implemented or checked.
- Bombing (`DoBombing`) and ground combat are separate from ship battles and
  are not covered.


## Torpedoes

`DpFromPtokBrcToBrc` and `CTorpHit` have both been read from the disassembly in
full, and everything they do is implemented.

### Accuracy

Jammers reduce accuracy; battle computers reduce *inaccuracy*. They cancel one
for one first, and whichever survives is applied:

```
jam -= computer                                  # they cancel, one for one
if computer is left over:  hit = 100 - (100 - base) * (100 - computer) / 100
else:                      hit = base * (100 - jam) / 100
hit = max(hit, 1)
```

### How many hit

Each torpedo is rolled separately with `Random(100)` — but **only up to 200 of
them**. Past that the routine takes the expectation, `count * hit / 100`, and
does not roll at all.

That threshold is load-bearing in an unobvious way. The damage *estimate*
multiplies its torpedo count by 200 before calling `CTorpHit`, purely as fixed
point so that fractional damage survives, and dividing by 200 afterwards. That
scaling always pushes the count past the threshold, so **the estimate never
rolls** — movement scoring is deterministic even though torpedo combat is not.
### Damage, and the volley split

The `hstTorp` arm of the firing loop is now transcribed in full, and two rules
in it were not previously written down.

**A hit does `dp/2` twice.** `FDamageTok(target, &dpT, dpT, ...)` is called with
the same figure in both the shield and the armour argument, where
`dpT = cTorpFire * dp / 2`. So a torpedo that hits puts half its damage into the
shield pool and half into the hull — that is what "damages shields and armour
together" means arithmetically. A torpedo that **misses still splashes the
shields**, for `dp/8`.

**A token does not always empty its tubes.** If the target would die to fewer
torpedoes than the token carries, it fires only as many as it needs and keeps
the rest for its next target:

```c
i = ptokTarget->csh;
if (i >= cTorpBase || cTorpHit * dp <= dpArmorLeft) {
    cTorpFire = cTorpHit;  cTorpMiss = cTorpBase - cTorpHit;    /* fire them all */
} else {
    for (; i <= cTorpBase; i++) {
        cTorpFire = (i * cTorpHit + cTorpBase - 1) / cTorpBase;
        cTorpMiss = i - cTorpFire;
        dpShieldCur = max(dpShieldLeft - cTorpMiss * dp / 8, 0) - cTorpFire * dp / 2;
        dpHitArmor  = cTorpFire * dp / 2;
        if (dpShieldCur < 0) dpHitArmor -= dpShieldCur;
        if (dpHitArmor >= dpArmorLeft) break;
    }
}
```

The hits are rescaled with the committed count, rounding up, so the hit-to-miss
ratio survives the trimming — the roll happened over the whole volley and is not
re-run. Implemented as `battle::torpedo_split`, driven by
`battle::fire_torpedoes`.

Three more rules from the same read:

- **A starbase gets +1 to every weapon's range**, from `grobj == grobjPlanet`.
- **Beam damage falls off with range** by `dp * dz / (10 * nominal_range)`, so a
  beam at its own maximum range does 10% less. Note the estimate applies this
  *before* beam deflection where `FAttack` applies deflection first; each step
  truncates, so the two orders are not interchangeable.
- **A missile doubles its damage** against a target whose shields are already
  down. The four missiles end the torpedo table, so the test is
  `item >= itorpJihadMissile`.

### The recording says which torpedoes missed

`FDamageTok` writes one kill record per call and a volley makes two — the splash
and the strike. It marks them apart through a flag the `GrfWeapon` enum does not
name:

```c
if ((lpbBattleCur[1] & bitFTorp) != 0) lpbBattleCur[1] |= 0xC0u;
```

That sits on the early-out path, taken when the damage never reached armour. So
a torpedo record of `0x04` reached the hull and one of `0xC4` (196) stopped at
the shields. Across the Exodus recordings the weapon flags are exactly
`{1: 73, 4: 140, 196: 108}` — beams, torpedo strikes, torpedo splashes — with no
other value, which is what identified `0xC0`.

It matters because it means the recording carries the **outcome of the roll**:
`dpCol = cTorpMiss * dp / 8` and `dpT = cTorpFire * dp / 2`, so where the shields
absorbed a whole record its `dpShield` divides back out to a count.

### What is left, and why

The firing loop is implemented. What cannot be done is *reproducing* a recorded
torpedo battle, because `CTorpHit` rolls each torpedo with `Random(100)` and
`../rng/prng.md` establishes that the generator's state cannot be recovered from
these files.

Recovering the rolls from the recording instead was tried, and this corpus is
too thin for it. Of 100 recorded volleys only **8** have both counts cleanly
recoverable; the rest carry a record capped by an exhausted shield pool, which
says only "at least this much". Those 8 come to 24 torpedoes with 16 hits — 67%
against the 56% the accuracy formula predicts. At that sample the difference is
about one standard deviation, so it neither confirms the formula nor challenges
it. `examples/torp_check` reproduces the figure.

Worth separating the three states this subsystem is now in:

| | state |
|-|-------|
| volley split, damage, missile bonus | transcribed from the firing loop; unit-tested |
| accuracy formula | transcribed from `CTorpHit`; **unverified** — 24 shots is no sample |
| replaying a recorded torpedo battle | needs the RNG, and that needs tutorial-mode turns |

Only the third is an acquisition problem. The second would be settled by any
game with more torpedo fire in it, tutorial mode or not.
