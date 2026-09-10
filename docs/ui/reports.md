# The report windows

Status: **in progress** — the columns, the sort and the grid are done; the
per-column click behaviour is not.

Four windows share one piece of code: **Planets**, **Fleets**, **Others'
Fleets** and **Battles**, listed together on the Report menu, all four
carrying F3. They are the same window with a different `irpt`, and all of it
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

## Clicking a row

`ExecuteReportClick` selects the object the row is about — and then, for
some columns, does what clicking that figure in a pane does. Clicking a
planet's Mine or Fact opens the industry popup, its Minerals or Mining Rate
or Min Conc the mineral popup for **whichever third of the cell** was
clicked, its Production the production dialog, a fleet's Destination or ETA
or Task the waypoint, a fleet's Fuel or Cargo the transfer dialog. A row of
the Battles report opens the VCR.

## What this project has

`crates/stars-ui/src/report.rs` holds the model: the four reports, their
columns, the state each keeps, the menu with its indices laid out as the
original lays them out, and the comparator with its tie-break. Sorting is
driven by a key per cell rather than by fifty-four hand-written comparisons,
which is the same order for every column this project can compute.

Two columns cannot be computed from a player's own file and read as empty:
another player's **Composition**, and the Others' Fleets class counts, both
of which need that player's ship designs. The original has the same problem
and solves it with a table of designs the player has seen; this project does
not keep one yet.

Not done yet: the grid itself, the click behaviour above, and the horizontal
scrollbar.
