# Mineral packets

Status: **flight and decay verified against real games**; throwing, catching,
arrival damage and Packet Physics terraforming transcribed from the binary
and unit-tested, not differentially checked.

A mineral packet is a `THING` ([`thing.md`](../formats/thing.md), `ith = 1`)
thrown from one planet's mass driver at another, carrying minerals across the
galaxy faster than any ship can. They are by far the most common object in a
real game: **95,798** of them across this repository's fixtures, against 24,193
minefields.

Model: [`stars_core::packet`](../../crates/stars-core/src/packet.rs).

## Speed

The stored `iWarp` is four bits and packets fly between warp 5 and 13, so the
field holds **the warp less four**: every routine that reads it says `iWarp + 4`
(`10b0:1d10`, `10b0:1f22`). A packet covers **the square of its warp** in light
years each year.

`MoveThings` (`10b0:18f4`) runs twice a year and packets move in both passes,
but not the same ones:

- **before production**, every packet moves a full year's distance and is
  marked as moved;
- **after production**, only the packets that have *not* moved — the ones
  thrown this year — move, and they get **half** a year.

## Decay

`FPacketDecay` (`10b8:6e9c`). The game's packet-decay setting is stored in the
packet itself, two bits of `iDecayRate`:

| setting | a year costs |
|--------:|--------------|
| 0 | nothing |
| 1 | 10% |
| 2 | 25% |
| 3 | 50% |

A **Packet Physics** player's packets lose half of that. Whatever the
percentage works out to, a packet loses at least **ten** kilotons of each
mineral it is carrying — **five** for Packet Physics — and never more than it
has; a packet with nothing left is gone. A move that covered only part of a year
decays by that part.

## Throwing

`FBuildObject` (`10b8:19b2`), the mineral-packet arm from `10b8:2496`,
implemented as [`stars_core::packet::fling`](../../crates/stars-core/src/packet.rs)
and run by the turn as the production queue finishes each packet item.

A packet item is **100 kT** of its mineral, or **40 of each** for a mixed
packet (the auto-build packet is thrown as a mixed one); a **Packet Physics**
race packs 70 and 25 instead, having paid less for them (`production.md`).
The year's count is thrown as one lot, capped at 32,760 kT.

The planet needs a **mass driver** — none, and the owner is told (`0xd1`) —
and a **destination** set (`0xd2`). The warp is the planet's setting, unless
that is under 5 or more than three over the driver's rating, when it is the
driver's own, a pair of drivers counting as one warp faster (`10b8:26aa`).
The packet's **decay code** is how far over the driver's (paired) warp it was
thrown, 0 to 3 — an Interstellar Traveller's one further, up to 3 — which is
what `FPacketDecay` reads back as none, a tenth, a quarter or a half.

A packet of the planet's own still sitting on the pad — same warp, same
target, same decay code, under 1,630 kT — takes the load instead of a new
one being made (`0xd4` against `0xd3`); a game with no room for another
object says so (`0x129`). A packet thrown this year has not moved, so the
second `MoveThings` pass gives it its half year.

`packet::tests` covers the two refusals, a hundred kilotons at the planet's
warp with a second lot joining it, the fall-back to a paired driver's warp,
and a Packet Physics race's lighter packets.

## Arrival

`10b0:1f03`, implemented in [`stars_core::packet::land`](../../crates/stars-core/src/packet.rs).
The receiving planet catches what its own mass driver can:

```
caught‰ = 1000                              if driver² >= packet²
        = driver² × 1000 / packet²          if it has a driver at all
        = 0                                 otherwise
```

where both are squared warps, and an **Inner Tech** receiver halves its driver's
square before the comparison (`10b0:2046`). Two mass drivers on one planet
count as one warp faster — `IWarpMAFromLppl` (`1048:7b10`) reports the pair
and the caller adds one (`10b0:1f55`) — and it counts **slots** rather than
drivers, so two in the same slot are not a pair. See
[`planet-pane.md`](../ui/planet-pane.md), whose Mass Driver row writes the
pair as a trailing `+`.

What the planet keeps is everything caught **plus a ninth of the rest**
(`10b0:20fd`), so even a planet with no driver keeps about 11% of what hits it.
`MANUAL.PDF` p. 25-2 says a third; the binary divides by nine.

### Damage

The rest is spent on the ground (`10b0:21e0`):

```
raw = (packet warp² − driver warp) × mass / 160
```

**The driver's warp is not squared.** The manual (p. 25-2) writes
`(spdPacket − spdReceiver) × wtPacket / 160` with both sides squared, and its
worked example (p. 25-3: a 1,000 kT warp 10 packet on a warp 5 driver) comes
to 469; the binary loads the plain warp — `[BP-0x42]`, the figure
`IWarpMAFromLppl` returned plus one for a pair — at `10b0:21f4`, where the
squared and Inner-Tech-halved figure sits in `[BP-0x40]` beside it and is
used only for the catch. The same packet does 593 here. With no driver at all
the two readings agree, which is the common case in the fixtures.

The defences take their share first: `CalcPctSurvive` (`10b0:28f9`), the
same routine a bombing run meets ([`bombing.md`](bombing.md)), and
`damage = ftol(raw × pct)`. Then, for an owned planet that is not Alternate
Reality (`10b0:2940`):

- **colonists killed** = `max(pop × damage / 1000, damage)` in hundreds
  (`10b0:2adb`), the manual's rule exactly; a planet left with nothing is
  `UninhabitPlanet`ed (`10b0:2aaf`) and its owner gets message `0xda`;
- **defences destroyed** = `defences × damage / 1000` (`10b0:2b61`) — when that
  is zero and there are defences, one with `Random(20) < damage` (`10b0:2b24`)
  — but never fewer than `damage / 20` (`10b0:2b89`) and never more than there
  are;
- a planet with nobody on it loses every defence (message `0x181`).

An unowned planet, an AR owner, or a hit whose damage rounds to nothing gets
message `0xd5` (the planet has a driver) or `0x146` (it has none); otherwise
`0xd6`/`0xd7` behind a driver and `0xd8`/`0xd9` without one, the second of
each pair when defences fell. Every one carries the planet, the mass as a
long and the thrower; the damage ones add the colonists killed and, when
there were any, the defences lost.

### Packet Physics terraforming

A packet thrown by a **Packet Physics** race terraforms the target as it lands
(`10b0:220c`), whoever owns it. For each mineral, the kilotons *not* caught are
taken a hundred at a time, and each lot gives `Random(200) < kT` — a coin's
toss for a full hundred, less for the last, smaller lot — one click on the
matching variable (ironium gravity, boranium temperature, germanium radiation);
each click won has a further `Random(10) == 0` chance of being **permanent**,
a click on the original value as well.

The clicks are then fitted to the thrower's race. The permanent ones move
the original toward the thrower's ideal and stop there; the ordinary ones
go through `FCanTerraformLppl` with `fHelp = 1` (`10b0:25fa`), so they move
toward the ideal and no further than the thrower's terraforming reach, and
nothing moves when the thrower could not terraform the planet at all. A
thrower **immune** to a variable does the opposite: half the clicks, pushing
the value *away* from the middle — down toward 1 below 50, up toward 99
from 50 — and the original the same way at full strength.

The thrower is told with `0x131`/`0x132` (a permanent change; on its own
planet, on somebody else's) and `0x133`/`0x134` (an ordinary one). The
manual's claim that Packet Physics packets do a third of the damage (p. 6-12)
is not in this routine.

What is **not** modelled: the thrower, when Packet Physics and the target
has a driver, also learns the target's starbase design (`10b0:1f7f` sets a
seen-by bit on the design); the state has no place for that.

### Worked example

The manual's own (p. 25-3): 1,000 kT of ironium at warp 10 onto a planet of
250,000 colonists (`pop = 2500`), no driver, no defences:

- caught 0‰, kept `0 + 1000 / 9 = 111‰`: **111 kT** reach the surface;
- `raw = (100 − 0) × 1000 / 160 = 625`, nothing stops it;
- killed `max(2500 × 625 / 1000, 625) = 1562` hundreds — **156,200
  colonists**, the manual's 156,250 before its rounding — leaving 938.

`packet::tests::an_uncaught_packet_kills_by_the_manuals_rule` pins this; its
neighbours cover a ninth kept on an empty world, defences falling with the
colonists, a colony wiped out, an AR owner unharmed, and a Packet Physics
packet moving a variable within a Total Terraform 3 reach.

## What is verified, and how

Every packet in the fixtures — **53,971** of them across the saves — survives a
load and a save with its position, target, warp, cargo and decay setting intact.

Better than that, the turn directories hold consecutive years of the same game,
so a packet can be **followed from one year into the next** and its flight
compared with what this engine would have done. Of **5,716** packets matched
across a year:

- **5,696 flew exactly the modelled distance** — the square of `iWarp + 4`;
- **5,695 decayed by exactly the modelled amount**, at one of two rates.

The two rates are the point of interest. 4,343 match the rate this engine
computes and another 1,352 match the *halved* Packet Physics rate — because a
player file carries only its own player's race, so for another player's packet
there is no way to know whether its owner halves decay. Both readings are
checked and one of them always fits.

The twenty-odd that fit neither are packets whose object **id was recycled**
between the two years — the same number given to a different packet, sometimes
with more minerals in it than the one before — which no matching by id can tell
apart.

## Also modelled

- **Taking from a packet.** A fleet at a packet's position may load from it
  (`MANUAL.PDF` p. 6-12): the Transport task's packet target,
  `waypoint-tasks.md`.
- **The thrower learning the target's starbase design**, as above:
  `GameState::revealed_designs`, written in full in their file that year.

## Source

- `MoveThings` `10b0:18f4` (movement, the two passes, arrival at `10b0:1f03`),
  `FPacketDecay` `10b8:6e9c`, `ThingDecay` `10b8:70c6`.
- The record: `docs/formats/thing.md`, `THPACK`.
