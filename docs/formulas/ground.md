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

- **Wreckage salvage.** Taking an inhabited planet runs `ITechLearnATech`
  against the loser's technology — the "wreckage discovered, research boosted"
  message.
- **Mineral discovery.** A settling can grant a random mineral concentration
  bonus, gated on a game flag and `Random(6)` / `Random(301)`.


## The inherited production queue

A freshly settled planet is given a queue built from its owner's **default
template**, which is why a newly colonised planet shows a queue immediately.
The template is `PLAYER.zpq1`, at offset 86 of the 192-byte player structure:

| Offset | Size | Field |
|--------|------|-------|
| 0 | 1 | `fNoResearch` — the new planet's "no research" flag |
| 1 | 1 | `cpq` — how many entries follow |
| 2 | 24 | `rgpq[12]` — twelve 2-byte entries |

Each entry's low six bits are the item; `DropColonists` reads them as
`entry & 0x3f`. It is the "zip" production queue the `ZipProdDlg` dialog edits.

### Two races filter it

```
Alternate Reality  drops every item <= iobjDefense   # no planetary installations
Claim Adjuster     drops the two terraform items     # terraforms from orbit, free
```

Implemented as `ground::template_allows`, and both hold across every AI
planet-turn in `fixtures/games/all-computer-players`:

| trait | planetary queue items ever seen |
|-------|--------------------------------|
| Alternate Reality | 3, 11, 12, 16, 17 — **never** an installation, in any form |
| Claim Adjuster | 7, 8 — **never** a terraform item |
| Hyper Expansion, Inner Strength, Packet Physics, Super Stealth | installations *and* terraforming |

Both are distinguishing rather than vacuous: every other trait uses exactly the
items these two omit. The Claim Adjuster sample is small — 48 entries against
5723 for Alternate Reality — so that half rests on less evidence.

### The template itself is not in the fixtures

`zpq1` is player state, not race state, and it is **not** in the serialised
race struct: that struct is a packed 0x68-byte format of its own (see
`../formats/race-r.md`), not a copy of the player structure's memory, and the
26 bytes the template needs do not fit in what the player block leaves over
after the relations table and the packed names.

So the *rule* is recovered and its filter verified, but the template a given
player would hand a new planet cannot be read from these saves. Predicting the
exact queue a settling produces needs it.
