# The scanner's toolbar

Status: **recovered and reimplemented**.

`TbWndProc` (`1068:0000`) and `DrawToolbar` (`1068:06f0`). A single row above
the scanner holding **eighteen buttons and one combo box**, hidden and shown
with View (Toolbar). `MANUAL.PDF` p. 5-12: *"There are six exclusive scanner
views, which you can use with one or more overlays. Use the toolbar above the
scanner to select views and overlays."*

## The layout table

`DrawToolbar` walks a table of **29 signed bytes** left to right, starting at
x = 4:

```
0 1 2 3 4 5 -1 6 -1 7 -2 -3 -1 8 -1 9 -1 11 17 -1 10 -1 12 13 -1 14 15 -1 16
```

A non-negative entry is a button index; `-1` is a six-pixel gap, `-2` a
two-pixel one, and `-3` the coverage combo. `DxOfBtn` (`1068:0bb2`) gives the
widths: 29 for a button, 11 for buttons 13 and 15, 6 / 2 for the gaps, and 60
or 70 for the combo depending on the height of the font.

The table lives in the **toolbar's own code segment** at offset 0, which is why
the decompiler renders the read as `*(char *)i` — a load from nowhere. It is
recovered from the disassembly, where the instruction is
`MOV AL, byte ptr CS:[BX + 0x0]`.

**The buttons are not in the order they appear.** The table interleaves them —
`… 9, 11, 17, 10, 12, 13 …` — so laying them out is the only way to see the
row. And what comes out is exactly the order `MANUAL.PDF` pp. 5-12..5-15
introduces them in:

| # | button | what it is | shown pressed when |
|---|--------|-----------|--------------------|
| 0 | Normal | the default view | `grbitScan & 0xf == 0` |
| 1 | Surface Minerals | view | `== 1` |
| 2 | Mineral Concentration | view | `== 2` |
| 3 | Planet Value | view | `== 3` |
| 4 | Population | view | `== 4` |
| 5 | No Player Information | view | `== 5` |
| 6 | Add Waypoints Mode | mode | `& 0x10` |
| 7 | Scanner Coverage | overlay, with the combo beside it | `& 0x20` |
| 8 | Mine Fields | **opens a menu**, does not toggle | `& 0x40` **and** all four groups shown |
| 9 | Fleet Paths | overlay | `& 0x80` |
| 11 | Planet Names | overlay | `& 0x400` |
| 17 | Ship Count | overlay | `& 0x1000` |
| 10 | Idle Fleets | filter | `& 0x100` |
| 12 | Ship Design Filter | filter | `& 0x200` |
| 13 | …its menu | narrow | never |
| 14 | Enemy Ship Class Filter | filter | `& 0x800` |
| 15 | …its menu | narrow | never |
| 16 | Zoom | opens the zoom menu | never |

That agreement is what identifies the buttons whose pictures are ambiguous on
their own. Two are pinned independently: button 8 by `ExecuteButton`, which
opens a minefield menu with a per-owner toggle each (`grbitScanMines`, four
bits); and button 13 by the same function, which lists the player's own sixteen
designs against `grbitScanShip`. Between them they fix both ends of the row.

The six views are a **radio group**, not toggles: `ExecuteButton` sets
`grbitScan = itb + (grbitScan & 0x3ff0)`, replacing the low four bits and
leaving every overlay alone. Pressing the view already showing does nothing.

## The pictures

`hdibToolbar` (id 178) is 432 by 23 — **eighteen cells of 24 by 23, one row**,
and every button's picture is used exactly once. `DrawBitmapButton`
(`1068:078c`) draws a 3-D frame, then blits `ibtn * 0x18` from it. A narrow
button gets only the first **7** pixels of its cell, and a pressed button has
both frame and picture nudged a pixel down and right.

## The coverage combo

An **editable** combo, not a plain list. It reads its text, takes the leading
digits, insists that what follows is either nothing or a `%`, and clamps the
result to `2..=100`; anything else reads as zero and clamps up to 2. The list
it snaps to is `(100 - pct) / 10`, so it runs from 100% down in tens.

The manual explains what it is for (p. 5-13): the overlay is drawn as though
every scanner were only that effective, so a player can see how close a ship
with matching cloaking would get.

## The state it starts in

`stars.ini` supplies the defaults, and `InitStuff` names them:

| setting | default |
|---------|---------|
| `grbitScan` | `0xe0` — the Normal view, with **scanner coverage, mine fields and fleet paths already on** |
| `grbitScanMines` | `0xf`, masked to four bits — every group shown |
| `vpctRadarView` | `100`, capped there |
| `grbitScanShip`, `grbitScanEShip` | `0` — both ship filters empty |

There is a sanity check beside them: if `grbitScan & 0xc00f` is more than 5 —
a view out of range, or one of the top two bits set — `grbitScan` and the
design filter are both cleared.

## The Mine Fields menu

Button 8 **does not toggle**; it opens a menu. Two commands, a rule, then a
tick for each of four groups — yours, friends', neutrals', enemies' — held in
`grbitScanMines`, a bit each. The overlay's own bit is `0x40`.

Three things about it differ from the two ship filters, and all three are the
sort of detail that is invisible until you look:

* **The overlay follows the filter exactly.** Unticking the last group turns
  the overlay **off**, and ticking one turns it on. The ship filters only ever
  switch themselves *on*; this one switches both ways.
* **Opening the menu with the overlay off empties the filter first.**
  `ExecuteButton` clears `grbitScanMines` before building the menu when
  `grbitScan & 0x40` is clear, so a player who turned the overlay off and comes
  back finds nothing ticked rather than their old choice.
* **The button shows pressed only when all four are shown.** A narrowed menu
  leaves it up, which is how the toolbar says the overlay is on but limited
  without opening anything.

There is no invert: this menu has two commands where the ship filters have
three.

The four groups are the ones the relations table gives — see
[`stars_core::relations::Party`] — and they are the same grouping the manual
gives the minefield colours by (p. 5-14): yours blue, friends yellow, enemies
and neutrals red. The map shares a colour between the last two; the menu keeps
them apart.

## The two ship filters

Buttons 12/13 and 14/15 are each a toggle with a menu hung off it, and the
thing to get right is that **they apply to different fleets**. `CShipsScanVis`
(`1058:4bf4`), which is what counts the ships the Ship Count overlay writes:

* the **Ship Design filter** (`0x200`, mask `grbitScanShip`) narrows only *this
  player's own* fleets, and picks by **design slot** — bit `n` is design `n`;
* the **Enemy Ship Class filter** (`0x800`, mask `grbitScanEShip`) narrows only
  *everybody else's*, and picks by the hull's **class**, which it reads from
  `(huldef.wFlags >> 10) & 0xf` — the field this project transcribes as
  `Hull::category`;
* a fleet neither applies to is counted whole.

So the two never fight over the same fleet, and turning both on filters your
ships one way and theirs another at the same time.

The eight classes are Colony, Freighter, Scout, Warship, Utility, Bomber, Miner
and Fuel Transport, and every ship hull falls in one; see
[`stars_core::design::ShipClass`]. A **starbase** stores category 0, which
would read as a colony ship, so it is refused rather than misfiled.

Both menus open with the same three commands — all, invert, none — then a rule,
then the entries. The design menu lists only the slots that hold a design,
skipping any whose `fFree` bit is set, and **each entry keeps its own slot's
bit**: emptying a design does not renumber the others. Both menus are built
from the same three strings in the original, so the enemy one is headed "All
Designs" too.

One touch worth keeping: **ticking something while the overlay is off turns the
overlay on**, because there is no sense in choosing a design and seeing nothing
change. Unticking the last one does *not* turn it off again, and the "none"
command does not either — the original only ever switches it on.

## What is reproduced

The row and its exact layout — every gap, every width, the combo where the
table puts it — the eighteen pictures out of the game's own bitmap when a copy
of the original has been found, the pressed look and its one-pixel nudge, the
radio group, every overlay toggle this project models, all three menus — mine
fields and the two ship filters — the zoom menu with the nine sizes the
original offers, the combo's parsing and clamping, and the state the toolbar
starts in.

Without the game's pictures each button falls back to a short label, so the
toolbar works either way.

## What is not

* The filters narrow the **ship counts**; the manual also says they narrow
  which planets get orbit rings, and this project does not draw orbit rings.
* None of the three masks is kept in `stars.ini`, nor the view, the overlays or
  the coverage — the original keeps all of them there between sessions. The
  defaults above are honoured; the saving is not.
* **Tooltips** are egui's rather than the original's `ShowTooltip`.
* **View (Toolbar)** to hide the row: this frontend has no View menu.
* The scanner-coverage overlay does not yet **vary** with the percentage; the
  combo is read and kept, and the overlay is still drawn at full strength.

## Where Find went

The original keeps Find in the **View menu** (`&Find...\tCtrl+F`), not on the
toolbar. This project used to have a Find box on its own approximation of the
toolbar; with the real row in place that box moved to the frontend's menu bar,
where it does not misrepresent what the original's toolbar holds.
