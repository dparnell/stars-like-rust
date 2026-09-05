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
pointer, a packed word holding the column and a few flags, and a help id. Read
out of the binary:

| # | column | height | draws | title |
|---|--------|--------|-------|-------|
| 0 | left | 85 | `DrawPlanShipBitmap` (`1048:3336`) | the planet, as a picture |
| 1 | left | 5 | `DrawPlanetMinSum` (`1048:12b2`) | **Minerals On Hand** |
| 2 | left | 6 | `DrawPlanetStats` (`1048:1716`) | **Status** |
| 3 | right | 22 | `DrawPlanetShipList` (`1048:377e`) | the fleets in orbit |
| 4 | right | 20 | `DrawPlanetProduction` (`1048:2c38`) | **Production** |
| 5 | right | 15 | `DrawPlanetStarbase` (`1048:22cc`) | the starbase's design name, or `< no starbase >` |

So the left column is the planet, its minerals and its status; the right column
is what is over it, what it is building and what it is building from. Every tile
can be collapsed by clicking its title bar (`fPopped`), and `FDrawTileNC`
(`1048:1086`) draws the frame and title they share.

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
| Damage | a percentage, or `none` |
| Mass Driver | `Warp: %d` or `none`, beside a gauge of the driver's warp against the best available |
| Destination | the planet packets are flung at, or `none`, beside a **Set Dest** button |

## What is *not* here

The planet's **environment** — gravity, temperature and radiation, with the
habitability value — is not in this pane, which is easy to assume it would be.
The tile table has no room for it: the environment gauges belong to
`DrawMineSurvey` (`1028:065a`), which fills a different pane of the frame. That
pane is its own spec.

## What this project does

`crates/stars-ui/src/views/planet.rs`, over methods on `App` that build each
tile's rows so the text can be tested without drawing it.

Reproduced: the two columns and the order of the tiles; Minerals On Hand and
Status in full, with the original's labels, formats and Alternate Reality
special cases; the Production tile's queue and its empty text; the Starbase
tile's title and its first rows; the fleets in orbit.

Not reproduced: the planet **picture** (this tile says in words what the picture
says at a glance), collapsing a tile by clicking its title, the mass driver and
destination rows and their gauge and button, the production tile's completion
line and Route button, and the editing that the original's list box allows —
the production queue is edited on the Planets screen instead.
