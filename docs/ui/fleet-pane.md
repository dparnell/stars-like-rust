# The fleet pane

Status: **layout, geometry and text recovered**; the tiles this project has the
data for are reimplemented, the rest are named below.

There is no separate fleet window. The fleet pane is the **planet pane's
window** showing a different set of tiles: `PlanetWndProc` (`1048:0000`) draws
`rgtilePlanet` when a planet is selected and `rgtileShip` (`1120:090e`) when a
fleet is, and `SetPlanetTitleBar` (`1048:3dec`) puts the fleet's name in the
title bar instead of the planet's.

## The tiles

Seven 16-byte `TILE` records, read out of the binary the same way as the planet
pane's — so the same two words per record, a **line count** and a pixel
remainder that `InitTiles` folds together as `dyFull + yTop * dyArial8`. See
`planet-pane.md` for the frame, the geometry and the open flag, which are
shared.

| # | column | lines | extra | `grbit` | draws | title |
|---|--------|-------|-------|---------|-------|-------|
| 0 | left | 1 | 85 | `0x80` | `DrawPlanShipBitmap` (`1048:3336`) | the fleet, as a picture |
| 1 | left | 3 | 5 | `0x40` | `DrawShipPlanet` (`1050:17b6`) | the planet it is at, or **`In Deep Space`** |
| 2 | left | 11 | 19 | `0x20` | `DrawShipOrders` (`1050:0000`) | **Fleet Waypoints** |
| 3 | left | 6 | 12 | `0x100` | `DrawShipWayPtOrders` (`1050:0912`) | **Waypoint Task** |
| 4 | right | 7 | 14 | `0x01` | `DrawShipCargo` (`1050:1a54`) | **Fuel & Cargo** |
| 5 | right | 12 | 16 | `0x200` | `DrawFleetComp` (`1050:1e72`) | **Fleet Composition** |
| 6 | right | 6 | 22 | `0x04` | `DrawPlanetShipList` (`1048:377e`) | **Other Fleets Here** |

**Four** tiles down the left column here where the planet pane has three. The
last is shared with the planet pane — the same routine, in the same place on
the right — so whichever you have selected, the pane's bottom right always
answers "what else is here?".

### `grbit` does not mean the same thing in both tables

`EnsureTileSize` (`1048:58df`) walks the two tables in **two loops with
different rules**, so a tile's resize cannot be read off its `grbit`:

| `grbit` | planet table | ship table |
|---------|--------------|------------|
| `0x80` | 10 | 10 |
| `0x04` | `(dyArial8 + 4) * 2` | `(dyArial8 + 4) * 2` |
| `0x40` | `(dyArial8 + 2) * 2` — the production queue | 6 — the location tile |
| `0x01` | nothing — Minerals On Hand | `dyArial8 * 4 + 2` — Fuel & Cargo |
| `0x08` | nothing | — |
| `0x100` | nothing | 2 |
| `0x20` | — | `dyArial8 + 9` |
| `0x200` | — | `dyArial8 * 3 + 8` |

Only `0x80` and `0x04` agree, and `0x04` is the one tile the two tables really
do share. Everything else has to be carried per record.

## Other Fleets Here

`DrawPlanetShipList` does not decompile — the decompiler dies on it — so this
was read out of the binary instruction by instruction.

It titles itself by which pane it is in: **`Fleets in Orbit`** (string `0x0338`)
when a planet is selected and **`Other Fleets Here`** (`0x0339`) when a fleet
is, the "other" being that fleet, which it leaves out — the caller passes it as
`idSkip`.

The tile is not a list of names. It is a **dropdown** of what is here, sized to
the tile and placed over it, and under that a **`Fuel `** gauge and a
**`Cargo `** gauge (strings `0x02ea` and `0x02e9`) for whichever is chosen,
their labels aligned on the wider of the two. Below them the routine positions
three buttons across the tile's foot.

Two states turn the gauges off, and both are reproduced:

* **nothing selected** — the combo returns `CB_ERR`, and the tile draws no
  gauges and disables its buttons;
* **not known in full** — the object's detail is not `7`. Full detail is only
  ever had of one's own fleets, so somebody else's is listed but not measured.

The cargo gauge is drawn by a different routine from the fuel gauge
(`1110:044e` against `1050:44b6`) because it is **segmented**: a bar each for
ironium, boranium and germanium, and one for colonists, in the colours the game
gives them elsewhere. That is reproduced too.

## Where it is

`DrawShipPlanet` titles its tile with the planet the fleet is orbiting, or
`In Deep Space` when it is not. Over a planet it also shows
`Mining Rate per Year:` — what the fleet's mining robots would bring up.

## Fleet Waypoints

A table of the leg the fleet is on:

| row | notes |
|-----|-------|
| `Coming From` | where this leg started |
| `Next Way Pt` | where it ends |
| `Warp Factor` | the speed set for the leg |
| `Distance` | in light years |
| `Travel Time` | the distance over the square of the warp |
| `Est Fuel Usage` | `%ldkT` |

There is a **Battle Plans...** button on this tile.

## Waypoint Task

What the fleet will do when it arrives, named from the same list the survey pane
uses.

## Fuel & Cargo

Fuel, then ironium, boranium, germanium and colonists, with **Jettison** and
**Xfer** buttons on the tile.

## Fleet Composition

One row per design in the fleet, with how many of it.

## What this project does

`crates/stars-ui/src/views/fleet.rs`, over methods on `App`, and the desktop
shell swaps it for the planet pane exactly as the original swaps tile tables —
one pane, chosen by what is selected.

Reproduced: the **table** itself, in `crates/stars-ui/src/tiles.rs`, with each
tile's height as `InitTiles` computes it and its own `EnsureTileSize` resize
carried per record rather than guessed from `grbit`; the geometry, frame and
title bars, which are the planet pane's and shared with it; **collapsing a tile
by clicking its title bar**, with the column reflowing under it; the tile order
and columns; the location tile's title including `In Deep Space`; the whole waypoints table, with travel time computed as the
distance over the square of the warp and the fuel estimate from the engine's own
model; the waypoint task; fuel and cargo; the composition; and the fleets-here
tile shared with the planet pane.

The fleet picture is drawn from the game's own ship sheets when a copy of the
original has been found — see `../formats/resources.md` — with the owner's race
emblem over its bottom-left corner, as `DrawFleetBitmap` overlays it.

Which ship stands for the fleet is `IshdefPrimaryFromLpfl` (`1038:3e1c`): the
design with the most ships, compared strictly so a tie stays with the earlier
design slot. One twist, and it is narrow — a **fuel transport**, hull 25 or 26,
has its count docked by one once chosen, so it loses a tie it would otherwise
have won and nothing more. A tanker that really is the most numerous ship still
holds the picture. A fleet of more than one design is marked `+n` beside the
picture rather than drawn as all of them.

Also reproduced: the last tile as described above — its two titles, the
dropdown, the skipped selection, and the two gauges with the cargo one
segmented by mineral.

Not reproduced: persisting the open tiles to `stars.ini`; the small-window
layout, since the frame has no `fSmallTiles` to set; the mining rate row, the
*Fuel & Cargo* tile's own gauges (that tile gives the figures as text), and the
buttons — Battle Plans, Jettison and
Xfer on tile 4, and the three along the foot of the last tile — whose dialogs
this project does not have.

One thing this project has to say that the original does not: a fleet id is the
**player's own numbering**, so two players each have a fleet 1. The dropdown
remembers what is chosen by owner *and* id, which is what the original's own
combo item data amounts to.
