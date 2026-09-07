# The race viewer

Status: **the six pages' contents reproduced**; their owner-drawn appearance is
not.

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
| 1 | Race | race name, plural name, password; the eight starting races as buttons; the leftover-points combo |
| 2 | Habitability | three `Immune to …` checkboxes stacked at one position, one per variable; the rest painted |
| 3 | Economy | one checkbox, `Factories cost 1kT less of Germanium to build`; the rest painted |
| 4 | Primary Racial Trait | the ten traits as radio buttons |
| 5 | Lesser Racial Traits | fourteen checkboxes, captions filled in at run time |
| 6 | Research Costs | `Costs 75% extra` / `Costs standard amount` / `Costs 50% less`, once per field |

Every page carries the same five buttons: Help, Cancel, `< Back`, `Next >`,
`Finish`. The viewer keeps Back and Next — and they **stop at the ends** rather
than wrapping, because a wizard's do — and drops the three that would change
something.

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

## What is not

* The **appearance**: the sliders, the bars and the eight race buttons are
  drawn in the original and are plain rows here.
* **Editing**. This is the read-only face of the wizard; there is no race
  creation wizard in this project at all, which is what File (Custom Race
  Wizard) opens.
* The **leftover advantage points** combo on page 1, which is a creation-time
  setting rather than a property of a finished race.
