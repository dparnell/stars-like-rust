# Format: `.xy` — universe definition

- **Status:** header + game-info (type 7) **decoded & verified**; planet array
  **structure decoded** — whole file **round-trips byte-for-byte** across five
  different universes (128–540 planets)
- **Original files analysed:**
  - `fixtures/incoming/turn0/Game.xy` and `turn1/Game.xy` (598 bytes, 3-player,
    small universe "A Barefoot JayWalk") — an **in-game** `.xy`;
  - `fixtures/xy/{02ca32d8,across,e8dda8f7,dancing}.xy` — **standalone**
    universe-definition files of 160, 160, 360 and 540 planets (4, 5, 5 and 11
    players).
- **Encoding:** block framing + Stars! stream cipher (see `blocks.md`); the
  planet region is **plaintext-packed** (not ciphered)
- **Implemented in:** `stars-formats::xy::{Universe, PlanetPosition}`
  (`tests/real_files.rs::{turn0,turn1}_xy_universe_round_trips`,
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
| 8      | 1    | players (low5)  | player count; `3` here (4/5/5/11 in the standalone files) |
| 10     | 2    | **planet count**| number of planets in the region; **verified** `128/160/160/360/540` |
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
[ 2-byte region header ][ planet_count × 4-byte records ][ optional trailer ]
```

The **planet count is authoritative from the game-info block** (offset 10, see
above) — *not* derived from the file length, because the region may carry a
trailer. This is what lets a single parser handle every universe size:

| File                | planet_count | region bytes | header | records   | trailer |
|---------------------|-------------:|-------------:|-------:|----------:|--------:|
| `turn0/Game.xy`     | 128          | 514          | 2      | 512       | 0       |
| `across.xy`         | 160          | 644          | 2      | 640       | 2       |
| `02ca32d8.xy`       | 160          | 644          | 2      | 640       | 2       |
| `e8dda8f7.xy`       | 360          | 1444         | 2      | 1440      | 2       |
| `dancing.xy`        | 540          | 2164         | 2      | 2160      | 2       |

The region is stored **plaintext** — parsing the raw on-disk bytes yields clean
coordinates, so it is *not* run through the stream cipher.

The **trailer** is empty for an in-game `.xy` (the sample) but is a 2-byte
`u16` in the standalone universe files, and in every case **equals the player
count** (game-info offset 8: `4`, `5`, `5`, `11`). It is preserved verbatim so
all files round-trip; the hypothesis that it is the player count is recorded but
not yet surfaced as a typed field.

Each 4-byte record is a little-endian `u32`:

| Bits  | Field        | Notes                                             |
|------:|--------------|---------------------------------------------------|
| 0..9  | x coordinate | 10 bits (0..1023)                                 |
| 10..19| y coordinate | 10 bits (0..1023)                                 |
| 20..31| name index   | 12 bits; index into the planet-name table         |

**Evidence:** with the count taken from the game-info block and a **2-byte**
region header, *every* file's records decode to coordinates cleanly bounded
within the 10-bit field with **no two planets sharing a position** (a physical
invariant). Shifting the alignment by ±2 bytes (a 0- or 4-byte header) instead
produces overlapping/garbage positions on all files, so the 2-byte header is
confirmed across five independent universes. The whole `.xy` file then
**re-encodes byte-for-byte** via `Universe::encode`.

**Caveats (still open):**

- The **axis assignment** (which 10-bit field is x vs y) follows the community
  convention and is not independently confirmed — it does not affect
  byte-accuracy. (The fleet-anchor bytes in the `.hst`, `a4 05 1c 06`, turned
  out **not** to be a position, so there is no external coordinate oracle yet.)
- The **2-byte region header** (varies per file: `0a 14`, `0f 34`, `0a e0`,
  `0d e8`, `0c 0c`) is preserved verbatim; its meaning (a count, seed, or flags)
  is not yet known.
- The **name index → text** mapping (the planet-name table) is not yet decoded.

### Next steps

- Confirm the x/y axis order and decode the name-index table (from a save whose
  planet names are visible, or the `STARS!.EXE` name table in Ghidra).
- Identify the 2-byte region header's meaning.
- Confirm the trailer really is the player count (a standalone `.xy` whose player
  count differs from any coincidental byte would settle it).

## Derived test vectors

- `Game.xy`: `game_id=0x2a031dd8`, size=1, density=1, players=3, planets=128,
  name=`"A Barefoot JayWalk"` (asserted in `tests/real_files.rs`).
- `fixtures/xy/*.xy`: planet counts `160/160/360/540` (from game-info offset 10)
  each round-trip byte-for-byte, with a 2-byte trailer equal to the player count
  (asserted in `tests/real_files.rs::xy_dir_universes_round_trip`).
