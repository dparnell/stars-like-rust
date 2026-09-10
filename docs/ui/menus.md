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

**Four items share F3.** The key does not open a particular report; it opens
whichever was last up, which is why all four carry it.

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
that opens them.

Not there, because there is nothing behind them yet: `Close`,
`Save And Submit`, `Print Map`, the whole `Dump to Text File` submenu,
`Introduction`, `Player's Guide` and `About Stars!`. `Wait for New` opens this
project's host mode, which is the nearest thing it has.

One item is this project's own and marked so in the code: `Production…` under
Commands, a third way to a dialog the original reaches two other ways, both of
which also work here.
