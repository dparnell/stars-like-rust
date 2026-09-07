# The race wizard

Status: **both faces built** — the read-only viewer and the editable Custom
Race Wizard; their owner-drawn appearance is not reproduced.

View (Race), `IDM_RACE_EDIT1` (`0x9c`), **F8**. The original opens the race
wizard itself on the player's own race, read-only — six dialogs,
`IDD_RACE_WIZARD_1` (146) through `_6` (151), walked with `< Back` and
`Next >`.

## The six pages

Recovered from the dialog templates, which is also where the trap is: pages 2
and 3 hold almost **no controls**. The habitability sliders and the economy
bars are painted in `WM_PAINT` rather than laid out, so the templates alone
describe a nearly empty dialog. What each page *says* had to come from the
race record instead.

| | page | what the template holds |
|-|------|-------------------------|
| 1 | Race | `Race Name:`, `Plural Race Name:` and `Password:` edits; eight buttons — `Humanoid`, `Rabbitoid`, `Insectoid`, `Nucleotid`, `Silicanoid`, `Antetheral`, `Random`, `Custom` (ids `0x10f`–`0x116`); `Spend up to 50 leftover advantage points on:` and its combo |
| 2 | Habitability | three `Immune to …` checkboxes stacked at one position, one per variable; the rest painted |
| 3 | Economy | one checkbox, `Factories cost 1kT less of Germanium to build`; the rest painted |
| 4 | Primary Racial Trait | the ten traits as radio buttons |
| 5 | Lesser Racial Traits | fourteen checkboxes, captions filled in at run time |
| 6 | Research Costs | `Costs 75% extra` / `Costs standard amount` / `Costs 50% less`, once per field |

Every page carries the same five buttons: Help, Cancel, `< Back`, `Next >`,
`Finish`. They **stop at the ends** rather than wrapping, because a wizard's
do. The viewer keeps Back and Next and drops the three that would change
something; the wizard keeps Cancel and Finish as well.

The caption counts the steps: string `0x010e`, `"Custom Race Wizard - Step %d
of 6"`.

## What page 5 settled

The fourteen checkboxes have consecutive ids `0x123`…`0x130` and their captions
are filled in at run time. `RaceWizardDlg5`'s `WM_INITDIALOG` loops `i` from 0
to 13, giving checkbox `0x123 + i` the string `0x132 + i` and its state from
**bit `i`** — so the string order, the checkbox order and the bit order are one
and the same thing.

That fixed two things this project had wrong:

* **bit 5 had no name at all** in `stars_core::race::lrt`. It is Ultimate
  Recycling;
* **`docs/formats/race-r.md`'s bit table was wrong from bit 6 down**, and had
  been marked confirmed on the strength of the shipped AI races decoding to
  "sensible, overlapping trait sets". Both orderings do that, so the argument
  never discriminated between them. The code does. See that file for the
  corrected table — the engine's own constants were right all along, so no
  behaviour changed.

## What is shown

Each page's settings, read out of the player's race record, in the wizard's own
order: the names and whether a password is set; the three habitable ranges with
their ideals — or `immune`, since a negative upper bound is the immunity marker
and the row is then one thing rather than a range and a flag — and the growth
rate; the seven economy figures and the cheap-factories trait; the primary
trait; all fourteen lesser traits, listed whether taken or not so that what is
*not* taken is as plain as what is; and the six research fields with their
three-way setting, plus whether expensive technologies start at level 3.

## The editable wizard

File (Custom Race Wizard) opens the same six pages with the settings editable —
`RaceCreationWizard` (`10e0:0000`), which the original also uses for the viewer
by passing `fReadOnly`.

### The advantage points

The counter is what the wizard is for. Every choice spends from one budget or
refunds into it, and the figure is drawn on every page — two lines,
`"Advantage"` over `"Points Left"` (strings `0x052f` and `0x0530`) — in red
when it is negative (`DrawRaceAdvantagePoints`, `10e0:55c6`). It is
[`stars_core::advantage_points`], already recovered for the New Game dialog;
see `docs/formulas/new-game.md`.

A race that costs more than the budget **cannot be saved**, and the original
says so in a message box naming the shortfall (string `0x0515`). Finish is
disabled here and carries the reason as its hover text — worded by this
project, as the game's own message text always is — rather than accepting the
press and refusing afterwards.

### The seven predefined races

The buttons on page 1 load `vrgplrDef` (`1120:0da2`), a `PLAYER[7]` in the data
segment, transcribed in `stars_core::presets`. They are exactly the seven `.r1`
files that ship with the game, which is how the table is checked: writing each
preset out under the header its shipped file carries reproduces that file **byte
for byte** (`crates/stars-core/tests/race_writing.rs`). Note the game's own
spellings, `Nucleotid` and `Antetheral`, which differ from the filenames they
ship in.

The eight are **radio buttons, not commands**: `RaceWizardDlg1` works out which
one to check by comparing the race being designed against each `vrgplrDef` entry
(`__fmemcmp` over the first `0x80` bytes of the player struct — everything but
the two names), and checks the eighth, `Custom`, when none matches. So the page
always says what the race currently is, and pressing `Custom` keeps whatever the
wizard holds.

Two details worth having:

* pressing a button replaces the race and its emblem, but the **names only if
  the race is still called after one of the seven** — the original compares the
  name box against the seven strings before refilling it, so a name the player
  typed survives;
* choosing `Random` **disables `Next >`**, because a random race is not one
  there is anything to edit.

### What Finish writes

A `.rN` file: a plaintext header stamped `FileType::Race`, turn 1, player 31,
one encrypted type-6 block holding the race, and an empty footer — the shape
described in `docs/formats/race-r.md`. `stars_core::save::race_file` builds it;
the file dialog uses the original's own filter, string `0x0531`, `"Stars! Race
Files|*.R*|"`, and its failure message is string `0x010b`.

Every one of the seven shipped race files is written back byte for byte from
what was read out of it, which covers the header, the cipher, the framing and
every field of the record at once.

That closes the loop with the New Game dialog, whose player rows already load a
race from a file: a race designed here can be played in the next game.

### What byte 81 bit 6 turned out to be

Writing the files back byte for byte is what settled it. `PLAYER.grbitAttr` is
a `uint32_t` at `+0x4e`, so the lesser-trait word at offset 78 and the
"checkbox" byte at 81 are one field: the checkboxes are bits 24 to 31 of it.
Bit 6 of that byte is bit 30, `ibitRaceAIPlayer` — set on the shipped
`random.r1` and on nothing else, because that is the template the computer
players are built from. It is modelled now rather than dropped, so a race file
round-trips exactly. See `docs/formats/race-r.md`.

## What is not

* The **appearance**: the sliders, the bars and the eight race buttons are
  drawn in the original and are plain rows here.
* The **slider bounds**. The original's habitability and economy sliders have
  ends this project has not recovered; the wizard clamps only to what the file
  can hold and lets the points counter be the constraint, which it is.
* The **password**. Page 1 has the box, but what a race file stores is a salt
  of the password rather than the password (`docs/formats/orders-x.md`), and
  nothing is written from the box yet.
* The **Help** button, which the original wires to its own help file.

## A name this project had wrong

`Prt::name` called `Prt::It` **"Inner Tech"**. Page 4's ten radio buttons carry
the game's own captions, and that one reads `Interstellar Traveler`; "Inner
Tech" is this project's shorthand for the trait, which had leaked into the one
place a name is shown to a player. The captions are also where `Hyper-Expansion`
and `Inner-Strength` get their hyphens.
