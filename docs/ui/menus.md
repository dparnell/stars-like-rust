# The menu bar

Status: **recovered in full**; reproduced in the desktop shell, with the gaps
listed below.

There is exactly one menu resource, `0x6d4`, and it holds the whole bar: six
menus, in this order. Ids are the `WM_COMMAND` values, and the accelerator is
part of the item's own text after a tab.

## `&File`

| id | item |
|----|------|
| `0xed8` | `&New...\tCtrl+N` |
| `0x081` | `Custom &Race Wizard...` |
| `0xed9` | `&Open...\tCtrl+O` |
| `0x071` | `&Close` |
| `0xeda` | `&Save\tCtrl+S` |
| `0xedb` | `Save &And Submit\tCtrl+A` |
| | — |
| `0x0d5` | `&Print Map` |
| | — |
| `0xee2` | `E&xit` |

## `&View`

Its own spec: `view-menu.md`.

## `&Turn`

| id | item |
|----|------|
| `0x06a` | `&Wait for New` |
| `0x069` | `&Generate\tF9` |

Two items, and the tutorial names the menu by name: *"For a change of pace,
instead of hitting F9, select Generate from the Turn menu."*

## `&Commands`

| id | item |
|----|------|
| `0x07d` | `&Ship Design...\tF4` |
| `0x07e` | `&Research...\tF5` |
| `0x7dc` | `&Battle Plans...\tF6` |
| `0x7de` | `&Player Relations...\tF7` |
| | — |
| `0x10e` | `&Change Password...` |

Five items and one rule. The production queue is **not** here: it is reached
from the planet tile's Change button and from the `q` key.

## `&Report`

| id | item |
|----|------|
| `0x8fd` | `&Planets...\tF3` |
| `0x8ff` | `&Fleets...\tF3` |
| `0x900` | `&Others' Fleets...\tF3` |
| | — |
| `0x901` | `&Battles...\tF3` |
| | — |
| `0x05f` | `&Score...\tF10` |
| | — |
| | `&Dump to Text File` ▸ `0x055` `&Universe Definition`, `0x054` `&Planet Information`, `0x053` `&Fleet Information` |

**All four show F3, and none of them is the F3 accelerator.** The text after
the tab is drawn, not bound. The key has an id of its own — `0x8fe`, which
appears on no menu — and `CommandHandler` (`1020:448f`) turns it into one of
the four by asking what is open:

```c
if (hwndReportDlg == 0)            wParam = 0x8fd;  /* Planets */
else if (vprptCur == &vrptPlanet)  wParam = 0x8ff;  /* Fleets */
else if (vprptCur == &vrptFleet)   wParam = 0x900;  /* Others' Fleets */
else                               wParam = 0x901;  /* Battles */
```

and the open path then **closes** Battles rather than reopening it
(`1020:4727`). So F3 walks round: nothing, planets, your fleets, everybody
else's, battles, nothing again. Esc closes whichever is up.

Two more things the handler does. The item of the report that is open
carries a **check mark** — `CheckMenuItem(..., MF_CHECKED)` on the way in,
and `ReportDlg`'s `WM_DESTROY` takes it off again. And **Battles is the only
one of the four that toggles**: choosing any other while it is already open
closes and reopens it, but choosing Battles while Battles is up just closes
it, which is the same test the F3 cycle leans on.

## `&Help`

| id | item |
|----|------|
| `0x9c2` | `&Introduction` |
| `0x101` | `&Player's Guide\tF1` |
| | — |
| `0x100` | `Technology &Browser\tF2` |
| `0x9c5` | `&Tutorial` |
| | — |
| `0x063` | `&About Stars!...` |

`&Tutorial` is how the tutorial is started, which string `0x51a` says out
loud: *"To make the tutorial reappear complete your task or choose Tutorial
from the Help menu."*

## What this project does

All six menus, in the original's order, with the original's items and
accelerators where there is something behind them. The four report windows are
this project's **screens**, so they are listed under Report, which is the menu
that opens them — with the resource's captions, its separators, and a check
mark on the one that is open.

Not there, because there is nothing behind them yet: `Close`,
`Save And Submit`, `Print Map`, the whole `Dump to Text File` submenu,
`Introduction`, `Player's Guide` and `About Stars!`. `Wait for New` opens this
project's host mode, which is the nearest thing it has.

Two items are this project's own and marked so in the code. `Production…`
under Commands is a third way to a dialog the original reaches two other
ways, both of which also work here. `Players` at the foot of the Report menu
is a summary screen the original has no equivalent of; it is kept out of the
F3 cycle, which is the original's four and only those, though Esc leaves it
the same way.

The map is on no menu, in the original or here: Esc is the way back to it.
