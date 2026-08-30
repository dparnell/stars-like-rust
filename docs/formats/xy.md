# Format: `.xy` — universe definition

- **Status:** header + game-info (type 7) **decoded & verified**; planet array
  **structure decoded** — whole file **round-trips byte-for-byte**
- **Original files analysed:** `fixtures/incoming/turn0/Game.xy` and
  `turn1/Game.xy` (598 bytes, 3-player, small universe "A Barefoot JayWalk")
- **Encoding:** block framing + Stars! stream cipher (see `blocks.md`); the
  planet region is **plaintext-packed** (not ciphered)
- **Implemented in:** `stars-formats::xy::{Universe, PlanetPosition}`
  (`tests/real_files.rs::{turn0,turn1}_xy_universe_round_trips`)

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
| 8      | 1    | players (low5)  | player count; `3` here                        |
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

After the game-info block (offset `0x54` in `Game.xy`) **514 bytes** remain,
laid out as a **2-byte region header** followed by **128 × 4-byte planet
records** (`514 = 2 + 128 × 4`; 128 matches the planet count from the `.hst`).
The region is stored **plaintext** — parsing the raw on-disk bytes yields clean
coordinates, so it is *not* run through the stream cipher.

Each 4-byte record is a little-endian `u32`:

| Bits  | Field        | Notes                                             |
|------:|--------------|---------------------------------------------------|
| 0..9  | x coordinate | 10 bits (0..1023)                                 |
| 10..19| y coordinate | 10 bits (0..1023)                                 |
| 20..31| name index   | 12 bits; index into the planet-name table         |

**Evidence:** at this offset all 128 records decode to coordinates cleanly
bounded within the 10-bit field (`x∈[15,987]`, `y∈[1,945]`) with **no two
planets sharing a position**; at any other offset the values overflow/overlap.
The whole `.xy` file then **re-encodes byte-for-byte** via `Universe::encode`.

**Caveats (still open):**

- The **axis assignment** (which 10-bit field is x vs y) follows the community
  convention and is not independently confirmed — it does not affect
  byte-accuracy. (The fleet-anchor bytes in the `.hst`, `a4 05 1c 06`, turned
  out **not** to be a position, so there is no external coordinate oracle yet.)
- The **2-byte region header** (`0a 14` here) is preserved verbatim; its meaning
  (a count, seed, or flags) is not yet known.
- The **name index → text** mapping (the planet-name table) is not yet decoded.

### Next steps

- Confirm the x/y axis order and decode the name-index table (from a save whose
  planet names are visible, or the `STARS!.EXE` name table in Ghidra).
- Identify the 2-byte region header's meaning.

## Derived test vectors

- `Game.xy`: `game_id=0x2a031dd8`, size=1, density=1, players=3,
  name=`"A Barefoot JayWalk"` (asserted in `tests/real_files.rs`).
