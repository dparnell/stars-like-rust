# The message pane

Status: **recovered and reimplemented** — behaviour, hit-testing, Goto, the
watermark, and the title bar's own bitmaps.

The pane along the bottom-left of the main window is where a player reads the
year's news. It shows **one message at a time** — not a list — and that shapes
everything about it: the title bar says which message of how many, three buttons
step and follow, and the filter decides which ones the stepping stops on.

Source: `MessageWndProc` (`1030:5c92`), `SetMsgTitle` (`1030:7218`),
`DecorateMsgTitleBar` (`1030:799c`), `HtMsgBox` (`1030:7d8c`), `IMsgNext`
(`1030:7808`) and `IMsgPrev` (`1030:78d8`).

## Layout

```
┌──────────────────────────────────────────────┐
│ [f]   Year: 2401  Messages: 3 of 12   [m][v] │  title bar
├──────────────────────────────────────────────┤
│                                              │
│  Fleet #3 has laid 40 mines.                 │  message text, scrolled
│                                              │  when it does not fit
├──────────────────────────────────────────────┤
│  [ Prev ]   [ Goto ]   [ Next ]              │
└──────────────────────────────────────────────┘
```

The title bar is a strip `dyArial8 * 2` tall with a 3D frame; the text is
centred in it, truncated to the width less 48 pixels so the decorations at each
end are never overdrawn. The three buttons are `0x2c` wide and
`dyArial8 * 3 / 2` tall; a fourth, `0x32` wide, appears only in send-message
mode.

## The title bar

`Year: %d%c  Messages: %d of %d` (`idsYearDCMessagesDD`), or
`Year: %d%c  Messages: (none)` (`idsYearDCMessagesNone`) when there are none.
The message number is `iMsgCur + 1`, so the pane reads **`0 of 12`** while it
sits before the first message — which is where it lands when every message of
the year is filtered.

Three of its decorations are controls, hit-tested by `HtMsgBox`:

| where | what | when |
|-------|------|------|
| `[f]` a square at the left, as tall as the bar | silences the kind of message being shown | only while a real message is shown, and not while one is being written |
| `[m]` a `0x18`-wide strip before the right square | switches to writing a message to another player | only when the game is not `fSinglePlr` |
| `[v]` a square at the right, as tall as the bar | shows the silenced messages anyway (`fViewFilteredMsg`) | only when something the player has been sent *is* silenced |

That last condition is worth keeping: the original walks the sent-message and
filtered-message bitfields together — `0x31` bytes each, so 392 ids fit — and
draws the button only where they overlap, and if they do not overlap at all it
forces `fViewFilteredMsg` back off. There is no point offering to reveal
messages that do not exist.

**And the shape of the test is not the obvious one.** Send-message mode does
not merely disable the right-hand square: `HtMsgBox`'s condition is

```c
if (pt.x < right - square || writing) { ...mode strip... }
else                                  { ...reveal square... }
```

so while a message is being written the whole right side goes down the *mode*
branch, and the square's own rectangle answers as `htMsgMode` rather than as
nothing. Reproduced as written.

## The buttons

| | label | enabled when |
|---|-------|--------------|
| 0 | `Prev` | `IMsgPrev(0) != -1` |
| 1 | `Goto`, or `View` for a battle, `Reply` for another player's message, `Done` while writing one | the message points at something (`mdMsgObj != 0`) |
| 2 | `Next` | `IMsgNext(0) != -1` |
| 3 | `Delete` (`idsDelete`) | only while writing a message |

**Stepping skips what is filtered.** `IMsgNext(fFilteredOnly)` walks forward
until the filter bit of the message's id differs from `fFilteredOnly`; with
`fViewFilteredMsg` set it does not filter at all and simply steps. `IMsgPrev` is
the same backwards. So the same two routines serve both "step over the hidden
ones" and "step through only the hidden ones".

## The keys

| key | what |
|-----|------|
| ↓ | next message (with a modifier: the last) |
| ↑ | previous (with a modifier: the first) |
| Home / End | the first / the last |
| Enter | Goto |
| `+` | silence the kind of message being shown — `SetFilteringGroups`, then `DirtyGame(1)` |
| `-` | show the silenced messages, or stop showing them |

`+`, `-` and Enter are forwarded to the pane from the frame wherever the player
is (`mdi.c:1880`), so they work without clicking on it first.

## What the text says

Three cases, and only the first is the message itself:

* the message, formatted from its id and parameters;
* `idsMessagesHaveSentYearFilteredIfWant` — "All the messages you have been sent
  this year are filtered out…" — when the pane is before the start and there are
  messages;
* `idsMessageTypeHasFilteredWillShownDefault` — "This message type has been
  filtered out and will not be shown by default anymore." — the message the
  player has just silenced, still on screen.

When a filtered message is on screen because the player asked to see the
filtered ones, the word **FILTERED** (`idsFiltered`) is drawn *diagonally*
across the text, corner to corner (`DiaganolTextOut`).

## Where Goto goes

`SetMsgTitle` classifies the message's object word. It is not a plain id:

| word | goes to |
|------|---------|
| `-1` | nothing; the button is dead |
| `-2` | the Research dialog (mode 3, posts `&Research...` `0x7e`) |
| `-3` | the Ship Design dialog (mode 5, `0x7d`) |
| `-4` | the Score sheet (mode 8, `0x5f`) |
| `-5` | the serial-number box (mode 9, dialog `0x56`) |
| `-7` | the Battles report (mode `0xb`, `0x901`, unless it is already up) |
| `0x4800` | Player Relations (mode 7, `0x7de`) |
| `-6` | a `THING`, whose id is the first parameter — the scanner centres on it |
| `0xc000` set | a component, in the part browser |
| `0x4000` clear, negative | a fleet, id in the low 15 bits — and *nothing* if that fleet no longer exists |
| `0x4000` clear, positive | a planet, id as it stands |
| `0x4000` set | a place on the map, from the first two parameters: how a battle report finds its battle |

A filtered message's button is dead even while it is on screen.

Goto **selects on the map**: `SelectAdjPlanet(0, id)` or
`SelectAdjFleet(0, id)`, the same two calls a click on the scanner makes. It
opens no report — the reports are windows of their own. After a fleet it
posts the scanner a `v`, the key that centres on the selection, which is how
the tutorial's *"Hit the v key to pinpoint it"* and Goto come to do the same
thing.

There is a two-stage Goto worth noting, and the second stage is not a count
of clicks. `1030:6e4c` compares `sel.grobj` and `sel.idpl` against the
message's object: a production message whose planet is **already the
selection** opens **Change Production** instead of selecting it again. Which
is the same shape as the Battles report's two-step, where a row selects the
place first and plays the recording only once the place is already
selected.

## What this project does

`crates/stars-ui/src/views/messages.rs`, driven by the pane state and methods on
`App` so the behaviour is testable without drawing anything.

Reproduced: one message at a time; the title bar and its two filter controls,
including the condition for showing the second; Prev/Next stepping over what is
filtered; the three bodies of text; Goto to a planet or a fleet; the keys.

Also the **diagonal FILTERED watermark**. `DiaganolTextOut` builds a `LOGFONT`
at `lfWeight = 900` — the heaviest there is — points its `lfEscapement` down
the message rectangle's own diagonal, and then shrinks the size until the text
fits with **eight pixels** to spare in both directions, starting from the
longer of the two sides and giving up entirely on a rectangle under ten pixels
either way. This solves the same fit in one step rather than looping, since the
extent scales with the size, and turns the galley by the same angle.

Goto goes where the original sends it: a planet or a fleet becomes the
selection on the map, a space object is found by the **id** its message
carries — a `-6` message's first parameter is the `THING`'s own id, not an
index — and a battle at a place opens the **VCR**, which this project now
has.

### The title bar's bitmaps

`FCreateStuff` (`1000:07ba`) loads two: the colour strip `hbmpMsg`, bitmap
**134**, and the one-bit `SRCAND` mask `hbmpMono`, bitmap **199**.
`DecorateMsgTitleBar` blits each decoration as the usual pair — the mask
with `SRCAND`, then the colour with `SRCPAINT` — so the glyph comes out
transparent over whatever the bar is painted in.

The two strips are packed **separately**, 16 by 66 and 15 by 84, and a
glyph's row in one is not its row in the other:

| decoration | colour row | mask row | size |
|------------|-----------|----------|------|
| filter, this kind still shown | 0 | `0x1c` | 15 × 14 |
| filter, this kind silenced | `0x0e` | `0x2a` | 14 × 12 |
| reveal, filtered ones hidden | `0x29` | `0x45` | 15 × 15 |
| reveal, filtered ones showing | `0x1a` | `0x36` | 15 × 15 |
| from another player | `0x38` | — | 15 × 9 |

Two things there are easy to get wrong. The filter square **changes size**
as well as row when the kind is already silenced — 15 × 14 becomes
14 × 12 — and the last one is **not masked at all**: the original blacks a
17 × 11 rectangle with `PATBLT` and `SRCCOPY`s the glyph into the middle of
it, so it is a picture on a black ground rather than a shape.

This project draws the two controls from those bitmaps, read at run time
out of the player's own copy of the game, and falls back to the short
labels it used to have when there is no copy to read. The third — the
envelope on the mode strip — is drawn straight over a black rectangle,
as the original blits it, whenever the game is not single-player.

## Writing to other players

The mode strip — the `0x18` before the right square, wearing the envelope
`DecorateMsgTitleBar` blits at `right − 0x2e` whenever the game is not
`fSinglePlr` — and **Reply** on a letter from another player both go through
`MessageWndProc`'s mode arm (`1030:6298`), which sets `gd` bit 8. Entering
from one of the year's own messages presets the dropdown to Everybody
(`viInRe = 0`) and keeps whichever letter was last in hand (`iMsgSendCur`);
entering from a letter presets it to the letter's sender and puts in hand
the letter already replying to it — recipient the sender, `iInRe` this
message's index — or a fresh one past the last.

In the mode `SetMsgTitle` (`1030:7218`) retitles the pane `Send Messages
(%d of %d)` (`idsSendMessagesDD`: the letter in hand from one, over the
letters written), shows `To:` (`idsTo3`) at the left of the text box's top
row, the recipient dropdown (`hwndMsgDrop`, filled at creation with
`Everybody` (`idsEverybody`) and then every player by `PszPlayerName`)
across the row to `0x54` short of the right, the **Delete** button
(`rghwndMsgBtn[3]`, `idsDelete`, `0x32` wide) in that gap, and the edit box
(`hwndMsgEdit`) under them; the middle button reads **Done** (`idsDone`).
Prev is live only when a letter lies before the one in hand; Done and Next
are always live. The box shows the letter in hand — its recipient and text
— or the preset and nothing for a fresh one.

Every button goes through `FFinishPlrMsgEntry(dInc)` (`1030:9bd6`), which
reads the box: an **empty** box deletes the letter in hand (the hand
stepping back one when it can), and then Prev (`−1`) or Next (`+1`) moves
the hand from there; a full box saves it — replacing the letter's recipient
and text, or making a new letter with `iInRe = iMsgCur` when the hand is
past the last — and the hand moves by `dInc`, never below the first. Prev
is `−1`, Next `+1`, Done `0` and then the mode bit off, and Delete is
`1000`, which the routine reads as an empty box. Down and Up are Next and
Prev while writing, Enter is Done, and Home and End do nothing. The letters
are `../formats/player-message.md`.

Reproduced: `App::start_writing`, `finish_letter` and the buttons on it,
`message_recipients`, `can_write_messages`; the letters go into the order
file from `App::outgoing` and to the host with the turn. Not reproduced:
`ptSticky…`-style positions, and the `det` bits `MarkPlayersThatSentMsgs`
sets in the recipients' files, which nothing reads.

The window Gotos above are `MessageWndProc`'s Goto arm (`1030:6d8d`, a
`switch` on `mdMsgObj`); each opens the same window here, except the
serial-number box, which this project has no use for, so that button alone
stays dead.
