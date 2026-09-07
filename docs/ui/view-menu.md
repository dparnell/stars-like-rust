# The View menu

Status: **structure reproduced**; one of its seven entries stands in for
something the original does more fully.

Menu resource, submenu 1, in the original's own order:

| id | item | |
|----|------|-|
| `0x0b3` | `&Toolbar` | checkable |
| | — | |
| `0x1068` | `&Find...` | Ctrl+F |
| | `&Zoom` ▸ | `0xf3d`…`0xf45`, nine sizes |
| | `&Window Layout` ▸ | `0x082`…`0x084` |
| `0x098d` | `Player &Colors` | checkable |
| | — | |
| `0x09c` | `&Race...` | F8 |
| `0x09e` | `&Game Parameters...` | |

## Toolbar

Hides and shows the scanner's toolbar. `MANUAL.PDF` p. 2-9 offers it as a way
to make room: *"If the screen still seems too cramped try hiding the Toolbar
using the menu item View (Toolbar). Most of the Toolbar functions are
available"* from the menus.

It is stored here the other way up, as `toolbar_hidden`, so that a
default-built `App` still shows it.

## Zoom

The same nine sizes the toolbar's magnifying glass offers — 25, 38, 50, 75,
100, 125, 150, 200 and 400 percent — which are the nine steps `iScanZoom`
has. The menu is a second way to the same thing, and the current size is
ticked.

## Window Layout

`iWindowLayout`, set straight from the menu id (`wParam - IDM_VIEW_LAYOUT_0`),
after which the original calls `EnsureTileSize` — passing whether the layout is
the small one, the only one it treats specially — and `RefitFrameChildren`.

The original is rearranging a frame full of tiled child windows. This frontend
has one split to give, between the panes and the scanner, so it gives that:
each layout has its own panel identity, so choosing one really moves the split
and a width dragged out by hand is remembered per layout.

## Player Colors

See `scanner.md` — it colours planet names and ship counts, and nothing else.

## Race

`IDM_RACE_EDIT1`. Opens the race wizard on the player's own race, read-only —
six pages walked with Back and Next. See `race-wizard.md`.

## Game Parameters

What the game was set up with, and — on its own page in the original — what has
to be done to win it. `MANUAL.PDF` p. 2-3 sends a player there for exactly
that: *"To view the winning conditions once the game has begun, choose the View
(Race) menu item, then turn to page 3 of the View Game Parameters dialog that
appears."*

Everything on it comes out of the `.xy`'s game block, which is the only place
it is kept: a `.mN` or `.hst` alone knows none of it, and the window then says
so rather than inventing anything.

**With one exception: the year.** The `.xy` is written once when the game is
created and its `turn` counter stays at zero for ever after — all four years of
the `no-random-events` fixture carry `turn = 0` — so the year comes from the
save's own header instead. See `../formats/xy.md`.

The victory conditions are the Score sheet's, listed the same way, with a
condition the game is not playing for shown greyed beside its setting.

## What is not reproduced

* The original's Game Parameters dialog is **paged**; this is one panel.
* Window Layout rearranges tiled child windows in the original; here it moves
  one split.
