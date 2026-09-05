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
| 6 | right | 22 | `DrawPlanetShipList` (`1048:377e`) | the other fleets here |

The last tile is shared with the planet pane — the same routine, in the same
place on the right — so whichever you have selected, the pane's bottom right
always answers "what else is here?".

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

Not reproduced: the fleet picture and its owner's emblem, the mining rate row,
the fuel and cargo *gauges* (the figures are given as text), and the three
buttons — Battle Plans, Jettison and Xfer — whose dialogs this project does not
have.
