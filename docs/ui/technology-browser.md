# The Technology Browser

Status: **structure, walk and panel recovered and reimplemented**; the
component prose is written afresh rather than copied, for the reason below.

`BrowserDlg` (`10d8:1ed8`), reached with **F2** or Help (Technology Browser),
with `DisplayComponentInfo` (`10d8:2ac6`) painting the panel. `MANUAL.PDF`
pp. 8-2..8-3 describes it.

It is **modeless** — it sets `hwndBrowser` and ticks its menu item, and stays
open alongside whatever else is on screen — and it shows **one component at a
time** rather than a list.

## The controls

Dialog resource **128**, `Technology Browser`, 349 by 247 dialog units — and
that is the whole of its size; nothing resizes it afterwards. Five controls:

| id | class | x, y | w x h | caption |
|----|-------|------|-------|---------|
| `0x42e` | BUTTON | 7, 7 | 50x14 | `<- Prev` |
| `0x10b` | COMBOBOX | 68, 7 | 83x12 | the category dropdown |
| `0x42f` | BUTTON | 158, 7 | 50x14 | `Next ->` |
| `0x10a` | BUTTON | 6, 232 | 130x11 | `Show Only Available Technology` |
| `0x2` | BUTTON | 157, 230 | 50x14 | `Close` |

Neither Prev nor Next carries an accelerator, which is unusual for this
program and is what the resource says.

Everything between the top row and the foot is a **child window** of the class
`starsbrowser` (`DS:0x21c`) that `DisplayComponentInfo` paints. It is not in
the template: `BrowserDlg` creates it (`10d8:21ce`), and it is sized from the
**font** rather than from any string —

```
x      = 6
y      = dyArial8 * 3 / 2 + 12
width  = 0x158, and 0x28 wider again when dyArial8 > 14
height = dyArial8 * 12 + 0x48 + 6 + a global the frame layout carries
```

— so the large-font layout gets a panel forty pixels wider, and nothing about
either dimension depends on the category names.

The dropdown is filled from consecutive string ids **1087 to 1103**: `All`,
then the sixteen kinds alphabetically — `Armor`, `Beam Weapons`, `Bombs`,
`Electrical`, `Engines`, `Mechanical`, `Mine Layers`, `Mining Robots`,
`Orbital`, `Planetary`, `Scanners`, `Shields`, `Ship Hulls`,
`Starbase Hulls`, `Terraforming`, `Torpedoes`. The dialog indexes a parallel
table of category flags with the same counter; that table is where the
decompiler loses the symbol, and a byte search of the data segment did not
find it, so the mapping here is taken from the names — which are unambiguous —
and the order from the string ids.

The browser opens on the first **armour**, which is what `BrowserDlg` sets when
it has nothing remembered (`vpartBrowser.grhst = hstArmor`).

## Walking

Prev and Next step through the items of a category and roll over into the next
or previous one, all the way round. What they stop at depends on the checkbox:

* **checked** — only what `FLookupPart` says the player can build *now*;
* **unchecked** — anything the player is allowed to build eventually, which is
  what lets the browser show a component and explain what is missing. A
  component a **racial trait** forbids is still shown; one the **Mystery
  Trader** has not handed over is not, because the player has no way of knowing
  it exists.

That last distinction is the one worth care: the loop skips a `-1` from
`FLookupPart` only when `FShouldPartBeHidden` agrees, and that function is
about the Trader alone.

## The panel

For the component in view, all of it relative to the player looking — the
manual's "the Technology Browser always displays cost and other information
relative to your race type and current level of knowledge" (p. 8-3):

* its name and category, with `UnAvail` beside it when it cannot be built;
* `Cost:` — the **true** cost, miniaturised against the player's own
  technology, not the table price;
* `Mass:` in kT, left out for a planetary installation, which is never carried;
* `Tech Req:` — the fields it needs and the levels, each drawn **red** when the
  player is short of it and ordinary when they are not (p. 8-3);
* the figures that only make sense for its kind — a beam's range, power and
  initiative; a scanner's range and its penetrating range; a bomb's kill
  percentage and buildings destroyed; a hull's armour, initiative, capacities
  and slot count; an engine's free warp; and the rating in the six tables that
  share a `Special` shape, labelled for what it means there;
* and whatever stands between the player and building it.

### The notes are written, not copied

The original carries about **a hundred and fifty** sentences for that last
part — "This mass driver requires the primary racial trait of 'Packet
Physics'.", "Stargates are not available if the primary race trait is
'Hyper-Expansion'." — one per component per condition.

This project does not copy them, for the same reason it does not copy the
game's message text: they are authored prose rather than data. Component
*names* and the tables they sit in are transcribed wholesale, because those are
data; sentences are not.

Instead the notes are generated from `crate::parts::requirements`, which is
`FLookupPart`'s gate restated as a list of named conditions —
`Prt(Ss)`, `NotLrt(OBRM)`, `Trader(ENGINE)` and so on. The browser turns the
ones a player fails into a sentence of our own. Two things follow: the wording
is ours, and it **cannot drift from the rule**, because the rule is the same
list the designer's parts list is filtered by.

## What is reproduced

The window's own **geometry** — the template's five controls where the resource
puts them, and the panel child sized from the font as `BrowserDlg` sizes it.
The modeless window, the seventeen-entry dropdown and its order, the walk with
its wrap and its two filtering modes including the Mystery Trader distinction,
the panel's costs, mass, per-kind figures, and the technology requirements
marked met or not against the player's own levels — plus a note for every
condition the player fails, and the component's own picture.

## What is not

* Nothing of the panel: the component's **picture** is drawn too, out of the
  game's own sheets when a copy of the original has been found —
  `docs/formats/resources.md`.
* The original's **prose**, as above.
* The **hover help** the Research dialog's benefits list shares with this
  panel, which puts up a component's details from `rghsFutureTech`.
