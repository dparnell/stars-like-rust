# The Player Relations dialog

Status: **recovered and reimplemented**, including the dialog resource, which
is where several of the facts live.

`RelationsDlg` (`10f0:0088`), reached from **Commands (Player Relations)** or
**F7** — the menu resource spells it `&Player Relations...\tF7`, id `0x7de`.
Dialog `IDD_RELATIONS` (2008).

It is **modal**, and it is small: a list of the other players, three radio
buttons, and Close.

## The controls

Read out of the dialog template, resource 2008, which is where the captions
and the layout come from. `Player Relations`, 198 x 80 dialog units, MS Sans
Serif 8:

| id | control | x | y | w | h | style |
|----|---------|---|---|---|---|-------|
| `0x2` | `Close` | 152 | 16 | 40 | 14 | `0x50010001` (default push button) |
| `0x76` | `&Help` | 152 | 36 | 40 | 14 | `0x50010000` |
| `0xffff` | the `&Player:` label | 6 | 4 | 64 | 10 | `0x50020000` |
| `0x7d3` | the listbox of other players | 6 | 16 | 75 | 65 | `0x50a10001` |
| `0x7d5` | `&Friend` | 96 | 20 | 42 | 12 | `0x50000009` (auto radio) |
| `0x7d4` | `&Neutral` | 96 | 38 | 42 | 12 | `0x50000009` |
| `0x7d6` | `&Enemy` | 96 | 56 | 42 | 12 | `0x50000009` |

Four things fall out of that table, and none of them is guessable from the
code alone:

* **The radios are stacked Friend, Neutral, Enemy** — that is the order of
  their `y` coordinates — but their **values** are Neutral 0, Friend 1, Enemy
  2, because `CheckRadioButton` is called with `0x7d4 + relation` and the
  handler stores `wParam - 0x7d4` (`10f0:0392` writes `wParam + 0xf82c` as a
  byte, which is the same thing). The display order is not the value order.
* There is **no Cancel**. Close commits.
* The listbox style is `0x50a10001`: `LBS_NOTIFY` and `WS_VSCROLL`, but **not**
  `LBS_SORT` and not owner-drawn. So the players come out in index order and
  the names are plain black text.
* There are **seven** controls, and the `Relation` group box is not one of
  them. It is drawn in `WM_PAINT`; see below.

And one over-run, which is the resource as shipped: the listbox's 65 units
start 16 down on a dialog only 80 tall, so it runs one unit past the foot.
Every other control stays inside. Templates 92 and 93 have over-runs of their
own, so this is a habit rather than a one-off.

The listbox is the only control the dialog leaves to Windows, so its colours
are the system's rather than the dialog's: `COLOR_WINDOW` white behind
`COLOR_WINDOWTEXT` black, `COLOR_HIGHLIGHT` — navy in the shipped scheme —
behind `COLOR_HIGHLIGHTTEXT` white for the selected row, and the single dark
line `WS_BORDER` draws round it. Not a sunken 3-D well: that is what the
*frame* below gets, and the two are drawn by different code.

Close ends the dialog with `EndDialog(hwnd, selected + 3)` — the selected
player's index, mapped back and offset by three. Nothing reads it: the caller
in `mdi.c` discards `DialogBox`'s result. It is reproduced by not reproducing
it.

## The frame round the radios

`WM_PAINT` (`10f0:019f`) builds it at run time rather than placing a
`GROUPBOX`:

1. `GetWindowRect` of `&Friend` (`0x7d5`), mapped to client coordinates —
   that gives the frame's top-left.
2. `GetWindowRect` of `&Enemy` (`0x7d6`); its **bottom-right** point is mapped
   (`ScreenToClient` is handed `&rc.right`, so it converts the far corner) and
   becomes the frame's bottom-right.
3. `ExpandRc(&rc, dyArial8, dyArial8 / 2)` (`1040:2f0c`) — grow by a whole line
   across and half a line down.
4. `_Draw3dFrame(hdc, &rc, -1)` (`1040:336a`), which paints the Windows 3.1
   groove: an outer ring in `COLOR_BTNSHADOW` along the top and left and
   `COLOR_BTNHIGHLIGHT` along the bottom and right, then the same ring one
   pixel in with the two swapped.
5. `Relation` (string 904) in `rghfontArial8[1]` — Arial 8 **bold** — at
   `x = rc.left + 8`, `y = rc.top - dyArial8 / 2`, so the caption straddles the
   top edge rather than sitting inside it.

Measuring from the outer two radios rather than from the template is what keeps
the frame right whatever the font does to the control heights. Since the frame
is sized in `dyArial8` and everything else on the dialog is sized in dialog
units, the two have to move together: the dialog's font is what *defines* a
dialog unit, so a line of it is eight vertical units by construction, and any
placed control gives the scale away.

`WM_CTLCOLOR` (`10f0:02aa`) answers with `hbrButtonFace` for every control
**except** the listbox, which it lets fall through to the default. That is why
the listbox is the only white thing on an otherwise button-face dialog.

## What the list says

Each row is `PszPlayerName(i, 0, 0, 0, 0, NULL)` (`1038:11f2`) — every flag
off. That is the race's **singular** name (`PLAYER.szName`, offset `0x80` of
the 0xc0-byte player block at `DS:0x59a2`) and nothing else: no leading "the",
no plural, no player number.

A player with no name at all falls back to string 1374, `"Player %d"`, and the
original then appends `"'s"` (`1038:13df`) — which the *named* branch never
does. It reads like a possessive form leaking out of the wrong branch, but it
is what the binary shows, so it is what this shows.

## Where it opens

`WM_INITDIALOG` calls `StickyDlgPos(hwnd, &ptStickyRelationsDlg, 1)`
(`1040:3094`), and Close calls it again with `fInit = 0` to save. The
remembered point starts at `(-1, -1)`, which means *centre on the screen*;
after that the dialog comes back where it was last left, nudged back on-screen
if the desktop has since shrunk. It is one of a family of sticky positions the
game keeps, one global per dialog — `ptStickyRelationsDlg` is `DS:0xd92`.

`WM_INITDIALOG` then falls straight through into the `WM_ERASEBKGND` handler
(`10f0:0177`), which calls `FillRect` with `wParam` as its DC. On
`WM_INITDIALOG` `wParam` is the handle of the control that would take the
focus, not a DC, so that call does nothing. It is harmless — the return value
of 1 is the one `WM_INITDIALOG` wants — but it is a fall-through, not a design.

## What it edits

`PLAYER.rgmdRelation`, a byte per player at offset `0x70` of the player block,
preceded by a count. It is the player's **own** table: it says how they regard
everyone else and nothing about how anyone regards them, and only its owner can
change it.

The listbox holds everybody **but** the local player, which is why the original
maps its selection back with `if (index >= idPlayer) index += 1`. The dialog
opens on the first entry — player 0, unless that is you, in which case player 1
— and `CheckRadioButton` is seeded from that player's relation.

## When it is refused

`IDM_GAME_RELATIONS` does nothing at all when any of three things is true: no
game is loaded, `idPlayer` is `-1` (a host with no player of its own), or the
game is a **single-player** game — `GAME.wCrap` bit 2,
`stars_formats::game_flag::SINGLE_PLAYER`. The menu item stays enabled and
choosing it simply returns.

"Single player" is narrower than "one human". The fixtures settle it: the
sixteen-player game in `no-random-events` and the fifteen-player
`all-computer-players` — one human and fourteen computers — are both **not**
single-player, and both would open the dialog. The tutorial is, and would not.

That is not the same as the table being unused. The tutorial is the one game in
this repository's fixtures whose table is actually filled in — `[0, 2]`, so the
tutorial player regards player 1 as an enemy — and it is the one game whose
dialog would refuse to open. The table is read whatever the dialog does:

* **remote terraforming** helps a friend's planet and harms anyone else's
  (`crate::terraform::remote_intent`);
* the **scanner's minefield filters** sort fields into your own, friends' and
  neutrals'-and-enemies' — the strings are ids 1281 and 1282, "Mine Fields of
  Friends" and "Mine Fields of Neutrals" — which is why closing the dialog
  invalidates the scanner.

## The order it writes

`LogChangeRelations` (`1048:9340`) writes a `rtLogRelations` record (type 38)
carrying **the whole table**, `game.cPlayer` bytes of it. Before writing, it
looks at the previous log record and, if that is already a relations record,
rewinds over it — so however many relations a player changes in a session, the
log carries **one** record, holding the final state.

The original writes it on `WM_DESTROY`, gated by a dirty flag. This
reimplementation writes it on each change and replaces the previous record,
which leaves exactly the same single record in the log; the difference is not
observable in the file. Both are tested.

`WM_DESTROY` (`10f0:045a`) does one other thing first, and it is a dead store:
it computes `grbitScan & 0xf` into a stack slot nothing reads. Presumably it
was meant to be `grbitScan &= 0xf` — clearing the scanner's filter bits so they
are re-derived against the new table — and the assignment lost its target. What
does happen is the `InvalidateRect` on `hwndScanner` right after it, which
repaints the scanner and is enough on its own.

## What is reproduced

The dialog laid out from its own template, its listbox bordered and white
against a button-face dialog, its rows black on white with a navy selection
bar, holding the singular race names in index order, the
hand-drawn `Relation` groove with its caption over the top edge, the three
radios in the original's stacking order with the original's values, the seeding
on the first other player, the mapping that skips the local player, the
single-player refusal, and the one-record log with the whole table in it.

## What is not

* The **sticky position**: the dialog is a window the shell places, so it does
  not remember where it was last dragged to.
* The frame's caption is drawn in the same face as the rest of the dialog. The
  original selects `rghfontArial8[1]`, Arial 8 **bold**; egui's default font
  set has no bold proportional face, which is the same gap the status bar
  works around.
* The **Help** button, which goes to help context `0x43b`.
* The **tutorial hook**: `LogChangeRelations` advances the tutorial when player
  0 changes a relation.
* The dialog is drawn as a window rather than a true modal; nothing else on
  screen is blocked while it is open.
