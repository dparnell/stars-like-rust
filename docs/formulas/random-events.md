# Subsystem: Random events

- **Status:** transcribed from the binary and unit-tested; the message ids
  and their parameter shapes are confirmed against the Exodus game's message
  records, the numbers themselves are not verifiable from the fixtures (no
  saved game has two consecutive years around an event)
- **Ghidra routine(s):** `RandomEvents` (`10b8:3064`), `MeteorStrike`
  (`10b8:560e`), `PlanetaryClimateChange` (`10b8:5c54`),
  `DiscoverNewMinerals` (`10b8:5e0c`), `MysteryTrader` (`10b8:5efa`),
  `TossNonAutoBuildItems` (`10b8:5aec`)
- **Manual reference:** `MANUAL.PDF` p. 7-12 ("Disaster Strikes Planet X!":
  a comet resets the production queue, and usually brings minerals with it)
  and p. 19-2 (the Mystery Trader, "a transdimensional being with technology
  to sell")
- **Uses RNG:** yes — every event is a roll, and each rolls for itself
- **Implemented in:** `crates/stars-core/src/events.rs`

`RandomEvents` is the last thing `Produce` (`10b8:0000`) does, after the
research advance (call at `10b8:0c87`), and it is skipped when the game's
"no random events" flag is set — `GAME` word `+0x10`, bit 7,
`game_flag::NO_RANDOM` in `stars-formats`. The New Game wizard's checkbox
is the only way to clear it. It runs four routines in a fixed order, each
with its own roll, so a year can have any combination of them:

```
MeteorStrike → PlanetaryClimateChange → DiscoverNewMinerals → MysteryTrader
```

The routines share one gate, "may this planet be hit":

```
strikeable = owner == none  ||  pop < 5,100  ||  turn > 19
```

(`rgwtMin[3] < 0x33`, in hundreds of colonists; `game.turn > 0x13`). So an
established colony is safe for the first twenty years of a game, an empty
world or a colony that has only just landed never is.

## Meteor strike (`MeteorStrike`, `10b8:560e`)

| draw | range | meaning |
|------|-------|---------|
| `Random(20)` | 0 | a strike happens this year at all |
| `Random(cPlanet)` | any | the planet |
| — | | stop unless *strikeable* **and** `turn > 9` |
| `Random(4)` | 0..=3 | the strike's *size* |
| `Random(3)` × 3 | | shuffle the order the minerals are *named* in |
| `Random(250) + 50` × 3 | 50..=299 | kT landing on each mineral |
| `Random(3 − i)`, i = 0, 1 | | shuffle which minerals get the big deposits |

Every player is told (`FSendPlrMsg`), id `0x83 + size` — or `0x87 + size` for
the planet's owner, unless that owner is Alternate Reality (`rsMajorAdv == 8`),
who gets the general form. The object and the first parameter are the planet.
The routine also passes the shuffled naming order as parameters two to four,
but the Exodus game's `.m6` holds an `0x84` with the planet as its only
parameter, so only the planet is stored.

Then, on the planet:

1. **Population.** The owner, unless AR, loses `25 + 20 × size` percent:
   25%, 45%, 65% or 85%, computed as `pop − pop × pct / 100` in 32-bit
   arithmetic.
2. **Minerals.** `size + 1` of the three minerals (at most three), in the
   shuffled order, each gain `Random(17000) + 3000` kT on top of the 50 to
   299 every mineral already got, and their concentration rises by
   `Random(50) + 50`, plus `Random(15) + 15` more for a size-3 strike, capped
   at 200.
3. **Climate.** The first `size + 1` habitability variables (gravity,
   temperature, radiation, in that order, at most three) each move by
   `Random(3) + 3` clicks — plus another `Random(3) + 3` for a size-3 strike
   — in a direction decided by `Random(2)`, clamped to `1..=99`. The
   *original* value moves by the same amount, so terraforming already done
   is neither undone nor credited.
4. **The queue.** `TossNonAutoBuildItems`: everything in the production
   queue that is not an auto-build item is thrown away — the manual's "all
   work in progress is lost".

### Worked example

A size-2 strike on a planet of 100,000 colonists (`pop = 1000`) with
concentrations `[40, 40, 40]` and environment `[50, 50, 50]`, owned by a
non-AR race:

- `pct = 25 + 20 × 2 = 65`; `killed = 1000 × 65 / 100 = 650`; 350 hundreds,
  35,000 colonists, are left.
- Three minerals get 50 to 299 kT; three of them (`size + 1 = 3`) get 3,000
  to 19,999 more, and rise in concentration by 50 to 99 each — `40 + 50..99`
  is at most 139, under the cap.
- Gravity, temperature and radiation each move 3 to 5 clicks either way; the
  originals with them.

`events::tests::a_meteor_strike_does_what_the_routine_does` pins exactly
this: the population arithmetic, that every mineral gained at least 50 kT
and `size + 1` of them thousands, the same number of raised concentrations,
the moved variables tracking their originals, and a ship leaving the queue
while an auto-build item stays.

## Climate change (`PlanetaryClimateChange`, `10b8:5c54`)

| draw | range | meaning |
|------|-------|---------|
| `Random(20)` | 0 | a change happens at all |
| `Random(cPlanet)` | any | the planet — stop unless *strikeable* (no turn-10 gate here) |
| `Random(3)` | 0..=2 | the variable: gravity, temperature, radiation |
| `Random(3) + 3` | 3..=5 | the shift; when it comes out at exactly 3, it is re-drawn as `Random(3) + 6`, 6..=8 |
| `Random(2)` | ≠ 0 | negative |

The owner, if any, gets message `0xfd` with the planet and the variable
(the message is sent *before* the shift is drawn). The variable and its
original move together, clamped to `1..=99`, and the queue loses its
non-auto items, exactly as after a strike.

So the shift is 4 or 5 two times in three, and 6, 7 or 8 otherwise — a
change of 3 clicks never happens.

## New minerals (`DiscoverNewMinerals`, `10b8:5e0c`)

| draw | range | meaning |
|------|-------|---------|
| `Random(15 − size)` | 0 | a discovery happens at all: one in 15 (tiny) to one in 11 (huge) |
| `Random(cPlanet)` | any | the planet — stop unless `turn > 9` (owned or not, any population) |
| `Random(3)` | 0..=2 | the mineral |
| `Random(15) + 5` | 5..=19 | the rise in concentration, only while it is under 180 |

The owner, if any, gets message `0xfe` with the planet and the mineral — sent
whether or not the concentration was low enough to rise. No cap is applied
beyond the "under 180" gate, so a concentration of 179 can reach 198.

`game.mdSize` is the galaxy size, 0 tiny to 4 huge.

## Mystery Trader (`MysteryTrader`, `10b8:5efa`)

Nothing before turn 40 (`game.turn > 0x27`). Then the chance depends on the
year:

| year | chance |
|------|--------|
| `turn % 100 == 71` | 1 in 2 |
| `turn % 100 == 33` | 1 in 3 |
| `turn & 0x7f == 49` | 1 in 4 |
| odd | never |
| even, otherwise | 1 in 7 |

(The three special cases are checked in that order, so year 71 beats the
`& 0x7f` test even when both hold.) When the roll lands, `LpthNew` makes a
Trader `THING` and the draws fill it in:

- **warp** `Random(5) + 8`, 8 to 12, in the low four bits of the word at
  `+0xa`;
- **two places along an edge** `Random(size × 400 + 361) + 1020` — a random
  coordinate 1,020 to 1,020 + 360 + 400 × size, twenty light years inside
  each end of the galaxy's span;
- **which edges** `Random(2)`: 0 starts at 1,020 and crosses to
  `size × 400 + 1380`, 1 the other way;
- **which axis** `Random(2)`: 0 starts on the top or bottom edge (the
  random coordinate is *x*, the edge is *y*), 1 on the left or right.

Position is at `+2/+4`, destination at `+6/+8`, so a Trader always crosses
the whole galaxy to the opposite edge.

**What it carries** (`+0xe`, the part bits of `docs/formats/thing.md`):

- `chances` is 5 before turn 100, 3 before 250, 2 after; one more for a warp
  under 10, one fewer for a warp over 10.
- `Random(10) < chances`: it carries nothing in particular — except that
  `Random(6) == 0` makes it the lifeboat, `0x1000`.
- otherwise `1 << Random(13)`. If that is one of `0x40`, `0x80`, `0x400`,
  `0x800` it is drawn again, and a second result that is early for the year
  — `0x80` before turn 120, `0x400` before 150, `0x800` before 180 — is
  dropped to nothing with `Random(2) != 0`.

Every player gets message `0x12b` (299) with object `−6` (`THING_OBJECT`)
and the Trader's full id, `0x6000 | id`, as the parameter — the Exodus
game's `.m6` holds it as `[24576]`, Trader 0.

### Worked example

Turn 71 of a small galaxy (`size = 1`): the chance is one in two. A Trader
that sets out with warp 9 (`Random(5) = 1`), along-edge draws 200 and 500,
`Random(2) = 0` for the edges and `1` for the axis starts at
`(1020, 1220)` — *x* is the edge 1,020, *y* is 1,020 + 200 — bound for
`(1780, 1520)`: `1 × 400 + 1380 = 1780` and `1020 + 500`. Its carrying
chances are `5 + 1 = 6` in ten, before turn 100 with a warp under 10.

`events::tests::a_trader_sets_out_from_an_edge_for_the_opposite_one` checks
the edge geometry and the message.

## What the file layer already knew

The message ids were seen before the routines were read:
`fixtures/exodus/*.m6` carries 132 (`0x84`, a size-1 strike on somebody's
planet) and 134 (`0x86`, size 3), each with the planet as its one parameter,
and 299 with `[24576]` and object `−6`. The Trader `THING` layout is in
`docs/formats/thing.md`, and its later life — moving, being met, trading —
in `wanderers.md`.

## Open questions

- The ids `0x87..=0x8a` (a strike on *your* planet) have not been seen in a
  fixture; the wording of the routine's branch is what says they exist.
- `TossNonAutoBuildItems` is transcribed as "keep the auto-build items";
  whether it also keeps a partly-built item's progress is not checked, since
  the queue model carries progress on the item itself.
