# UI: the Cargo Transfer dialog

- **Status:** in progress — fleet to planet, fleet to fleet (the
  fleets-here tile's **Cargo**), the planet pane's form, and the Ship
  Transfer mode (**Split**, and the tile's **Merge**) done; deep-space
  jettison and mineral packets still to come
- **Ghidra routine(s):** `TransferDlg` (`1050:5686`), `DrawXferDlg`
  (`1050:6908`), `GetXferLeftRightRcs` (`1050:6b46`), `FSetupXferBtns`
  (`1050:6bea`), `DrawFleetCargoXferSide` (`1050:72de`), `DrawPlanetXferSide`
  (`1050:79ca`), `FTrackXfer` (`1050:5a16`), `XferSupply` (`1050:64cc`),
  `LogMakeValidXfer` (`1048:99f6`)
- **Manual reference:** `MANUAL.PDF`, the Fleet pane's Xfer button
- **Implemented in:** `crates/stars-ui/src/views/transfer.rs`,
  `App::open_xfer`, `App::open_xfer_with_fleet_here`,
  `App::open_xfer_between` and the `xfer_*` methods,
  `App::transfer_between`; `crates/stars-ui/src/views/split.rs`,
  `App::open_split`, `App::open_merge_with_fleet_here` and the `split_*`
  methods, `App::split_fleet_many`, `App::transfer_ships`

Three buttons raise it, each through `TransferStuff` (`1050:51fe`) with
two objects and a mode:

* the **Xfer** button on the fleet pane's location tile ("Orbiting Stove
  Top"): the fleet in hand and the planet it orbits (`ShipCommandProc`,
  `rghwndBtn[7]`). The tutorial uses it on pages 12, 14, 19, 23 and 26 to
  load colony ships and freighters;
* the **Cargo** button on the Other Fleets Here tile: the fleet in hand
  and whatever the tile's dropdown shows (`rghwndBtn[0]`, looked up with
  `FLookupOrbitingXfer`) — another fleet, the player's own or not;
* the same button on the planet pane's Fleets in Orbit tile: the
  **planet** on the left and the chosen fleet on the right
  (`PlanetWndProc`).

`TransferStuff` copies both objects into `pxfer` (`FLookupObject`) and the
dialog works on the copies; on OK it stores the left copy and then the
right (`FLookupFleet(-1, …)` / `FLookupPlanet(-1, …)`), and
`LogChangeFleet` / `LogChangePlanet` fold the two stores into one
`LogMakeValidXfer` record naming the left object first with each quantity
what it gained.

## The template

Dialog resource `Cargo Transfer`, at `0x3461d5` in the executable: 280 by
163 dialog units and three controls, all along the foot — **OK** (id 1) at
(155, 145), **Cancel** (id 2) at (200, 145) and **Help** (`0x76`) at
(245, 145), each 30 by 13. Everything above them is painted. The same
template serves the **Split** button, titled `Ship Transfer`
(`idsShipTransfer`), and is stretched half again when a fleet has more than
ten designs to split.

## What is painted

`GetXferLeftRightRcs` cuts the client area in two at the middle, insets each
half by four vertically and by `4 + (dyArial8 + 3)` horizontally, then lets
each half back out by `dyArial8 + 1` on its outer side. In each half:

* a **square** as wide as the half, in a raised frame, with a **title bar**
  a line and two tall carrying the fleet's or planet's name;
* six **rows** under it, a line and six apart, starting three under the
  title bar, their labels right-aligned 75 pixels in: `Fuel`, `Cargo`,
  `Ironium`, `Boranium`, `Germanium`, `Colonists` (`idsFuel` and the five
  after it);
* a fleet of the player's own has a **gauge** on every row, from 81 pixels
  in to four from the edge (`DrawFleetGauge`) — fuel against the tank, the
  cargo total against the hold, and each kind against the hold;
* another player's fleet (`iPlayer != idPlayer`) has all six labels but
  **figures** in sunken frames where the gauges would be — `%ld mg` for
  fuel, `%ld kT` for the holds — and nothing on the cargo row
  (`DrawFleetCargoXferSide`);
* a planet has figures for the four holds (`%ld kT`) and neither a fuel
  row nor a cargo row (`DrawPlanetXferSide`).

Down the middle (`FSetupXferBtns`) go pairs of arrow buttons, `dyArial8 + 3`
square, one pair for each of the four holds and one for fuel, each pair on
its row — none for the cargo total. The fuel pair is left out when the other
side is a space object. `UpdateXferBtns` (`1050:66a8`) deadens an arrow
whose giver has none of the kind (`ChgCargo` with no change reads the
figure) or whose taker, if a fleet, has no room (`GetFuelFree`,
`GetCargoFree`); a planet's fuel arrow is always dead. Over every gauge
of the player's own lies an invisible button that makes it a drag target
(`FTrackXfer` refuses the drag on a side whose `iPlayer` is not the
player's).

## What the controls do

`FTrackXfer`:

* an **arrow** moves one unit while held, ten with Shift, a hundred with
  Ctrl, a thousand with both. The **left** arrow moves cargo **into the
  left side** (`XferSupply` with a positive count: what the right side has,
  as far as the left has room — `ChgCargo` on each copy), the **right**
  arrow into the right. `ChgCargo` (`1050:6034`) moves no fuel on a planet,
  which has no tank, and no colonists on a fleet not known in full
  (`det != 7`, another player's), so minerals and fuel can be handed to a
  stranger's fleet but nobody's people move;
* a **press or drag in a gauge** reads the pointer's distance along the bar
  (less two pixels of frame) as a share of the tank or hold, and moves the
  difference between that and what is aboard. Dragging to the far end fills
  the hold, which is what page 12 means by "drag in the Colonists gauge
  until the hold carries 25kT".

The dialog works on **copies** of both sides (`pxfer`), so every move shows
at once and nothing reaches the game until **OK**, when `TransferStuff`
writes the difference as one `LogMakeValidXfer` order — a bit per cargo kind
moved and one quantity per bit. **Cancel** drops the lot.

## What this project does

`App::open_xfer_between` copies two objects — `XferObject::Fleet` or
`XferObject::Planet` — into an `XferDialog` with what each has, each
side's tank and hold, and which sides are the player's own; `open_xfer`
is the location tile's pair and `open_xfer_with_fleet_here` the
fleets-here tile's, in either pane. `xfer_move` is an arrow (positive into
the left side), `xfer_set` a gauge on one side, `xfer_ok` logs the left
side's net change through `transfer_between` as one order — fleet and
planet, or fleet and fleet, the classes in the record's mode byte — and
`xfer_cancel` forgets it. Fuel moves only between two fleets. The view
records its arrows, its gauges (by row label, `"Colonists gauge"`, and
`"Colonists gauge (right)"` for the right side) and its three buttons
under the scope `"xfer"`, which is how `tutorial_ui.rs` loads the colony
ship on page 12.

## The Split and Merge buttons: Ship Transfer

The same template, titled `Ship Transfer`, with a row for each design
aboard either side (`rgXferValidHulls`, in slot order) instead of the
cargo rows: the fleet in hand on the left, on the right either a new fleet
(**Split**, `LpflNewSplit`; named for the next free fleet number, which is
how page 26 knows the split-off ships become **Fleet #10**) or the fleet
the Other Fleets Here tile is showing (the tile's **Merge**,
`ShipCommandProc` `rghwndBtn[2]`, `TransferStuff(…, 1)` with two fleets),
a pair of arrows a row, and OK, Cancel and Help along the foot. The right arrow moves
one ship of that design across, ten with Shift and so on as the cargo arrows
do; the left arrow brings one back. OK logs the split as one order through
`App::split_fleet_many`, which — as `LpflNewSplit` (`1038:3372`) does —
gives the new fleet a copy of every waypoint the old one had. Cancel drops
it. Between two existing fleets OK logs one ships record through
`App::transfer_ships` — the fleet in hand named first, each quantity what
it gained, so negative for ships crossing to the other — and a fleet
left with no ships is gone (`FDeleteFleet` in `TransferStuff`; the replay
drops an emptied fleet). The view records its arrows as `"{design} <"`
and `"{design} >"` and its buttons under the scope `"split"`; a fleet of
one design and one ship has nothing to split and the button is dead.

Not yet: Jettison in deep space, and mineral packets in the tile's
dropdown with the Robber Baron's take from one.
