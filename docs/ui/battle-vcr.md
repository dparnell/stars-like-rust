# The battle VCR

Status: **playback, transport and layout recovered**; the board's artwork and
the two token panels are not the original's.

`BattleVCR` (`10e8:0000`) opens it and `VCRDlg` (`10e8:0e90`) runs it, with
`DrawVCR` (`10e8:1c62`) painting the board and `SetVCRBoard` (`10e8:08d8`)
winding the recording to a step. A player reaches it from a battle message's
**View** button, or from the Battles report.

The playback model is `docs/formulas/combat.md`'s business and
`crates/stars-ui/src/vcr.rs`'s: the VCR **plays the recording** rather than
re-simulating it, because the gameplay generator's state is not in the save
files. This spec is about the screen.

## The controls

Dialog resource **160**, `Battle VCR`, 260 by 270 dialog units. Seven buttons
in one row along the foot, all 32 by 13 at `y = 244`, and nothing else — the
whole 260 by 244 above them is the board and the two token panels, painted
rather than placed:

| id | x | caption |
|----|---|---------|
| `0xa1` | 4 | `\|<<` |
| `0xa2` | 39 | `<` |
| `0xa3` | 76 | `>/\|\|` |
| `0xa4` | 114 | `>` |
| `0xa5` | 150 | `>>\|` |
| `1` | 187 | `&Done` |
| `0x76` | 224 | `&Help` |

The five transport buttons carry **icons** at run time — `rghiconVCR`, seven of
them loaded in `FCreateStuff` (`1000:0014`) — so the captions above are what
shows without them. They are the resource's own text either way, and the third
one says what it is: `>/||` is a single button that plays *and* pauses.

## Which buttons are alive

`EnableVCRButtons` (`10e8:48f6`) decides that, and where the focus goes, from
one number — `viStepVCRCur`, the step the board stands at. It runs from **-1**,
the board before anything has happened, to `vcStepVCR`:

| buttons | alive when |
|---------|-----------|
| `0xa1`, `0xa2` — back to the start, step back | `(unsigned)viStepVCRCur < 0x8000`, which is `>= 0` written the short way |
| `0xa3`, `0xa4`, `0xa5` — play, step, to the end | `viStepVCRCur < vcStepVCR` |

and then, because the button under the pointer may have just been disabled, it
**moves the focus by hand at either end**: to `0xa3` (play) at the start, and to
`1` (Done) at the end. In between it leaves the focus where it is.

That `-1` is worth keeping. It is a real state, not an empty one: the board
stands with every token where it began and nothing yet done to it, which is
what `SetVCRBoard` rebuilds from the recording's token list whenever the player
steps backwards past a frame it has already applied.

## Winding backwards

`SetVCRBoard(iStep)` only ever moves **forward** through the record list. To go
back it first rebuilds the whole board from the recording's own token table —
resetting every token's ships, damage and shields — sets `viStepVCRCur` to -1,
and then replays from the beginning up to the step asked for. Round changes are
noticed on the way, and every token with the regenerating-shields bit has its
shields put back at each one.

## What this project does

`crates/stars-ui/src/views/battles.rs` over `crates/stars-ui/src/vcr.rs`.

Reproduced: the transport's five buttons with the resource's own captions and
order, and `EnableVCRButtons`' rule for which are alive and where the focus
lands, tested against a real recording; play stopping of its own accord at the
last frame, which is what the forward buttons dying amounts to; and the
playback itself, which is the module's own business.

Not reproduced: the button **icons**, which need the game's own; `DrawVCR`'s
board and its two token panels, which are drawn here as a grid of coloured
marks; and the Help button, which has nothing behind it.
