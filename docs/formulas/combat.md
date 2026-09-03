# Subsystem: Combat

- **Status:** in progress — board, movement, targeting, accuracy and damage resolution implemented; the firing loop that drives them is not
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
  design, the recorded range and the target's state. Only the *first* damage a
  token takes is checkable this way, because after that its state depends on
  the full firing order.

`crates/stars-core/tests/combat_vectors.rs` checks the movement table, torpedo
accuracy, the starting-square table and Chebyshev distance against
`../vectors/combat.json`, whose cases are quoted from the manual.

## Open questions

- **The firing loop itself** — which token shoots when, and at what — is not
  implemented. `FAttack` walks weapon slots in initiative order, picks targets
  by the tactic scoring above, and spills overkill onto neighbours; the damage
  each shot does is implemented, but the sequencing is not.
- Nine of the 31 checkable first beam hits do not reproduce. They differ by one
  ship killed in both directions, which points either at the "first hit"
  assumption (a token can be damaged by an attack that recorded no kill entry
  against it) or at a modifier not yet modelled. Worth revisiting with the
  firing loop, which would remove the need for that assumption entirely.
- Torpedo resolution consumes RNG per torpedo, so reproducing a recorded battle
  needs the generator in the right state — which needs the firing order.
- `grfWeapon` bit meanings in the kill records are not yet mapped.
- The three-phase movement order and the heaviest-first rule within a phase are
  documented here from the manual but not yet implemented or checked.
- Bombing (`DoBombing`) and ground combat are separate from ship battles and
  are not covered.
