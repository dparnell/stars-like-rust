# Format: block framing + header + encryption — the shared container

- **Status:** framing, file-header, and payload **encryption recovered &
  verified byte-for-byte** on real files from **two independent games** (turns
  0, 1 **and** 3, all file types); per-format record layouts pending
- **Original files analysed:**
  - one 3-player game captured at two points —
    `fixtures/incoming/turn0/Game.{xy,m1,m2,m3,hst}` (fresh game, turn 0 / year
    2400) and `fixtures/incoming/turn1/Game.{xy,m1,m2,m3,hst,h1,h2,h3,x1}`
    (after the first turn was generated, adding history `.hN` and orders `.xN`);
  - a second, **independent** game (different `game_id`) —
    `fixtures/games/tutorial/tutorial.{xy,hst,m1,m2,h1,x1}` (the shipped
    2-player "Tutorial Game", partly played; `.m1`/`.h1`/`.x1` at **turn 3**)
- **Encoding/compression:** framing is plaintext; non-header/footer block
  payloads use the Stars! PRNG **stream cipher** (recovered — see below)
- **Checksum/CRC:** footer block (type 0) carries a year (`.m`/`.hst`) or
  checksum (`.r`); not yet decoded per-format
- **Implemented in:** `stars-formats::{block, header, crypt, file}`
- **Cross-checked against:** `stars-4x/starsapi` and `ricks03/TotalHost`
  (`StarsBlock.pm`), both derived from analysis of `STARS!.EXE`

## Overview

Every Stars! on-disk file (`.xy`, `.mN`, `.hN`, `.xN`, `.rN`, `.hst`) is a flat
sequence of **blocks**: a 16-bit little-endian header word (6-bit type id +
10-bit payload size) followed by the payload. The **first** block is the
plaintext file-header (type 8); the **footer** (type 0) is also plaintext.
Every other block's payload is XOR-encrypted with a keystream from the Stars!
PRNG that is **seeded from the header** and runs **continuously** across the
file.

`stars-formats::file::StarsFile::{decode,encode}` combine all three layers:
decoding yields blocks with decrypted payloads; encoding reproduces the original
bytes exactly (proven on the four fully-framed sample files).

## Top-level layout (per block)

| Offset | Size   | Type     | Name    | Notes / source                                  |
|-------:|-------:|----------|---------|-------------------------------------------------|
| 0x0000 | 2      | u16 (LE) | header  | `size = header & 0x03FF`, `type = header >> 10` |
| 0x0002 | `size` | u8[]     | payload | encrypted for all but type 8 (header)/0 (footer) |

Header-word bit layout (`type` high 6 bits, `size` low 10 bits):

```
 bit: 15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
      +--------------+  +-----------------------------+
      |   type id    |  |        payload size         |
      +--------------+  +-----------------------------+
```

## File-header block (type 8, plaintext) — 16-byte payload

| Offset | Size | Field     | Decoding                                             |
|-------:|-----:|-----------|-----------------------------------------------------|
| 0      | 4    | magic     | ASCII `"J3J3"`                                       |
| 4      | 4    | `game_id` | u32; identical across every file of a game          |
| 8      | 2    | version   | `major = w>>12`, `minor = (w>>5)&0x7F`, `inc = w&0x1F` |
| 10     | 2    | turn      | year = `2400 + turn`                                 |
| 12     | 2    | player    | `player = w & 0x1F` (0-based; 31 = shared/host); `salt = w >> 5` (11 bits) |
| 14     | 2    | dts       | `file_type = w & 0xFF`; flags in bits 8..12         |

`dts` low byte → file type: `0 .xy`, `1 .x`, `2 .hst`, `3 .m`, `4 .h`, `5 .r`.
Flags (bit above the low byte): `8 done(.x)`, `9 in_use`, `10 multi(.m)`,
`11 game_over`, `12 shareware`.

**Verified values** (sample game): `game_id = 0x2a031dd8`, `version = 0x2840`
(→ 2.x); `turn = 0` for the turn-0 set and `turn = 1` for the turn-1 set; player
word low-5 bits = `0,1,2` for `m1,m2,m3` and `31` for `.xy`/`.hst`; `dts` low
byte = `0/1/2/3/4` observed for `.xy`/`.xN`/`.hst`/`.mN`/`.hN` respectively.

## Encryption

### Seeding (`initDecryption`)

From the header's `salt` (11 bits) and other fields:

```
index1 = salt & 0x1F
index2 = (salt >> 5) & 0x1F
if (salt >> 10) == 1: index1 += 32   else: index2 += 32
rounds = ((game_id & 3)+1) * ((turn & 3)+1) * ((player & 3)+1) + shareware
seedA, seedB = PRNG warmed up `rounds` times from (PRIMES[index1], PRIMES[index2])
```

`PRIMES` is a 128-entry table lifted from the binary. **It contains a known
anomaly: entry 55 is `279` (not the prime `269`).** We must reproduce it to stay
bit-compatible (`stars_formats::PRIMES`).

### PRNG (`nextRandom`)

A subtractive combination of two Park–Miller LCGs (Schrage's method):

```
newA = (A % 53668)*40014 - (A / 53668)*12211;  if newA < 0: newA += 0x7fffffab
newB = (B % 52774)*40692 - (B / 52774)*3791 ;  if newB < 0: newB += 0x7fffff07
r = newA - newB;  if newA < newB: r += 2^32
A, B = newA, newB;  keystream word = r & 0xFFFFFFFF
```

### Stream cipher (`decryptBytes` = `encryptBytes`)

Process the payload in 4-byte **little-endian** words, XOR each with the next
PRNG word. A trailing partial word is zero-padded so it still consumes a whole
keystream word (the padding is dropped from the output); this keeps the next
block's keystream aligned. XOR is its own inverse, so one routine both encrypts
and decrypts (`StarsRng::apply`).

## Block-type registry (from `StarsBlock.pm`, community-documented)

| id | Name                              | id | Name                          |
|---:|-----------------------------------|---:|-------------------------------|
| 0  | FileFooterBlock (plaintext)       | 24 | FleetSplitBlock               |
| 1  | ManualSmallLoadUnloadTaskBlock    | 25 | ManualLargeLoadUnloadTaskBlock|
| 2  | ManualMediumLoadUnloadTaskBlock   | 26 | DesignBlock                   |
| 3  | WaypointDeleteBlock               | 27 | DesignChangeBlock             |
| 4  | WaypointAddBlock                  | 28 | ProductionQueueBlock          |
| 5  | WaypointChangeTaskBlock           | 29 | ProductionQueueChangeBlock    |
| 6  | PlayerBlock                       | 30 | BattlePlanBlock               |
| 7  | PlanetsBlock (.xy game info)      | 31 | BattleBlock                   |
| 8  | FileHeaderBlock (plaintext)       | 32 | CountersBlock                 |
| 9  | FileHashBlock                     | 33 | MessagesFilterBlock           |
| 10 | WaypointRepeatOrdersBlock         | 34 | ResearchChangeBlock           |
| 12 | EventsBlock                       | 35 | PlanetChangeBlock             |
| 13 | PlanetBlock                       | 36 | ChangePassword / Password     |
| 14 | PartialPlanetBlock                | 37 | FleetsMergeBlock              |
| 16 | FleetBlock                        | 38 | PlayersRelationChangeBlock    |
| 17 | PartialFleetBlock                 | 39 | BattleContinuationBlock       |
| 19 | WaypointTaskBlock                 | 40 | MessageBlock                  |
| 20 | WaypointBlock                     | 41 | AI record (.h)                |
| 21 | FleetNameBlock                    | 42 | SetFleetBattlePlanBlock       |
| 23 | MoveShipsBlock                    | 43 | ObjectBlock                   |
|    |                                   | 44 | RenameFleetBlock              |
|    |                                   | 45 | PlayerScoresBlock             |
|    |                                   | 46 | SaveAndSubmitBlock            |

Only types 8 (header) and 0 (footer) are anchored against our own files so far;
the rest are used to *interpret* decrypted payloads and are validated as each
per-format record layout is decoded.

## Verified round-trips

`crates/stars-formats/tests/real_files.rs` asserts `encode(decode(bytes)) ==
bytes` for every fully-framed file across both captured turns — `.hst`, `.mN`
(turn 0 and turn 1), plus the turn-1 `.hN` history and `.xN` orders — checks the
shared `game_id`, per-player numbering, and turn, and decrypts the `.xy` header
+ game-info. The turn-1 files confirm the turn-dependent seeding (`rounds`
depends on `turn`) is correct on non-zero turns.

The `tutorial_*` tests do the same for the shipped **Tutorial Game** — a
second game with a different `game_id`, whose `.m1`/`.h1`/`.x1` were saved at
**turn 3**. Because `rounds` also depends on `game_id` and `player`, this second
game independently confirms the seeding is game-agnostic and correct on a third
distinct turn value.

## Open questions / next

- Per-format **record layouts** for each decrypted block (players, planets,
  fleets, designs, production queues, …). The `.hst`/`.mN` **block inventory** is
  now decoded and the **planet-id word** verified — see `hst.md`; the per-field
  planet/fleet/design layouts remain.
- `.xy` **planet array** — **fully decoded** (see `xy.md`): `planet_count`
  plaintext 4-byte records (`xoffset:10 | y:12 | nameid:10`, x is a running
  sum) directly after the game-info block + a trailer (2 bytes in-game, 4
  standalone); coordinates and **names** (via the embedded master name table)
  are recovered, and the whole `.xy` round-trips byte-for-byte via
  `xy::Universe` across six universes (24–540 planets, two games). Only the x/y
  axis order and the trailer's `02 00` prefix remain open.
- Footer contents per extension (year vs checksum).
- `.rN` race file layout — **decoded** from `fixtures/r/` (see `race-r.md`): a
  single type-6 block holding habitability, growth, economy, research, PRT and
  LRTs.

## A file can contain several files

A `.mN` may hold more than one complete Stars! file, one after another: header,
blocks, footer, then another header. It happens when a player receives a new
turn before opening the previous one — the new turn is **appended** rather than
replacing what is already there.

`fixtures/games/exodus/2416/exodus.m6` is one: 297 blocks, with a file header at
index 0 (turn 15, player 5) and another at index 147 (turn 16, player 5).

This matters because the keystream is seeded from the header's game id, salt,
turn and player. **Each segment must be decrypted with its own keystream.**
Decrypting the whole stream from the first header produces correct bytes for
the first segment and noise for everything after it.

That bug survived in this project until delivery Step 4 because it is invisible
to a round-trip test: re-encrypting the noise with the same wrong keystream
reproduces the input byte-for-byte. It was found only when the battle records —
which happen to sit at the very start of a segment — decoded as high-entropy
garbage in exactly those files that carry two turns.

The lesson generalises: `write(read(x)) == x` proves the container is
*reversible*, not that it was *understood*. Semantic checks on decoded content
are what catch this class of error.

`StarsFile::segments` exposes the boundaries; `StarsFile::latest_segment` is
the current turn. `planet_records_in` and `player_records_in` take a block
slice so callers can decode one segment rather than mixing turns together.
