# Subsystem: Landing colonists — settling and invasion

- **Status:** weights and the winner rule recovered; contested survivor counts not
- **Ghidra routine(s):** `10b8:34e2` `DropColonists`, called from `DoOrders`
- **Manual reference:** `MANUAL.PDF` — ground combat
- **Uses RNG:** no
- **Implemented in:** `crates/stars-core/src/ground.rs`

One routine does both jobs, because they are the same act: colonists are put
down on a planet, and what happens next depends on whether anyone was already
there. Every drop aimed at a planet in the same turn is resolved together, so
two players who both send colonists to an empty world contest it, and an
invasion by several players at once is a single fight.

## The weights

```
attack[prt]  = 165 for War Monger, 0 for Alternate Reality, else 110   # 10b8:3766
defence[prt] = 200 for Inner Strength, else 100                        # 10b8:3820
```

A War Monger's colonists fight at 165% of their number; Inner Strength defends
at double. An Alternate Reality race weighs **zero** — it cannot take a planet
with colonists at all, which follows from living on its starbases.

## Resolution

```
attack  = sum over landings of colonists * attack[prt] / 100
defence = population * defence[prt] / 100          # 0 for an empty planet

if the planet is empty:
    the heaviest single landing takes it
else if attack < defence:
    the defender holds, losing population * attack / defence
else:
    UninhabitPlanet, and the heaviest landing settles what is left
```

## What is not transcribed

The survivor counts after a **contested** landing. `DropColonists` scales them
through several 32-bit terms that the decompiler has flattened past confident
reading, and nothing in the fixtures separates a contested landing from an
uncontested one: all 513 colonisations in
`fixtures/games/all-computer-players` are onto empty planets with a single
claimant, and the corpus has no invasion at all — only 56 planets change hands
in 101 turns, and those need the fleet orders to attribute.

`resolve_landings` therefore returns the uncontested answer in those cases and
reports the attacker's surplus for a captured planet, rather than guessing at
the original's formula.

## Also in `DropColonists`, not yet transcribed

- **The new planet's production queue.** A freshly settled planet is given a
  queue built from the owner's default template (`rgplr[player].zpq1`), which is
  why newly colonised planets show a queue immediately. Worth doing: it is
  checkable against the 513 settlings.
- **Wreckage salvage.** Taking an inhabited planet runs `ITechLearnATech`
  against the loser's technology — the "wreckage discovered, research boosted"
  message.
- **Mineral discovery.** A settling can grant a random mineral concentration
  bonus, gated on a game flag and `Random(6)` / `Random(301)`.
