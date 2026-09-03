# Format: `.rN` — race definition

- **Status:** container **verified byte-for-byte**; race record **largely
  decoded** (habitability, growth, economy, research %, per-field research cost,
  PRT, LRTs, checkbox flags, **singular + plural names**) — a couple of flag bits
  remain; a typed read-only view is implemented
  (`stars-formats::race::RaceRecord`)
- **Reference used:** TotalHost `StarsRace.pl` (Rick Steeves), which lays out the
  full type-6 player/race record field-by-field; its offsets were cross-checked
  against the fixtures below and drive the typed decoder
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
| 1      | 1    | ship designs         | count; always `0` in a race file (player-block field)                   |
| 2      | 2    | planets              | `d[2] + ((d[3]&0x03)<<8)`; `0` in a race file (player-block field)       |
| 4      | ~2   | fleets / sb designs  | fleets `d[4]+((d[5]&0x03)<<8)`, starbase designs `(d[5]&0xF0)>>4`; `0` in a race file |
| 6      | 1    | flags: logo/fullData | `logo = d[6]>>3`, **`fullData = d[6]&0x04`** (always set for `.rN`); selects name framing |
| 7      | 1    | AI flags             | bit1 = AI enabled, bits2–3 = AI skill, bits5–7 = AI PRT (player-block field) |
| 8      | 2    | homeworld            | planet id; player-block field (`0` in a race file)                      |
| 10     | 2    | player rank          | player-block field (`0` in a race file)                                 |
| 12     | 4    | password             | player-block field; inverts to `FF FF FF FF` when set human-inactive    |
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
| 26     | 6    | tech levels          | Energy,Weapons,Prop,Const,Elec,Bio, each `0..=26`; player-block field (`0` in a race file); decoded as `ResearchState::levels` |
| 32     | 24   | tech points          | resources accumulated toward the next level, per field (4 bytes each); `0` in a race file; decoded as `ResearchState::points` |
| 56     | 1    | research %           | resources spent on research; defaults to `15` (`0x0F`) — per `StarsRace.pl` |
| 57     | 1    | research field       | **current field = low nibble**, next-field policy = high nibble (`0..5` a field, `6` stay, `7` lowest field); player-block field (`0` in a race file) |
| 58     | 4    | research last year   | resources research received from the most recent turn (`lResLastYear`); player-block field (`0` in a race file) |
| 62     | 1    | resource/colonist    | colonists per resource, in thousands (Humanoid `10` = 1 res / 1000 pop) |
| 63     | 1    | produce per factory  | resources per 10 factories (Humanoid `10`)                              |
| 64     | 1    | factory build cost   | resources to build one factory (Humanoid `10`)                          |
| 65     | 1    | factories operated   | factories per 10,000 colonists (Humanoid `10`, Rabbitoid `17`)          |
| 66     | 1    | produce per mine     | minerals per 10 mines (Humanoid `10`)                                   |
| 67     | 1    | mine build cost      | resources to build one mine (Humanoid `5`)                              |
| 68     | 1    | mines operated       | mines per 10,000 colonists (Humanoid `10`)                             |
| 69     | 1    | spend leftover pts   | selector for surplus advantage points (Humanoid `0`, Rabbitoid `4`)     |
| 70     | 6    | research cost/field  | one byte per tech field (Energy,Weapons,Prop,Const,Elec,Bio); **`0` = costs 75% extra, `1` = normal, `2` = costs 50% less**; Humanoid = all `1` |
| 76     | 1    | **PRT**              | primary racial trait: `0`=HE `1`=SS `2`=WM `3`=CA `4`=IS `5`=SD `6`=PP `7`=IT `8`=AR `9`=JOAT |
| 77     | 1    | (unknown)            | always `0` in samples; possibly a second PRT byte (per `StarsRace.pl`)   |
| 78     | 2    | **LRT bitfield**     | lesser racial traits, little-endian u16 (bit layout below); Humanoid = `0` |
| 81     | 1    | checkbox flags       | **bit 5** = *expensive tech starts at level 3* (Nucleoid `0x20`), **bit 7** = *factories cost 1 less germ.* (Rabbitoid `0x80`); bit 6 seen set on Random (`0x40`), TBD |
| 82     | 2    | MT items             | u16; player-block field (`0` in a race file)                            |
| 112    | var  | player relations     | `len = d[112]`, then one byte per player; `0`-length in a race file; the names follow |
| var    | var  | race names           | two length-prefixed, nibble-packed strings (singular + plural); see below |

> **Corrected in delivery Step 4.** Two fields were previously documented the
> wrong way round. The per-field research cost at offset 70 was recorded as
> `0/1/2 = costs-less/normal/costs-more`; `GetTechLevelCost` (`10d8:1dba`)
> shows the opposite — a stored `0` multiplies the cost by 1.75 and a stored
> `2` halves it, so `0` is *costs 75% extra*. Offset 57 was recorded as
> "current (`>>4`) / next"; `UpdateResearchStatus` (`10b8:80fe`) reads the
> **current** field from the low nibble. Both are confirmed by the Exodus
> fixture, whose War Monger player stores `2` for Weapons and `0` for every
> other field, and whose research points accumulate in the field named by the
> low nibble.

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
(e.g. "Humanoid" / "Humanoids") — decoded via the shared Stars! packed-string
codec (`stars-formats::strings`, documented in `strings.md`).

**Where the names start.** The names are the last thing in the record, but their
offset is not fixed: it depends on the `fullData` flag (bit 2 of the flags byte
at offset 6, always set for `.rN` files and full player blocks). Following
TotalHost's `StarsBlock.pm`:

```
if fullData (data[6] & 0x04):
    index = 112 + data[112] + 1       # after the player-relations table
else:
    index = 8                          # short player block
```

**Field framing.** From `index`, each name is a length-prefixed packed string:

```
[ len:u8 ][ len bytes of nibble-packed characters ]
```

The singular field is `data[index ..= index + data[index]]`; the plural field is
everything from just after it to the end of the record. Both are decoded with
`stars-formats::strings::decode_field`.

The decoded names match the built-in races exactly: Humanoid/Humanoids,
Insectoid/Insectoids, **Nucleotid/Nucleotids**, Rabbitoid/Rabbitoids,
Silicanoid/Silicanoids, **Antetheral/Antetherals** (note the last two are spelt
as the game stores them, not as the wizard labels them). `RaceRecord` now
exposes `singular_name` / `plural_name`.

## Open questions

- The remaining checkbox bits at offset 81 (bit 6 is set on the Random race;
  meaning unknown) and the unknown byte at offset 77.
- The exact numeric scaling of the economy bytes (62–68) — the field *positions*
  and their in-game meaning are now taken from `StarsRace.pl`, but the raw byte →
  displayed-rate conversions (e.g. how `resource/colonist` maps to "1 per 1000")
  still want an in-game cross-check.
- The `spend leftover points` selector (offset 69) — the raw byte is exposed, but
  the full enumeration of values is not yet mapped.
- The player-block-only fields (homeworld, rank, password, tech levels/points,
  resource priority, MT items) are documented from `StarsRace.pl` but not yet
  decoded here — they are all `0` in a `.rN` file and belong to the `.mN`/`.hst`
  player-block work.

## Derived test vectors

- `crates/stars-formats/tests/race_files.rs` — round-trips all seven default
  fixtures, asserts the `[8, 6, 0]` block shape and `0xFF` race marker, decodes
  their PRTs via `RaceRecord`, asserts each decoded **singular/plural name**, and
  asserts the **economy block, research % and checkbox flags** against the real
  values (Humanoid defaults, Nucleoid's *expensive-tech* and Rabbitoid's
  *factories-cost-1-less* checkboxes).
- `crates/stars-formats/src/strings.rs` — unit tests for the packed-string
  codec (single-nibble table, `B` escape table, `F` literal-byte escape).
- `crates/stars-formats/tests/exodus_files.rs` — decodes the seven AI races and
  asserts each PRT + LRT set from the table above (and that they share the
  IFE|OBRM|LSP base with no stray bits).
- `crates/stars-formats/src/race.rs` — unit tests for `Prt`/`Lrt` decoding,
  immune-axis handling, and the LRT bit masks.
