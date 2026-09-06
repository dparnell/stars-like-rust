# The Score sheet

Status: **all three faces recovered and reimplemented**; the wording of the
victory sentences is written afresh rather than copied.

`ScoreXDlg` (`1108:0f66`), reached with **F10** or Reports (Score), and
described in `MANUAL.PDF` p. 2-3:

> Track the score using Reports (Score) menu item (or by pressing F10). The
> Score sheet shows your score and current ranking, and a history of scores
> since the game began. If Public Player Scores is selected in the game setup,
> all player's scores and rankings appear in the Score sheet.

It is **modeless** — it sets `hwndScoreXDlg` and stays open alongside whatever
else is on screen — and it is one window with **three faces**, not three
dialogs.

## The controls

| id | control |
|----|---------|
| `0xc6` | the button that cycles the face |
| `2` | Close |
| `0x76` | Help |

The face lives in two bits of `gd` and the button advances it with
`(face + 1) % 3`, so it goes round rather than back and forth. Both it and the
timeline's chosen figure are outside the dialog, so closing and reopening the
sheet comes back to the same view. The title changes with the face — string ids
1210 to 1212, `Player Scores`, `Victory Conditions`, `Progress Timeline` — and
`InitScoreDlg` (`1108:13b6`) resizes the window to fit whichever it is on.

## Where the figures come from

The host works them out in `UpdatePlayerScores` (`10b8:6258`) and writes a
`SCOREX` per player into every player's file; the client only ever reads them.
That is the whole reason the sheet is not recomputed at this end: **a player
file describes only its own player's planets and fleets**, so every other row
in it is knowledge that cannot be derived, only read.

Three flags in the first word of a score block decide how a row is used:

* `fValid` (bit 5) — the row carries figures. A file holds a row for every
  player and fills in only the ones this player may see, so a game without
  Public Player Scores gives fifteen **blank** columns, not fifteen columns of
  zeroes;
* `fWinner` (bit 14) — this player has met enough conditions to win;
* `fHistory` (bit 15) — the row is one year of a `.hN` timeline rather than the
  current standing, and the second word is then the **turn** rather than the
  rank.

`io.c` files every type-45 record it reads into the player's timeline, keeping
one row per turn in turn order and dropping the oldest once a hundred and one
have arrived (the buffer is `0x978` bytes, which is 101 × 24). A row *without*
`fHistory` is filed under the game's own turn — which is why the timeline
reaches the current year even though the history file stops at the last one.
The fixtures show it plainly: `all-computer-players/2450/Game.h1` holds turns 1
to 49 and the `Game.m1` beside it holds the row for turn 50.

## Face 1 — Player Scores

`DrawScoreReport` (`1108:1e0c`). A **column per player** and a row per figure:
Planets, Starbases, Unarmed Ships, Escort Ships, Capital Ships, Tech Levels,
Resources, Score (string ids 435–442) and then Rank.

The three ship counts are stored packed — thirteen bits of value and three of a
shift — and this is the only place they are unpacked, which is why a large
fleet is reported roughly.

Colour carries three separate meanings, all of them the same pure blue:

* the **largest** figure in each row, and a tie colours both;
* **rank 1**;
* a player the `fWinner` bit names.

A player the game knows to be dead is drawn in grey **and has no figures at
all** — the original computes the row maximum over every player but then
declines to draw a dead one's numbers. So a dead player is a grey name over an
empty column, which is different from a player with nothing left.

## Face 2 — Victory Conditions

`DrawVCReport` (`1108:168e`). The same shape: a column per player, and nine
rows of sentences down the left with a tick where a player has met one.

The nine lines are the game's ten victory settings less one: the tech condition
takes **two** of them — a level and a number of fields — so `TECH_FIELDS` has
no line of its own and the numbering slips by one after it. That off-by-one is
in the original too: the checkbox loop indexes the settings with
`i + (i > 1)`.

Two details are worth keeping:

* the planet condition is **set** as a percentage and **shown** as a count.
  The original multiplies the setting by the galaxy's planet total and divides
  by a hundred before printing it, so a 40% condition in a 500-planet galaxy
  reads "Owns 200 planets.";
* a condition the game is **not** playing for is still listed, in grey, with
  its setting. That is what lets a player see what the game is not asking of
  them.

The last two lines — how many conditions win, and how many years must pass —
are settings rather than something a player can meet, and take no tick.

### The sentences are written, not copied

The original assembles each line from a lead string, a value and a trailing
string (ids 938–964: `Owns `, ` planets.`, `Attains Tech `, `in `, `fields.`,
…). This project does not copy the game's prose — the same rule that governs
its message text and the Technology Browser's component notes — so the wording
here is our own. The **structure and the numbers** are the original's, and both
are checked by test.

## Face 3 — Progress Timeline

`DrawHistoryReport` (`1108:2494`). One figure, drawn year by year, a line per
player, with the title "History of *something*" over it. In the original the
title is a popup menu — the cursor turns to a hand over it — offering the same
eight figures as the scoreboard's rows, with the trailing colon dropped
(`LFetchScoreXVal`, `1108:2f94`, fetches by that same index).

**The horizontal axis** is the last hundred years at most. The span is the
current turn rounded up to a multiple of five; past fifty it rounds to tens
instead, and past a hundred it stops growing and the window slides. Gridlines
fall every five years, or ten once the span is past fifty, and are labelled
with the year — the original adds `0x960`, 2400.

**The vertical axis** climbs a ladder rather than being computed. The maximum
is taken over every row the file holds, floored at five, and the gridline
interval is the first of these it fits under:

| up to | interval |
|-------|----------|
| 12 | 1 |
| 25 | 2 |
| 60 | 5 |
| 120 | 10 |
| 300 | 25 |
| 600 | 50 |
| 1,200 | 100 |
| 6,000 | 500 |
| 12,000 | 1,000 |
| beyond | `max / 12` rounded down to a multiple of 500 |

which is why the axis snaps to familiar numbers and always ends up with about a
dozen lines.

**The colours** are the game's own sixteen, `rgcrPlrHistory` at `1120:002e` —
sixteen `COLORREF`s, which are `0x00bbggrr`, so player 0 is `#f0f03f` and
player 3 is pure blue. The player looking at the graph is drawn **white**
instead, which is the same rule the scanner follows: `MANUAL.PDF` p. 5-16 says
"your planets remain white", and sends a player here to find out which colour
is theirs. This project's invented palette has been replaced by that table.

A player with a single year gets a dot rather than a line — the original calls
`SetPixel`.

## What is reproduced

The modeless window and its cycling button with the three titles; the
scoreboard's rows, its packing, and all three of its highlights including a
dead player's empty column; the victory report's nine lines, their settings,
the planet condition's percentage-to-count conversion and the grey of a
condition the game is not playing for; the timeline's window, both axes and
their ladders, the year labels, the player colours and the white line for
oneself.

## What is not

* The original's **prose** for the victory sentences, as above.
* The player names as **rotated** column headers, which the original draws with
  a 90° Arial (`rghfontArial8[4]`). The columns are the same columns; the names
  read across rather than up.
* The dialog's exact geometry, which `InitScoreDlg` computes from the widest
  string, the player count and the system metrics.
* The **tutorial hooks**: opening, closing and cycling the sheet all call
  `AdvanceTutor`.
