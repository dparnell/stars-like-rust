# Wormholes and the Mystery Trader

Status: **carried, moved, travelled and traded with, as far as the fixtures
allow.** The Trader's arrival behaviour is not modelled.

Two things in a Stars! galaxy move without anybody ordering them to. Both are
`THING`s ([`thing.md`](../formats/thing.md)) and both are now in the model:
[`stars_core::wormhole`](../../crates/stars-core/src/wormhole.rs).

## Wormholes

A wormhole comes in a **pair**: two ends, each naming the other, joining two
distant parts of the galaxy for whoever finds them. An end sits still for years
and then jumps somewhere else entirely, which is why a route through one is
never dependable.

**Whether it jumps** is `PctWormholeMoves` (`1110:0adc`):

```
chance% = years sitting still / 5 − (2 − stability)     clamped to 0..=6
```

So a rickety end (stability 0) has to sit for ten years before it will move at
all, a rock-solid one (3) is restless from the first, and none of them jumps
more than six years in a hundred.

**Where it goes** is chosen by trying up to a hundred positions and scoring each
with `IValidateWormholePos` (`1110:064c`) — a jump picks anywhere in the galaxy,
otherwise it drifts within twelve light years — and taking the first that scores
zero, or the least bad. The score is a set of penalties: on top of a planet, a
fleet or another object, or outside the galaxy, is unusable; near the edge or
near a planet costs points; and **its own partner is kept furthest away of all**,
the penalty reaching seventy light years rather than thirty. That last is what
stops a pair collapsing into one corner, which would make it useless.

A jump also **forgets who had seen it**: `grbitPlr` is cleared, so everyone has
to find it again.

### Going through

A fleet is ordered to a wormhole by naming it as a waypoint target — `grobj` 8,
a `THING` rather than a planet. When it **arrives**, `MoveFleets`
(`10b0:4ce4`) takes it out of the far end:

```
fleet.pt        = partner.pt      the fleet is at the other end of the galaxy
waypoint.pt     = partner.pt      and its orders follow it there
near.grbitPlrTrav |= player
far.grbitPlrTrav  |= player       both ends remember the traveller
far.grbitPlr      |= player       and the far end is now in view
```

The two masks are easy to mistake for one another, and the field names invite
it. What our binary does with them:

| Field | Set by | Cleared by |
|-------|--------|------------|
| `grbitPlr` (+2) | a scanner in range (`SetVisPFPlanets`, `1070:abde`), and coming out of this end | a jump |
| `grbitPlrTrav` (+4) | going through, at **both** ends | never |

So `grbitPlr` is who can see it *now* and `grbitPlrTrav` is who has ever been
through it. The community reconstruction has the scanner setting `grbitPlrTrav`
instead; `SetVisPFPlanets` in our binary sets `grbitPlr`, and the fixtures
agree — of the 1,309 ends that record somebody having been through them, **713
are not in that traveller's view now**, which could not happen if one mask
implied the other.

## The Mystery Trader

The Trader crosses the galaxy at speed, and one year in twenty-five it changes
its mind (`10b0:1af7`): it always **speeds up** by a warp factor, and one time
in three it also picks a new destination somewhere on the **edge** of the map.
Then it covers the square of its warp toward wherever it is going.

### Trading with it

Meeting it is the point of it. `DoThingInteractions(1)` (`1110:0b3a`) runs
after movement and considers every fleet that has come to rest **exactly** on
the Trader.

A fleet carrying fewer than **5,000 kT** of ironium, boranium and germanium
between them is turned away, and told so on the year it arrives (`fHereAllTurn`
suppresses the repeat). A fleet carrying enough is **kept** — the Trader
absorbs it, ships and cargo alike — and in exchange the player gets one of:

1. **The technology the Trader is carrying**, if they do not already have it.
   That is `THTRADER.grbitTrader`: a single one of thirteen `GrbitTrader` bits,
   *not* a player mask, despite sitting next to one. Across the 859 Trader
   records in the fixtures it is always `0` or a single bit.
   `IdmGiveTraderPart` (`1110:1a96`) sets the player's bit and picks the
   message, with its own wording for a hull and for the Genesis Device.
2. **Technology levels**, otherwise:

   ```
   levels = (cargo − 5000) / 1200 + 6            capped at 10
   then, by the sum of the player's six levels:
       ≥ 108 → 1     ≥ 96 → 2     ≥ 84 → −3     ≥ 72 → −2     ≥ 60 → −1
   ```

   Each level goes three times in four to a field picked at random and
   otherwise — and whenever that field is already at the ceiling — to the field
   the player is furthest behind in. The ceiling is 26, or **10 in a shareware
   game** (`PLAYER.fCrippled`).

   A level is not written down but **paid for** (`1110:10e3`): the field's
   accumulated research is doubled and the outstanding cost of the next level
   added, which comes to `cost + spent` — so the level always lands and the
   player's part-finished research survives it.
3. **Nothing**, for a player who has already researched everything: one year in
   five the Trader finds a part in the hold after all, and the rest of the time
   it has nothing to give.

Each Trader trades **once** with each player; `grbitPlr` is what records it, so
the mask that says who can see it also says who has already been.

When the part drawn is one the player already holds, the Trader draws again, up
to twenty-five times. If all twenty-five come back held, the result is
`grbitTraderLifeboat` — which is not a part at all but a **ship**.

### The ships

The Trader gives one of three designs of its own: `M.T. Lifeboat`, `M.T. Scout`
and `M.T. Probe`, entries 19 to 21 of the game's built-in design table
(`rgshdefT`, already transcribed in `crates/stars-core/src/startup.rs`). Nobody
can build them — their hulls, 29 and 30, are not for sale — so a fleet of them
is the only way they exist.

```
offset = Random(4 − (turn > 100))        0 a quarter of the time: the Lifeboat
if offset > 0: offset = Random(2) + 1    otherwise the Scout or the Probe
ships  = Random(3) == 0 ? 2 : 1
if turn > 100 and not a single-player game: ships += Random(turn / 100 + 1)
ships  = min(ships, 5)
if offset > 0: ships += Random(ships + 1)
```

So the Lifeboat comes alone or in pairs, the other two can come in numbers, and
a long game gives more of them — except a **single-player** game, which is held
to the early-game figures (`GAME.fSinglePlr`, bit 2 of `wCrap`).

The ships need one of the player's sixteen design slots. A design they already
have that is the same ship is reused — `IshFindSimilarDesign` (`1038:7c5e`)
compares the hull, the number of slots and then each slot's **count**, plus its
item and category wherever the slot is filled, so the *name* does not matter —
and failing that the first free slot is taken. With no slot free, or with 512
fleets already, the Trader is reported as having tried and failed (message
`0x150`).

The new fleet appears where the Trader is, with **full tanks**, and is marked
`fHereAllTurn` so that nothing this year treats it as having just arrived.

An **AI player gets nothing at all** and is not told either: the arm returns
before it reaches any of this.

## What is verified

- **2,672 wormhole ends and 589 Traders** survive a load and a save unchanged.
- **1,764 wormhole ends have both halves in view and every one of those pairs
  is mutual** — each end names the other, and no end names itself. The rest have
  only one end visible, which is exactly what a wormhole nobody has been through
  looks like.
- **527 of 547 Trader-years** flew the modelled distance: the square of its
  warp, before or after the speed-up.
- **1,309 wormhole ends that somebody has been through**, 728 of them with both
  halves in the same file, and in every one of those the traveller is recorded
  at both ends — which is what `MoveFleets` guarantees. 713 of the 1,309 are no
  longer in the traveller's view, which is how the two masks were told apart.
- **859 Trader records**, every one carrying a single `GrbitTrader` bit or
  nothing: the field is a technology, not a player mask.

The twenty that did not are the Trader's own doing. In each, its warp went
**down** and its destination changed — and the course change only ever speeds it
up, so those are not the same flight continuing but a new pass beginning. The
Trader's arrival behaviour, which the original handles with a "decided to make
another pass" message, is not modelled.

Movement itself cannot be checked exactly for wormholes: where one jumps is a
hundred dice rolls deep, and reproducing it would need the original's random
stream in the same state.

Neither trading nor traversal can be checked against the fixtures directly:
both need two consecutive years in which the event happens, and no captured
game has one. The ship gift leaves a permanent trace where the others do not —
a design on hull 29 or 30 — and there is **none in any fixture**: nobody in the
captured games ever got one, so even that cannot be confirmed from data. They are checked against the binary, and by construction in
`crates/stars-core/tests/trading.rs`.

## Not modelled

- **The Trader's arrival** at its destination, and its departure.
- **The AI's shortcut** (`1110:1631`): an AI player of level 2 or better with a
  starbase planet within 100 light years of the Trader gets the same goods for
  free, paid for out of the planet's surface minerals, without sending a fleet.
- Wormhole **visibility**: who can see an end is carried through a file, set by
  traversal and cleared on a jump, but not recomputed from anybody's scanners.
- `NoAutoTrackFleet`: the original stops a fleet auto-tracking the wormhole it
  has just used. This engine does not model auto-tracking at all.

## Source

- `MoveThings` `10b0:18f4` — the wormhole arm at `10b0:194c`, the Trader's at
  `10b0:1af7`.
- `PctWormholeMoves` `1110:0adc`, `IValidateWormholePos` `1110:064c`.
- `MoveFleets` `10b0:4ce4` — the wormhole traversal check.
- `DoThingInteractions` `1110:0b3a`, `IdmGiveTraderPart` `1110:1a96`,
  `WFromLpfl` `1038:2b10`, `CostOfDevelopingItem` (research).
- `PLAYER.grbitTrader` at offset `0x52`, `PLAYER.fCrippled` at `0x54` bit 1.
- The records: `docs/formats/thing.md`, `THWORM` and `THTRADER`.
