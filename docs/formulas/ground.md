# Subsystem: Landing colonists — settling and invasion

- **Status:** transcribed in full, unit-tested; unverified against a real invasion (the corpus has none)
- **Ghidra routine(s):** `10b8:34e2` `DropColonists`, called from `DoOrders`
- **Manual reference:** `MANUAL.PDF` — ground combat
- **Uses RNG:** only for the wreckage and the artifact a settling turns up
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

## Sorting the landings

Each drop record (`COLDROP`: player, planet, colonists, `fCanColonize`) is
looked at in turn:

1. an **Alternate Reality** race's colonists, unless they may colonise an
   empty planet, "were reduced to protoplasmic blobs" (`0x57`);
2. colonists on an **empty** planet without `fCanColonize` — put down by the
   Cargo Transfer dialog rather than a Colonize order (`log.c` sets the flag
   only for an inhabited destination; `FQueueColonistDrop` always sets it) —
   die "because you did not colonize the planet first" (`0x02`);
3. an inhabited planet with a **starbase** kills every landing (`0x58`);
4. the rest count: `colonists[player] += n`, and
   `power[player] += (n × attack[prt] / 100) × pctSurvive`, truncated.

`pctSurvive` is `CalcPctSurvive`'s share for the planet — what its defences let
through of a bomb — raised for troops to `pct + (1 − pct) / 4`
(`10b8:35c0`, the constants at `1120:1d8e` and `1d92` being 1.0 and 4.0).

## Resolution

```
if the planet is held:
    defence = population × defence[prt] / 100
    if defence > Σ power:
        each attacker: massacred (0x00 / 0x03), or, when pctSurvive < 1,
            "\P were destroyed by planetary defenses" (0x01 / 0x04) with
            P = 10000 × (pctSurvive − 1) — a negative number, the original's
        population -= population × Σ power / defence
        done
    UninhabitPlanet; the attackers now fight over an empty planet

best = −1; second = 0; tie = false
for player in 0 .. cPlayer with colonists[player] != 0:
    sides += 1
    if power[player] >= best:
        if power[player] == best: tie = true
        else: tie = false; second = best; best = power[player]; winner = player
if best < 0: nothing landed
if tie:      everyone dies (0x06 each; 0x05 to the former holder, who loses the planet)

winner keeps:
    left = colonists[winner]                                if Σ power == 0 or best == 0
         = colonists[winner] × (best × (Σ power − defence) / Σ power) / best   otherwise
    if second > 0: left = left × (best − second) / best
    at least 1
```

Notes on the transcription:

- `cMax` starts at `−1` and the "nothing landed" test is on its **high word**
  (`10b8:3be0`): a claim of **zero** power still wins, which is how an
  Alternate Reality race settles a planet (its message is `0x0b`, "deployed
  the Orbital Construction Module"), and it is then given a starbase on the
  spot — `fStarbase`, `pctDp = 0`, its first starbase design's `cBuilt` and
  `cExist` up by one.
- `second` is only replaced when a **new maximum** is found, so it is the best
  power seen *before* the winner in player order. A winner numbered below every
  rival loses nothing to them; one numbered above a rival loses the rival's
  share. This is the original's own quirk and is kept.
- Messages on a taking: `0x0c` to the winner (loser as `| 0x20`), `0x0d` to the
  other attackers, `0x07` to the loser (winner as `| 0x30`, the count as a
  long); on a settling with rivals `0x08` / `0x09` (winner as `| 0xb0`); alone,
  `0x0a` or `0x0b`.
- The loser's six technology levels become the winner's wreckage
  (`ITechLearnATech`, below).

Worked examples are the unit tests in `crates/stars-core/src/ground.rs`: a
200-hundred landing on 100 defenders keeps `200 × (220 − 100) / 220 = 109`; with
defences letting 60% of a bomb through, troops fare at 70%, the power is 154
and `200 × (154 − 100) / 154 = 70` remain.

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
(`PLAYER.grbitTrader`): `learn_from_battle`'s `trader_seen`, filled by
`MarkTechsSeen` from the wrecks.

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
