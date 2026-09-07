# The Host Mode dialog

Status: **built**, except for the auto-generate timer and the host password.

`HostModeDialog`, `IDD_HOST_MODE` (115). In the original this is not a dialog
over the game — it **is** host mode. `BringUpHostDlg` hides the map window and
runs this dialog in a loop, so a host sees nothing else, and the program enters
that mode by opening a host file. This project opens a `.hst` as a game like any
other, so here it is a window over that game, reached from the Turn menu. That
is the one departure worth knowing about.

## The template

260x220 dialog units. Fifteen controls, and a large empty area on the left:

| id      | control | what it is                          |
|---------|---------|-------------------------------------|
| `0x409` | static  | the game's name (`game.szName`)      |
| `0x40a` | static  | the file's base name (`szBase`)      |
| `0x7e0` | static  | `Next year is:`                      |
| `0x407` | button  | `&Generate Now`                      |
| `0x408` | button  | `&Auto Generate`                     |
| `0x7df` | button  | `&Password...`                       |
| `2`     | button  | `&Close`                             |
| `0x76`  | button  | `&Help`                              |
| `0x7e1` | static  | time since the last change           |

Everything from y=28 down the left-hand side is **painted**, not laid out:
that is the player list, and `DrawHostDialog2` is what draws it. The template
alone would describe a dialog with no players in it.

## The player list

One row per player: a blue diamond, the player's number, their name and where
their turn has got to. `CFindTurnsOutstanding` works the status out and puts it
in `rgOut`, which indexes seven consecutive strings from `idsTurned` (`0x02cc`)
— with *dead* at `0x02cb`, index `-1`:

| `rgOut` | status | outstanding? |
|--------:|--------|--------------|
| -1 | dead | no |
| 0 | turned in | no |
| 1 | still out | yes |
| 2 | partially done | yes |
| 3 | corrupted | yes |
| 4 | not on the right year | yes |
| 5 | not in the right game | yes |

The first two are drawn in dark green (`RGB(0, 127, 0)`) and the rest in dark
red; a player marked `fHacker` gets ` - HACKER` after the status.

How the status is reached, from `CFindTurnsOutstanding`:

* a **computer player** is never waited for and is simply *turned in*;
* a **dead** player is *dead* and is not waited for either;
* otherwise the `.xN` beside the game decides. Missing is *still out*; one that
  will not decode is *corrupted*; one for another game or another year says so;
  and one whose header does not carry the submitted flag (`flag_done`,
  `gd.fPartialTurn`) is *partially done*.

The count of outstanding turns is what `Generate Now` asks about.

## Generate Now

The original reads the modifier keys as the button goes down and turns one
generation into a run of them — `iPassCnt` is 9 with Shift, 99 with Ctrl and
999 with both — then asks before doing any of it: one question for forcing a
run (string `0x0562`) and another for generating with turns still out. Both
are asked here, in this project's own words.

Generating writes the new files for everybody, because that is what a host
generation is: the submitted orders are replayed, the year runs, and every
player gets a turn file. That is `save_new_game`, the same writer a new game
uses.

## Time since last change

`ctickLast` is reset whenever a player's status changes, and the dialog redraws
on a **ten-second timer** — the original's `SetTimer(hwnd, 0, 10000, NULL)` —
which is also when it re-scans for order files. The elapsed time is written in
one of four formats, and those are reproduced: `%d seconds` under a minute,
then `m:ss`, then `h:mm:ss`, then `d days h:mm:ss`.

## What is not

* **Auto Generate**, and the `Auto Generate Options` dialog behind it
  (`IDD_HOST_OPTIONS`, 1026). The template has the checkbox *When all players
  are in* and three force-generate radio buttons, two of them with empty
  captions that are filled at run time from fragments — `Every … hours.` and
  `Up to … minutes after … player(s) are left out.` (strings `0x050c`,
  `0x050d`, `0x0510`, `0x0511`, `0x0518`). The numbers in them are painted by
  `DrawHostOptions`, which is a stub in the reconstruction, so how they are
  edited is not recovered. The button is where the original leaves it with no
  option set: disabled.
* The **host password**. The button is here but disabled: the original
  keeps the host's salt in a leading `rtChgPassword` record inside the `.hst`
  (`file.c` reads it right after the player blocks), and this project neither
  writes nor reads that record, so there is nowhere to put one — and opening
  the player's Change Password dialog here would set the wrong password. See
  `docs/ui/change-password.md` for the dialog's host face.
* **Right-clicking a player** to switch them between a person and a computer
  player, which flips their password salt and re-marks the files.
* The **Help** button.
