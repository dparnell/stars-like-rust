# The About box — Help (About Stars!...)

Status: **recovered**: the template, the version line, the rolling
credits and the ordering box — behind a box of this project's own.

## This project's box first

Help (About Stars!...) opens **this project's** About box, not a copy
of the original's: the same template (resource 90, its geometry and its
roll) with the captions replaced — *Stars-re for Windows, macOS and
Linux*, *A reverse-engineered rewrite of Stars!*, this crate's version
beside the original's (*Version 0.0.1 — rebuilt from Stars! 2.60j*), the
original's copyright as a reference, *Not affiliated with its authors or
publisher*, and a roll that opens with how this program was made (the
reverse engineering in Ghidra with the NB09 symbols, the rewrite in
Rust and egui, the assets read from the player's own copy) before it
goes on to the original's seventy-seven credit lines under *Stars!
2.60j, by*. Its **About Stars!...** button opens the original's box,
described below, which is where its **Order Info...** lives.
`crate::dialog::ABOUT_PROJECT`; `App::about_project_version`,
`App::about_project_credits`; `views::about::view`. The original's box
is `views::about::original`.

`CommandHandler` answers menu id `0x63` (`1020:3050`) with
`DialogBox(hInst, 90, hwnd, About)`; `About` is at `1018:1252`.

## The template

Dialog resource 90, `About Stars!`, 192 by 175 dialog units in MS Sans
Serif 8 (`crate::dialog::ABOUT`):

| id | class | at | text |
|----|-------|----|------|
| — | static, `SS_ICON` | 8, 8, 18×20 | `StarsIco` — the game's icon |
| — | static, centred | 36, 8, 156×8 | `Stars! for Microsoft Windows` |
| — | static, centred | 36, 18, 156×17 | `The Advanced Interstellar Strategy Game` |
| `0x401` | static, centred | 4, 32, 184×8 | `Demo Version` — overwritten at `WM_INITDIALOG` |
| — | static, centred | 4, 48, 184×8 | the copyright line |
| — | static, centred | 4, 60, 184×66 | `Published by empire Interactive` |
| `0x41f` | static | 4, 70, 184×66 | empty — the credits roll through it |
| — | static, centred | 4, 140, 184×8 | the web address |
| `0x76` | button | 32, 156, 48×14 | `&Order Info...` |
| `0x1` | button | 112, 156, 48×14 | `OK` |

The publisher's static is 66 units tall and overlaps the credits; that
is the resource as shipped, and since a centred static draws on its top
line nothing shows for it.

## What the procedure does

- `WM_INITDIALOG` (`1018:12a0`): `iAbout1st = -11`, `iAboutPartial = 0`;
  control `0x401` gets `SzVersion()`, and a fifty-millisecond timer
  starts (`SetTimer(hwnd, 0xe, 0x32, 0)`).
- `SzVersion` (`1018:1212`) is `wsprintf(idsVersionD02dC, 2, 0x3c, 'j')`
  — the string is `Version %d.%02d%c`, so the box says **Version
  2.60j**. (The file is called 2.7j; the box does not.)
- `WM_ERASEBKGND` fills the client with `hbrButtonFace`; `WM_CTLCOLOR`
  hands every static the same brush.
- `WM_TIMER` (`1018:1320`) is the roll. `iAboutPartial += 2`; when it
  reaches `dyArial8` it is zeroed and `iAbout1st` steps on, wrapping to
  `-11` past `0x4e`. Then, on control `0x41f`'s DC, in `rghfontArial8[1]`
  (Arial 8 **bold**), button-face behind and `crButtonText` in front,
  clipped to the control: ten lines from `iAbout1st`, each `dyArial8`
  tall and centred (`RcCtrTextOut`), the first shifted up by the
  partial; lines below zero are skipped and the loop stops past `0x4c`.
  Line *n* is string `idsDesignProgramming + n` — `0x277` to `0x2c3`,
  seventy-seven of them: the two designers, the AI, the artists, the help
  file, the technical adviser, the music, the sound effects and thirty-five
  play testers, with blank lines between the headings. Starting at `-11`
  means the roll begins empty and the first line rises in from the
  bottom.
- `WM_COMMAND`: `1` or `2` (OK, Esc) kills the timer and ends the dialog;
  `0x76` runs dialog **97** — `empire Interactive Ordering Information`,
  172 by 124, six centred lines and an OK (`crate::dialog::ORDER_INFO`),
  with `OrderInfo` at `1018:151e` — over it.

## What this project does

`views::about::draw_box` draws either template control by control: the icon
from the executable's resources (`StarsIco`, through `Art::icon`), the
statics centred on their top line, the version from
`stars_formats::resources::text::version` (the string table, decoded at
run time from the player's copy — `../formats/resources.md`), and the
credits from `text::credits`, rolled by egui's clock rather than a
timer: `About::position` turns the seconds since the box opened into
`(iAbout1st, iAboutPartial)` by the rule above, so a redraw at any
moment lands where the timer would have. Bold is painted as a second
pass a hair to the right, egui's set having no bold face. Without the
executable the roll is empty and the version line is the template's
`Demo Version`, which is what the resource holds before `SzVersion`
writes over it.

`Order Info...` puts the second box up as its own window; OK on either,
or the window's close, takes it down. `tests/about.rs` checks the roll's
arithmetic tick by tick, the strings against the executable, and the
buttons through a real egui pass.

## What is not

* Modality: the original blocks the frame while the box is up; here it
  is a window like the rest.
* The strings are the game's own and are not in this repository; the
  template captions, which are the resource's, are.
