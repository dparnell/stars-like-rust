# The Ship and Starbase Designer

Status: **layout, rules and wording recovered**; the parts and slots are
reimplemented, the pieces that need the game's bitmaps are named below.

`ShipBuilder` (`10c8:008c`) opens dialog resource `0x5c` and `SlotDlg`
(`10c8:0550`) runs it. The player reaches it with **F4** or the Commands (Ship
Design…) menu item. `MANUAL.PDF` chapter 9 describes the same screen from the
outside and is quoted below where it agrees.

It is one window with two faces, which the original carries in a single
variable, `mdBuild`:

* a **browser** over designs and hulls, with two radio groups, a dropdown and
  three buttons;
* an **editor** over one design, reached by **Copy** or **Edit**, with the
  parts list on one side and the hull's schematic on the other.

`ShowMainControls` (`10c8:0160`) is what swaps them: it hides the radios, the
dropdown and the three buttons, shows **OK**, and relabels the button beside it
from **Done** to **Cancel**.

## The controls

The dialog is **resource 92** (`0x5c`), `Ship & Starbase Designer`, 351 by 250
dialog units in MS Sans Serif 8pt with fifteen controls, placed at 11, 52
rather than centred:

| id | class | x, y | w x h | caption |
|----|-------|------|-------|---------|
| `0x810` | BUTTON | 14, 8 | 91x13 | `Ships` |
| `0x811` | BUTTON | 14, 21 | 91x13 | `Starbases` |
| `0x812` | BUTTON | 14, 48 | 91x13 | `Existing Designs` |
| `0x813` | BUTTON | 14, 62 | 91x13 | `Available Hull Types` |
| `0x814` | BUTTON | 14, 76 | 91x13 | `Enemy Hulls` |
| `0x815` | BUTTON | 14, 90 | 91x13 | `Components` |
| `0x816` | BUTTON | 13, 114 | 86x16 | `&Copy Selected Design` |
| `0x817` | BUTTON | 13, 134 | 86x16 | `&Delete Selected Design` |
| `0x818` | BUTTON | 13, 154 | 86x16 | `&Edit Selected Design` |
| `0x81a` | COMBOBOX | 180, 20 | 142x78 | the dropdown |
| `0x81b` | EDIT | 186, 52 | 132x15 | the name field, limited to 31 characters |
| `0x80c` | LISTBOX | 114, 90 | 135x170 | the parts list |
| `0x76` | BUTTON | 281, 134 | 33x13 | `&Help` |
| `1` | BUTTON | 281, 222 | 33x13 | `OK` |
| `2` | BUTTON | 281, 238 | 33x13 | `Cancel` |

The Design radios are **plural** — `Ships` and `Starbases` — and all three
buttons say `Selected Design`, which the manual's prose does not.
`fStarbaseMode = wParam - 0x810` and `mdBuild = wParam - 0x812`, so both groups
are read straight off the id.

Like the Production dialog's, this template has an oddity of its own: the parts
list is 170 units tall starting at 90, which is **ten units past the bottom** of
a dialog 250 tall. It is clipped to what is there.

`ShowMainControls` (`10c8:0160`) swaps the two faces by hiding or showing nine
of the fifteen — the two Design radios, the four View radios and the three
buttons — and then does two things that read backwards:

* **OK is hidden in the browser and shown in the editor**;
* the button beside it is relabelled `Done` for the browser and `Cancel` for
  the editor.

So the browser's only way out is the button the template calls `Cancel`, and
the dropdown, the name field and the parts list are not touched by the swap at
all.

The four **View** buttons, in order, are `Existing Designs`, `Available Hull
Types`, `Enemy Hulls` and `Components`, and each fills the dropdown differently
(`FillBuildDD`, `10c8:5e80`):

| View | the dropdown holds |
|------|--------------------|
| Existing Designs | the player's own designs, in slot order |
| Available Hull Types | every hull the player may build |
| Enemy Hulls | every design the player has seen an opponent fly, prefixed with that player's name |
| Components | nothing to do with designs: it becomes the **parts filter** |

Only **Existing Designs** enables Edit and Delete. **Copy** is enabled whenever
there is a free design slot — sixteen for ships and ten for starbases — which
is the manual's "Stars! will gray the Copy Selected Design button… once you
reach 16 designs" (p. 9-6).

## The schematic

The interesting discovery. The hull schematic is **not drawn by code**: every
hull carries its own layout, and the designer reads it.

`HULDEF.rgbrc[16]` (`+0x7F`) is one byte per slot — **low nibble the column,
high nibble the row** — on a grid of 32-pixel half-cells, and every slot is two
cells square. `UpdateSlotGlobals` (`10c8:6528`) turns that into rectangles:

```
left   = (rgbrc[i] & 0x0f) * 32 + xLeft
top    = (rgbrc[i] >> 4)   * 32 + yTop
right  = left + 64
bottom = top  + 64
```

and the origin is one of two, depending on which window is asking:

| window | `xLeft` | `yTop` |
|--------|---------|--------|
| the designer (`hwndSlotDlg != 0`) | `ptslotGlob.x - 0x14a` | 32 |
| the `grPopupShdef` pop-up | 12 | `dyArial8 + 12` |

The **plaque** — the `n of m` under the picture — is a fixed offset from
whichever origin was used, `+0x102` across and `+0x111` down. And the hull's
**cargo bay** comes out of `HULDEF.wrcCargo` on the same grid: its high byte is
the top-left cell and its low byte the bottom-right, each nibble a half-cell
like `rgbrc`.

`HULDEF.wrcCargo` (`+0x7D`) packs two more of those bytes: the high byte is the
cargo space's top-left cell and the low byte its bottom-right. `0xFFFF` means
the hull has no hold.

So the Small Freighter's `31 37 35` and `3355` read as a single row — engine at
column 1, the hold from column 3, armour at 5, the scanner at 7 — and the
Dreadnought's thirteen slots fan out over four rows. Every one of the 37 hulls
is recorded in `../vectors/hull-schematics.json` and checked against the
transcription by `crates/stars-core/tests/ship_design.rs`.

`DrawSlotDlg` (`10c8:2650`) fills them in:

| slot | picture | line under it |
|------|---------|---------------|
| empty engine slot | the category's bitmap | `needs %d` |
| empty, anything else | the category's bitmap | `up to %d` |
| filled | the component's bitmap | `%d of %d` |

with one exception: a starbase special in a slot that holds exactly one, with
exactly one fitted, gets **no** line at all.

The category's bitmap is a cell of bitmap 119 chosen by `IEmptyBmpFromGrhst`
(`10c8:6716`) — see `../formats/resources.md`, *The empty design slots*, for
the table and the four byte-swapped entries that leave an empty Elect, Mech,
Orbital-or-Elect or Mine-Elect-Mech slot wearing the *Combo* picture. The
component's is its `ibmp` cell of the seven component sheets. Both are drawn
here, scaled with the grid, and the line is printed over the picture's foot as
the original prints it (`bottom − dyArial6 − 4`).

The cargo space reads three lines: `Cargo` / `<n>kT` / `max` on a ship, and
`<n>kT` (or `Unlimited`) / `Space` / `Dock` on a starbase. Its figure is the
*design's* capacity, cargo pods included, not the bare hull's.

**A quirk kept on purpose.** The dock is drawn as a circle for the Space Dock
and the Death Star and as a rectangle for everything else — including the Space
Station and the Ultra Station, which also have docks. The binary really does
compare against those two ids and nothing else:

```
10c8:285a   cmp word es:[bx], 0x21     ; 33, Space Dock
10c8:285e   jnz  +3
10c8:2860   jmp  ellipse
10c8:2866   cmp word es:[bx], 0x24     ; 36, Death Star
10c8:286a   jz   +3
10c8:286c   jmp  rectangle
```

## The plaque

Under a schematic in **Existing Designs** the original blits a small plaque and
prints `%ld of %ld` on it: how many ships built to the design still exist, and
how many were ever built (`SHDEF.cExist` and `SHDEF.cBuilt`). The manual
explains it on p. 9-5.

This project does not keep either counter, so the first figure is counted off
the fleets in play and the second shows the same number rather than an invented
one.

## Dragging parts

`IDropPart` (`10c8:5476`) is the whole interaction, and it is short enough to
state completely.

**What a drag looks like.** `FTrackSlot` (`10c8:306a`) captures the mouse and
carries the component's 64-pixel picture, offset by where it was grabbed,
setting the cursor from `IDropPart`'s answer as it goes — the trash can where
letting go would take the part off, the no-way cursor where the slot would
refuse it. Here the picture rides **centred on the pointer** (the pointer is
what the drop is judged by, so the part stays visibly under it) and a slot
that would refuse the drop shows the not-allowed cursor.

**How many a drag carries.** Checked with `GetAsyncKeyState` at the moment of
the drop, not the pick-up — read here at the drop too
(`App::designer_drag_at_drop`):

| held | from the parts list | off a slot |
|------|---------------------|------------|
| nothing | one | **one** |
| Shift | four | four, or all of it if fewer |
| Ctrl | **a hundred** — more than any slot holds, so the slot fills | all of it |

which is the manual's "hold down the CTRL key to drag as many items as the slot
can hold, or the SHIFT key to drag four parts at a time" (p. 9-3).

**Dropping on a slot.** Accepted only when all three hold:

1. the slot is not already full;
2. it is empty, **or** holds exactly the same component — different components
   never stack, even in a slot that would take either;
3. if it is empty, the slot's category mask accepts the component at all.

The new count is `min(current + carried, capacity)`, and a drag off another
slot loses exactly what the target gained. Anything else is a `MessageBeep`.

One special case sits above all of that: **an engine slot is all or nothing**.
Dropping anything on it sets the carried count to a hundred, so it fills; and
dropping an engine back on the parts list takes the whole stack off whatever
the drag was carrying.

**Dropping anywhere else** removes the component, if the pointer is on the left
half of the window — which is where the parts list is while editing. Both are
done here: a stack let go over the parts list, or anywhere on the left half of
the dialog, comes off the design.

**One addition the original does not have:** with a slot selected, **Delete**
or **Backspace** empties it (`App::designer_clear_slot`), so a design can be
cleared without a drag. The original has no key for it — `SlotDlg` handles
no key messages, and `FTrackSlot` (`10c8:306a`) is reached only from the
mouse; its right button raises the component's pop-up, not a removal.

## What OK and Cancel do

**OK** writes the working copy (`shdefBuild`) back into its slot, clears the
design's ship counters, recomputes its cost and logs the change. It is refused
for a **ship** with nothing in slot 0:

> This ship design does not have any engines. You must add engines before the
> design will be accepted.

A starbase is not asked for an engine.

**Cancel** throws the working copy away, and if **Copy** had just created the
design, frees the slot again — `fHullCopy` is the flag that remembers.

## Copying

`MakeNewName` names the copy: `Long Range Scout` becomes `Long Range Scout (2)`
and that becomes `(3)`. Only the one digit is touched, so a ninth copy wraps to
`(0)` rather than reaching `(10)`, and a name already 28 characters long is
returned unchanged — which is how two designs can end up sharing a name.

Copying a **foreign** design strips out every component the player cannot build
(`FLookupPart` on each slot), which is the manual's "although the results may
not be perfect" (p. 9-2). Copying a foreign design whose *hull* is unbuildable
is refused outright:

> You can't copy this ship design because you can't build the hull it's based
> on.

## Deleting

Deleting frees the slot, destroys every ship built to the design and removes it
from every production queue. Nothing is recycled — the manual is emphatic
(p. 9-5) — so the original asks first, with wording that counts what will be
lost:

> You currently have %d %s%s and %d in production%s. If you delete this design,
> these ships will be destroyed and/or removed from the queues. Are you sure?

with shorter variants when only ships or only queued ships exist.

**Edit** is refused, rather than warned about, whenever ships exist or any are
queued: editing in place would silently change the shape of ships already
flying. Copy is the way round it.

## How a design reaches the host

Not through the state file. `LogChangeShDef` (`log.c`) writes an `rtLogShDef`
order into the `.xN` whenever a design is created, changed or deleted, and the
host replays it — the same route a waypoint order or a queue edit takes. The
header word is `mdChg:4, iPlr:4, ishdef:5, junk:3`, with **1** in the low
nibble when a design body follows and **0** for a bare delete.

`ishdef` there is the **whole** slot: `SHDEF.det` packs it as five bits and a
starbase design lives at 16..=25, so its top bit is set — and that same bit is
what the embedded record calls its "starbase" flag. See `../formats/orders-x.md`.

## The numbers panel

`DrawBuildSelHull` (`10c8:451e`) draws two columns under the schematic, under
the heading `Cost of one <name>` — with ` Hull` appended while browsing bare
hulls, so it reads `Cost of one Scout Hull`.

The left column is `Ironium` / `Boranium` / `Germanium`, each with `kT`, then
`Resources` without one, then `Mass: <n>kT` for a ship. The right column is
`Max Fuel:` in `mg`, `Armor:` and `Shields:` in `dp` (or `none`), and
`Rating:`. At 800×600 and above — the `mdScreenSize` test — it adds
`Cloak/Jam`, `Initiative/Moves` and `Scanner Range`; the manual notes the same
("some of the values shown in this figure are not displayed in resolutions
lower than 800x600", p. 9-3).

The cost shown is the **true** cost, not the table price: see
`../formulas/design.md` for miniaturisation and the two starbase adjustments.

## What is reproduced

The two faces and the swap between them, both radio groups and all four views,
the dropdown's four different fillings, the three buttons and exactly when each
is enabled, the sixteen and ten design limits, the schematic laid out from each
hull's own `rgbrc` table with the cargo box and its wording, the empty-slot
`needs`/`up to` labels and the `%d of %d` on a filled one, the plaque, the
whole of `IDropPart` including Ctrl and Shift and the engine special case, the
engineless-ship refusal, `MakeNewName`, the foreign-design strip, the delete
warning, and the cost and statistics panel with true costs.

## What is not

* **The pictures without the game's own sheets.** The slot pictures, the
  **hull picture** and the **parts list** are drawn from the game's own
  sheets when a copy of the original has been found — see
  `../formats/resources.md` — and fall back to a slot naming the categories
  it accepts, a box with the picture number in it, and plain rows, when it
  has not.

  The two arrows under the hull picture are real: every hull owns **four**
  pictures and `BuildDlg` walks those four. It splits the index into the hull's
  base and a variant, steps the variant with `(iCur + 4 ± 1) & 3`, and puts the
  base back, so the choice wraps inside the hull's own group and can never land
  on another hull's ship. A new design starts on the first of its hull's four —
  which is why a hull's `ibmp` is a *base*, not a picture.
* `Rating:` — `LComputePower`, which has not been read yet — and the
  `Cloak/Jam` and `Initiative/Moves` rows, which need `PctCloakFromHuldef`,
  `PctJammerFromHul` and `InitFromHuldef`.
* `SHDEF.cBuilt`, so the plaque's second figure repeats the first.
* The starbase **upgrade** credit: building a new starbase over an existing one
  costs less, part by part, and that is `GetProductionCosts` rather than the
  designer.
* The dialog's exact pixel geometry, in the **browser**. The original places
  its controls from the dialog resource and from `ptslotGlob`; this lays the
  same pieces out in the same arrangement without matching coordinates. The
  **editor** is laid out from `ptslotGlob` as the original's is:
  `ShipBuilder` is called with a client of 610 by 450 (`CommandHandler`,
  `1020:4e14`), and `SlotDlg`'s edit branch (`10c8:1f67`) moves the parts
  list to (16, 32) with the category filter at (16, 8) — the column the
  radios had, both 240 wide, the list 266 tall — and the name field to
  (610 − 264, 8); `DrawSlotDlg` draws the hull's picture at (610 − 338, 6)
  with the two arrows under it at (610 − 317, 75) and 14 to the right
  (`rgrcBuildSpin`), and `UpdateSlotGlobals` starts the slot grid at
  (610 − 330, 32) — the same origin, so each hull's `rgbrc` table keeps
  its slots clear of the picture — in 32-pixel half-cells; the numbers
  panel (`DrawBuildSelHull`) fills the left half of the client from
  `yBuildInfoSum` (340) down, under the list; OK, Cancel and Help sit
  along the foot at 610 − 226, − 148 and − 74, 68 wide. On the way out of
  the editor the list goes back to the right (610 − 256, 32) and is hidden
  unless the view is Components. The editor here takes a 610 by 450 client
  scaled with the width the window has and places every piece so; the
  browser still keeps the template's own proportions.
* Tutorial gating (`FTutorialEnabledShipBuilder`), the help file, and the
  sticky dialog position.
