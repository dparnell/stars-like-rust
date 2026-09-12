# The planet pane

Status: **layout and text recovered**; the tiles this project has the data for
are reimplemented, the rest are named below.

The pane at the top left of the main window reports the selected planet. It is
not one panel but **six tiles in two columns**, each with its own small title
bar, laid out from a table rather than from code.

Source: `PlanetWndProc` (`1048:0000`), the tile table `rgtilePlanet`
(`1120:07fc`), and the six drawing routines it points at.

## The tiles

The table is six 16-byte `TILE` records — `yTop`, `dyFull`, `grbit`, a function
pointer, a packed word, and a help id. Read out of the binary:

| # | column | lines | extra | `grbit` | draws | title |
|---|--------|-------|-------|---------|-------|-------|
| 0 | left | 1 | 85 | `0x80` | `DrawPlanShipBitmap` (`1048:3336`) | the planet, as a picture |
| 1 | left | 6 | 5 | `0x01` | `DrawPlanetMinSum` (`1048:12b2`) | **Minerals On Hand** |
| 2 | left | 8 | 6 | `0x08` | `DrawPlanetStats` (`1048:1716`) | **Status** |
| 3 | right | 6 | 22 | `0x04` | `DrawPlanetShipList` (`1048:377e`) | the fleets in orbit |
| 4 | right | 10 | 20 | `0x40` | `DrawPlanetProduction` (`1048:2c38`) | **Production** |
| 5 | right | 8 | 15 | `0x100` | `DrawPlanetStarbase` (`1048:22cc`) | the starbase's design name, or `< no starbase >` |

The two number columns are the record's first two words, and neither is a
height on its own. `InitTiles` (`1000:0eb8`) folds them together:

```
dyFull = dyFull + yTop * dyArial8
```

— so the first is a **line count** and the second what is added to it, which is
why Minerals On Hand is six (three minerals, a rule, mines and factories) and
the picture is one line plus eighty-five pixels. `InitTiles` then walks each
column writing every tile's top, so the tops sitting in the shipped image are
*output*, not input, and mean nothing until it has run.

`EnsureTileSize` (`1048:58df`) adjusts three of them when the window layout
changes, matching on `grbit`: the ship list by `(dyArial8 + 4) * 2`, the
production queue by `(dyArial8 + 2) * 2`, and the picture by a flat ten. The
other three are the same size either way.

So the left column is the planet, its minerals and its status; the right column
is what is over it, what it is building and what it is building from.

### The frame

`FDrawTileNC` (`1048:1086`) draws what every tile shares, and the geometry is
all in it:

```
left   = iCol * 0xc6 + 4          two columns, 198 apart, from x = 4
right  = left + 0xbe              190 wide
bottom = top + (open ? dyFull : dyArial8 + 3)
```

and `ReflowColumn` stacks them from `y = 4` with four pixels between one and
the next. Inside that: a 3-D frame around the tile, a title bar one pixel in
and `dyArial8 + 2` tall with a 3-D frame of its own, the title **centred** in
Arial 8 bold in `crButtonText` on `crButtonFace`, a **seventeen-pixel button**
at the title bar's right end with a shadow line down its left, and the body
starting at `top + dyArial8 + 4`.

Bit 7 of the packed word is the open flag — despite the `fPopped` name it is
set when the tile is **open**, and all six ship open. Clicking the title bar
clears it, the tile shrinks to `dyArial8 + 3`, and `ReflowColumn` takes
everything below it up. The setting is written to `stars.ini`.

The pane's own title bar is the planet's name (`SetPlanetTitleBar`,
`1048:3dec`), or `Planet View` when nothing is selected.

## Minerals On Hand

| row | format | notes |
|-----|--------|-------|
| ironium, boranium, germanium | `%ldkT` | each named in its own colour |
| — | | a rule across the tile |
| Mines | `%d of %d` | built, of what the population can run |
| Factories | `%d of %d` | |

Alternate Reality has no factories and no population cap on mines, so it gets
`%d*` for the mines it is running and `n/a` for factories.

## Status

| row | format |
|-----|--------|
| Population | grouped with commas |
| Resources/Year | `%d of %d` — what production may spend, of what the planet makes; the first is the second less the research skim |
| — | a rule |
| Scanner Type | the best planetary scanner's name, or `none` |
| Scanner Range | `%d light years` under a hundred, `%d l.y.` at a hundred or more, `%d/%d l.y.` when it penetrates, `none` |
| — | a rule |
| Defenses | `%d of %d` |
| Defense Type | the best defence the player can build, or `none` when none are built |
| Def Coverage | two percentages, the second in brackets: what survives an ordinary bomb, and what survives a smart one |

Alternate Reality reads `Organic` for its scanner — it scans from the starbase,
by population — and `n/a` down the defence rows, which it cannot build.

Note the units: **under a hundred light years the original spells the words
out** and switches to the abbreviation above it, which is easy to get backwards.

## Production

The queue in build order, or `--- Queue is Empty ---`; then `Completion:` for
the entry being built, and a `Route to` row with a **Route** button. The tile is
a real list box in the original, with the queue editable from it.

## Starbase

Titled with the base's design name, or `< no starbase >` when there is none.

| row | format |
|-----|--------|
| Dock Capacity | `%lddp`, `Unlimited`, or `none` |
| Armor | `%lddp` |
| Shields | `%lddp` or `none` |
| Damage | a percentage in dark red, or `none` |
| — | a rule |
| Mass Driver | `Warp: %d`, or `none` |
| Destination | the planet packets are flung at, or `none` |

`DrawPlanetStarbase` (`1048:22cc`) draws Dock Capacity from the **hull's**
`wtCargoMax` — `0xffff` is `Unlimited` — and the Damage figure in dark red
(`SetTextColor(0x00007f)`, put back straight afterwards, so it is the one row
of the four with a colour of its own).

The damage figure is held in **500ths of the base's armour**, the unit a
damaged ship stack uses too, so the percentage is the stored figure over five.
The routine rounds the figure **up to five** before dividing, which floors the
reading at one percent: a base scratched by two 500ths still reads `1%`. That
is not academic — the fixtures hold twos, threes and fours; see
[`planet.md`](../formats/planet.md).

Under the two driver rows comes a row of its own: a **Set Dest** button
filling the left third of it, and — only when there is a driver — a gauge
filling the rest.

### The driver, and the `+`

`IWarpMAFromLppl` (`1048:7b10`) walks the starbase design's slots and keeps
the best mass driver in them. The drivers are the orbital specials from index
**7** (`Mass Driver 5`) to index **15** (`Ultra Driver 13`), the seven below
them being stargates, and a driver's rating *is* its warp.

The routine also reports whether a **second** driver of that same rating is
fitted, which the row writes as a trailing `+` and which
[`packets.md`](../formulas/packets.md) counts as one warp faster. It is kept
slot by slot: a strictly better driver takes over and clears the flag, an
equal one sets it. So 5, 5 then 7 is not a pair and 5, 7 then 7 is — and two
drivers **in one slot** are not a pair either, because the routine never
looks past a slot being occupied.

Three gates come first. The planet must be owned, it must have a starbase,
and for **another player's** planet their starbase design must be known in
full (`det == 7`) — otherwise the row reads `none` however much armour the
tile above it is willing to name.

### The gauge

`DrawMassWarpGauge` (`1048:2afa`) draws the launch speed, which is the
planet's own four-bit field plus four, floored at warp 5. The bar is one
segment of the raw field against a total of the **rating less one**, and its
colour says how hard the launch is being pushed:

| speed | brush |
|-------|-------|
| at or under the rating (plus one for a pair) | `hbrPurple`, `0x7f007f` |
| one or two warps over it | `hbrYellow`, `0x00ffff` |
| three warps over it | `hbrRed`, `0x0000ff` |

Three over is also as far as the gauge will be dragged: `ClickInShipOrders`
(`1050:8783`) hands the drag a minimum of 1 and a maximum of the rating less
one, which is warp 5 to the rating plus three. The label is `Warp %ld`,
centred on the bar.

### Set Dest

`ClickInPlanetOrders` (`1048:515c`) tracks the button and latches bit 8 of
the frame's flags word. `ScannerWndProc` (`1058:04bb`) then narrows the next
click's hit test to **planets alone**, takes the planet it lands on as the
destination — except the selected planet itself, which clears the destination
instead — and clears the bit again, so the button arms one click rather than
turning a mode on. A **shift-click** on the map with a planet selected takes
the same path without the button, provided the planet has a driver.

The button is drawn with `DrawBtn` style `8`, or `0xc` — its disabled style —
when there is no driver, and clicking the Mass Driver row itself pops up the
driver's component card (`grPopupComponent` for orbital special
`warp + 2`).

## What is *not* here

The planet's **environment** — gravity, temperature and radiation, with the
habitability value — is not in this pane, which is easy to assume it would be.
The tile table has no room for it: the environment gauges belong to
`DrawMineSurvey` (`1028:065a`), which fills a different pane of the frame. That
pane is its own spec.

## What this project does

`crates/stars-ui/src/views/planet.rs` over `crates/stars-ui/src/tiles.rs`, with
methods on `App` building each tile's rows so the text can be tested without
drawing it.

Reproduced: the **table** itself, in `crates/stars-ui/src/tiles.rs` — the two
columns, the order, and each tile's height as `InitTiles` computes it, with
`EnsureTileSize`'s three adjustments; the **geometry** — 190-wide columns 198
apart from x = 4, tiles stacked from y = 4 four pixels apart; the **frame** —
the 3-D border, the centred bold title on its own barred strip, the button at
its right end and the shadow line beside it; and **collapsing a tile by
clicking its title bar**, with the column reflowing under it exactly as
`ReflowColumn` reflows it. Minerals On Hand and Status in full, with the
original's labels, formats and Alternate Reality special cases and the three
mineral labels in `rgcrMin`'s own colours; the Production tile's queue and its
empty text; the Starbase tile's title, its rows — the Damage figure in its own
dark red — its warp gauge with the three colours of risk, and the Set Dest
button with the click it arms; the fleets in orbit.

### What the packed word holds

Each record's word at `+10` is three fields — `column:3, id:4, fPopped:1` —
and all three are kept in `stars.ini` under `[Windows] PlanetTiles`, with
the fleet pane's under `ShipTiles`.

`ReadIniTileSettings` (`1000:1a20`) reads the string a letter at a time. A
`*` moves to the second column, and only the first one does anything, so
there are two columns and no more. Each letter names a tile by its **id**
counting from `a`, and the letter's **case** is `fPopped`: upper is open.
The order of the letters is the order of the tiles — the routine swaps each
named tile up to the next slot as it goes — so the setting carries the
arrangement as well as which tiles stand open.

The six planet tiles are ids 0, 1, 4, 5, 6 and 7, three to a column, so the
shipped setting reads `ABE*FGH`; the seven fleet tiles are 0, 5, 3, 4 and
then 1, 9, 8, which is `AFDE*BJI`.

This project keeps **which tiles stand open** and reads the rest. It draws
the tiles in the table's own order and offers no way to move them, so a
file arranged in the original keeps its order and columns through a round
trip rather than being flattened to this one's.

### The small layout

`fSmallTiles` is not about the screen. `FrameWndProc` (`1020:072d`) asks
`EnsureTileSize` (`1048:587e`) for `iWindowLayout == 2`, so it is the
**Window Layout** the View menu offers and its smallest setting is the one
that shrinks the tiles.

The routine does not recompute a height: it **moves each by a delta**,
taking the tile's resize off on the way into the small layout and adding it
back on the way out, and it is guarded by a bit saying which state the
tables are already in so that it cannot be applied twice. Two things follow.
The table ships holding the **large** size, so that is the one to treat as
given. And the difference between the two layouts is the resize **once**,
not twice.

Which tiles move, and by how much:

| table | `grbit` | moves by |
|-------|---------|----------|
| planet | `0x80` | a flat 10 |
| planet | `0x04` | `(dyArial8 + 4) * 2` |
| planet | `0x40` | `(dyArial8 + 2) * 2` |
| fleet | `0x01` | `dyArial8 * 4 + 2` |
| fleet | `0x200` | `dyArial8 * 3 + 8` |
| fleet | `0x20` | `dyArial8 + 9` |
| fleet | `0x04` | `(dyArial8 + 4) * 2` |
| fleet | `0x80` | a flat 10 |
| fleet | `0x100` | a flat 2 |
| fleet | `0x40` | a flat 6 |

Not reproduced: the planet **picture** when no copy of the game is found (this
tile says in words what the picture says at a glance); reordering the tiles or
moving one between columns; the production tile's completion line and Route
button; and
the editing that the original's list box allows — the production queue is
edited on the Planets screen instead.
