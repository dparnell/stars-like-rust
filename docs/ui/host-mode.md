# The Host Mode dialog

Status: **built**. What is missing is missing from the game as well — see
*Auto Generate* below.

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

## Auto Generate

The button starts a watch: every ten seconds host mode counts the turns still
out, and the moment none are it generates the year and writes everybody's
files. That is `HostTimerProc`, on the `SetTimer(NULL, 0x0D, 10000, ...)` beat
`BringUpHostDlg` starts when the dialog returns `-1`. While it waits, the
original writes what it is waiting for into the frame's title — `Host Mode %d
Player(s) Out`, strings `0x031b`, `0x0421` and `0x031c` — which this shows in
the dialog, since the dialog is still on screen here.

One thing stops it: `gd.fAllAis`, every player being a computer player. Then
there is nobody to wait for and the host would generate for ever, so the button
is disabled and the original says why (string `0x02c8`).

### The options that are not there

`IDD_HOST_OPTIONS` (1026), `Auto Generate Options`, is in the resources — a
checkbox *When all players are in.* and three force-generate radio buttons, two
with empty captions to be filled at run time from the fragments `Every …
hours.` and `Up to … minutes after … player(s) are left out.` (strings
`0x050c`, `0x050d`, `0x0510`, `0x0511`, `0x0518`). The `TIMER` struct behind it
has the fields to match: `mdForce`, `fAutoGenWhenIn`, and a union of `hrsForce`
with `minForce:12, cPlr:4`.

**None of it works in 2.7j.** Read out of `stars.2.7j.exe`:

* `DrawHostOptions` (`1020:7706`) is ten instructions — `push bp; mov bp,sp;
  sub sp,2; push si; push di; pop di; pop si; mov sp,bp; pop bp; retf`. It
  draws nothing, so the two empty captions stay empty and the numbers are never
  painted.
* `HostOptionsDialog` (`1020:75ce`) handles `WM_PAINT` (calling that stub),
  `WM_ERASEBKGND`, `WM_INITDIALOG`, `WM_CTLCOLOR` and `WM_COMMAND` for OK,
  Cancel and Help — and nothing else. It never reads a control, and never sets
  one from `vtimer`.
* Nothing in the program writes `vtimer` (`1120:3ef0`) except one instruction
  in `InitStuff`, `mov word ptr [0x3ef2], 1` — `fAutoGenWhenIn = 1`. Every
  other reference is the same four-part test
  `(!gd.fAllAis && (vtimer.fAutoGenWhenIn || vtimer.mdForce))`, and `mdForce`
  is never anything but zero.
* The host dialog's template has no `Options` button either: `IDC_HOST_OPTIONS`
  (`0x405`) is handled in `HostModeDialog` but no control carries that id.

So the feature was taken out of the interface and left in the code. Auto
generate has exactly one setting, hard-wired on: *when all players are in*.
That is what is built here, and there is nothing to configure.

> This corrects what this file said when host mode was first built — that Auto
> Generate was disabled "where the original leaves it with none set". The
> original's own `fAutoGenWhenIn = 1` means the button is live in every game
> with a person in it; the button here now is too.

## Time since last change

`ctickLast` is reset whenever a player's status changes, and the dialog redraws
on a **ten-second timer** — the original's `SetTimer(hwnd, 0, 10000, NULL)` —
which is also when it re-scans for order files. The elapsed time is written in
one of four formats, and those are reproduced: `%d seconds` under a minute,
then `m:ss`, then `h:mm:ss`, then `d days h:mm:ss`.

## What is not

* The **Auto Generate Options** dialog, because there is nothing behind it —
  see above. A dialog that reads nothing and writes nothing is not worth
  reproducing.
* Nothing else of the dialog. `Password...` sets the **host's** password —
  which is not the local player's — and it is written into the host file on the
  next save rather than the instant it is chosen, where the original writes the
  file on the spot. See `change-password.md` and
  `../formats/hst.md#the-hosts-password-changepassword-type-36`.
* **Right-clicking a player** to switch them between a person and a computer
  player, which flips their password salt and re-marks the files.
* The **Help** button.
