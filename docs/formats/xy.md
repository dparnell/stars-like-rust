# Format: `.xy` — universe definition

- **Status:** header + game-info (type 7) **decoded & verified**; planet array
  **fully decoded** — absolute coordinates + planet **names** recovered, and the
  whole file **round-trips byte-for-byte** across six different universes
  (24–540 planets) from two independent games
- **Original files analysed:**
  - `fixtures/incoming/turn0/Game.xy` and `turn1/Game.xy` (598 bytes, 3-player,
    small universe "A Barefoot JayWalk") — an **in-game** `.xy`;
  - `fixtures/games/tutorial/tutorial.xy` (182 bytes, 2-player, 24-planet
    "Tutorial Game") — a second, **in-game** `.xy` from a different game;
  - `fixtures/xy/{02ca32d8,across,e8dda8f7,dancing}.xy` — **standalone**
    universe-definition files of 160, 160, 360 and 540 planets (4, 5, 5 and 11
    players).
- **Encoding:** block framing + Stars! stream cipher (see `blocks.md`); the
  planet region is **plaintext-packed** (not ciphered)
- **Implemented in:** `stars-formats::xy::{Universe, PlanetPosition, Planet}`
  and the master name table `stars-formats::names`
  (`tests/real_files.rs::{turn0,turn1}_xy_universe_round_trips`,
  `tests/real_files.rs::tutorial_xy_universe_round_trips`,
  `tests/real_files.rs::xy_dir_universes_round_trip`)

## Overview

The `.xy` file is the **shared universe definition**: game parameters, victory
conditions, and the fixed positions/ids of every planet. Unlike the other
formats it is **not** a clean sequence of blocks to EOF — after the plaintext
header and the encrypted game-info block, the remainder is a planet region that
does **not** re-frame as blocks.

## Top-level layout

| Order | Block                       | Notes                                    |
|------:|-----------------------------|------------------------------------------|
| 1     | FileHeaderBlock (type 8)    | plaintext; see `blocks.md` (file_type=0) |
| 2     | PlanetsBlock (type 7)       | encrypted; **game info**, not planets    |
| 3     | planet region (raw)         | **not standard block framing** — see below |

Because of region 3, `StarsFile::decode` returns an error for `.xy`; the
dedicated `xy::Universe` parser handles all three parts and re-encodes the file
byte-for-byte.

## Game-info block (type 7) — decrypted payload

Offsets are into the **decrypted** 64-byte payload (from `decryptGameInfo` in
`StarsBlock.pm`, confirmed against `Game.xy`):

| Offset | Size | Field           | Notes                                        |
|-------:|-----:|-----------------|----------------------------------------------|
| 4      | 2    | `mdSize`        | universe size class (0=tiny … 4=huge); `1` here |
| 6      | 2    | `mdDensity`     | planet density; `1` here                      |
| 8      | 1    | players (low5)  | player count; `3` here (2 tutorial; 4/5/5/11 in the standalone files) |
| 10     | 2    | **planet count**| number of planets in the region; **verified** `24/128/160/160/360/540` |
| 12     | 2    | `mdStartDest`   | starting-distance / clumping param            |
| 16     | 2    | `wCrap`         | bit-flags (see below)                         |
| 20     | 12   | `rgvc[0..12]`   | victory conditions (per-condition byte)       |
| 32     | 32   | game name       | NUL-padded ASCII; `"A Barefoot JayWalk"`      |

`wCrap` bit-flags: `0 ExtraFuel`, `1 SlowTech`, `4 AIsBand`, `5 BBSPlay`,
`6 VisibleScores`, `7 NoRandomEvents`, `8 Clumping` (bits 9..11 = generator seed
`wGen`; 12..15 unused).

Victory conditions (`rgvc[i]`: bit 7 = active, low 7 bits = value, decoded per
index): `0 PlanetControl`, `1 TechLevel`, `2 TechFields`, `3 Score`,
`4 ScoreExcess`, `5 Production`, `6 CapitalShips`, `7 HighScoreAt`,
`8 MustMeet`, `9 LeastYears`.

## Planet region (decoded)

After the game-info block the region is laid out as:

```text
[ planet_count × 4-byte records ][ trailer ]
```

There is **no** leading region header — records begin immediately after the
game-info block. The **planet count is authoritative from the game-info block**
(offset 10, see above), *not* derived from the file length, because the region
always carries a trailer. This is what lets a single parser handle every
universe size:

| File                | planet_count | region bytes | records | trailer |
|---------------------|-------------:|-------------:|--------:|--------:|
| `tutorial.xy`       | 24           | 98           | 96      | 2       |
| `turn0/Game.xy`     | 128          | 514          | 512     | 2       |
| `across.xy`         | 160          | 644          | 640     | 4       |
| `02ca32d8.xy`       | 160          | 644          | 640     | 4       |
| `e8dda8f7.xy`       | 360          | 1444         | 1440    | 4       |
| `dancing.xy`        | 540          | 2164         | 2160    | 4       |

The region is stored **plaintext** — parsing the raw on-disk bytes yields clean
coordinates, so it is *not* run through the stream cipher.

The **trailer** is a 2-byte `00 00` for an **in-game** `.xy` (both the
`incoming/` sample and the tutorial game confirm this across two independent
games), and a 4-byte `02 00 <players> 00` for a **standalone**
universe-definition file (the constant `02 00` followed by the player count as a
`u16`: `4`, `5`, `5`, `11`). It is preserved verbatim so all files round-trip.

Each 4-byte record is a little-endian `u32`, matching the community
`struct position { unsigned xoffset:10; unsigned y:12; unsigned nameid:10; }`:

| Bits  | Field       | Notes                                                |
|------:|-------------|------------------------------------------------------|
| 0..9  | `xoffset`   | 10 bits; **delta** added to a running x total         |
| 10..21| `y`         | 12 bits; **absolute** y coordinate                    |
| 22..31| `nameid`    | 10 bits; index into the master planet-name table      |

**x is a running sum.** A planet's absolute x is the sum of all `xoffset`s up to
and including its record, so planets are stored in **non-decreasing x** order
(the original tools require x to never decrease planet-to-planet). This is why
`xoffset`s are small (deltas), and `Universe::planets_resolved` reconstructs the
absolute `(x, y)` and the resolved name for each planet.

**Evidence:** with records starting immediately after the game-info block and
the count taken from that block, *every* file decodes so that (1) each `nameid`
is a **unique** index `< 999` that resolves in the master name table, (2)
running-sum x gives **no two planets sharing a position**, and (3) the x/y
spans scale with the universe size class (tiny tutorial ≈ 360, small `Game` ≈
780, large `e8dda8f7`/`dancing` ≈ 1180). Any other record alignment breaks the
name uniqueness or the position uniqueness. The whole `.xy` file then
**re-encodes byte-for-byte** via `Universe::encode`.

## Planet-name table

Each planet's `nameid` (10 bits, `0..=998`) indexes a fixed master list of 999
planet names that ships inside `STARS!.EXE`; the save files store only the
index. The table is embedded in the crate at
`crates/stars-formats/data/star-names.txt` (recovered via the Map2XY tool's
`planets.txt`, whose README notes the names "really come from Stars!.exe") and
exposed through `stars-formats::names::planet_name`. Every planet in all six
sample universes resolves to a **unique** entry, e.g. tutorial planet 0 →
`"Lever"`, planet 23 → `"Bloop"`.

**Caveats (still open):**

- The **axis assignment** (which coordinate is x vs y) follows the community
  `struct position` convention and is not independently confirmed — it does not
  affect byte-accuracy.
- The absolute-x **origin** is taken as 0 before the first `xoffset`; whether
  there is an implicit base offset (y minima sit near ~1000) is not confirmed,
  but it does not affect round-tripping.
- The standalone trailer's constant `02 00` prefix is preserved verbatim; its
  meaning is not yet known.

### Next steps

- Confirm the x/y axis order and any absolute-coordinate origin against an
  in-game screen or the `STARS!.EXE` layout in Ghidra.
- Identify the standalone trailer's `02 00` prefix.

## Derived test vectors

- `Game.xy`: `game_id=0x2a031dd8`, size=1, density=1, players=3, planets=128,
  name=`"A Barefoot JayWalk"`; all 128 planets resolve to unique names and
  positions, 2-byte `00 00` trailer (asserted in `tests/real_files.rs`).
- `fixtures/xy/*.xy`: planet counts `160/160/360/540` (from game-info offset 10)
  each round-trip byte-for-byte, with a 4-byte `02 00 <players> 00` trailer
  (asserted in `tests/real_files.rs::xy_dir_universes_round_trip`).
- `tutorial.xy`: `game_id=0x008cef49`, players=2, planets=24,
  name=`"Tutorial Game"`, 2-byte `00 00` trailer (a second in-game `.xy`);
  planet 0 → `"Lever"`, planet 23 → `"Bloop"`; round-trips byte-for-byte
  (asserted in `tests/real_files.rs::tutorial_xy_universe_round_trips`).
