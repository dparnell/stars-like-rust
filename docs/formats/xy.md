# Format: `.xy` — universe definition

- **Status:** header + game-info (type 7) **decoded & verified**; planet array
  **pending**
- **Original files analysed:** `fixtures/incoming/Game.xy` (598 bytes, 3-player,
  small universe "A Barefoot JayWalk")
- **Encoding:** block framing + Stars! stream cipher (see `blocks.md`)
- **Implemented in:** header/cipher via `stars-formats`; planet decoding TBD

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

Because of block 3, `StarsFile::decode` currently returns an error for `.xy`
(framing hits an impossible block near offset `0xFE`). The header and game-info
are decoded directly in `tests/real_files.rs::xy_header_and_game_info_decode`.

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

## Planet region (open)

After the game-info block (offset `0x54` in `Game.xy`) 514 bytes remain that:

- do **not** re-frame as blocks (the next "header" implies an impossible size);
- are **not** a plain continuation of the game-info keystream (decrypting them
  as such yields noise);
- are **not** obviously the raw planet array under the naive `x:10|y:10|id:12`
  packing.

Planet coordinates *are* recoverable elsewhere: the `.hst`/`.mN` **PlanetBlock
(type 13)** decrypts cleanly to sequential planet ids `00,01,02,…`, so universe
geometry can be sourced there while the `.xy` planet region is worked out.

### Next steps

- Decode PlanetBlock (13) records from `.hst`/`.mN` to get planet id/x/y and the
  planet count for this universe.
- Re-derive the `.xy` planet region format from the original `create.c`-derived
  logic (how many planets, record size, and whether the region uses a distinct
  keystream/seed or an unencrypted packing).

## Derived test vectors

- `Game.xy`: `game_id=0x2a031dd8`, size=1, density=1, players=3,
  name=`"A Barefoot JayWalk"` (asserted in `tests/real_files.rs`).
