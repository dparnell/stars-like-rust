# Mineral packets

Status: **flight and decay verified against real games**; catching and arrival
transcribed but not differentially checked; launching not modelled.

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

## Arrival

`10b0:1f03`. The receiving planet catches what its own mass driver can:

```
caught‰ = 1000                              if driver² >= packet²
        = driver² × 1000 / packet²          if it has a driver at all
        = 0                                 otherwise
```

where both are squared warps, and an **Inner Tech** receiver halves its driver's
square before the comparison. Two mass drivers on one planet count as one warp
faster.

What the planet keeps is everything caught **plus a ninth of the rest**
(`10b0:20fd`), so even a planet with no driver keeps about 11% of what hits it.
The rest is spent on the ground: damage of

```
(packet warp² − driver warp²) × mass / 160
```

which kills colonists and defences. A **Packet Physics** sender terraforms the
target with the share that was not caught, rolling per hundred kilotons.

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

## Not modelled

- **Launching.** A planet's production queue can build packets, and a mass
  driver throws them; neither is wired up, so packets only exist in a game that
  was loaded with them.
- **Catching.** A planet's mass driver is not modelled either, so an arriving
  packet is treated as uncaught: the planet keeps its ninth and takes the
  damage. That is the right answer for an unowned target and the worst case for
  an owned one.
- **The damage itself** is computed and reported but not yet applied to
  colonists and defences, and the Packet Physics terraforming on arrival is not
  performed.

## Source

- `MoveThings` `10b0:18f4` (movement, the two passes, arrival at `10b0:1f03`),
  `FPacketDecay` `10b8:6e9c`, `ThingDecay` `10b8:70c6`.
- The record: `docs/formats/thing.md`, `THPACK`.
