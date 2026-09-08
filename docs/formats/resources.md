# The pictures, in the executable's resource table

Status: **decoded & verified** — implemented in `stars_formats::resources`,
tested against a real `stars.exe` by `crates/stars-formats/tests/resources.rs`.

Everything the game draws — planets, ships, race emblems, component pictures,
the toolbar — is a Windows bitmap in `stars.exe`. There are **thirty-eight** of
them and they are most of the file: 2.37 MB of the 4.2 MB executable.

## Nothing is extracted into this repository

The pictures are read out of a copy of the original **at run time**, from
whatever the player already has. This project transcribes *data* — tables,
offsets, formulas, component names — and does not copy the game's authored
content, which is the same rule that governs its message text and the
Technology Browser's component notes. Artwork is as authored as content gets.

So `stars-desktop` looks for a copy of the original and, finding one, draws
with it; finding none, it draws exactly as it did before. **The pictures are an
improvement and never a requirement**, and every test that uses them has a twin
that runs without them.

Where it looks, in order: `STARS_EXE`; the directory the save was opened from,
and its parent; the working directory; and `binary/` under it, which is where a
checkout of this project keeps its own copy. File > *Use the original's
pictures…* points it at one by hand.

## The resource table

`stars.exe` is a 16-bit **NE** (New Executable). The chain is the documented
one:

| Where | What |
|-------|------|
| `0x3c` | `e_lfanew`, the offset of the NE header |
| NE + 0 | `"NE"` |
| NE + `0x24` | the resource table, as an offset from the NE header |
| NE + `0x26` | the resident-name table — equal to the above when there are no resources |

The resource table itself:

* a `u16` **alignment shift**, 6 in this executable, so offsets and lengths in
  it are counted in 64-byte units;
* then a run of **type** records, each `{ u16 type, u16 count, u32 reserved }`
  followed by `count` entries of
  `{ u16 offset, u16 length, u16 flags, u16 name, u16 handle, u16 usage }`;
* ended by a type of zero;
* then the names, as length-prefixed strings at offsets from the **start of the
  resource table**.

A type or a name with bit 15 set is an integer id with that bit cleared;
otherwise it is an offset to one of those strings. A length is rounded **up**
to the alignment unit, so a resource is usually a little shorter than the space
reserved for it.

What is in this one:

| Type | Count |
|------|-------|
| `RT_BITMAP` (2) | 38 |
| `RT_DIALOG` (5) | 36 |
| `RT_CURSOR` (1) / `RT_GROUP_CURSOR` | 11 / 11 |
| `RT_ICON` (3) / `RT_GROUP_ICON` | 10 / 10 |
| `RT_MENU` (4) | 1 |
| `RT_ACCELERATOR` (9) | 2 |
| `10000`, `10002`, `10004` | 1 each |

Five bitmaps are named rather than numbered — `CARGOBMP`, `DOCKBMP`,
`SCANNERBMP`, `SCREEN50BMP`, `UNKNOWNPLANETBMP` — and the table stores those
names in upper case while the code asks for them in mixed case, so the lookup
here compares them without regard to case as `LoadBitmap` does.

## The bitmaps

Each `RT_BITMAP` is a plain device-independent bitmap: a 40-byte
`BITMAPINFOHEADER`, a palette of `1 << bpp` entries of blue-green-red-and-a-
spare-byte, then the pixels. None is compressed. The game uses three depths —
1, 4 and 8 bits a pixel — and rows are padded out to four bytes and stored
**bottom-up**, so decoding turns them the right way up once, here, and
everything above this layer works top-down.

There is no transparency anywhere: the fourth palette byte is not alpha, and
every one of these is blitted with `SRCCOPY`.

## Which picture is which

The code names none of them — a picture is a number — but `InitStuff` loads
every one in a single run, so the whole catalogue is recoverable from that one
function and is transcribed in `stars_formats::resources::art::CATALOGUE`.

The interesting ones are **sheets**: a grid of fixed-size cells indexed by a
single number, blitted out of with `DibBlt`.

| Sheet | Ids | Cell | Layout |
|-------|-----|------|--------|
| component pictures (`rghdibInventory`) | 500–506 | 64 | 8 across, 4 down, 32 a sheet |
| ships (`rghdibShips`) | 552–556 | 64 | 4 **down** a column, 8 columns |
| ships, small (`rghdibShipsT`) | 557–561 | 32 | the same index, half size |
| planets (`hdibPlanets`) | 112 | 64 | 7 across, 4 down, 28 in all |
| race emblems (`hdibRaces` / `T` / `X`) | 133 / 80 / 79 | 32 / 16 / 8 | 8 across, 4 down, 32 in all |
| the toolbar (`hdibToolbar`) | 178 | 24 × 23 | 18 across, one row |

Two things about that table are easy to get wrong and are worth stating.

**The sheets do not agree on which way round the index runs.** Because a bitmap
is stored bottom-up, the original's row arithmetic counts rows from the bottom
— `(3 - ((ibmp >> 3) & 3)) * 0x40` for a component, and the same shape for
ships and emblems, which on a four-row sheet lands back on the plain row number
once the picture is the right way up. The **planets** are the exception: their
blit is `iOffset / 7 << 6` with no subtraction, so that index runs *up* the
decoded picture, and it is the planets that need flipping here rather than the
others.

**Ships run down the columns, not across them.** `((i & 0x1f) >> 2) << 6` is
the column and `i & 3` the row, so pictures 0 to 3 are one column.

**There are exactly 148 ship pictures, and the hulls own them four at a
time.** `DrawFleetBitmap` reduces its index modulo `0x94` before looking
anything up, and 148 is not arbitrary: it is the thirty-two ship hulls and five
starbase hulls with **four pictures apiece**. Every hull's own `ibmp` is a
multiple of four — 0, 4, 8, … 144 — and between them the thirty-seven cover
every picture there is. So a hull's four are exactly one column of one sheet,
and the modulo is what keeps the narrow fifth sheet in bounds: its five columns
are the last twenty pictures.

**The last sheet of a set is narrower**: component sheet 506 is 256 wide rather
than 512, and ship sheet 556 is 320. Indices in their right-hand halves name
cells that are not there. Nothing in the binary says which of those are ever
used, so this does not guess: the crop fails and the caller draws nothing.

## What indexes them

* **Race emblems** — `PLAYER.logo`, `0..32`, straight into the sheet.
* **Planets** — the planet's own id: `iOffset = (id + 8) % 28`, so a planet
  keeps one face for the whole game and neighbours do not share one.
* **Component pictures** — the `ibmp` field of a component's table entry,
  transcribed as `picture` in `stars_core::components`. All 239 of them are in
  `0..=209`. Ten land on the last, narrow sheet and **every one of those is in
  its left half**, which confirms from the other side that the sheet really is
  four cells wide and that the cells this refuses to name are cells the game
  never asks for.
* **The toolbar** — the button's own index. See `../ui/toolbar.md`: the row's
  order comes from a 29-entry table, not from the indices.
* **Ships** — a **design's** own picture, which is its hull's base plus the
  one of the four its owner chose. `BuildDlg` spins between them with a pair
  of arrows: it splits the index into base and variant, steps the variant with
  `(iCur + 4 ± 1) & 3`, and puts the base back, so the choice wraps inside the
  hull's own group and can never land on another hull's ship.

  A **fleet** is drawn as its *primary* design — `IshdefPrimaryFromLpfl`
  (`1038:3e1c`), the design with the most ships, comparing strictly so a tie
  stays with the earlier slot. One twist: a **fuel transport**, hull 25 or 26,
  has its count docked by one once chosen, which costs it a tie and nothing
  more. A tanker that really is the most numerous ship still holds the picture.

  The owner's **race emblem** is blitted over the ship's bottom-left corner —
  16 pixels on the 64-pixel picture, 8 on the 32-pixel one — from the medium
  and small emblem sheets.

* **The scanner's own glyphs** — `ScannerBmp`, a 48x101 sheet the map draws
  several things out of, by fixed corner rather than by index:

  | cell | what |
  |------|------|
  | `(16, row × 11)`, 11x11 | the orbit rings, three colours |
  | `(29, row × 19)`, 19x19 | the same at the larger size, used when the planet is the selected object |
  | `(0, 0x5c)`, 9x9, mask at `(9, 0x5c)` | the wormhole |

* **The minefield brushes** — `rghbrPat[0..3]`, resources 460, 461 and 462:
  three 8x8 two-colour tiles of scattered dots, one per kind of field. They are
  pattern brushes rather than pictures, tiled across a circle and anchored to
  the map's origin; `Art::pattern` uploads one with `Repeat` wrapping and the
  drawn pixel opaque, so a caller tiles and tints it.

  The wormhole is the one that needs its **mask**: `DrawScanner` blits the mask
  with `SRCAND` and then the glyph with `SRCPAINT`, which is how a shaped
  sprite goes on a background. `Art::sprite_masked_at` folds the pair into one
  texture — a Windows AND-mask is white where the background shows through, so
  that is where the alpha goes to zero. See `../ui/scanner.md`.

## Source

- `InitStuff` (`init.c`), where every bitmap is loaded.
- `DibBlt` calls in `build.c` (components), `ship.c` (ships), `planet.c`
  (planets) and `mine.c` (emblems).
- The resource table layout is Microsoft's, and is confirmed against this
  executable byte for byte by the tests.
