# Format: `.rN` — race definition

- **Status:** container **verified byte-for-byte**; race record **largely
  decoded** (habitability, growth, economy, research, PRT, LRTs) — a few
  bitfields and the packed name encoding remain; a typed read-only view is
  implemented (`stars-formats::race::RaceRecord`)
- **Original files analysed:** `fixtures/r/{antetherial,humanoid,insectoid,
  nucleoid,rabitoid,random,silicanoid}.r1` — the six built-in default races plus
  a "random" race, exported from the race wizard; **plus** the seven shipped AI
  expansion races `fixtures/games/exodus/Races/{BIGPRO,DEFENDER,ECOBOOM,
  FLEXIBLE,JUMPERS,OFFENDER,SNEAK}.R1` — the first fixtures with **LRTs
  enabled**, which pinned down the LRT bit layout
- **Encoding/compression:** standard Stars! container (see `blocks.md`); the
  single race record lives in an encrypted **type-6 block**
- **Checksum/CRC:** plaintext footer (type 0), empty (0 bytes) in every sample
- **Implemented in:** round-trips via `stars-formats::file::StarsFile`; the
  verified fields are exposed as a typed read-only view via
  `stars-formats::race::RaceRecord` (`from_file` / `from_payload`)

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
| 78     | 2    | **LRT bitfield**     | lesser racial traits, little-endian u16 (bit layout below); Humanoid = `0` |
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

**Lesser Racial Traits** (offset 78, u16 bitfield). Bit positions **confirmed**
against the seven shipped AI races (the default races mostly have `LRT = 0`, but
Insectoid carries `0x2108` = ISB|NAS|MA, so it also helped):

| bit | LRT  | name                        | bit | LRT  | name                        |
|----:|------|-----------------------------|----:|------|-----------------------------|
| 0   | IFE  | Improved Fuel Efficiency    | 7   | OBRM | Only Basic Remote Mining    |
| 1   | TT   | Total Terraforming          | 8   | NAS  | No Advanced Scanners        |
| 2   | ARM  | Advanced Remote Mining      | 9   | LSP  | Low Starting Population     |
| 3   | ISB  | Improved Starbases          | 10  | BET  | Bleeding Edge Technology    |
| 4   | GR   | Generalized Research        | 11  | RS   | Regenerating Shields        |
| 5   | UR   | Ultimate Recycling          | 12  | CE   | Cheap Engines               |
| 6   | NRSE | No Ram Scoop Engines        | 13  | MA   | Mineral Alchemy             |

The seven AI races decode to sensible, overlapping trait sets — all share the
base IFE|OBRM|LSP — and no bit outside this 14-bit range is ever set, which is
what confirms both the field location and the bit order:

| race        | PRT  | LRT bits | traits                    |
|-------------|------|---------:|---------------------------|
| BIGPRO      | IS   | `0x0a89` | IFE, ISB, OBRM, LSP, RS   |
| DEFENDER    | SD   | `0x0a81` | IFE, OBRM, LSP, RS        |
| ECOBOOM     | CA   | `0x0a81` | IFE, OBRM, LSP, RS        |
| FLEXIBLE    | JOAT | `0x0681` | IFE, OBRM, LSP, BET       |
| JUMPERS     | IT   | `0x0a81` | IFE, OBRM, LSP, RS        |
| OFFENDER    | WM   | `0x2a81` | IFE, OBRM, LSP, RS, MA    |
| SNEAK       | SS   | `0x0a81` | IFE, OBRM, LSP, RS        |

Each race's **PRT also matches its file name** (OFFENDER→WM, DEFENDER→SD,
SNEAK→SS, JUMPERS→IT, FLEXIBLE→JOAT, ECOBOOM→CA eco, BIGPRO→IS), an independent
cross-check that the PRT byte and record alignment are correct.

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
- The 4-bit name character table (to turn packed bytes back into text).
- Whether spent/leftover advantage points are stored anywhere in the record.

## Derived test vectors

- `crates/stars-formats/tests/race_files.rs` — round-trips all seven default
  fixtures, asserts the `[8, 6, 0]` block shape and `0xFF` race marker, and
  decodes their PRTs via `RaceRecord`.
- `crates/stars-formats/tests/exodus_files.rs` — decodes the seven AI races and
  asserts each PRT + LRT set from the table above (and that they share the
  IFE|OBRM|LSP base with no stray bits).
- `crates/stars-formats/src/race.rs` — unit tests for `Prt`/`Lrt` decoding,
  immune-axis handling, and the LRT bit masks.
