# Format: `.hst` / `.mN` — host & player state (block inventory)

- **Status:** container **verified byte-for-byte**; **block inventory decoded**;
  the **player record** (see `player.md`), **planet record** (see `planet.md`),
  **fleet record** (see `fleet.md`) and **design record** (see `design.md`) are
  fully decoded & verified; the remaining per-record layouts (waypoints, events,
  battle plans) are **in progress**
- **Original files analysed:** `fixtures/incoming/turn0/Game.{hst,m1,m2,m3}` and
  the matching `turn1/` set (3-player game "A Barefoot JayWalk", 128 planets)
- **Encoding:** standard Stars! container (see `blocks.md`)
- **Implemented in:** `stars-formats::{file, block, records, player, planet, fleet, design}`
  (`StarsFile::block_counts`, `records::planet_headers`,
  `player::player_records`, `planet::planet_records`, `fleet::fleet_records`,
  `design::design_records`); tests in
  `tests/real_files.rs::hst_block_inventory_and_planets`,
  `tests/player_files.rs`, `tests/planet_files.rs`, `tests/fleet_files.rs` and
  `tests/design_files.rs`

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

**Fully decoded — see [`planet.md`](planet.md)** for the complete layout. The
first 16-bit word is `id` (low 11 bits) + `owner` (high 5 bits, `31` = unowned);
the second word is `det` + flag bits; then a sequence of presence-gated sections
(concentrations, environment, owner guesses, surface minerals, population,
installations, starbase, route).

**Verified against `Game.hst`:** the 128 planet blocks decode to the contiguous
ids `0..=127`; exactly three are owned (players 0/1/2), each a homeworld with
population 25,000, 10 mines/factories/defenses, a starbase, and (for the
Humanoid player) the centred environment 50/50/50. The 125 unowned planets are
the 11-byte record `header(4) + decay-bitmask(1) + concentration(3) +
environment(3)`.

> The earlier id(10)/flags(6) reading of the first word was **wrong** — the
> verified split is id(11)/owner(5). `records::PlanetHeader` now exposes `id` +
> `owner`; the full field-by-field view is `planet::PlanetRecord`.

## Fleet record (`Fleet`, type 16)

**Fully decoded — see [`fleet.md`](fleet.md).** The 14-byte header is
`id`(9)+`owner`(4) word, an ignored redundant player word, a `det`+flags byte
pair, the orbited planet id, the x/y position, and a ship-design bitmask; then
ship counts, an optional cargo hold, and full/partial tails.

**Verified against `Game.hst`:** the 14 starting fleets (6/4/4 per player) each
orbit their owner's homeworld (planets 69/112/32) with one ship and a fuel
supply. The earlier "8-byte signature" hypothesis was wrong — that run is just
the flags/orbit/position of fleets sharing a homeworld.

## Open questions / next

- Decode the `Player` (6) record fully (it shares a layout with the `.rN` race
  record — see `race-r.md` — plus per-game state: homeworld, tech levels,
  resources, relations).
- Decode `Waypoint` (19/20), `FleetName` (21) and `Design` (26) records.
- Footer (type 0) 2-byte contents (year vs checksum).

## Derived test vectors

- `tests/real_files.rs::hst_block_inventory_and_planets` — asserts 3 players,
  128 planets, contiguous ids `0..=127`, and exactly 3 inhabited planets.
- `tests/planet_files.rs::hst_planet_records_decode_homeworlds` — asserts the
  full decoded homeworld state (25,000 pop, 10/10/10 installations, starbase,
  Humanoid 50/50/50 environment).
- `tests/planet_files.rs::tutorial_hst_planet_records_decode` — 24 planets, 2
  homeworlds decode cleanly in a second, independent game.
- `tests/fleet_files.rs::hst_fleet_records_decode_starting_fleets` — 14 fleets
  (6/4/4 per player), each orbiting its owner's homeworld with one fuelled ship.
