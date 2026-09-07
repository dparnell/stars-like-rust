# The fleet pane

Status: **layout and text recovered**; the tiles this project has the data for
are reimplemented, the rest are named below.

There is no separate fleet window. The fleet pane is the **planet pane's
window** showing a different set of tiles: `PlanetWndProc` (`1048:0000`) draws
`rgtilePlanet` when a planet is selected and `rgtileShip` (`1120:090e`) when a
fleet is, and `SetPlanetTitleBar` (`1048:3dec`) puts the fleet's name in the
title bar instead of the planet's.

## The tiles

Seven 16-byte `TILE` records, read out of the binary the same way as the planet
pane's:

| # | column | height | draws | title |
|---|--------|--------|-------|-------|
| 0 | left | 85 | `DrawPlanShipBitmap` (`1048:3336`) | the fleet, as a picture |
| 1 | left | 5 | `DrawShipPlanet` (`1050:17b6`) | the planet it is at, or **`In Deep Space`** |
| 2 | left | 19 | `DrawShipOrders` (`1050:0000`) | **Fleet Waypoints** |
| 3 | left | 12 | `DrawShipWayPtOrders` (`1050:0912`) | **Waypoint Task** |
| 4 | right | 14 | `DrawShipCargo` (`1050:1a54`) | **Fuel & Cargo** |
| 5 | right | 16 | `DrawFleetComp` (`1050:1e72`) | **Fleet Composition** |
| 6 | right | 22 | `DrawPlanetShipList` (`1048:377e`) | **Other Fleets Here** |

The last tile is shared with the planet pane — the same routine, in the same
place on the right — so whichever you have selected, the pane's bottom right
always answers "what else is here?".

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

Reproduced: the tile order and columns; the location tile's title including
`In Deep Space`; the whole waypoints table, with travel time computed as the
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

Not reproduced: the mining rate row, the *Fuel & Cargo* tile's own gauges (that
tile gives the figures as text), and the buttons — Battle Plans, Jettison and
Xfer on tile 4, and the three along the foot of the last tile — whose dialogs
this project does not have.

One thing this project has to say that the original does not: a fleet id is the
**player's own numbering**, so two players each have a fleet 1. The dropdown
remembers what is chosen by owner *and* id, which is what the original's own
combo item data amounts to.
