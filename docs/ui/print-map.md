# Print Map — File (Print Map...)

Status: **the dialog and the page geometry recovered**; the pages come
out as pictures rather than on paper.

`CommandHandler` answers menu id `0xd5` (`1020:3288`) with the **Print
Map** dialog and then, on Print, the Windows print dialog (`PrintDlg`,
`1020:3395`) and a page loop on the printer's DC.

## The dialog

Resource 214, `Print Map`, 180 by 70 dialog units (`crate::dialog::
PRINT_MAP`); `PrintMapDlg` is at `1108:a1c2`:

| id | class | at | text |
|----|-------|----|------|
| `0x10c` | edit, centred | 10, 6, 24×12 | pages across |
| — | static | 36, 8, 20×12 | `by` |
| `0x10d` | edit, centred | 58, 6, 24×12 | pages down |
| — | static | 90, 8, 30×12 | `pages.` |
| `0x1` | button | 130, 6, 40×14 | `&Print` |
| `0x2` | button | 130, 24, 40×14 | `Cancel` |
| `0x76` | button | 130, 50, 40×14 | `&Help` |

- `WM_INITDIALOG` (`1108:a21e`): each box is limited to **one
  character** (`EM_LIMITTEXT 1`), set in `rghfontArial8[1]` (Arial 8
  bold), and filled with `vrgcPrintMapPage[i] + '0'` — the counts as last
  printed, `1` and `1` to start with (`1120:1694`). The window is placed by
  `StickyDlgPos` from `ptStickyPrintMapDlg`.
- `EN_UPDATE` on either box (`1108:a37e`): a first character outside
  `1`..`9` gets `MessageBeep` and the text set to everything after it —
  with a one-character box, nothing.
- **Print** (`1108:a2f2`): for each box in turn, a text that is not
  exactly one digit from 1 to 9 puts up *You must specify a number
  between 1 and 9* (`idsMustSpecifyNumberBetween19`, `0x527`), focuses
  the box and `break`s — out of the **loop**. `EndDialog(hwnd, 1)`
  follows regardless, so the frame prints with whatever the array held:
  a bad first box leaves both counts as they were, a bad second leaves
  only the first updated. Reproduced.
- **Help**: `WINHELP(HELP_CONTEXT, 0xc3c)` — *Printing a Map of the
  Universe* (`help.md`).

## The page loop

From `PrintDlg` the arm takes the printer's `HORZRES`, `VERTRES` and
`LOGPIXELSX/Y`, two fonts from `HfontPrinterCreate` — Arial at **8** and
**5** points for the printer's resolution, each with its line as
`tmHeight + tmExternalLeading` (`local_86`, `local_70`) — a margin of
`0x20` device pixels, and lays the whole print out once:

```
(across, down) = vrgcPrintMapPage
if (down < across) == (height < width): swap(across, down)
W = min(width  * across, 32000)
H = min(height * down,   32000)
if W < H:                       # taller than wide: text under the map
    W = min(W, H - dpi)
    square = W;  text = (32, W + 32);  legend = (W / 2, W + 32)
else:                           # wider than tall: text beside it
    H = min(H, W - dpi * 3 / 2)
    square = H;  text = (H + 32, 32);  legend = (H + 32, H / 2)
map = square - 64
```

The swap keeps the larger count along the page's longer side only in
the sense the comparison says: on a portrait printer page `1 by 2` and
`2 by 1` both come out two across. The map is a square, `square` on a
side from the print's origin, with the planets scaled into the `map`
inside its margins.

Each page `(i, j)` is the whole drawing offset by `(−width·i,
−height·j)`, clipped by the printer, under `STARTDOC "Stars! Universe
Map"`:

1. `Rectangle` round the square.
2. In the big font at `text`, a line apart: `Stars! Universe Map`
   (`0x520`), the game's name (`game.szName`), the player's name
   (`PszPlayerName(idPlayer, 1, 1, 1, 0)`), and `Year: %d` (`0x521`)
   with `turn + 2400`.
3. Unless the scanner is in the No Player Info view (`grbitScan & 0xf ==
   5`): the legend at `legend`, five lines (`0x522`..`0x526`: *= Your
   Planet*, *= Orbital Fort*, *= Starbase*, *= Unoccupied Planet*, *=
   Player #2's Planet*, each led by three spaces where the symbol goes);
   then, in the small font, `|` centred at `1.5` lines down less half a
   small line, `+` at `2.5`, and `2` at `4.5 lines + 8 − small line`; and
   `DrawPlanetPrintDot` — filled for the first line at `line/2 − 4`,
   plain at `3.5 lines − 4` and at `4.5 lines + 8`.
4. For every planet the player has a record of (`lpPlanets`), at
   `x = (X − 1000)·map/dGal + 32`, `y = (dGalInv − 1000 − Y)·map/dGal +
   32` — the y flipped as the scanner flips it: the player's own planets
   get `|` (an Orbital Fort: `fStarbase` with hull `0x20`) or `+` (any
   other base) centred at `y + 4 − small line` and the large dot;
   anybody else's the owner's number (`iPlayer + 1`) centred at `y −
   small line`.
5. For every planet of the universe (`rgptPlan`, `cPlanMax` of them):
   the small dot, and with the names overlay on (`grbitScan & 0x400`)
   the name centred fourteen below.

`DrawPlanetPrintDot` (`1038:7e44`) is three `PatBlt` strips for the
small dot (7 across) and four for the large (11 across), `BLACKNESS`.

## What this project does

There is no printer behind an egui window, and no print dialog. **Print**
lays the pages out with the arm's arithmetic on a US Letter sheet at 96
dots an inch (816 by 1056: `printmap::PAGE`) — `printmap::Layout` is the
block above, `Layout::plot` the planet mapping, `App::paint_print_page`
the page loop drawn with egui's painter — into a **preview** window that
shows one page at a time, stepped with Previous/Next, and offers **Save
pages…**: the desktop rasterises each page (`stars_ui::raster`) and
writes `<name>.bmp`, or `<name>-1.bmp`, `-2`… for several
(`stars_formats::resources::write_bmp`). The seven strings are the
game's own when a copy of the executable is at hand
(`resources::text`), this project's wording otherwise.

`tests/print_map.rs` covers the layout in both orientations and the
clamp, the plotting, the dialog's one-digit rule and the print-anyway
quirk, a page rendered to a picture, and the strings.

## What is not

* Paper: the Windows print dialog, `STARTDOC`/`NEWFRAME`, the printer's
  own resolution and fonts. The page is Letter at 96 dpi.
* `HfontPrinterCreate`'s face is the `[Fonts]` proportional face; the
  page uses egui's.
* The dialog's sticky position.
