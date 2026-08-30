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

After the game-info block (offset `0x54` in `Game.xy`) **514 bytes** remain.
Established facts about this region:

- it is **byte-identical across turn 0 and turn 1** — the universe geometry is
  fixed once and never rewritten (verified);
- `514 = 2 + 128 × 4` — with 128 planets in this game (confirmed via the
  `.hst` planet blocks) this strongly implies **4 bytes per planet** plus a
  2-byte prefix or trailer;
- it does **not** re-frame as blocks: naive framing yields a `type 5 (size 10)`,
  a `type 6 (size 5)`, then a bogus `type 0 (size 125)` that does not reach EOF;
- it is **not** a plain continuation of the game-info keystream (decrypting it as
  such yields uniformly-distributed noise, not clustered coordinates);
- the fleet position bytes from the `.hst` (`a4 05 1c 06`) do **not** appear
  literally in the region, so coordinates are **packed/encoded**, not stored as
  raw little-endian words.

Because the `.hst`/`.mN` **PlanetBlock (type 13)** decodes cleanly to sequential
planet ids (`records::planet_headers`, verified `0..=127`) but carries **no
coordinates**, the x/y geometry must live *here* in the `.xy` planet region.

### Next steps

- Nail the 4-byte planet packing: determine whether the 2 extra bytes are a
  leading count/seed or a trailing checksum, and how x/y (and a name index) are
  bit-packed into each 4-byte record. Cross-check by matching the three
  homeworld ids (`32`, `69`, `112`) to their fleet anchor positions.
- If the packing resists static analysis, recover the `.xy` planet writer from
  `STARS!.EXE` in Ghidra (reachable from new-game/universe generation) to read
  the exact bit layout and any per-region keystream/seed.

## Derived test vectors

- `Game.xy`: `game_id=0x2a031dd8`, size=1, density=1, players=3,
  name=`"A Barefoot JayWalk"` (asserted in `tests/real_files.rs`).
