# The menu bar

Status: **recovered in full**; reproduced in the desktop shell, with the gaps
listed below.

There is exactly one menu resource, `0x6d4`, and it holds the whole bar: six
menus, in this order. Ids are the `WM_COMMAND` values, and the accelerator is
part of the item's own text after a tab — **text, not a binding**: the
accelerator table is separate, and at least one key (F3) has an id that is on
no menu at all.

The resource is only half the story. `InitializeMenu` (`1020:5560`), the
`WM_INITMENU` handler, decides every frame what is greyed, what is ticked,
and what the File menu's tail holds; that is written up under each menu
below and gathered in [What the handler decides](#what-the-handler-decides).

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

`InitializeMenu` rebuilds the tail of this menu every time it opens: it
deletes ids `0x10cc`–`0x10d4` and reinserts up to **nine most recently used
files** from `vrgszMRU`, each captioned `&1 ` and the whole path — not the
file's name alone — and inserted by position at index 9, which puts them
between the last separator and `E&xit`.

The list lives in `stars.ini`, which `ReadIniSettings` (`1000:1db3`) reads
at startup:

```ini
[Files]
File1=C:\STARS\GAME.M1
File2=...
```

Three rules come out of that routine and out of `FLoadGame` (`1070:303d`),
which is what promotes an entry:

* a value of **fewer than four characters is no entry** and is thrown away;
* after reading, the slots are **compacted**, so a missing `File3` does not
  strand `File4`;
* promotion compares **case-insensitively** (`_fstricmp`) and
  short-circuits — a game already at the front is not promoted and nothing
  moves, which is also what decides whether the file needs writing again.

`File1` does double duty: `ReadIniSettings` copies it into `szBase` and sets
the startup-file bit, so a launch with nothing else to go on **reopens the
game last played**.

## `&View`

Its own spec: `view-menu.md`.

## `&Turn`

| id | item |
|----|------|
| `0x06a` | `&Wait for New` |
| `0x069` | `&Generate\tF9` |

Two items, and the tutorial names the menu by name: *"For a change of pace,
instead of hitting F9, select Generate from the Turn menu."*

`&Generate` is greyed only when no game is open. `&Wait for New` is greyed
then too, and in a **single-player** game, where there is nobody to wait
for.

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

Two of the five are greyed in a single-player game. `&Player Relations...`
goes dead outright. `&Change Password...` goes dead only while there is no
password set — a lone player can still take one **off**, so the item stays
alive while `lSaltCur` is non-zero.

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

## What the handler decides

`InitializeMenu` runs on `WM_INITMENU`, so all of this is decided as the menu
drops rather than stored anywhere.

| item | greyed when |
|------|-------------|
| `&Generate` (`0x69`) | no game |
| `&Wait for New` (`0x6a`) | no game, or single-player |
| `&Player Relations...` (`0x7de`) | no game, or single-player |
| `Save &And Submit` (`0xedb`) | no game, or single-player |
| `&Change Password...` (`0x10e`) | no game, or single-player **with no password set** |

Single-player is bit 2 of the `GAME` flag word at `+0x10` — the same word
whose other bits the tutorial's world sets (see `tutorial.md`).

Ticked, rather than greyed:

| item | ticked when |
|------|-------------|
| `Toolbar` (`0xb3`) | the toolbar is showing |
| `Player Colors` (`0x98d`) | `grbitScan & 0x2000` |
| `Zoom` ▸ one of six | by **position**, `iScanZoom + 4` |
| `Window Layout` ▸ one of five | by position, `iWindowLayout` |
| the open report | see `&Report` above |

And the whole **`&View` menu is greyed** — by position on the bar, not by id
— while there is no scanner window. With one, every item of the Zoom submenu
is re-enabled on the way past.

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

The five items that carry their own greying rule are asked about through
`App::menu_item_enabled`, so the rules are testable without a menu. The View
menu is alive whenever a game is open, because this shell's scanner is not a
window that can be absent.

Not there, because there is nothing behind them yet: `Close`,
`Save And Submit`, `Print Map`, the whole `Dump to Text File` submenu,
`Introduction`, `Player's Guide` and `About Stars!`. `Wait for New` opens this
project's host mode, which is the nearest thing it has.

The File menu's recently-used list is there, with all three of the rules
above and the startup file. The original writes `stars.ini` into the Windows
directory, which has no equivalent here, so this writes the same file in the
same format where each system keeps a program's settings — `%APPDATA%` on
Windows, `$XDG_CONFIG_HOME` or `~/.config` elsewhere.

That file is the one place the original keeps rather more than this project
yet does. `ReadIniSettings` also restores, and nothing here stores: the four
report windows' positions and sizes (`[Windows] ReportPlanWin` and its
three), **which columns each report shows and what it sorts on**
(`ReportPlanFld` and `ReportPlanSort`, the latter packing `icolSort` with
`fAscending` in bit 8 — and not `iSubsort`), the scanner's zoom, filters and
view mode, the window layout, the toolbar, the four zip orders and five
production templates, the default password, and the font names. All of it is
the same mechanism as the list above, so each is a key away.

Two items are this project's own and marked so in the code. `Production…`
under Commands is a third way to a dialog the original reaches two other
ways, both of which also work here. `Players` at the foot of the Report menu
is a summary screen the original has no equivalent of; it is kept out of the
F3 cycle, which is the original's four and only those, though Esc leaves it
the same way.

The map is on no menu, in the original or here: Esc is the way back to it.
