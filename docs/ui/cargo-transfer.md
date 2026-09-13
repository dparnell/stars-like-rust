# UI: the Cargo Transfer dialog

- **Status:** in progress — fleet to planet and the Ship Transfer mode
  (the **Split** button) done; fleet to fleet and deep-space jettison still
  to come
- **Ghidra routine(s):** `TransferDlg` (`1050:5686`), `DrawXferDlg`
  (`1050:6908`), `GetXferLeftRightRcs` (`1050:6b46`), `FSetupXferBtns`
  (`1050:6bea`), `DrawFleetCargoXferSide` (`1050:72de`), `DrawPlanetXferSide`
  (`1050:79ca`), `FTrackXfer` (`1050:5a16`), `XferSupply` (`1050:64cc`),
  `LogMakeValidXfer` (`1048:99f6`)
- **Manual reference:** `MANUAL.PDF`, the Fleet pane's Xfer button
- **Implemented in:** `crates/stars-ui/src/views/transfer.rs`,
  `App::open_xfer` and the `xfer_*` methods, `App::transfer_cargo_many`;
  `crates/stars-ui/src/views/split.rs`, `App::open_split` and the
  `split_*` methods, `App::split_fleet_many`

The **Xfer** button on the fleet pane's location tile ("Orbiting Stove Top")
opens it. The tutorial uses it on pages 12, 14, 19, 23 and 26 to load
colony ships and freighters.

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
* on the fleet's side each row is a **gauge** from 81 pixels in to four from
  the edge (`DrawFleetGauge`) — fuel against the tank, the cargo total
  against the hold, and each kind against the hold;
* on the planet's side the four holds are **figures** in sunken frames
  (`%ld kT`), and there is no fuel row;
* another player's fleet gets figures instead of gauges.

Down the middle (`FSetupXferBtns`) go pairs of arrow buttons, `dyArial8 + 3`
square, one pair for each of the four holds and one for fuel, each pair on
its row — none for the cargo total. The fuel pair is left out when the other
side is a space object. Over every fleet gauge lies an invisible button that
makes it a drag target.

## What the controls do

`FTrackXfer`:

* an **arrow** moves one unit while held, ten with Shift, a hundred with
  Ctrl, a thousand with both. The **left** arrow moves cargo **into** the
  fleet (`XferSupply` with a positive count: what the right side has, as far
  as the left has room), the **right** arrow out of it;
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

`App::open_xfer` copies the fleet and the planet it orbits into an
`XferDialog`; `xfer_move` is an arrow, `xfer_set` a gauge, `xfer_ok` logs
the net change through `transfer_cargo_many` as one order and `xfer_cancel`
forgets it. Fuel is offered only at a planet with a starbase, since a planet
has no tank of its own. The view records its arrows, its gauges (by row
label, `"Colonists gauge"`) and its three buttons under the scope `"xfer"`,
which is how `tutorial_ui.rs` loads the colony ship on page 12.

## The Split button: Ship Transfer

The same template, titled `Ship Transfer`, with a row for each of the
fleet's designs instead of the cargo rows: the fleet in hand on the left,
the new fleet on the right (named for the next free fleet number, which is
how page 26 knows the split-off ships become **Fleet #10**), a pair of
arrows a row, and OK, Cancel and Help along the foot. The right arrow moves
one ship of that design across, ten with Shift and so on as the cargo arrows
do; the left arrow brings one back. OK logs the split as one order through
`App::split_fleet_many`, which — as `LpflNewSplit` (`1038:3372`) does —
gives the new fleet a copy of every waypoint the old one had. Cancel drops
it. The view records its arrows as `"{design} <"` and `"{design} >"` and
its buttons under the scope `"split"`; a fleet of one design and one ship
has nothing to split and the button is dead.

Not yet: the fleet-to-fleet form (the fleets-here tile's Xfer), Jettison in
deep space, and the Robber Baron's take from a space object.
