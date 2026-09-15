# The battle VCR

Status: **playback, transport, layout and the board's drawing recovered**;
the panel beside the board is not yet the original's.

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

## The board

`DrawVCR` paints ten squares by ten of `dxyVCRSquare` pixels — 32, or 64 when
the window is large — three apart, from an origin of (10, 10), inside a sunken
frame. Every square is framed in black; an empty one is filled black
(`PatBlt BLACKNESS`), and an occupied one shows the stack on it through
`DrawFleetBitmap` (`1050:490e`): the design's own picture from the small ship
sheet (or the large), the owner's emblem over its bottom-left corner — eight
pixels on the small picture, sixteen on the large — and, for every further
stack on the same square up to three, a small cross in the next corner. When
the focus token stands on the square it is the one shown; otherwise the first.
The focus square (`viVCRFocus`, or the square `vbrcVCRFocus` names when no
token is picked) is framed in blue two pixels wide (`hbrBlue`).

Clicking a square picks the stack on it out, and the next stack along on the
next click; `PopupVCRMenu` (`10e8:4518`) offers the same choice from the right
button.

## The shots

`AnimateAttack` (`10e8:3ac2`) runs after the board for a record with kills.
It works through the kills a target at a time, or-ing their `grfWeapon` flags
(`KILL` byte 1 — see `../formats/battle.md`):

| bit | meaning | drawn |
|-----|---------|-------|
| 0 or 1 | a beam | two lines from the near edge of the attacker's square — a third of a square either side of its centre, on the edge facing the target — to the target's centre, in `hpenEnemy` (red) or, with bit 1, `hpenStarbase` (blue); then icon 0 on the target |
| 2 | a torpedo | while `fAnimate`, one of the four torpedo icons flown from the beam's point to the target in eight steps a square, each held `0x23 − 10 × viSpeedVCR` ticks; then icon 1 on the target |
| 6 | torpedoes deflected | no landing burst |

and last, on every target, icon 2 where ships were destroyed and icon 0 where
not. The seven icons are `rghiconVCR`, loaded by name in `FCreateStuff`:
`BANG1ICO`, `BANG2ICO`, `BANG3ICO` — bursts of growing size — and `TORP1ICO`
to `TORP4ICO`, the torpedo's four frames. All are 32 pixels square with an AND
mask; `stars_formats::resources::read_icon` reads them and
`art::VCR_ICONS` names them. The recordings in `fixtures/games` carry the
flags 1, 4, 12, 196 and 204 — a beam, a torpedo, a torpedo with bit 3, and
torpedoes deflected (`0xc4`, `0xcc`: bits 6 and 7 together).

## What this project does

`crates/stars-ui/src/views/battles.rs` over `crates/stars-ui/src/vcr.rs`.

Reproduced: the transport's five buttons with the resource's own captions and
order, and `EnableVCRButtons`' rule for which are alive and where the focus
lands, tested against a real recording; play stopping of its own accord at the
last frame, which is what the forward buttons dying amounts to; the playback
itself, which is the module's own business; the board as `DrawVCR` paints it,
with the game's pictures, emblems and icons when a copy of the original is at
hand (coloured squares and red bursts when not); the shots as `AnimateAttack`
draws them, the torpedoes flying from the moment a frame is entered —
stepped to or played — at fifteen milliseconds a step, the bursts landing
when they arrive, and a played frame held until they have; and the focus,
picked by clicking a square.

The transport's captions are the resource's ASCII marks in the dialog font.
They are drawn here as the same marks in shapes — a bar and a triangle or two;
the play button carries both halves of `>/||`, the triangle and the two bars,
with the half a press would do next in ink and the other faint, so it is told
from the step button's lone triangle — on push buttons in the Windows
face with its bevel, engraved when disabled, which reads as the original's
intent rather than its lettering.

The text panel beside the board follows `DrawVCR`'s lines: `Phase n/N,
Round r/R` (the step and its count both counted from one, so the board
before anything has happened reads phase 1); `Playback speed: s` with its
two spin buttons (`viSpeedVCR` 0 to 3, shown one higher; here it scales
the hold of a played frame from twice the resting six tenths of a second
down to half); then, once a step stands on the board, whose token acted —
the owner's name in the possessive, the design with its ship count when
more than one, blue when it is the focus — and for a shot `attacks
<owner's>`, the target's design (red when ships died), `at x,y doing`, the
shield damage, the armour damage, `no damage.` for torpedoes that all
missed, `destroying n ship(s).` and the deflection line in red; and, from
200 pixels down, the selection: `Selection: x,y`, the focus token's owner
and design (the ships lost since the start after a plus) in blue, `Dead`
or its initiative and moves on one line, armour (from the design, when the
player holds it) and damage on the next, shields, jamming when it has
any, tactic, and the primary and secondary target classes. The original's
dark blue and dark red are lifted to read on this shell's dark ground. Not
reproduced: the `Goto` button under the selection, and the Help button,
which has nothing behind it. Below the original's lines this project adds
its own list of the tokens, to pick one out by name.
