# The message pane

Status: **behaviour recovered and reimplemented**; the drawing is not the
original's.

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
| `[f]` a square at the left, as tall as the bar | silences the kind of message being shown | only while a real message is shown |
| `[m]` a 24-pixel strip before the right square | switches to writing a message to another player | only in a multi-player game |
| `[v]` a square at the right | shows the silenced messages anyway (`fViewFilteredMsg`) | only when something the player has been sent *is* silenced |

That last condition is worth keeping: the original walks the sent-message and
filtered-message bitfields together and draws the button only where they
overlap, and if they do not overlap at all it forces `fViewFilteredMsg` back
off. There is no point offering to reveal messages that do not exist.

## The buttons

| | label | enabled when |
|---|-------|--------------|
| 0 | `Prev` | `IMsgPrev(0) != -1` |
| 1 | `Goto`, or `View` for a battle, `Reply` for another player's message, `Done` while writing one | the message points at something (`mdMsgObj != 0`) |
| 2 | `Next` | `IMsgNext(0) != -1` |
| 3 | send | only while writing a message |

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
| `-2`, `-3`, `-4`, `-5`, `-7` | one of the game's own windows: a report, the score sheet, the serial-number box |
| `-6` | a `THING`, whose id is the first parameter — the scanner centres on it |
| `0xc000` set | a component, in the part browser |
| `0x4000` clear, negative | a fleet, id in the low 15 bits — and *nothing* if that fleet no longer exists |
| `0x4000` clear, positive | a planet, id as it stands |
| `0x4000` set | a place on the map, from the first two parameters: how a battle report finds its battle |

A filtered message's button is dead even while it is on screen.

There is a two-stage Goto worth noting: for a message about a planet whose id is
`0x3e`, `0x3f` or in `0xaf..=0xb4` — the production ones — the first Goto
selects the planet and the second opens **Change Production**.

## What this project does

`crates/stars-ui/src/views/messages.rs`, driven by the pane state and methods on
`App` so the behaviour is testable without drawing anything.

Reproduced: one message at a time; the title bar and its two filter controls,
including the condition for showing the second; Prev/Next stepping over what is
filtered; the three bodies of text; Goto to a planet or a fleet; the keys.

Not reproduced: the original's bitmaps (this has short labelled buttons in the
same places), the diagonal FILTERED watermark (it is written above the text
instead), writing messages to other players, and the Goto targets that need
windows this project does not have — the part browser, the report dialogs, the
battle VCR at a position. Those leave the button dead rather than lying about
where it would go.
