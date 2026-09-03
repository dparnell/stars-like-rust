# Subsystem: Combat

- **Status:** in progress — board, movement schedule, targeting, accuracy, damage and the beam firing loop implemented and verified; movement *scoring* and torpedo resolution are not
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

### How well it does

Measured against the recordings, with each token's real battle tactic:

| measure | before transcription | after |
|---------|---------------------:|------:|
| engine's square among our best-rated | 86% | 86% |
| our best set, as a share of candidates | 78% | 71% |
| gap | 8 points | **15 points** |

The hit rate should be ~100% for a correct implementation, since the engine
always picks a square it rates best. Two known gaps account for the rest, and
both are separate stubs:

- **`DzMoveRangeToConsider`** sets the search radius, which is weapon-range
  based rather than the movement allowance. Our candidate set is therefore not
  the engine's, and the engine's chosen square may not even be in it.
- **`FIsTargetOfMdTarget`** filters enemies by target class; without it every
  enemy counts as engageable, which inflates the damage a square appears to
  offer.

## Armour and shields## Armour and shields

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

- **`DzMoveRangeToConsider`** (the movement search radius) and
  **`FIsTargetOfMdTarget`** (the target-class filter) are both still stubs, and
  together account for the 14% of moves the scoring does not rate best. Both
  are small and self-contained.
- Even complete, movement cannot be reproduced exactly without the RNG, because
  every tie-break draws from it.
- Three of the eleven replayable battles still come out inverted. All three are
  symmetric duels where both ships can destroy the other in one volley, so the
  result turns on exactly when in the round each closed to range; firing once
  at the end of the round is an approximation there.
- **Torpedo resolution** is specified but not driven: hits are rolled per
  torpedo, so reproducing a recorded battle needs the generator in the right
  state, which in turn needs every earlier draw in the turn.
- Gattling weapons, which hit every target in range rather than one, are
  described in `FAttack` but not implemented.
- `FIsTargetOfMdTarget` — the primary/secondary target-class filter — is not
  implemented, so target selection currently considers every enemy in range.
- `grfWeapon` bit meanings in the kill records are not yet mapped.
- The three-phase movement order and the heaviest-first rule within a phase are
  documented here from the manual but not yet implemented or checked.
- Bombing (`DoBombing`) and ground combat are separate from ship battles and
  are not covered.
