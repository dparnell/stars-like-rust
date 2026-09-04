# Format: planet blocks (`rtPlanetA`/`B`/`C`, types 13 / 14 / 15)

- **Status:** **decoded & verified** against real host files; typed read-only
  view implemented in `stars-formats::planet` (`PlanetRecord`, `planet_records`)
- **Reference used:** TotalHost `StarsPlanet.pl` (Rick Steeves), itself derived
  from the `starsapi` project; offsets cross-checked against
  `fixtures/incoming/turn0/Game.hst` and `fixtures/games/tutorial/tutorial.hst`
- **Appears in:** `.hst` (type 13, full), `.mN` (13/14/15), `.hN` (type 14,
  partial). Every galaxy planet is one variable-length planet block.
- **Implemented in:** `stars-formats::planet`; tests in
  `tests/planet_files.rs`

## Block types

| Type | Name        | Detail       | Notes                                   |
|-----:|-------------|--------------|-----------------------------------------|
| 13   | `rtPlanetA` | full         | `.m`/`.hst` — full owner info + installations |
| 14   | `rtPlanetB` | partial      | `.h` history — no installations; 1-byte starbase; discovery year |
| 15   | `rtPlanetC` | minimal      | header only (id/owner/flags)            |

## Fixed header (4 bytes)

The record starts with two little-endian 16-bit words:

```
word 0 (offset 0):  bits 0..10  planet id      (11 bits, 0-based)
                    bits 11..15 owner player id (5 bits; 31 = unowned)
word 1 (offset 2):  bits 0..6   det (detail)   (0,1,3,4,7)
                    bit 7        fHomeworld
                    bit 8        fInclude
                    bit 9        fStarbase
                    bit 10       fIncEVO   (terraformed; original env stored)
                    bit 11       fIncImp   (installations present)
                    bit 12       fIsArtifact
                    bit 13       fIncSurfMin (surface minerals stored)
                    bit 14       fRouting  (route destination stored)
                    bit 15       fFirstYear
```

> **Correction:** an earlier revision of `records::PlanetHeader` split this first
> word as id(10)/flags(6). The verified split is **id(11)/owner(5)**; the "flags"
> live in the *second* word. Ids below 1024 decoded the same either way, which is
> why the bug went unnoticed until the owner field was needed.

`det` values seen: `1` = minimal, `3` = some, `4` = more (adds surface
minerals), `7` = all. Bits 3–6 of the flags word are always zero in samples.

## Variable sections (in order)

All sections are present-only; each is gated by `det` and/or a flag bit. Offsets
are cursor-relative, starting at 4.

### 1. Concentrations + environment — `det >= 3`, types 13/14

- **Concentration-decay bitmask** (1 byte at offset 4): two bits per mineral
  (ironium / boranium / germanium). A nibble value of `1` means one "turns until
  the concentration drops" countdown byte follows here (before the
  concentrations). `0`/other = no countdown byte. Cursor starts at 5.
- **Mineral concentration** (3 bytes): ironium, boranium, germanium (0..=255).
- **Environment** (3 bytes): gravity, temperature, radiation (0..=255).
- **Original environment** (3 bytes) — only if `fIncEVO` (terraformed).
- **Owner guesses** (2 bytes) — only if the planet is owned: a little-endian
  word split `popGuess = word & 0x0FFF` (× 1000 colonists) and
  `defGuess = word >> 12` (defense-coverage estimate index 0..=15).

### 2. Surface minerals + population — `det >= 4` and `fIncSurfMin`

- **Contents-length byte** (1 byte): four 2-bit fields selecting the byte-width
  of each following value via the map `[0,1,2,4]` → ironium, boranium,
  germanium, population (in that order).
- Each non-zero width is read as a little-endian value:
  - **surface ironium / boranium / germanium** (kilotons)
  - **population** — only if `det == 7`; stored ÷ 100, i.e. multiply by 100 for
    colonists.

### 3. Installations — type 13 only, `fIncImp`

Two little-endian 32-bit words (8 bytes total):

```
low32 : bits 0..7   iDeltaPop   (population-change accumulator)
        bits 8..19  mines       (12 bits)
        bits 20..31 factories   (12 bits)
high32: bits 0..11  defenses    (12 bits)
        bits 12..16 scanner     (5 bits; 31 = none)
        bits 17..21 unused
        bit 22      fArtifact
        bit 23      fNoResearch
        bits 24..31 unused
```

### 4. Starbase — `fStarbase` and owned

- **Type 14 (partial):** 1 byte, `design = byte & 0x0F`.
- **Type 13 (full):** one little-endian 32-bit word:

```
bits 0..3   design      (0..=15)
bits 4..15  damage %     (12 bits)
bits 16..25 fling dest   (10 bits, mass-driver target planet)
bits 26..29 warp         (4 bits)
bit 30      fNoHeal
bit 31      unused
```

### 5. Route destination — owned and `fRouting`

- 1 little-endian word; `routeDest = word & 0x03FF` (10-bit planet id). Upper 6
  bits unused.

### 6. Discovery year — type 14 only (`.h`), when the file detail is 4 or 7

- 1 little-endian word; the year is `word + 2400`.

## Verified values (sample game, turn 0 `Game.hst`)

All three homeworlds decode to the canonical Stars! starting state:

- **population** 25,000 colonists
- **installations** 10 mines, 10 factories, 10 defenses
- **starbase** present (design slot 0)
- **pop guess** 62,000; **surface minerals** ~400–560 kt each
- player 0 (all-normal **Humanoid** race) homeworld **environment 50/50/50**
- 125 unowned planets carry only concentrations + environment (11-byte record:
  header 4 + decay-bitmask 1 + conc 3 + env 3)

Tutorial host file: 24 planets, 2 homeworlds, all decode without error.

## Open questions / next

- The concentration-decay countdown bytes are skipped (not surfaced): confirm
  the two-bit encoding on a later-turn save where concentrations are dropping.
- Decode the `.x` **Planet Change** block (type 35): `planetId` (u16) + a 32-bit
  bitfield `fNoResearch / idFling / iWarpFling / idRoute` (per `StarsPlanet.pl`).
- Fleet (16), Design (26) and full Player (6) records still to be decoded.


## Partial records in `stars-core`

A file describes the planets its owner holds in full (type 13 with `det == 7`)
and everything else at whatever detail it has: type 14 carries environment and
mineral concentrations but no population or installations, type 15 only a
header.

`stars-core` keeps the two apart. `GameState::planets` holds only planets that
can be simulated; `GameState::known_planets` holds the rest, and
`Planet::detail` says which is which (`Full`, `Scanned`, `Minimal`). Mixing
them was tried and rejected: every consumer that iterates planets then has to
know not to trust a population that was never recorded, and the whole-turn
replay silently grew from 438 planet-years to 457 as a result.

The practical value of the partials is the environment of **unowned** planets,
which appears nowhere else. A host file for a 360-planet galaxy carries around
95 of them.

Note what a player file does *not* contain: any record of the galaxy beyond its
own planets. A player's file holds full records for exactly what it owns plus a
handful more, and there is no planet-knowledge block. What a player knows about
distant planets is not recoverable from these files.
