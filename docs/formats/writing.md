# Writing files: `.hst`, `.mN` and `.xy` from scratch

- **Status:** verified — every record type re-encodes byte for byte over the
  whole fixture corpus, and a real host file and its three turn files are
  rebuilt from a loaded `GameState` block for block
- **Implemented in:** `stars_formats::*::encode` (one per record type),
  `stars_formats::StarsFile::build`, `stars_core::save`
- **Source:** the decoders this inverts, each cited in its own spec

Until this landed, the crate could only write a file it had **read**: a block's
payload was carried verbatim, so saving an edit replaced a couple of blocks and
left the rest of the bytes alone. That is enough to save a game the player
opened, and useless for a game this project generated, which has no original.

## The contract

`write(read(bytes)) == bytes`, one record at a time, over every record in
`fixtures/`. That is what `crates/stars-formats/tests/round_trip.rs` asserts:

| Record | Block types | Blocks checked |
|--------|-------------|---------------:|
| planet | 13 / 14 / 15 | 318,940 |
| fleet | 16 / 17 / 18 | 459,430 |
| waypoint | 19 / 20 | 247,126 |
| design | 26 | 291,183 |
| player & race | 6 | 74,903 |
| battle plan | 30 | 32,650 |
| production queue | 28 | 26,942 |
| space object | 43 | 125,355 |
| order-log operations | `.xN` types 1-46 | 939 |
| **total** | | **1,577,468** |

Plus 230 distinct names through the packed-string encoder, and all **58** `.xN`
files rebuilt whole from their parsed logs — see `orders-x.md`.

The point of doing it record by record rather than file by file is that a
whole-file round trip cannot fail: the payloads are kept. Re-encoding from the
decoded fields is the only thing that proves the decoders lose nothing — and it
found two places where they did.

## What the round trip found

**Planet blocks were dropping the concentration-decay accumulators**
(`rgpctMinLevel`), which the mining formula reads. The section is a presence
bitmask followed by one byte per mineral whose accumulator is non-zero. Across
all 266,403 planet blocks with detail 3 or more, every two-bit field of that
mask reads only `0` or `1` and no stored byte is `0`, so "present exactly when
non-zero" is a measured rule rather than a guess. `stars-core` now reads the
real values instead of assuming every planet is at full concentration.

**The plural race and player name was read to the end of the block** instead of
being bounded by its own length byte. A block that carries padding after the
name — 54,185 of them — decoded that padding as trailing spaces in the name.

Smaller things, all now preserved rather than dropped:

- the `unused5` and `unused2` bitfields of a planet's installations word,
  non-zero in exactly one block in the corpus;
- bits 13-15 of a file header's `dts` word, set on some `.x` order files
  (`EXODUS.X6` carries `0xC101`);
- the tails on `.hN` partial-planet blocks;
- the four bytes of a player block from offset 82 to 85, which nothing has
  identified. The 26 after them are the default production queue; see
  `player.md`.

Two fields turned out to be **derivable** rather than worth storing, each
checked across the whole corpus: a fleet block's word at offset 2 is
`FLEET.iPlayer`, the owner repeated (all 459,430 blocks), and bits 13-15 of its
id word are the NB09 `junk` field, always zero.

## The packed-string encoder

`stars_formats::strings::encode_field` inverts the nibble codec. Every
character has exactly one table entry — the five tables partition the printable
set without overlap — so the only choice is what to do with a leftover nibble.
Padding with the `0xF` escape is what the real files do: the escape reads the
two nibbles after it, finds the block has ended, and emits nothing, so the pad
is invisible. Every packed string in the fixtures re-encodes byte for byte
under that rule.

## Assembling a file

An order file is assembled by `OrderLog::to_file`, which recomputes `cbLog`;
everything else goes through `StarsFile::build`.

`StarsFile::build(header, body, footer)` frames the header as plaintext,
encrypts each body block with the keystream the header seeds, and appends the
plaintext footer. `stars_core::save` builds the body:

```text
.hst   one player block each, one planet block each (with a queue block after
       any planet that has one), every player's ship designs, every player's
       fleets each followed by its waypoints, every player's starbase designs,
       the object section, five battle plans each
.mN    that player's block, their planets, their designs, their fleets and
       waypoints, their starbase designs, their battle plans
```

which is the order `fixtures/incoming/turn0/` uses.

## Checked against the original engine

The turn-0 fixture is loaded into a `GameState` and written out again, and the
result is compared with what Stars! itself wrote, block by block within each
type:

| File | Blocks rebuilt byte for byte |
|------|-----------------------------:|
| `Game.hst` | 190 |
| `Game.m1` | 27 |
| `Game.m2` | 20 |
| `Game.m3` | 20 |

Nothing is excused except the file header — whose version word and cipher salt
are ours to choose — and the two sections a `GameState` does not carry: the
**object section** (four wormholes in that game) and **messages**. Every player
block, planet block, design, fleet, waypoint, battle plan, production queue and
footer comes back identical.

Getting there settled several details that only a writer has to know, each
measured over the corpus rather than assumed:

- **A player block whose plural name is empty carries one extra zero byte**, and
  one with a plural name does not. Exact across all 74,903 player blocks.
- **Bits 0-1 of byte 6** of a full-data player block are set. All 7,040 of them.
- In a **host** file, only the first player's block carries the universe's
  planet count; every other player's reads zero. In a **turn** file the count is
  the number of planet blocks the file holds.
- A design block's first byte is the `det` field (`7`), whose bit 2 is the
  `fullData` flag, and bit 0 of its second byte is set.
- Starting designs record `turn 1`, their template's picture index (`HUL.ibmp`)
  and its stored armour (`HUL.dp`, zero for a ship and 1000 for a starbase).
- `PLANET.uPopGuess` is a quarter of the population; `PLAYER.idPlanetHome` sits
  at offset 8 and `PLAYER.lSalt` at 12, the latter carrying `0x094DABEE` for
  every computer player.
- The two racial traits the wizard offers that do **not** fit the sixteen-bit
  lesser-trait field — *expensive tech starts at level 3* and *factories cost
  one less germanium* — are stored as bits 5 and 7 of the checkbox byte at
  offset 81, at the positions `ibitRaceTech3` (29) and `ibitRaceCheapFact` (31)
  occupy in the engine's own `grbitAttr`. This corrected a wrong conclusion in
  `../formulas/new-game.md`.

## What is not written

- **Space objects** — minefields, mineral packets, wormholes, mystery traders —
  because `GameState` does not model them. The object section is written as a
  count of zero.
- **Messages, battle recordings and scores**, for the same reason. A fresh game
  has none of them, which is what the turn-0 fixture's own files look like.
- A **fleet's name** *is* written, in the type-21 block after its waypoints, but
  no fixture has one to check against; see `fleet.md`.
- **The `.hN` history files.** A host writes those; this project does not yet.
  The `.xN` order log **is** written — see `orders-x.md` — but with a zero
  registration serial, because this project has none.
- The player fields nothing has identified: the race emblem is written from
  `Player::logo`, and offsets 82 to 111 are zero.

## This is not the save path for a loaded game

A real save file is mostly data this project models partially or not at all. A
game opened from disk is still written back by replacing only the blocks the
player edited (`stars_ui::App::to_bytes`), so nothing unmodelled is lost.
`stars_core::save` is for files that have no original to preserve.
