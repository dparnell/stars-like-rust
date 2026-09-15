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

*(nothing outstanding — see the sections below.)*



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


## Wreckage salvage

Taking an inhabited planet copies the loser's six technology levels into
`rgTechBattle` and calls `ITechLearnATech` — the "wreckage discovered, research
boosted" message. Implemented as `ground::learn_from_wreckage` (the field
half alone) and, on the game state with the Mystery Trader half too, as
`ground::learn_from_battle`, which the turn runs for a planet taken and
`combat.md`'s battles run for the ships destroyed.

```
if the player already learned something this turn:  nothing
if Random(100) <= 49:                               nothing        # so half the time
repeat 6 times:
    f = Random(6)
    if myTech[f] < loserTech[f]:
        credit GetTechLevelCost(f, myTech[f] + 1) to rgResSpent[f]
        mark the player as having learned this turn
        stop
```

Before the six field tries the routine makes thirteen at a **Mystery Trader
part** (`10f0:9960`): `Random(13)` names a part bit; if the wreckage carried
any copies of it (`rgTechTrader`, counted by `MarkTechsSeen` up to 25), the
player lacks the part, and `Random(100)` comes under the count, the part is
theirs — told with the Trader's own wording moved up by `0x2f` (`0x13a` a
part, `0x13b` a hull), the item word as the object — and the year's find is
spent. A planet taken carries no Trader parts, so there the tries all miss.
The message for a field carries the place (`x, y`, or `-1` and the planet),
the field and the cost as a long, with object `-2`; a slow-tech game's
doubled cost is what the table gives.

Three things are worth drawing out.

**It is not a free level.** The routine credits the *resources* the next level
costs, into `rgResSpent` for that field. The level then arrives at the next
research tick like any other, which is why the message speaks of research being
boosted rather than a level gained.

**One per turn, from any source.** Bit 3 of the player's state word is set on
success and checked on entry, so a player who has already learned from a
Mystery Trader or another wreck this turn takes nothing from this one.

**The field is drawn, not chosen.** Six attempts each pick a field with
`Random(6)` and take the first where the loser led. A player behind in one
field of six is therefore likelier to come away empty-handed than the "half the
time" gate suggests.

### The Mystery Trader half

The same routine tries thirteen Mystery Trader parts first, each with its own
percentage in `rgTechTrader`, skipping any the player already holds
(`PLAYER.grbitTrader`). It is not modelled: the part table is not in any
fixture.

### Not verified

Nothing in the fixtures exercises this. Only 56 planets change hands across
`fixtures/games/all-computer-players`, none of them attributable to an invasion
without the fleet orders, and a credited research cost is indistinguishable in
a save file from research the player paid for itself. The transcription is
structural, and the unit tests check its shape rather than its output.


## The artifact a settling turns up

A planet carrying `fIsArtifact` gives its new owner a windfall when it is
settled, and the flag is cleared so it is found only once. Implemented as
`ground::artifact_bonus`.

```
field     = Random(6)
resources = Random(301) + 100
if colonists < 10: resources = colonists * resources / 10
```

**It pays research, not minerals.** An earlier revision of this document called
it "a random mineral concentration bonus", written from a skim of the
decompilation. It is not: the amount goes into `rgResSpent` for the chosen
technology field, at `player * 0xc0 + 0x59c2 + field * 4`, which is the same
place [wreckage salvage](#wreckage-salvage) credits. The two mechanics pay out
in exactly the same currency.

A thin first landing is worth proportionally less — a colony below ten (a
thousand colonists) scales the windfall by its size — so dropping a token
colonist load to grab an artifact is deliberately unrewarding.

Two gates sit around it, and the caller applies them: the planet must carry the
artifact, and bit 7 of the game options word must be clear. That bit is the
option that switches artifacts off; this project has not identified its label
in the binary, so it is described by what it does.

`stars-core`'s `Planet` now carries `artifact`, which the format layer was
already decoding from bit 12 of the planet flags word and the loader was
discarding.

### Not verified

As with the salvage, nothing in the fixtures exercises it: a research credit is
indistinguishable in a save from research the player paid for, and no fixture
records a planet with an artifact being settled.
