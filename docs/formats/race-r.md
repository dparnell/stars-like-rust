# Format: `.rN` — race definition

- **Status:** container **verified byte-for-byte**; race record **largely
  decoded** (habitability, growth, economy, research, PRT, LRTs) — a few
  bitfields and the packed name encoding remain
- **Original files analysed:** `fixtures/r/{antetherial,humanoid,insectoid,
  nucleoid,rabitoid,random,silicanoid}.r1` — the six built-in default races plus
  a "random" race, exported from the race wizard
- **Encoding/compression:** standard Stars! container (see `blocks.md`); the
  single race record lives in an encrypted **type-6 block**
- **Checksum/CRC:** plaintext footer (type 0), empty (0 bytes) in every sample
- **Implemented in:** round-trips via `stars-formats::file::StarsFile`
  (`tests/race_files.rs`); a typed race model is not yet extracted

## Overview

A `.rN` file stores one race definition produced by the race wizard, so it can
be loaded when creating a player in a new game. It is a fully block-framed
Stars! file with exactly three blocks:

```
[ FileHeaderBlock (type 8, plaintext, 16 bytes) ]
[ PlayerBlock     (type 6, encrypted, 125–129 bytes) ]   <- the race record
[ FileFooterBlock (type 0, plaintext, 0 bytes) ]
```

The header identifies the file as a race (`dts` low byte = 5,
`FileType::Race`) and uses the "no specific player" marker (31) in the player
field. `encode(decode(bytes)) == bytes` holds for all seven fixtures, proving
the shared framing/header/cipher machinery handles `.rN` unchanged.

## Race record (decrypted type-6 block)

Offsets are into the **decrypted** type-6 payload. All values are single bytes
unless noted; multi-byte integers are little-endian.

| Offset | Size | Field                | Notes / evidence                                                        |
|-------:|-----:|----------------------|-------------------------------------------------------------------------|
| 0      | 1    | player id            | always `0xFF` here → "race-only" block (vs. 0-based id in `.m`/`.hst`)   |
| 6      | 1    | flags/bitfield       | varies per race (e.g. Humanoid `0x0F`); meaning TBD                      |
| 16     | 1    | gravity center       | habitability click 0–100; **`0xFF` = immune** to gravity                |
| 17     | 1    | temperature center   | click 0–100; `0xFF` = immune to temperature                             |
| 18     | 1    | radiation center     | click 0–100; `0xFF` = immune to radiation                               |
| 19     | 1    | gravity low          | lower habitable bound (click); `0xFF` when immune                       |
| 20     | 1    | temperature low      |                                                                         |
| 21     | 1    | radiation low        |                                                                         |
| 22     | 1    | gravity high         | upper habitable bound (click); `0xFF` when immune                       |
| 23     | 1    | temperature high     |                                                                         |
| 24     | 1    | radiation high       |                                                                         |
| 25     | 1    | growth rate %        | max population growth (Humanoid 15, Rabbitoid 20, Silicanoid 6, …)       |
| 56     | 1    | constant `0x0F`      | `15` in every sample; purpose TBD                                        |
| 62     | 6–7  | economy settings     | factory & mine produce/cost/count (Humanoid `10,10,10,10,10,5,10`)      |
| 70     | 6    | research cost/field  | one byte per tech field (Energy,Weapons,Prop,Const,Elec,Bio); `0/1/2` = costs-less/normal/costs-more; Humanoid = all `1` |
| 76     | 1    | **PRT**              | primary racial trait: `0`=HE `1`=SS `2`=WM `3`=CA `4`=IS `5`=SD `6`=PP `7`=IT `8`=AR `9`=JOAT |
| 78     | 2    | **LRT bitfield**     | lesser racial traits, little-endian u16; Humanoid = `0` (none)          |
| 81     | 1    | bitfield             | varies (Rabbitoid `0x80`, Random `0x40`, Nucleoid `0x20`); TBD          |
| ~113   | var  | race names           | two length-prefixed, 4-bit-packed strings (singular + plural); see below |

> The habitability layout was confirmed by the perfectly-centred **Humanoid**
> race (center 50 / low 15 / high 85 on all three axes) and cross-checked
> against the asymmetric ranges of Antethereal, Rabbitoid, etc.
>
> The **PRT** byte at offset 76 was confirmed by name: Humanoid→JOAT(9),
> Insectoid→WM(2), Rabbitoid→IT(7), Silicanoid→HE(0), matching Stars! lore.

## PRT / LRT reference

**Primary Racial Trait** (offset 76):

| id | PRT | id | PRT |
|---:|-----|---:|-----|
| 0  | HE — Hyper Expansion       | 5 | SD — Space Demolition       |
| 1  | SS — Super Stealth         | 6 | PP — Packet Physics         |
| 2  | WM — War Monger            | 7 | IT — Interstellar Traveler  |
| 3  | CA — Claim Adjuster        | 8 | AR — Alternate Reality      |
| 4  | IS — Inner Strength        | 9 | JOAT — Jack of All Trades   |

**Lesser Racial Traits** (offset 78, u16 bitfield): IFE, TT, ARM, ISB, GR, UR,
NRSE, OBRM, NAS, LSP, BET, RS, CE, MA (exact bit positions to be confirmed
against races that enable specific LRTs).

## Race name encoding

The record ends with two strings — the **singular** and **plural** race name
(e.g. "Humanoid" / "Humanoids"). Each is stored as:

```
[ len:u8 ][ len bytes of 4-bit-packed characters ]
```

For Humanoid the two run `06 B7 DE DB 16 74 D6` (singular, 6 packed bytes) and
`07 B7 DE DB 16 74 D6 9F` (plural, 7 bytes — same prefix plus one byte for the
trailing "s"). The 4-bit packing uses Stars!'s character lookup table; decoding
that table to recover the literal text is still **pending**.

## Open questions

- The early flags byte (offset 6) and the bitfield at offset 81.
- Exact split/ordering of the economy block at offsets 62–69 (factory vs. mine
  produce/cost/count) and the meaning of offset 69.
- LRT bit positions (need fixtures with known LRTs enabled).
- The 4-bit name character table (to turn packed bytes back into text).
- Whether spent/leftover advantage points are stored anywhere in the record.

## Derived test vectors

- `crates/stars-formats/tests/race_files.rs` — round-trips all seven fixtures
  and asserts the `[8, 6, 0]` block shape and the `0xFF` race marker.
