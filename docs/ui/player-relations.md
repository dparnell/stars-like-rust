# The Player Relations dialog

Status: **recovered and reimplemented**, including the dialog resource, which
is where several of the facts live.

`RelationsDlg` (`10f0:0088`), reached from **Commands (Player Relations)** or
**F7** — the menu resource spells it `&Player Relations...\tF7`, id `0x7de`.
Dialog `IDD_RELATIONS` (2008).

It is **modal**, and it is small: a list of the other players, three radio
buttons, and Close.

## The controls

Read out of the dialog template at file offset `0x347f40`, which is where the
captions and the layout come from:

| id | control | at |
|----|---------|-----|
| `0x7d3` | the listbox of other players | (6, 16) 75 × 65 |
| `0x7d5` | `&Friend` | (96, 20) |
| `0x7d4` | `&Neutral` | (96, 38) |
| `0x7d6` | `&Enemy` | (96, 56) |
| `2` | `Close` | (152, 16) |
| `0x76` | `&Help` | (152, 36) |
| `0xffff` | the `&Player:` label | (6, 4) |

The window is captioned `Player Relations` and is 198 × 80 dialog units in MS
Sans Serif 8.

Two things fall out of that table and are worth stating, because neither is
guessable from the code alone:

* **The radios are stacked Friend, Neutral, Enemy** — that is the order of
  their `y` coordinates — but their **values** are Neutral 0, Friend 1, Enemy
  2, because `CheckRadioButton` is called with `0x7d4 + relation` and the
  handler stores `wParam - 0x7d4`. The display order is not the value order.
* There is **no Cancel**. Close commits.

Close ends the dialog with `EndDialog(hwnd, selected + 3)` — the selected
player's index, mapped back and offset by three. Nothing reads it: the caller
in `mdi.c` discards `DialogBox`'s result. It is reproduced by not reproducing
it.

The group box around the three is drawn by hand in `WM_PAINT` rather than being
a control: the dialog measures the rectangle from `0x7d5` to `0x7d6`, expands
it, draws a 3-D frame and writes `Relation` (string 904) into the top of it.

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

## What is reproduced

The dialog and its listbox, the three radios in the original's stacking order
with the original's values, the seeding on the first other player, the mapping
that skips the local player, the single-player refusal, and the one-record log
with the whole table in it.

## What is not

* The **`Relation` group box** is a real group box here rather than a
  hand-drawn 3-D frame.
* The **Help** button, which goes to help context `0x43b`.
* The **tutorial hook**: `LogChangeRelations` advances the tutorial when player
  0 changes a relation.
* The dialog is drawn as a window rather than a true modal; nothing else on
  screen is blocked while it is open.
