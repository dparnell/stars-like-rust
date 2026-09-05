# Wormholes and the Mystery Trader

Status: **carried, moved and checked as far as the fixtures allow.** Wormhole
traversal and the Trader's arrival and trading are not modelled.

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

## The Mystery Trader

The Trader crosses the galaxy at speed, and one year in twenty-five it changes
its mind (`10b0:1af7`): it always **speeds up** by a warp factor, and one time
in three it also picks a new destination somewhere on the **edge** of the map.
Then it covers the square of its warp toward wherever it is going.

Meeting it is the point of it — a fleet that intercepts trades minerals for
technology (`IdmGiveTraderPart`, `1110:1a96`) — and none of that is modelled.

## What is verified

- **2,672 wormhole ends and 589 Traders** survive a load and a save unchanged.
- **1,764 wormhole ends have both halves in view and every one of those pairs
  is mutual** — each end names the other, and no end names itself. The rest have
  only one end visible, which is exactly what a wormhole nobody has been through
  looks like.
- **527 of 547 Trader-years** flew the modelled distance: the square of its
  warp, before or after the speed-up.

The twenty that did not are the Trader's own doing. In each, its warp went
**down** and its destination changed — and the course change only ever speeds it
up, so those are not the same flight continuing but a new pass beginning. The
Trader's arrival behaviour, which the original handles with a "decided to make
another pass" message, is not modelled.

Movement itself cannot be checked exactly for wormholes: where one jumps is a
hundred dice rolls deep, and reproducing it would need the original's random
stream in the same state.

## Not modelled

- **Going through a wormhole.** A fleet can be sent to one, but it comes out
  nowhere: the far end is not applied.
- **The Trader's arrival**, its departure, and trading with it.
- Wormhole **visibility**: who can see an end is carried through a file and
  cleared on a jump, but not recomputed from anybody's scanners.

## Source

- `MoveThings` `10b0:18f4` — the wormhole arm at `10b0:194c`, the Trader's at
  `10b0:1af7`.
- `PctWormholeMoves` `1110:0adc`, `IValidateWormholePos` `1110:064c`.
- The records: `docs/formats/thing.md`, `THWORM` and `THTRADER`.
