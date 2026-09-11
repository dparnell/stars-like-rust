# The report windows

Status: **verified** — the columns, the sort, the grid, the per-column
click routing, all seven of its pop-ups, all four tables and the three
tutorial pages that watch the sort are done.

Four windows share one piece of code: **Planets**, **Fleets**, **Others'
Fleets** and **Battles**, listed together on the Report menu, all four
showing F3 — which is one key that walks round them rather than four
accelerators for one command; see `menus.md`. They are the same window with a different `irpt`, and all of it
lives in segment `0x1108`:

| routine | address | what it does |
|---------|---------|--------------|
| `ReportDlg` | `1108:0018` | the window: sizing, scrollbars, clicks |
| `DrawReport` | `1108:0bae` | the header row and the grid |
| `DrawReportItem` | `1108:3398` | one cell |
| `DxReportColHdr` | `1108:305e` | a column's header text and its width |
| `SortReportCache` | `1108:589c` | which rows, and in what order |
| `ICompReport` | `1108:5bb8` | the comparator |
| `ReportColumnPopup` | `1108:74d4` | the menu a header opens |
| `ExecuteReportClick` | `1108:7cd6` | what clicking a row does |

## The state each report keeps

`RPT` is `0x36` bytes, and the four live side by side at `1120:1494`
(`vrptPlanet`, `vrptFleet`, `vrptEFleet`, `vrptBattle`). Every one of them
starts out identical apart from `irpt` and the column count:

| field | offset | start | meaning |
|-------|--------|-------|---------|
| `grbitVisible` | `+0x00` | `0x0000ffff` | one bit per column, set when shown |
| `irpt` | `+0x04` | 0–3 | which report |
| `cFields` | `+0x06` | 15/12/12/15 | how many columns |
| `cFieldFirst` | `+0x08` | 1 | leftmost scrolled-to column |
| `icolSort` | `+0x0a` | 0 | the column sorted on |
| `fAscending` | `+0x0c` | 1 | direction |
| `irowFirst` | `+0x0e` | 0 | first row shown |
| `ptDlg` | `+0x10` | (-1,-1) | where the window was left |
| `ptSize` | `+0x14` | (600,400) | how big it was left |
| `rgbdx[16]` | `+0x1a` | 0 | each column's width, in **half** pixels |
| `cRows` / `cRowsVis` | `+0x2a` | 0 | rows held, rows that fit |
| `iSubsort` | `+0x2e` | 0 | which mineral, for the columns that ask |

`vprptCur` points at whichever is open, and is null when none is — which is
how the tutorial asks "is a report up?".

## The columns

Headers come from four runs of consecutive strings, `iCol` added to the
first: `idsPlanetName` (`0x459`), `idsFleetName` (`0x472`), `idsFleetName2`
(`0x47e`) and `idsLocation4` (`0x48a`).

**Planets** (15): Planet Name, Starbase, Population, Cap, Value, Production,
Mine, Fact, Defense, Minerals, Mining Rate, Min Conc, Resources, Driver Dest,
Routing Dest.

**Fleets** (12): Fleet Name, ID, Location, Destination, ETA, Task, Fuel,
Cargo, Composition, Cloak, Battle Plan, Mass.

**Others' Fleets** (12): Fleet Name, ID, Location, Warp, Mass, Composition,
\# of Ships, Unarmed, Scout, Warship, Bomber, Utility.

**Battles** (15): Location, SB, Sides, Units, Ours, Theirs, Unarmed, Scout,
Warship, Bomber, Utility, Our Dead, Their Dead, Ours Left, Theirs Left.

The five class columns count hulls by the category in `HULDEF+0x7b`:
Scout is 2, Warship 3, Utility 4, Bomber 5, and **Unarmed** is everything
else — colony ships, freighters, miners and tankers together. Battles count
through `CBattleUnits`, whose `grbitBU` is a bitmask: bit 0 our tokens, bit 1
theirs, bit 2 include starbases, and bits 3–7 the five classes. So `0xff` is
Units, `0xfd` Ours, `0xfe` Theirs, and `5` and `6` — bit 2 with no class bit,
which only a starbase can satisfy — are what the **SB** column asks to decide
whether the base there was ours (`O`) or theirs (`T`).

### Widths

`DxReportColHdr` measures the header in the dialog font and then widens it by
a rule per column — twice the text for a planet name, four and a half times
for a fleet's Task, at least fifteen digit-widths for a planet's Starbase —
and rounds the answer up to an even number of pixels, which is why `rgbdx`
can store it in a byte. Nothing here is a stored constant; every width is
computed from the text on the day.

## The sort

### You do not click a header to sort

`ReportDlg` sends `WM_LBUTTONDOWN`, `WM_LBUTTONDBLCLK` and `WM_RBUTTONDOWN`
to the same place. A click **anywhere in the header row** opens a menu, and
the sort is an item on it. The button only decides which one
`TrackPopupMenu` is told to watch for.

The menu, for an ordinary column:

```
Sort by Population
Reverse Sort by Population
────────────────
Hide the Population column
────────────────
Show the Fact column          ← one per hidden column
```

Column 0 is the exception: it is the column that never scrolls off, and it
cannot be hidden. The original writes its Hide item anyway and then lets the
first Show item overwrite it — the entry is built and thrown away, which is
worth knowing only because it explains why the code looks the way it does.
When nothing is hidden the trailing separator goes too.

### Four columns sort by a chosen mineral

The planets' **Minerals**, **Mining Rate** and **Min Conc**, and the fleets'
**Cargo**, open onto a submenu instead:

```
Sort by Min Conc  ▸  Ironium
                     Boranium
                     Germanium
                     ────────────────
                     Weighted Average
Reverse Sort by Min Conc  ▸  (the same four)
```

The names come from a table of `char *` at `1120:04cc` — Ironium, Boranium,
Germanium, Colonists, Fuel — of which the planets' columns take three and the
fleets' Cargo takes four. The last item is `idsWeightedAverage` (`0x55f`) and
sets `iSubsort` past the end of the list, which every comparator reads as
"all of them": for Min Conc that is genuinely an average of a sort, for the
other three a plain sum.

`ReportColumnPopup` hands `PopupMenu` a flat array with two markers in it — a
null entry opens a submenu whose title is the entry after it and whose
contents run to the next null, and `"\xff"` is a separator — and then works
out what was chosen from the **flat index** rather than from any record of
what it built:

```c
iSubsort  = (iSel - 2) % (5 + 3 + (irpt == 1));
if (iSubsort > 3 + (irpt == 1)) iSubsort = 3 + (irpt == 1);
fAscending = iSel < 7;
```

### One bug, reproduced

`fAscending = iSel < 7` is right for a three-mineral submenu, where Weighted
Average sits at index 6 and the reverse submenu starts at 8. The fleets'
Cargo column has a fourth name in it, which pushes Weighted Average to index
7 — and the direction test does not follow it. **Sort by Cargo → Weighted
Average sorts descending**, from either submenu. This project reproduces it,
with a test that says so.

### The previous column breaks ties

This is the part worth knowing. Sorting does not simply forget what you
sorted by last:

```c
vicolSortPrev  = vprptCur->icolSort;
viSubsortPrev  = vprptCur->iSubsort;
vfAscendingPrev = vprptCur->fAscending;
vprptCur->icolSort = icol;
```

and `ICompReport` ends every comparison with

```c
if (order != 0 || second_pass || (col == vicolSortPrev && sub == viSubsortPrev)
    || vicolSortPrev < 0)
    return order;
col = vicolSortPrev; sub = viSubsortPrev; asc = vfAscendingPrev; second_pass = 1;
```

So sorting by population and then by name leaves equally-named planets in
population order — and the second pass uses the **previous** direction, not
the current one. `vicolSortPrev` starts negative, which is how the very first
sort knows it has nothing to fall back on.

Those three are **globals, not `RPT` fields**. Sorting one report pushes its
column down into the memory the other three will tie-break against. Sort the
planets by population, open the fleets report, sort it by name, and equally
named fleets come out in the order the planets' population sort left in the
globals — which is meaningless, and is what the original does.

`SortReportCache` saves the previous sort too, but only when the column
actually changes. In practice that branch never fires: the menu has already
written `icolSort` by the time it calls, and opening a window re-sorts on the
column it stored.

### What gets sorted

`SortReportCache` builds an array of indices and `qsort`s that, leaving the
objects alone:

| report | rows |
|--------|------|
| Planets | your own planets — `iPlayer == idPlayer` **and** full detail |
| Fleets | fleets you own |
| Others' Fleets | fleets you do not, stopping at `0x3fb` |
| Battles | every recording in the file |

The last two share one index array (`vlprgidMisc`), so opening either
invalidates the other's cache. This project sorts with a **stable** sort
where the original uses `qsort`: two rows that even the tie-break cannot
separate keep the order the galaxy gives them, rather than whatever the
quicksort's partitioning happened to leave.

## Scrolling

Neither bar scrolls by pixels. The vertical one counts **rows** and the
horizontal one counts **columns**, and column 0 is outside both: it never
scrolls away, and the horizontal bar does not even start until past it.

`ReportDlg`'s `WM_SIZE` says how many rows fit —
`(client height - 0x24) / (dyArial8 + 4)` — and `WM_VSCROLL` moves by one
row a line and by `cRowsVis - 1` a page. `WM_HSCROLL` moves by one column
a line and by **three** a page, and turns its position back into a first
column by walking up from column 1, stepping over hidden columns for
nothing and spending one step on each visible one.

`SetHScrollBar` (`1108:7b6c`) works out the range. It walks from the last
column **back** to column 1, taking each visible column's width off the
room left once the name column and the vertical scrollbar have had theirs;
every column that takes the remainder below zero is one more the bar has to
reach. With nothing left over the bar is hidden and `cFieldFirst` goes back
to 1.

Both bars are placed against the grid rather than the window: the vertical
one runs beside the rows only, from `dyArial8 + 6` down, and the horizontal
one sits under the last row, starting at the name column's right edge and
stopping short of the vertical one.

### A second off-by-one, reproduced

That walk reads the visibility bit of the column **above** the one it is
measuring. `grbit` starts at `1 << cFields` — one past the last column —
and shifts right once a turn, while the width subtracted is the column
below it. With every column shown this cannot be told apart, because
`grbitVisible` starts with all sixteen bits set and the phantom bit is set
too. Hide one column and the column to its **left** is the one left out of
the measurement, so the bar's range comes out one short. Reproduced, with
a test that says which column is wrong and why.

## Clicking a row

`ExecuteReportClick` selects the object the row is about — the same
selection a click on the map makes, so the panes follow — and then, for
about half the columns, does what clicking that same figure in a pane does.
The two are not alternatives: opening the production queue also leaves the
planet selected.

| report | column | what else |
|--------|--------|-----------|
| Planets | Planet Name | the starbase pop-up, but only for a click in the eight pixels at the right where the bars are drawn |
| Planets | Starbase | the starbase pop-up |
| Planets | Population, Value | the population pop-up |
| Planets | Production | opens the production queue (`ChangeProduction`) |
| Planets | Mine, Fact | the industry pop-up, mines or factories |
| Planets | Defense | the component pop-up for the best defence the race can build |
| Planets | Minerals, Mining Rate, Min Conc | the mineral pop-up, for **whichever third of the cell** was clicked |
| Planets | Resources | the resources pop-up |
| Fleets | Destination, ETA, Task | takes hold of waypoint 1 — but only when the fleet is going somewhere (`FDestIsWP0`) |
| Fleets | Fuel, Cargo | opens the cargo transfer dialog |
| Fleets | Composition | the fleet pop-up |
| Others' Fleets | any | selects it and scrolls the map to it |
| Battles | any | moves the selection to where the battle was; a **second** click opens the VCR |

Two guards. A planets report **refuses the whole click**, selection
included, while the production queue is up — `MessageBeep` and nothing else
— and a fleets report does the same while a transfer dialog is up.

Which third of a mineral cell was clicked is worked out as
`for (i = 1; i < 4 && i * dx / 3 <= x; i++)`, then `i - 1`: thirds of the
column's width, left to right, ironium first.

### What this project does with it

All of the routing, and the actions it has: the selection, the production
queue, waypoint 1, the VCR. **All seven pop-ups** are raised.

Three come from panels this project already had or could build straight
out: the fleet summary, the component panel for the best defence (the
Technology Browser's own), and the **mineral** pop-up, which `DrawPopup`
builds inline rather than through a helper:

```
        Ironium
   On Surface: 1234kT
   Mineral Concentration: 45 (30)
   Mining Rate: 12kT/yr
```

Each row reads `Unknown` instead of a figure when there is no figure — no
surface total for a planet nobody has landed on, no concentration for one
nobody has scanned — and the rate row is left out rather than filled in.
The note after the concentration is a home world's: `(30)` while it is
below thirty, because a home world mines as if it were thirty
(`MANUAL.PDF` p. 6-5), and `(HW)` above.

Three more are **sentences** rather than tables. `PtDisplayFactoryMineInfo`,
`PtDisplayResourceInfo` and `PtDisplayPlanetPopInfo` each stream a dozen
string fragments together in alternating faces, picking between them on
who owns the planet, whether it is worth anything, and whether there is
room to grow. The *choices* are the specification and are reproduced; the
*wording* is this project's own, because the game's prose is the game's.
So, for instance, where the original writes

> You have 12 mines on Stove Top.  You may build up to 25; however, your
> colonists are currently capable of operating only 20 of them.

this says the same three figures in its own sentence. Each panel keeps its
structure as data — `IndustrySummary`, `ResourceSummary`,
`PopulationSummary` — so what is asserted in tests is the facts, not the
phrasing. Two details of the originals are easy to miss and are kept: the
resources sentence **stops early** when nothing is allocated to research,
rather than saying so twice, and a hostile world's kill rate is printed in
**tenths** of a percent.

The seventh, `grPopupShdef`, is a design drawn with the designer's own
panel — `DrawSlotDlg` and `DrawBuildSelHull`, the two the dialog itself
draws. Rather than a second copy of that drawing, `App::designer_peek`
holds the design the pop-up is showing and every `designer_*` accessor
answers about it, so the panel is drawn by the designer's code with the
dialog shut. The original sizes that window from a formula
(`dyArial8 * 7 + 0x13a` tall); this sizes it from the schematic the design
actually needs, with the same seven lines of room under it.

Worth noting how the original sizes the four panel pop-ups: `Popup`
(`10c0:0c7c`) **runs the drawing routine with `fPrint = 0`** and takes the
point it returns. Measuring by laying the text out is the same trick.

There is no cargo transfer dialog here: cargo moves through the fleet
pane's tiles, so Fuel and Cargo select the fleet and stop there.

## What this project has

`crates/stars-ui/src/report.rs` holds the model: the four reports, their
columns, the state each keeps, the menu with its indices laid out as the
original lays them out, and the comparator with its tie-break. Sorting is
driven by a key per cell rather than by fifty-four hand-written comparisons,
which is the same order for every column this project can compute.

`crates/stars-ui/src/views/report.rs` draws it, and the four report screens
— Planets, Fleets, **Others' Fleets** (new: the original's third report had
no screen here before) and Battles — are that grid. Header cells are raised
rings with no fill, centred except the first; rows are separated by the
shadow lines `PATBLT` draws; widths follow `DxReportColHdr`'s rules against
the font actually in use. Clicking a header opens the column menu with both
buttons, as the original does; clicking a row selects what it is about.

One difference worth stating: two columns **cannot be computed from a
player's own file** and read as
empty: another player's Composition, and the Others' Fleets class counts,
both of which need that player's ship designs. The original has the same
problem and solves it with a table of designs the player has seen; this
project does not keep one yet.

The Battles report is the fourth table, and the VCR is what it was always
meant to be: a window of its own (`hwndVCRDlg`) that opens over the report,
not a screen. Two details of `ExecuteReportClick` come with it. A row moves
the selection to where the battle happened and **returns** — only a click
that finds the selection already there falls through to `BattleVCR` — and
that call is guarded by `hwndVCRDlg == 0`, so a recording already playing
is never swapped for another; the window has to be closed first. A battle
in deep space has no planet to select, so it opens at once.

Nothing on this screen is outstanding.

## What the tutorial asks of it

Three pages watch the sort, and they watch different amounts of it. The
arms sit behind two latch bits — bit 9 and bit 10 of the tutor's flag word —
which are what stops a page asking twice.

| page | text | the arm |
|------|------|---------|
| 46 | "Click on the title of the Value column and Sort by Value" | latches as soon as **any** report is open; `icolSort == 4` only chooses the paragraph |
| 55 | "Reverse Sort by Mineral Concentration - Weighted Average" | latches on `icolSort == 0xb && !fAscending && iSubsort == 3` |
| 59 | "Open the Planet Summary Report and sort by Population" | latches on `icolSort == 2` |

None of them asks **which** report is open: the arms test `vprptCur` for a
null and read `icolSort` off whatever it points at. This project derives
that pointer from the screen, which is the same thing here — the four
report screens are the four report windows.
