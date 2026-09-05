# Wormholes and the Mystery Trader

Status: **carried, moved, travelled and traded with, as far as the fixtures
allow.** A Trader's whole life is now modelled — it crosses, it arrives, it
turns round or it leaves.

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
a `THING` rather than a planet. The id it stores is the thing's **full** id,
whose top three bits are the `ith` saying what kind of thing it is; without
checking those, a fleet bound for wormhole 1 would be caught by anything else
that happened to be object 1. All 3,686 thing waypoints in the fixtures carry
their kind — 3,346 name a mineral packet and 340 a wormhole.

When the fleet **arrives**, `MoveFleets` (`10b0:4ce4`) takes it out of the far
end:

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

A galaxy may hold **more than one** Trader: 374 of the fixture files carry two
and some carry three. Each flies, trades and remembers who it has met on its
own.

The Trader crosses the galaxy at speed, and one year in twenty-five it changes
its mind (`10b0:1af7`): it always **speeds up** by a warp factor, and one time
in three it also picks a new destination somewhere on the **edge** of the map.
A Trader already at warp 13 has stopped changing its mind. Then it covers the
square of its warp toward wherever it is going.

### Arriving, and leaving

Reaching its destination ends the pass (`10b0:1da3`), and what happens then
turns on whether the galaxy holds another Trader:

* **another Trader exists** — this one leaves for good;
* **it is the only one** — a coin flip decides between leaving and staying.

A Trader that stays **makes another pass**: it sits where it arrived, picks a
fresh destination the same way a course change does, and its warp becomes

```
warp = max(warp − 2, 6) + 1
```

— slower than the pass it just flew, but never below 7, so a Trader already
down at warp 6 or 7 comes back *faster*. It spends the year turning round
rather than moving. Every player is told (`0xc0`,
`idmMysteryTraderHasDecidedMakeAnotherPass`), as they are for a course change
(`0x130`).

The removal happens as the loop reaches it, so a second Trader arriving in the
same year may find itself alone by the time its own turn comes — and then get
the coin flip that the first one did not.

**A fleet on its way to a Trader that has gone** has its waypoint turned into a
plain position at the Trader's last known spot, and is told so
(`idmMysteryTraderHeadingHasVanishedOrdersHave`, `0x110`). The original does
this per player as it writes their file, and applies it to a Trader that is
merely *out of view* as well; only the *gone* half is modelled here, because
this engine has no per-player visibility pass.

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

### The computer players' shortcut

`DoThingInteractions` runs a **second loop, over planets** (`1110:1631`), after
the one over fleets. A computer player of skill 2 or better with a **starbase**
planet within a hundred light years of the Trader trades with it where it
stands: no fleet, no journey, and nothing for anybody else to see. It is the
AI's substitute for the errand a person has to run.

The terms are the same shape as a fleet's, and the prices are not:

| | fleet | planet |
|---|---|---|
| who may | anybody | a computer player of skill 2 or 3, with a starbase |
| what it costs to be heard | 5,000 kT aboard | 3,500 kT on the surface at skill 2, 5,000 at skill 3 |
| a part | the fleet | **every kiloton on the planet** |
| technology | the fleet | the threshold only |
| how many levels | 1 to 10, by cargo and by how advanced | always 6 |
| draws for a part not yet held | 25 | 50 |
| told about it | yes | no |

The part path taking the planet's whole stockpile is not a rounding of the
threshold: `wtNext` still holds the sum of all three minerals when the payment
loop runs, and only the technology path resets it to the price
(`1110:1a5c`). The six levels go one at a time into whichever field is furthest
behind, and are refused outright to a player within six levels of the ceiling
(`maxTech × 6 − 6`).

Either way the Trader marks the player off in `grbitPlr`, so the shortcut and a
fleet meeting are the same one chance.

Two details of the original are worth recording rather than copying:

* The planet scan **stops** at the first planet more than a hundred light years
  east of the Trader. That is safe because the `.xy` stores each planet's x as
  an offset from the one before, so the array is in ascending x order by
  construction. Testing every planet, as here, comes to the same thing.
* The shareware tech cap is read as `rgplr[lpfl->iPlayer].fCrippled` — off the
  **fleet pointer left over from the loop above**, not the planet's owner. It is
  harmless because `fCrippled` describes the game file rather than the player
  and is the same for everybody in a game, but it is a stale variable, and this
  engine reads the planet owner's flag instead.

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

- **2,672 wormhole ends and 859 Traders** survive a load and a save unchanged.
  The Trader figure was 589 until a galaxy was allowed to hold more than one:
  the model kept a single Trader, so 270 of them were being dropped on load and
  would have been lost from any file written back.
- **1,764 wormhole ends have both halves in view and every one of those pairs
  is mutual** — each end names the other, and no end names itself. The rest have
  only one end visible, which is exactly what a wormhole nobody has been through
  looks like.
- **807 of 807 Trader-years** flew the modelled distance: the square of its
  warp, before or after the speed-up — or the remainder of the leg, on the year
  it arrived.
- **20 Trader-years ended a pass**, and every one of them matches the
  another-pass rule exactly: the Trader stands on the destination it had, its
  warp is `max(warp − 2, 6) + 1`, and it has a new heading. These are the same
  twenty years that would not fit the flight model before arrival was
  understood.
- **1,309 wormhole ends that somebody has been through**, 728 of them with both
  halves in the same file, and in every one of those the traveller is recorded
  at both ends — which is what `MoveFleets` guarantees. 713 of the 1,309 are no
  longer in the traveller's view, which is how the two masks were told apart.
- **859 Trader records**, every one carrying a single `GrbitTrader` bit or
  nothing: the field is a technology, not a player mask.

Movement itself cannot be checked exactly for wormholes: where one jumps is a
hundred dice rolls deep, and reproducing it would need the original's random
stream in the same state.

Neither trading, in either of its forms, nor traversal can be checked against
the fixtures directly:
both need two consecutive years in which the event happens, and no captured
game has one. The ship gift leaves a permanent trace where the others do not —
a design on hull 29 or 30 — and there is **none in any fixture**: nobody in the
captured games ever got one, so even that cannot be confirmed from data. Those
are checked against the binary, and by construction in
`crates/stars-core/tests/trading.rs`.

## Not modelled

- **Visibility**: a waypoint following a Trader that is merely out of scanner
  range is left alone, where the original would cut it loose when it writes
  that player's file. The same goes for a wormhole or a minefield, which the
  original cuts loose the same way, with their own messages (`0x111`, `0x112`).
- Wormhole **visibility**: who can see an end is carried through a file, set by
  traversal and cleared on a jump, but not recomputed from anybody's scanners.
- `NoAutoTrackFleet`: the original stops a fleet auto-tracking the wormhole it
  has just used. This engine does not model auto-tracking at all.

## Source

- `MoveThings` `10b0:18f4` — the wormhole arm at `10b0:194c`, the Trader's at
  `10b0:1af7`.
- `PctWormholeMoves` `1110:0adc`, `IValidateWormholePos` `1110:064c`.
- `MoveFleets` `10b0:4ce4` — the wormhole traversal check.
- `DoThingInteractions` `1110:0b3a` — the fleet arm, the part arm at
  `1110:1180`, and the computer players' planet arm at `1110:1631`.
- `IdmGiveTraderPart` `1110:1a96`, `WFromLpfl` `1038:2b10`,
  `IshFindSimilarDesign` `1038:7c5e`, `CostOfDevelopingItem` (research).
- `PLAYER.grbitTrader` at offset `0x52`, `PLAYER.fCrippled` at `0x54` bit 1.
- The records: `docs/formats/thing.md`, `THWORM` and `THTRADER`.
