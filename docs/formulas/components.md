# Data: Component Tables

- **Status:** verified (engines, armour, shields, scanners, planetary, beams, torpedoes, hulls); stock designs not yet transcribed
- **Ghidra addresses:** see the table below
- **Manual reference:** the Technology Browser chapters describe every component
- **Uses RNG:** no
- **Implemented in:** `crates/stars-core/src/components.rs`

Every buildable component is a static table in the executable's data segment.
These are not formulas but they are a prerequisite for several: fuel use needs
engines, scanner range needs scanners, and build cost needs all of them.

| Table | Address | Entries | Transcribed |
|-------|---------|--------:|-------------|
| `rgengine` | `1008:0000` | 16 | yes |
| `rgarmor` | `1008:04e0` | 12 | yes |
| `rgscanner` | `1008:0768` | 16 | yes |
| `rgshield` | `1008:0ae8` | 10 | yes |
| `rgplanetary` | `1008:16b8` | 15 | yes |
| `rgtorp` | `1008:2180` | 12 | yes |
| `rgbeam` | `1008:2450` | 24 | yes |
| `rghuldef` | `1008:29f0` | 32 | yes |
| `rgshdefT` | — | 22 | no |
| `rghuldefSB` | `1008:4872` | 5 | yes |
| `rgshdefSBT` | — | 4 | no |

## Layout

Every component begins with the same header, then adds its own fields:

| Offset | Size | Field |
|-------:|-----:|-------|
| 0x00 | 2 | `id` |
| 0x02 | 6 | `rgTech[6]` — levels required in each field |
| 0x08 | 32 | `szName` — NUL-padded |
| 0x28 | 2 | `cMass` — kT |
| 0x2A | 2 | `resCost` — resources to build |
| 0x2C | 6 | `rgwtOreCost[3]` — ironium, boranium, germanium |
| 0x32 | 2 | `ibmp` — bitmap index (presentation only) |

Type-specific tails: engines add `grfAbilities` and `rgcFuelUsed[12]`; armour
and shields add `dp`; scanners add `dRange` and `grfAbilities`; planetary items
add `grAbility`; beams add `dRangeMax`, `dp`, `init`, `grfAbilities`; torpedoes
add `dRangeMax`, `dp`, `init`, `dHitChance`.

## Two conventions worth knowing

- **A negative range means penetrating.** Scanners store one number: its
  magnitude is the normal range, and if it is negative the component also
  penetrates planets at half that range. The four `Snooper` planetary scanners
  are the ones with negative ranges. See `scanning.md`.
- **A zero fuel entry means free.** `rgcFuelUsed` is indexed by warp factor
  `0..=11`; a zero is not "no data" but "this engine burns nothing at this
  speed", which is how ramscoops and the low warps of ordinary engines are
  expressed. See `movement.md`.

## Regenerating

The tables are transcribed from the reconstructed NB09 source (`parts.c` in a
`sirgwain/stars-decompile` checkout under `tmp/`) by a generator script, then
checked against our own binary. The generator parses the C designated
initializers and emits `crates/stars-core/src/components.rs`; it lives in the
session scratchpad rather than the repo because it is a one-shot import, but
the parsing rule is simple enough to rewrite: find `<TYPE> <name>[N] = {`,
split the top-level `{...}` entries, and read `.field = value` pairs.

## Verification

`crates/stars-core/tests/component_tables.rs` decodes raw bytes read out of
`stars.2.7j.exe` — recorded with their addresses in
`../vectors/components.json` — using the layout above, and asserts our
transcription agrees. Four entries are covered, one per structure shape
(engine, armour, beam, planetary), together with the table lengths and the
negative-range convention.

## Hulls

A hull is a `HULDEF`, 143 bytes: a `HUL` (the shared header plus `wtEmpty`,
`resCost`, `rgwtOreCost`, `ibmp`, `wtCargoMax`, `wtFuelMax`, `dp`, sixteen
`HS` slots and `chs`), then a packed word at `+0x7B` carrying the battle
category, `wrcCargo` at `+0x7D` and `rgbrc[16]` at `+0x7F`.

The last two are **presentation**, but recovering them is what makes the ship
designer's schematic possible: they say where each slot and the cargo space sit
on the designer's grid. See `../ui/ship-design.md`, and
`../vectors/hull-schematics.json` for all 37 of them.

Two figures are stored in units the game converts before showing them:
`wtCargoMax` is `0xFFFF` on the three starbases with an unlimited dock, and a
**starbase's costs are stored doubled** — see `design.md`.

## Open questions

- The stock ship and starbase designs (`rgshdefT`, `rgshdefSBT`) are not
  transcribed.
- `grfAbilities` bit meanings are only partly known: bit 0 marks a ramscoop on
  an engine. The rest await the ship-design work.
