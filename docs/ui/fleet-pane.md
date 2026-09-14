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

What the fleet will do when it arrives. `DrawShipWayPtOrders` (`1050:0912`)
draws the tile and `UpdateOrdersDDs` (`1050:93ee`) fills its dropdowns, and
both work on **`sel.iwpAct`** — the waypoint the scanner has in hand, not
always the first leg. Set a different waypoint on the map and this tile follows
it.

It is up to three controls deep.

**The task**, across the whole tile. Ten entries, strings `0x63` to `0x6c`, one
per id in order, so the caption *is* the id:

| id | caption | id | caption |
|----|---------|----|---------|
| 0 | `(no task here)` | 5 | `Scrap Fleet` |
| 1 | `Transport` | 6 | `Lay Mine Field` |
| 2 | `Colonize` | 7 | `Patrol` |
| 3 | `Remote Mining` | 8 | `Route` |
| 4 | `Merge with Fleet` | 9 | `Transfer Fleet` |

**A second choice**, for the four tasks that need one. The ten payload bytes
are a union, and each task reads its setting from a different word:

| task | word | list |
|------|------|------|
| `Lay Mine Field` | 0 | ` for 1 year ` … ` for 5 years`, then `iindefinitely` |
| `Patrol` | **1** | ` within %d l.y.`, then ` any enemy` — labelled `Intercept` |
| `Transfer Fleet` | 0 | the other players, named with capital, plural and "the" |
| `Transport` | 0–4 | one word per cargo |

Patrol's setting is in word **one** because word zero is the warp it patrols
at, which the original draws as a gauge under the dropdown.

`iindefinitely` is not a transcription slip. String `0x386` really does carry a
doubled letter where its neighbours in the same list (` within %d l.y.`,
` any enemy`, ` for %d year%c`) all have a leading space, so the space was
typed as an `i`. The table is transcribed rather than corrected.

**Transport's cargo table**, an action and a quantity for each of the five
kinds, shown **one cargo at a time**: a cargo dropdown — the tile's second —
then an action dropdown for that cargo and a quantity box where the action
takes one, with the blue diamond beside the cargo row. The cargo dropdown
lists **fuel first** and then the four hold kinds: `UpdateOrdersDDs` maps its
index 0 to slot 4 and index *n* to slot *n* − 1, while the stored words stay
in slot order. Each word is packed
`quantity:12, action:4`, so no quantity can exceed 4095.

The ten actions are strings `0x6d` to `0x76`, again one per code in order —
`(no action)`, `Load All Available`, `Unload All`, `Load Exactly...`,
`Unload Exactly...`, `Fill Up to %...`, `Wait for %...`, `Load Dunnage`,
`Set Amount to...`, `Set Waypoint to...`. Two details are worth having:

* **`Load Dunnage` becomes `Load Optimal`** (string `0x77`) when the cargo
  chosen is fuel. It is the only entry whose wording depends on the cargo.
* The quantity box is **greyed** for codes 0, 1, 2 and 7 — the four that say
  "all", "none" or "whatever is left" and so need no figure.

The unit beside the box follows the cargo — `kT` for the three minerals, `00`
for colonists, who are counted in hundreds, `mg` for fuel — except that
`Fill Up to %` and `Wait for %` override all three with `%`.

**The note**, under everything, red when it is a warning. Which note appears
when is the original's; the wording in this project is its own, since the
original's notices are authored prose.

| task | note |
|------|------|
| `Scrap Fleet` | the fleet is broken up, some minerals recovered |
| `Merge with Fleet` | **warning** unless the waypoint's class is a fleet |
| `Colonize` | **warning** with no colonists aboard, otherwise the dismantling note |
| `Lay Mine Field` | how many mines a year, or a **warning** with no layer aboard |

### Choosing Merge with Fleet picks the fleet

`ShipCommandProc` (`1050:2640`), on the task dropdown changing: the task
is written, the ten payload bytes zeroed — Patrol clears its two words,
Transfer its one, and **Lay Mine Field starts at five years** — and then,
for Merge with Fleet on a waypoint that does not already name a fleet
(`1050:3653`), a fleet is found for it. The design this fleet has most of
is the key; the player's own live fleets standing on the waypoint's
point, other than this one, are walked in list order: one with ships of
that design is taken, and the search ends at the first such that has no
orders beyond where it stands; failing any, the first fleet at all is
taken, a fleet with no further orders replacing one that has. The
waypoint's class becomes *fleet* and its id that fleet's full word, which
is how page 48's Prune reads *Cotton Picker #6* once the Mini-Miner is
told to merge there. With nobody there the task stays on the planet
waypoint, which is the warning above.

### Where the patrol list parts company

The binary's own list has **twelve** entries: eleven ` within %d l.y.` from 50
to 550 in fifties, then ` any enemy`. This project's
`stars_core::patrol::patrol_range` — written from `save.c` — treats the
setting that would be 550 as "as far as it takes" instead, giving eleven. The
two cannot both be right, and this has not been settled against our own
binary: the multiply-by-fifty the range would need does not appear in it, so
the reader has yet to be found.

The tile is built from `patrol_range` rather than from the original's list, so
that what it offers is exactly what the engine honours. It is the one place in
this pane where the community reconstruction has been followed over the
resource, and it is marked so it can be put right when the reader turns up.

## Fleet Waypoints

`DrawShipOrders` (`1050:0000`): a list box four rows tall of the fleet's
waypoints (a click takes one in hand, as a click on the map does), then one
location row — `Next Way Pt` while the fleet's own position is in hand,
`Coming From` and the stop before when a later waypoint is — and the leg
into the waypoint in hand: `Distance`, `Warp Factor` (a gauge when a later
waypoint is in hand), `Travel Time` and `Est Fuel Usage` in red when it is
more than the tank holds. Along the foot the **Repeat Orders** checkbox
(`fRepOrders`) and a blue diamond.

## Fuel & Cargo

`DrawShipCargo` (`1050:1a54`): `Fuel` and a gauge, `Cargo` and a gauge, then
ironium, boranium, germanium and colonists as figures, each name in its own
colour. A press in either gauge is the Xfer button.

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
model; the **waypoint task**, editable, for the waypoint the map has in hand —
the task list, the second choice each task needs and where each keeps it, the
whole Transport cargo table with its packing and its greying rules, and the
four notes; fuel and cargo; the composition; and the fleets-here
tile shared with the planet pane.

The fleet picture is drawn from the game's own ship sheets when a copy of the
original has been found — see `../formats/resources.md` — with the owner's race
emblem over its bottom-left corner, as `DrawFleetBitmap` overlays it.

The first tile is laid out as `DrawPlanShipBitmap` (`1048:3336`) lays it out:
the picture twelve pixels in and six down (two in the small layout), and the
buttons — Prev, Next and Rename for a fleet; Prev and Next for a planet — in a
**column to its right**, each `dyArial8 * 3 / 2` tall and three apart,
starting two pixels above the picture, and as wide as the tile's inside less
95. The tile has no words of its own in the original: the picture and the
buttons are the whole of it. This used to flow the buttons in a row under
three lines of text, which put them below the tile's foot where they could be
neither seen nor pressed — the tutorial's page 4 ("press the Next button in
the tile showing Long Range Scout #2") is what found it.

The Fleets in Orbit tile's three buttons are pinned to its foot for the same
reason, three abreast.

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

Not reproduced: the two notes that need a component scan this project does not
yet do — Colonize's "no colonisation module aboard" warning, and the whole of
Remote Mining's, which is either a mining-rate estimate in the three mineral
colours or one of three warnings; the patrol **warp** gauge under the Intercept
dropdown, which is the other half of that task's payload; persisting the open
tiles to `stars.ini`; the small-window
layout, since the frame has no `fSmallTiles` to set; the mining rate row, the
*Fuel & Cargo* tile's own gauges (that tile gives the figures as text), and the
buttons — Battle Plans, Jettison and
Xfer on tile 4, and the three along the foot of the last tile — whose dialogs
this project does not have. The location tile's own **Xfer** is there, and
opens the Cargo Transfer dialog (`cargo-transfer.md`); its **Jettison**, for
a fleet in deep space, is drawn but dead.

One thing this project has to say that the original does not: a fleet id is the
**player's own numbering**, so two players each have a fleet 1. The dropdown
remembers what is chosen by owner *and* id, which is what the original's own
combo item data amounts to.
