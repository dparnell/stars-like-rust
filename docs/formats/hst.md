# Format: `.hst` / `.mN` — host & player state (block inventory)

- **Status:** container **verified byte-for-byte**; **block inventory decoded**;
  planet-id word **decoded & verified**; per-record field layouts (planet
  attributes, players, fleets, designs) **in progress** (hypotheses below)
- **Original files analysed:** `fixtures/incoming/turn0/Game.{hst,m1,m2,m3}` and
  the matching `turn1/` set (3-player game "A Barefoot JayWalk", 128 planets)
- **Encoding:** standard Stars! container (see `blocks.md`)
- **Implemented in:** `stars-formats::{file, block, records}`
  (`StarsFile::block_counts`, `records::planet_headers`); tests in
  `tests/real_files.rs::hst_block_inventory_and_planets`

## Overview

The **host** file (`.hst`) holds the full, authoritative game state; a **player**
file (`.mN`) holds one player's view of it. Both are ordinary block-framed
Stars! files and share the same block vocabulary — the `.hst` is essentially the
superset (all players, all planets) and each `.mN` is a filtered projection.

## `.hst` block inventory (sample game, turn 0)

Decoded via `StarsFile::block_counts`. In file order the blocks are:

| Block (type)                    | Count | Notes                                         |
|---------------------------------|------:|-----------------------------------------------|
| `FileHeader` (8)                | 1     | plaintext; seeds the cipher                   |
| `Player` (6)                    | 3     | one full player record per player             |
| `Planet` (13)                   | 128   | one per planet; ids `0..=127` (see below)     |
| `Design` (26)                   | 15    | ship/starbase designs (all players)           |
| `Fleet` (16) + `Waypoint` (20)  | 14+14 | starting fleets, each followed by a waypoint  |
| `BattlePlan` (30)               | ~15   | per-player battle plans                       |
| `Object` (43)                   | 5     | minefields / packets / other map objects      |
| `FileFooter` (0)                | 1     | plaintext; 2 bytes (year/checksum — TBD)      |

`.mN` files have the same shape but only the owning player's `Player` block and a
much smaller planet/fleet set (only what that player has seen/owns).

## Planet record (`Planet`, type 13)

Each planet block begins with a little-endian **16-bit header word**:

```
bits 0..9   planet id   (0-based; 0..=1023)
bits 10..15 flags/mask   (field-presence)
```

**Verified:** the 128 planet blocks of `Game.hst` decode to the contiguous ids
`0..=127`. Minimal (undiscovered) planets use flags `62` (word `0xF800 | id`) and
an **11-byte** payload; the three **homeworlds** use a longer **33-byte** payload
with a different mask (`0`, `2`, `4` observed). `records::PlanetHeader` exposes
the id, the flags byte, and the payload length; `is_extended()` distinguishes
inhabited planets (payload > 11 bytes).

### Minimal 11-byte record — column analysis (hypothesis)

Column ranges across the 125 minimal planet blocks (offsets into the decrypted
payload):

| Offset | Observed        | Hypothesis                                   |
|-------:|-----------------|----------------------------------------------|
| 0–1    | `0xF800 \| id`  | **id word** (verified)                       |
| 2      | `0x07` constant | part of the flags/mask framing               |
| 3      | `0x01` / `0x11` | a per-planet flag bit (bit 4)                |
| 4      | `0x00` constant | —                                            |
| 5,6,7  | 1..≈119         | **mineral concentrations** (ironium/boranium/germanium); exceed 100 |
| 8,9,10 | 4..≈97          | **environment** (gravity/temperature/radiation); capped ≤ 100 |

The mineral/environment split is inferred from the value ranges (concentrations
can exceed 100, environment clicks cannot) and is **not yet confirmed** against
the manual or a second sample, so it is documented here but **not** surfaced as
typed fields in `records`. Note the 11-byte record carries **no coordinates** —
planet x/y live in the `.xy` planet region (see `xy.md`).

## Fleet record (`Fleet`, type 16) — partial

The starting fleet blocks (22–23 bytes) share a common 8-byte signature
`07 09 45 00 a4 05 1c 06` across **all** fleets of **all** players, so that run
is **not** a position (fleets sit at three different homeworlds). The leading and
trailing bytes vary per fleet (fleet id + a design/cargo reference). Full fleet
layout — including where the fleet's planet/coord anchor is stored — is still to
be recovered.

## Open questions / next

- Confirm the planet mineral-vs-environment byte split (needs the manual's
  homeworld values or a second, different sample game).
- Decode the extended 33-byte planet record (population, factories, mines,
  starbase design ref, defense).
- Decode the `Player` (6) record fully (it shares a layout with the `.rN` race
  record — see `race-r.md` — plus per-game state).
- Decode `Fleet` (16) / `Waypoint` (20) / `Design` (26) records.
- Footer (type 0) 2-byte contents (year vs checksum).

## Derived test vectors

- `tests/real_files.rs::hst_block_inventory_and_planets` — asserts 3 players,
  128 planets, contiguous ids `0..=127`, and exactly 3 inhabited planets.
