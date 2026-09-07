# Format: `.hst` / `.mN` — host & player state (block inventory)

- **Status:** container **verified byte-for-byte**; **block inventory decoded**;
  the **player record** (see `player.md`), **planet record** (see `planet.md`),
  **fleet record** (see `fleet.md`), **design record** (see `design.md`),
  **waypoint** (see `waypoint.md`), **battle plan** (see `battleplan.md`),
  **production queue** (see `production.md`), **player-scores** (see
  `score.md`) and **space objects** (see `thing.md`) records are fully decoded &
  verified; the only remaining per-record layout is the **message** record
  (type 12), whose text needs the message-template catalogue (deferred to the
  simulation phase)
- **Original files analysed:** `fixtures/incoming/turn0/Game.{hst,m1,m2,m3}` and
  the matching `turn1/` set (3-player game "A Barefoot JayWalk", 128 planets)
- **Encoding:** standard Stars! container (see `blocks.md`)
- **Implemented in:** `stars-formats::{file, block, records, player, planet, fleet, waypoint, design, battleplan, production, score}`
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
| `ChangePassword` (36)           | 0..1  | the **host's** password salt; only when one is set (see below) |
| `Planet` (13)                   | 128   | one per planet; ids `0..=127` (see below)     |
| `Design` (26)                   | 15    | ship/starbase designs (all players)           |
| `Fleet` (16) + `Waypoint` (20)  | 14+14 | starting fleets, each followed by a waypoint  |
| `BattlePlan` (30)               | 15    | 5 per player (see `battleplan.md`)            |
| `ProductionQueue` (28)          | 0+    | planet build lists after turn 1 (`production.md`) |
| `Object` (43)                   | 5     | 1 count record + 4 `THING`s (fresh game: 4 wormholes) — `thing.md` |
| `FileFooter` (0)                | 1     | plaintext; 2 bytes (year/checksum — TBD)      |

`.mN` files have the same shape but only the owning player's `Player` block and a
much smaller planet/fleet set (only what that player has seen/owns).

## The host's password (`ChangePassword`, type 36)

Four bytes, little-endian: the salt of the host's password, the same value and
the same `LSaltFromSz` fold a player's password uses (`../ui/change-password.md`).

It is **written only in a host file and only when there is a password**, in the
one position the loader looks for it — straight after the last `Player` block:

```c
/* save.c, WriteDataFile */
for (i = 0; i < game.cPlayer; i++) ... WriteRtPlr(&rgplr[i], NULL);
if (iPlayer == iNoPlayer && lSaltCur != 0)
    WriteRt(rtChgPassword, 4, &lSaltCur);
```

```c
/* file.c, the loader */
if (dt != dtHost)                       lSaltCur = rgplr[iPlayer].lSalt;
else if (hdrCur.rt == rtChgPassword)  { lSaltCur = *(int32_t *)rgbCur; ReadRt(); }
else                                    lSaltCur = 0;
```

So a host file with no password carries no such block, which is why **no fixture
in this repository has one** — none of those games was hosted with a password.
The block type is the same number as the `rtChgPassword` order operation, which
is a different thing in a different file: there it is a *player* changing their
own turn password (`orders-x.md`).

`stars_core::GameState::host_password` carries it, and the writer puts it back
in the same place.

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

- Decode the `Message` (`rtMsg`, type 12) record body — needs the message-id →
  text-template catalogue (belongs with the simulation core).
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
