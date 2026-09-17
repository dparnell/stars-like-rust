# Stargates

Status: **transcribed and unit-tested** — the fixtures hold no jump to check it
against: no fleet in the corpus has a leg at warp 11, and a jump leaves no
record of its own beyond the messages.

- **Ghidra routine(s):** `10b0:354f`–`3d0d` (the stargate branch of
  `MoveFleets`), `1080:0cfe` `FStargateJump`, `1080:152e`
  `MdCalcStargateDamage`, `IStargateFromLppl` and `FFleetCanJumpgate`
  (`util.c`), `1038:75e2` `FCanFleetUseStargates`, `1080:1e52`
  `AutoRouteFleet`
- **Manual reference:** the player's guide's *Stargate Navigation* and
  *Stargates* topics (`STARS!.HLP`, contexts `0x1b0b00` and `0xc885e`), and
  *Interstellar Traveler* (`0x3586f7`)
- **Uses RNG:** yes — one `Random(100)` per ship past the limits, and a
  `Random(500)` for each of those that is lost
- **Implemented in:** `crates/stars-core/src/stargate.rs`, the stargate leg
  in `crates/stars-core/src/turn.rs`

## The order

A waypoint's warp nibble holds 0 to 10 for a flown leg and **11**
(`iWarpStargate`) for a jump. The Fleet Waypoints tile's warp gauge runs to 11
(`ClickInShipOrders`, `1050:7cda`, sets its maximum at `0xb`) and draws that
position as *Use Stargate* (`idsUseStargate`). The client suggests it itself
(`IWarpBestForWaypoint`, `1058:7a18`, last thing) when `FCanFleetUseStargates`
answers exactly `1`: both planets the player's own with a gate on the starbase,
nothing in the hold unless the race is Interstellar Traveler, and every design
judged undamaged by the table below. A friend's gate is an *uncertain* answer
there and is not offered, though the jump itself is allowed.

`AutoRouteFleet` does the same for a ship leaving the yard along a planet's
route: warp 11 when both ends are the owner's, gated, the fleet empty and its
heaviest design's jump clean.

## The gates

A gate is one of the first seven entries of `rgspecialSB` (`1008:4d8a`), fitted
to an orbital slot of the starbase's design; `IStargateFromLppl` takes the first
such slot. Each carries two ratings — `grAbility`, the mass limit in kT, and
`grAbility2`, the range in light years, `-1` for unlimited:

| Index | Gate | Mass (kT) | Range (ly) |
|------:|------|----------:|-----------:|
| 0 | Stargate 100/250 | 100 | 250 |
| 1 | Stargate any/300 | any | 300 |
| 2 | Stargate 150/600 | 150 | 600 |
| 3 | Stargate 300/500 | 300 | 500 |
| 4 | Stargate 100/any | 100 | any |
| 5 | Stargate any/800 | any | 800 |
| 6 | Stargate any/any | any | any |

Source: the words at `+0x34` and `+0x36` of each 56-byte entry, read from the
binary; `crates/stars-core/src/stargate.rs` carries the ranges as `RANGE`
because the other specials share a 54-byte struct with no second rating.

## What is checked before the jump (`MoveFleets`, `10b0:354f`)

In order, each failure sending one message and leaving the fleet where it is:

1. The fleet's current waypoint names a planet with a gate — else, if every
   design carries a **Jump Gate** (`ispecialMJumpGate`, index 9 of
   `rgspecialM`), no source gate is needed; else `0xde`
   (`idmAttemptedUseStargateStargateExistsThere`).
2. The source planet is the player's own or a **friend's** (`rgmdRelation ==
   1`); else `0xe6`.
3. The leg ends at a planet — the one it names, or the one standing at its
   coordinates — with a gate; else `0xe2`.
4. That planet is the player's own or a friend's; else `0xe5`.
5. Without a source gate, the destination gate stands in for it
   (`fSrcOnly`), and the cargo rule below is skipped.
6. Unless the race is Interstellar Traveler: colonists aboard at a planet not
   the player's own refuse the jump (`0x15e`); otherwise every mineral and
   colonist aboard is **put down on the source planet** and the player told
   (`0xec` minerals, `0xed` colonists, `0xee` both — and the planet's owner
   too, when that is somebody else).

The distance is `DGetDistance` between the two waypoints, truncated to a whole
light year. A jump crosses no minefield and burns no fuel.

## The damage (`MdCalcStargateDamage`, `1080:152e`)

For a ship of mass `wt` (the design's **empty** mass, `SHDEF.hul.wtEmpty` — cargo
never counts, which is the Interstellar Traveler's advantage), sent `d` light
years from gate `src` to gate `dst`:

```
range = src.range, or 10000 when unlimited
if d > range × 5                                     → refused: too far    (0xe3)
if src.mass > 0 and wt > src.mass × 5                → refused: too massive (0xe4)
if dst.mass > 0 and wt > dst.mass × 5                → refused: too massive (0xe4)
survive = 10000
if d > range:          survive  = (range × 5 − d) × 2500 / range;  < 1 → lost
if 0 < src.mass < wt:  survive *= (src.mass × 5 − wt) × 2500 / src.mass / 10000; part < 1 → lost
if 0 < dst.mass < wt:  survive *= (dst.mass × 5 − wt) × 2500 / dst.mass / 10000; part < 1 → lost
damage% = (10000 − survive) / 100
```

Only the **source** gate's range counts; both gates' mass limits do. Each cut is
linear from nothing at the limit to everything at five times it, and the cuts
multiply.

Worked examples on the 100/250 gate (test vectors in
`crates/stars-core/tests/stargates.rs`):

| d | wt | survive | damage |
|--:|---:|--------:|-------:|
| 250 | 100 | 10000 | 0% |
| 500 | 100 | (1250 − 500) × 2500 / 250 = 7500 | 25% |
| 100 | 200 | 7500 × 7500 / 10000 = 5625 | 43% |
| 500 | 200 | 5625 × 7500 / 10000 = 4218 | 57% |
| 1250 | 100 | 0 | lost |
| 1251 | 100 | — | refused |

## The jump (`FStargateJump`, `1080:0cfe`)

Every design in the fleet is judged first; one refusal refuses the fleet. A
design judged *lost* loses every ship. If no design is judged otherwise the
fleet is gone — `fDead`, message `0xe7` — and the same if the losses below
empty it.

For a design with damage `pct` in `1..=99`:

- **losses**: each ship rolls `Random(100) < pct / 3` to be lost — never for an
  Interstellar Traveler, whose `pctKill` is 0. A lost ship that belonged to the
  already-damaged part of the stack (`pctSh` of the count, at least one) leaves
  that part with `Random(500) < pctDp`;
- **damage**: the design's armour `dp` gives `old = dp × pctDp / 500` (at least
  1 when any) and `new = dp × pct / 100` (at least 1). If `dp ≤ new + old` the
  already-damaged ships are finished too. The survivors' damage becomes
  `((old × damaged + new × left) / left) × 500 / dp`, at least 1, and all of
  them are now damaged (`pctSh = 100`);
- the fleet is marked `fNoHeal` for the year.

The player hears of any loss: `0xe8` for under a quarter of the ships that set
out, `0xe9` up to half, `0xea` beyond, `0xeb` when the count needs a long. The
lost ships' share of the cargo goes with them (`FleetTransferCargoBalance`).

On arrival the fleet stands at the far planet with its leg consumed, has not
been "here all turn", and a fleet of another player chasing it is pointed at
where the gate was rather than followed through (`NoAutoTrackFleet`): this
engine rewrites that chaser's leg to the spot as a point in space, where the
original sets `fNoAutoTrack` on the waypoint and keeps the fleet as its object.

## Open questions

- Nothing in the fixtures jumps, so none of this is checked against the
  original's arithmetic beyond the transcription.
- The chaser's `fNoAutoTrack` flag is not carried on this engine's waypoint
  (see above).
