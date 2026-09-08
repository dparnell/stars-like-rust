# The scanner

Status: **controls and geometry recovered**; the map is drawn from them, the
original's artwork is not.

The largest window, and the one the game is actually played on: the galaxy, the
things in it, and the toolbar that decides how much of that you are looking at.

Source: `ScannerWndProc` (`1058:0032`), `DrawScanner` (`1058:108a`),
`PtToScan` (`1058:0efc`), `LogicalToScan` (`1058:744e`), `vrgpctZoom`
(`1068:0da4`).

## The geometry

**Zoom** has nine steps, held in `iScanZoom` as `-4..=4`. `vrgpctZoom` names
them for the menu:

| step | −4 | −3 | −2 | −1 | 0 | 1 | 2 | 3 | 4 |
|------|----|----|----|----|---|---|---|---|---|
| menu | 25% | 38% | 50% | 75% | 100% | 125% | 150% | 200% | 400% |

but `PtToScan` does not multiply by those numbers. It shifts:

```
-4  d >> 2        1  (d * 5) >> 2
-3  (d * 3) >> 3  2  (d * 3) >> 1
-2  d >> 1        3  d << 1
-1  (d * 3) >> 2  4  d << 2
```

So the menu's **38% is really three eighths**, 37.5%, and every step truncates
towards zero. The percentages are labels; the shifts are the geometry.

**The map is drawn upside down.** `LogicalToScan` is

```
x = PtToScan(x − xScanTop)
y = PtToScan((dGalInv − y) − yScanTop)
```

— the galaxy's y is mirrored about the universe's height before scaling, so a
planet stored near `y = 0` appears at the *bottom* of the scanner.
`xScanTop`/`yScanTop` are the scroll position.

## The toolbar

**Six views**, one at a time:

| | |
|---|---|
| `Normal View` | planets by who holds them |
| `Surface Mineral View` | what is on each planet's surface |
| `Mineral Concentration View` | what is in the ground |
| `Planet Value View` | how good each planet is for this race |
| `Population View` | how many people live there |
| `No Player Info View` | the map with everybody's colours taken off |

**Overlays and filters**, any number at once:

| | |
|---|---|
| `Planet Names Overlay` | names beside the dots |
| `Scanner Coverage Overlay` | how far each scanner sees |
| `Mine Fields Overlay` | the fields, as patterned circles |
| `Fleet Paths Overlay` | each fleet's waypoints, leg by leg |
| `Ship Counts Overlay` | how many ships in each fleet |
| `Idle Fleets Filter` | show only the fleets with nothing to do |
| `Ship Design Filter` | show only chosen designs, in this player's own fleets |
| `Enemy Ship Class Filter` | show only chosen classes, in everybody else's |
| `Add Way Points Mode` | clicking the map adds a waypoint instead of selecting |

There is also a `Scanner Effective %` slider (`vpctRadarView`) and a
`Zoom Menu`.

## Which way a fleet's arrow points

A fleet is drawn as one of **eight arrows** out of `hbmpScanShip` (id 88), a
16-by-72 sheet holding two columns of eight: nine pixels across in the
right-hand column and seven in the left, the smaller used once the scanner is
zoomed out past life size.

`GetScanFleetOrientation` (`1058:978c`) finds the direction, and **where it
looks depends on whose fleet it is**:

* **your own** — the next waypoint, and only when that leg has a warp set. A
  fleet with no orders, or one whose next leg is unset, has no course;
* **anybody else's** — the direction recorded with the sighting, which counts
  only when the `fdirValid` flag is set and the recorded warp is non-zero. A
  fleet seen at a distance has no waypoints to read, which is why this field
  exists at all.

`GetDxDyOrientation` (`1058:987c`) then turns the vector into an octant:
`(atan2(dy, dx) + π) × 4 / π + 0.5`, truncated, then `(9 - n & 7) & 7` to put
them in the sheet's order — anticlockwise from south-west:

| 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|
| SW | W | NW | N | NE | E | SE | S |

Two details are worth keeping. The routine uses **two different π constants** —
a seven-digit one added to the angle and a ten-digit one divided by — and both
are transcribed as they stand rather than folded into one, because that is what
the executable holds. And **a fleet going nowhere gets arrow 0**, which is the
same picture as one heading south-west: the index is set to zero before the
angle is looked at, and the original never tells the two apart.

The arrow is blitted **through a mask**, so its colour comes from the pen
rather than the bitmap; this project uploads the one-bit sheet as a stencil and
tints it, which is the same idea. Without the game's own sheet a fleet is the
small square this project drew before.

### The direction is stored biased, not signed

The two bytes a partial fleet record carries are **biased by `0x7f`** — the
value meant is `byte - 0x7f`. This project had them typed as two's-complement
`i8`, which is a different number for every byte from `0x80` up.

`io.c` copies them into `FLEET.dirFltX`/`dirFltY` untouched and
`GetScanFleetOrientation` reads them back as `(dirLong & 0xff) - 0x7f`, so the
code settles it on its own. The fixtures look at first as though they disagree —
`0x00` is far and away the commonest byte, 21% of all of them, which under a
bias would be a strong westward heading. It is not a heading at all: filter to
the fleets whose direction is actually valid and `0x00` drops out of the top
ten entirely. The spike is the **unset** field.

### The fleets themselves

`DrawScanner`'s fleet loop makes one of **three** decisions per fleet, and the
first of them is the one this project had wrong: a fleet **in orbit** — its
`idPlanet` is not `-1` — is **not drawn at all**. It contributes to
`rgWhatsHere[planet]` instead, and the planet's ring is its mark. Only a fleet
in **deep space** gets an arrow of its own, centred on its point (`pt - ptD/2`,
so the sprite is centred rather than hung off the corner).

A fleet sitting on the **selected point** is drawn a third way: an 11x11 glyph
out of `ScannerBmp` at `(0xb, 0x24)` for one of this player's and `(0xb, 0x2f)`
for anybody else's, in place of the arrow. The test is against `ptSelMain`, the
**point**, not against the selected fleet, so every fleet standing there gets
the glyph — and it is a deep-space branch, so a fleet in orbit at the selected
point still gets the larger ring instead.

The arrow's colour is set with `SetTextColor` before the blit and comes from
the same three the minefields use — `rgcrScanMine`, blue / yellow / red by
**relation**, not a colour per player. Two different enemies' fleets are the
same red. The selected-point glyph takes no colour: the two cells are already
coloured in the sheet.

**The ship filters hide arrows too.** The loop skips any fleet whose
`CShipsScanVis` count is zero, so the design and enemy-class filters narrow
what is drawn on the map as well as the rings and the counts — with one
exception: the **selected** fleet is drawn whatever the filters say, so
filtering cannot lose what the pane is showing.

The **ship count** is drawn above whichever mark was used, when the counts
overlay is on and the fleet is not marked `fNoCount`: `pt.y - ptD.y/2 - 2`
above an arrow, `pt.y - 7` above a selected-point glyph, and `pt.y - 5 - 2` or
`pt.y - 9 - 2` above an orbit ring depending on which of the two sizes it is.

### The ship counts

`DrawScanFleetCount` (`1058:47d2`) writes **one number per location**, not per
fleet — "the number of ships at a location", as `MANUAL.PDF` p. 5-15 puts it.
It is handed one fleet and walks the **circular list** `LinkFleets`
(`1038:1bb4`) builds out of the fleets sharing a point, adding up what
`CShipsScanVis` counts of each, and on the way round it sets `fNoCount`
(`wFlags & 0x800`) on every fleet in the ring so that none of the others writes
the same number again. `LinkFleets` clears that flag when it rebuilds the
rings.

Two limits: the total is **capped at 999**, and a spot totalling nothing is not
written at all — so the ship filters can empty a location of its number while
ships are still sitting there.

**The digits are a bitmap, not text.** They come from `hbmpNumbers` — bitmap
**249**, loaded as `LoadBitmap(hInst, 0xf9)` at start-up — a 44x7 one-bit sheet
of eleven 4x7 cells: the ten digits at `x = digit * 4`, and a star in the
eleventh that the counts never use. Each is blitted through a mask and then
painted, so the colour comes from the pen, exactly as the fleet arrows are
drawn.

**The layout is by hand**, five pixels apart, and the three cases do not share
a left edge:

| ships | digits at, relative to the location's x |
|---|---|
| 1–9 | `-1` |
| 10–99 | `-4`, `+1` |
| 100–999 | `-6`, `-1`, `+4` |

The top of every digit is **seven pixels above** the `y` the routine was
handed, and that `y` differs by the mark the fleet under it was given:
`pt.y - ptD.y/2 - 2` above a deep-space arrow, `pt.y - 7` above the glyph of a
fleet on the selected point, and `pt.y - 5 - 2` or `pt.y - 9 - 2` above an
orbit ring by which of the two sizes it is.

The last of those has a consequence worth knowing: the orbit call sits
**inside the ring arm**, which is guarded by `uVar8 < 3` (`uVar8` is
`grbitScan & 0xf`, the view). So in Planet Value, Population and No Player
Information a fleet in orbit writes no number at all, while one in deep space
at the same point still writes one — the numbers over the colonies simply
disappear in those three views.

### The planet names

The names are not drawn with the planets. They come in a **second pass** over
every planet, after the fleets, which also draws the base dots — the "thousand
dim points of light" — and it is that pass, not the first, which the **Planet
Names** bit (`grbitScan & 0x400`) turns on.

**Where.** `CtrTextOut(hdc, pt.x, pt.y + 5 + iVar21, name, 0)`, and
`CtrTextOut` (`1040:2534`) is `TextOut(hdc, x - width/2, y, …)` — so the name
is **centred on the planet** and its **top** sits five pixels under the
planet's own point. `iVar21` is **11 more** when `iScanZoom >= 3` *and* the
view is **Population** (`1058:2db1`): that is the one view with something of
its own drawn under the planet for the name to clear.

**In what.** A jump table on `iScanZoom` (`1058:2d63`) picks one of four font
globals:

| zoom | font |
|---|---|
| -1 | `rghfontArial6[0]` — Arial 6pt |
| 0, 1, 2 | `rghfontArial8[0]` — Arial 8pt |
| 3 | `rghfontArial8[1]` — Arial 8pt, **bold** |
| 4 | `rghfontArial10[1]` — Arial 10pt, **bold** |

`FCreateFonts` (`1000:0ab2`) builds each array by asking for face names from
the string table at `idsArial2 + i`, and the constant `0x0537` is there in our
own binary. It never sets `lfWeight`, so the weight is carried by the **face
name**: index 0 is `Arial` and index 1 `Arial Bold`. The point size becomes
pixels through `MulDiv(points, LOGPIXELSY, 72)` — four thirds of a pixel per
point at the 96 dpi the game ran on. egui has no bold family loaded, so this
project fakes the two bold sizes by writing the name twice half a pixel apart;
everything else is transcribed.

The background is left alone: `SetBkMode(hdc, TRANSPARENT)` before the loop,
so a name never carries a box.

**Which planets get one.** Names are drawn for planets **slightly off-screen**
as well. The pass skips a planet outside the visible rectangle — unless names
are on, in which case it carries on to the name with the dot suppressed, and
draws it as long as the planet is within **50 units left or right** and **20
above or below** the edges (`1058:2f6d` onwards). Names also stop entirely
below `iScanZoom > -2` (`1058:2f63`), where the dots are too close together for
a name to sit beside one. This project fits the whole galaxy in the window, so
the margin has nothing to do; the zoom cut-off is reproduced.

## Orbit rings

A planet with fleets in orbit is drawn with a ring round it, and the ring's
**colour says whose** they are. `DrawScanner` keeps a byte per planet and adds
**one** for a fleet of this player's and **two** for anybody else's, refusing to
add the same kind twice and stopping at three — so the three values are exactly
"mine", "theirs" and "both":

| | ring |
|---|---|
| only this player's fleets | grey/white |
| only other players' | red |
| some of each | magenta |

The ring is a blit out of the scanner's own sheet (`ScannerBmp`), which holds
the three colours at **11 pixels** and again at **19**, each with a mask below
them at y = 69. The larger one is used when the planet **is the selected
object** — `bVar34` is a comparison against `ptSelMain`, not a zoom test, which
is easy to assume and wrong.

**The rings belong to the first three views.** The whole arm is guarded by
`uVar8 < 3`, so Normal, Surface Minerals and Mineral Concentration draw rings
and Planet Value, Population and No Player Information do not — those three
redraw the planets themselves and would paint over them.

**The ship filters narrow the rings.** A fleet earns its planet a ring only if
`CShipsScanVis` (`1058:4bf4`) counts it, which is the same function the ship
counts go through — so the design and enemy-class filters apply here exactly as
they apply there. That is what `MANUAL.PDF` p. 5-15 means by "only those planets
orbited by the selected ships will have orbit rings". The **fleet paths**
overlay is gated on the same count, so the filters narrow that too.

## Clicking the same spot again

`ScannerWndProc`'s `WM_LBUTTONDOWN` arm works in an order that matters:

1. `FFindNearestObject` finds what was clicked;
2. `ChangeScanSel(&scan, 1)` **selects it**;
3. `if (!fSameSpot) return;` — a click on a new spot stops here;
4. only on the spot already selected does `FGetNextObjHere` step round.

So a click selects **what is under the pointer** — the planet when the pointer
is on the planet, whatever orbits it notwithstanding — and it is the *second*
click on the same spot that cycles.

> Getting that round the wrong way is what this project did at first: every
> click went through the cycle, so clicking a planet with fleets in orbit
> selected a fleet. Worse, the hit test ran off egui's `interact_pointer_pos`,
> which is `Some` on every frame the button is **held**, so holding the button
> down spun through everything on the spot several times a second. The hit test
> now runs only on the frame of a click.

The cycle is the planet, then each fleet in fleet order, then round to the
planet. Two restrictions make it narrower than it looks, and both come from the
same place:

* it is called with **`fOnlyOurs`**, so another player's fleets are not in the
  cycle;
* it returns to the planet only when `sel.pl.iPlayer == idPlayer`, so **another
  player's planet is not either** — where there is no planet of your own the
  cycle wraps straight back to the first fleet.

A spot can therefore be crowded and still cycle through nothing. Clicking one
of those things still *selects* it; it just does not take part in the walk.

Two smaller rules fall out of the same function. If the walk comes back to what
was already selected it reports no change rather than re-selecting it — so a
spot holding one thing stays put when clicked again. And selecting a fleet
keeps the planet it orbits, because `sel` holds both and the status bar names
the planet whichever is in front; the **pane** follows the selection, so
cycling swaps between the planet's tiles and the fleet's as it goes.

## The right-click menu

`WM_RBUTTONDOWN`, without the tape's modifier, puts up a popup of everything at
the point clicked and selects whatever is chosen from it.

The list is built in a fixed order: the **planet** at that point, a separator,
then **every fleet there — whoever owns it**. This is not the ours-only cycle
above: a right click is how you reach another player's fleet sitting on top of
your own. The separator is dropped when there are no fleets to put under it
(`if (c == 2 && planet) c = 1`), and the current selection is passed to
`PopupMenu` as `iChecked`, which ticks it.

After the fleets come the `THING`s at that point — minefields, packets and
wormholes, tagged `0x2000` — behind a separator of their own, which the original
writes only when something came before them. Choosing one selects it, and the
**Mine Survey pane** switches to its summary; see `mine-survey-pane.md`.

A space object never takes part in the click-again cycle: `FGetNextObjHere` is
reached only when what was clicked is a fleet or a planet. The right-click menu
is how you get to one, which is what it is for.

All four kinds are listed, the Mystery Trader included.

The menu is placed on the object rather than on the pointer, so it stays with
what it is about; Escape or a click elsewhere puts it away.

### The planets

`DrawScanner` draws the planets in **two loops**, and which is which matters.

The first walks `rgptPlan` — **every position in the universe**, not only what
this player knows — and puts a 3x3 dot from `ScannerBmp` at `(0xb, 0xf)` on
each. Everybody knows where the planets are; only what is on them is unknown,
so an unexplored system is still on the map. (The selected position gets the
11x11 starburst at `(0, 0x21)` here instead, in views 0 to 2.)

The second walks the planets this player knows about and puts the real mark on
top — `SCAN_LNormalScannerMode`, which the Normal view and the other views'
fallbacks share:

| what | cell | size |
|------|------|------|
| nobody owns it | `(0xb, 0x12)` | 3x3 |
| this player's | `(0xb, 0)` | 5x5 |
| a **friend's** | `(0xb, 10)` | 5x5 |
| anybody else's | `(0xb, 5)` | 5x5 |
| and selected | `(0, 0)`, `(0, 0x16)`, `(0, 0xb)`, or `(0, 0x21)` unowned | 11x11 over the mask at `(0, 0x45)` |

The sheet's own colours are green for this player, yellow for a friend and red
for the rest. Only a **friend** is set apart: the original reads the relations
table for `== 1`, so a neutral is drawn exactly like an enemy.

Three flags go on top of an owned planet, each a small filled square with a
black edge — 3x3 at the offsets below, and 5x5 a pixel further out when the
planet is the selected one:

| flag | where | colour |
|------|-------|--------|
| starbase | `pt + (3, -4)` | **blue**, or **yellow** when the design's hull is `0x20` — hull 32, the Orbital Fort, so a fort is yellow and anything built beyond one is blue |
| stargate | `pt + (-5, -4)` | green |
| mass driver | `pt + (-1, -6)` | purple |

Only the starbase flag is drawn here: this engine has no equivalent of
`IStargateFromLppl` or `IWarpMAFromLppl`, which look through a starbase's slots
for the two parts.

### Planet Value

View 3 draws **two concentric discs** rather than a dot: an outer ring and a
brighter core, sized and coloured by `PctPlanetDesirability` — and by
`PctPlanetOptValue`, what the planet would be worth **terraformed**, when the
plain value is negative.

| value | outer | inner | radius |
|-------|-------|-------|--------|
| `>= 0` | green | white | `value / 11 + 2`, capped at 10 |
| `>= 0` only after terraforming | dark yellow | yellow | as above |
| `< 0` | grey | red | `-value / 5 + 2`, capped at 10 |

The inner disc is `r - 2`, or `r - 1` when that would be under 3, and at least
1. A **Claim Adjuster** takes the terraformed value straight and is never shown
the yellow pair, because its planets are at their optimum every year. A planet
whose environment is not known has no value to show and gets nothing.

`PctPlanetOptValue` is the environment moved as far toward the race's ideal as
terraforming reaches, then measured — `stars_core::terraform::optimal_env` fed
to `ai::colonise::pct_planet_opt_value`, both of which this project already had.

Over an inhabited planet the view plants a **flag**: a pole 21 pixels tall with
a 7x6 banner, over a patch of background cleared with the black stock brush.
`MANUAL.PDF` p. 5-13 says "Blue flags mark your planets, yellow flags mark your
friends' planets and red flags mark planets of neutrals and enemies. Planets
without flags are uninhabited." The code is finer than the manual: a **neutral**
gets the radar brush and an **enemy** the red one, so the two are told apart on
the map. This follows the code.

### The mineral views

Views 1 and 2 draw a **three-bar histogram** beside each planet in the three
mineral colours, with an axis in the button-face colour — a corner, along the
bottom and up the left.

The offsets are `vrgScanPO`, five numbers for the ordinary zoom and five for
the small one: `{7, 12, 19, 4, 6}` and `{3, 10, 11, 2, 3}` — x offset, y
offset, axis length, bar width, bar spacing.

A surface bar is `(amount + max/40) / (max/20)` and a concentration bar is
`conc / 5`, both capped at 20 pixels and halved when the map is zoomed out
below life size. `max` is `cMinGrafMax` (`1120:04f8`), which ships at **5000**
— and the manual notes it is the *same* scale as the Summary pane's mineral
graph, so rescaling that rescales these bars too.

Surface minerals need a planet this player has **been to**; a concentration
needs only one that has been scanned.

### Population

View 4 draws a disc whose radius is a step up a nineteen-entry ladder, plus
two. The ladder is at **`1058:0000`** — the very start of the scanner's own
code segment, which is why the reconstruction shows the lookup with no base at
all — and holds, in the hundreds of colonists the files count in:

```
25  50  100  200  400  800  1000  1500  2250  3000
4000  5000  6000  7500  9000  11000  14000  18000  25000
```

So 2,500 colonists is radius 2 and 2,500,000 is radius 20. The colours are the
manual's: green for this player's planets, yellow for a friend's, red for
everybody else's, and a planet nobody lives on is "small and grey".

Another player's population is a **guess** in the original — the planet's
`uGuesses` field shifted left twice — which this engine does not keep, so their
planets are drawn at the smallest size rather than guessed at.

### No Player Information

View 5 skips the whole planet loop (`if (cPlanet != 0 && uVar8 != 5)`), so all
that is left is the base dot on every position: "just a thousand dim points of
light", as the manual puts it.

### Minefields

`grbitScan & 0x40` puts them up, and `DrawScanner` draws each as a circle whose
radius is the **square root of its mine count** — but not as a flat disc. It
walks the fields in three groups, and inside each group by kind, filling every
one with a **pattern brush**:

* **the fill** is `rghbrPat[kind]`, one of three 8x8 monochrome brushes
  (resources 460, 461 and 462), so the hatch says whether the field is
  standard, heavy or a speed bump. `SetBrushOrg` anchors it to the map's
  origin, so the dots hold still when the map moves rather than sliding with
  each circle;
* **the colour** is `rgcrScanMine[group]`, and `MANUAL.PDF` p. 5-14 names the
  three: **yours blue, a friend's yellow, and anybody else's red**. The map
  shares one colour between neutrals and enemies where the menu keeps them
  apart;
* a field **armed to detonate** is drawn **red** whoever owns it. That is the
  second pass the loop makes over kind 0 — and only kind 0, since a standard
  field is the only one that can be armed;
* the centre gets a small mark of its own, but only when **no planet is
  sitting on it** (`rgptPlan` is searched for the point first), since a planet
  would cover it anyway.

Not reproduced: `DrawRadarCircle`'s `fHollowOut`, which hollows each circle out
of the ones already drawn so a pattern brush cannot paint an overlap twice.
Drawing the same anchored pattern twice comes to the same thing here, because
the dots land in the same places either way.

### Packets, wormholes and the Trader on the map

`DrawScanner`'s own loop over `lpThings` draws the three of them, each a
different way, and none of it is a sprite chosen by this project:

**A mineral packet** is an outline the game draws itself, sized by the zoom —
`iScanZoom < 1` gives a half-width of 2, under 3 gives 3, and 5 above that. Its
shape says whether it is going anywhere: a packet whose **warp field is zero**
is a yellow **diamond**, drawn a pixel outside that half-width on each side,
and one under way is a **square**, in the ship colour or **red** when it is not
this player's.

**A wormhole** is a nine-pixel blit out of `ScannerBmp` at `(0, 0x5c)` with its
mask beside it at `(9, 0x5c)`, centred on the hole — the mask `AND`ed in
(`0x8800c6`) and then the image `OR`ed (`0xee0086`), which is the ordinary way
of putting a shaped glyph on a background. `Art::sprite_masked_at` does that
pair in one texture. A **line joins a pair**, drawn from the lower of the two
ids so it is drawn once, and only for a player who has **been through it**
(`grbitPlr`, this engine's `traversed_by`).

**The Mystery Trader** is not a glyph of its own at all: `DrawScanner` selects
`hbmpScanShip` — the fleets' own eight-way arrow sheet — tints it **yellow**
and orients it with `GetDxDyOrientation` along the way to its destination. So
it is a fleet arrow in another colour, and it goes through the same code here.

Planets and fleets outrank all three under the pointer, which is the order
`FFindNearestObject`'s mask implies. Without a copy of the original the
wormhole falls back to two rings, as everything else falls back when the
artwork is missing.

## Player colours

Two things on the map — a minefield's fill and a fleet's arrow — take their
colour not from a player but from `rgcrScanMine` (`1058:0026`, file offset
`0x58526`, which reads `00 00 ff 00 | ff ff 00 00 | ff 00 00 00`): three
COLORREFs, pure blue, pure yellow and pure red, keyed by **relation**. This
project keeps the keying and **lightens all three by the same amount**
(`SCAN_YOURS`, `SCAN_FRIEND`, `SCAN_OTHER`), which is the one deliberate
departure here: a pure blue arrow a few pixels across is hard to see against
the map's black on a modern display. The same three stand in for the
selected-point glyph and the orbit rings when no copy of the game is found and
the sprites are missing.

`grbitScan & 0x2000` is **Player Colors**, the **View menu's** own item
(`0x98d`, in submenu 1) and the one bit of `grbitScan` no toolbar button
touches. It starts **off**: the `0xe0` `stars.ini` defaults to does not include
it.

It colours exactly two things, and the handler says so itself — after toggling
the bit it redraws the scanner only when `grbitScan & 0x1400` is set, which is
planet names or ship counts.

**Planet names.** An owned planet's name takes its owner's colour out of
`rgcrPlrHistory` and this player's own is written as literal white rather than
looked up. An **unowned** planet's name is never coloured at all — the routine
only reaches for a colour once it has established the planet has an owner — so
it keeps the white the loop starts with. See [the planet names](#the-planet-names)
below for where and in what the name is actually written.

**Ship counts.** See [the ship counts](#the-ship-counts) below for how the
number is built and drawn. The colour is that player's only when every fleet at
the spot belongs to one player, and it comes out of `rgcrPlrHistory` — the
Score sheet's history colours, which is why the manual (p. 5-16) sends a player
there to find out whose is whose. Mixed spots, and your own, stay white: the
routine starts its walk with `iPlr = -1`, sets it to the first owner it counts
and back to `-2` the moment a second one turns up, and both `-2` and your own
number come out white.

## Giving orders by dragging

`Add Way Points Mode` turns the map from something you look at into something
you give orders with: a click appends a leg to the selected fleet
(`FAddWayPoint`, `1058:7504`), and a drag that starts on one of that fleet's
existing waypoints moves it instead (`FHandleWayPointDrag`, `1058:8176`,
finding the waypoint with `FNearAWayPoint`, `1058:8074`). Waypoint 0 is where
the fleet *is* and cannot be dragged.

**The client picks the warp for you**, and the rule is worth having in full.

`IFindIdealWarp` (`1050:a76e`) gives the fleet's cruising speed: the fastest
warp at which the engine burns **less than 121%** fuel, backed off to a **free**
warp (zero fuel) if one lies one, two or three steps below — the difference is
not worth the fuel — and capped at warp 9 for every engine but the five that can
hold warp 10 (Interspace-10, the Enigma Pulsar, Trans-Star 10, and the
Trans-Galactic Mizer and Galaxy scoops). A design with no engine answers 0, and
the fleet takes the lowest answer aboard.

`IWarpBestForWaypoint` (`1058:7a18`) then works from that. It will push the
speed *up* when the leg is going somewhere that is not the player's own planet
and the fuel is comfortable, and it walks it back *down* while the fuel does not
fit. Last of all comes the rule that decides most legs:

```
years = ceil(distance / warp²)
while warp > 2 and ceil(distance / (warp − 1)²) == years:
    warp -= 1
```

— **never fly faster than you need to arrive in the same year.** At a hundred
light years, warp 9 and warp 8 both arrive in two years and warp 7 does not, so
the answer is warp 8; at ninety-eight it is warp 7. A leg the fleet can take
through a stargate is warp 11, the pseudo-warp that means "use the gate".

## The measuring tape

A **right-drag** across the map stretches a rubber-band line, drawn in XOR so
it can be rubbed out again, and the status bar reports what is at the far end
and how far that is (`FHandleMeasuringTape`, `1058:9974`). Three details:

* the far end **snaps** to whatever is nearest — the original re-runs
  `FFindNearestObject` on every mouse move and moves the end onto what it
  finds;
* holding **Shift** widens the search mask from `0x4f` to `0x8f`, so the tape
  catches more;
* the original's gesture is a **middle**-drag or **Shift**-and-right-drag
  (`msg == WM_MBUTTONDOWN || (WM_RBUTTONDOWN && (wParam & MK_SHIFT))`), because
  a plain right-click is the menu above. Here a right-*drag* is the tape and a
  right-*click* is the menu, which keeps both gestures without a modifier;
* nothing is drawn until the pointer has moved **more than two units** from
  where it started, which stops a stray right-click leaving a mark.

### The status bar

`DrawScannerSBar` (`1058:62d8`) draws two rows along the bottom of the scanner,
each cell in its own sunken frame (`DrawLockLight`). The top row — only when the
window is wider than 359 pixels — carries the object's **id**, its **x** and its
**y**, then its **name**: a planet's, a fleet's, an object's, `Deep Space
Waypoint`, or the tape's own `Deep Space`. The bottom row carries the
**distance**, and when the caller has not supplied a second point it adds
`from <name>` — the selected object.

### How a distance is worded

`PszGetDistance` (`1038:3f00`) is small and worth transcribing exactly:

```
hundredths = (long)(distance * 100 + 0.5)      round to nearest hundredth
print "%ld.%ld l.y."   with  hundredths / 100  and  hundredths % 100
```

(The wide form is `%ld.%ld Light Years`; which one is used depends on the font
height, not the window.)

That format carries a **quirk that is the game's, and is kept here**: the
remainder is printed with `%ld` and so has **no leading zero**. Three and five
hundredths of a light year reads `3.5`, and twenty and two hundredths reads
`20.2`. The reconstruction drops both the `× 100` and the `+ 0.5`; the binary
has them, in two loaded constants at `DS:0x1cc2` and `DS:0x1cba`.

## Find

`FSelectSz` (`1058:945a`) takes what was typed and goes there. Its search order
is not the obvious one:

1. an **exact planet name** wins outright;
2. failing that, a **fleet** — by name, or by number;
3. failing that, the first planet whose name **starts with** what was typed.

So an exact fleet name beats a partial planet name, and every comparison ignores
case. Whatever is found is selected, and the map is scrolled to put it on screen
(`FEnsurePointOnScreen`).

The fleet half has its own small grammar. `Fleet` at the front is skipped, then
spaces, then a `#`, then spaces; what is left, if it begins with a digit **1 to
9**, is read as a number up to 512. So `Fleet #7`, `#7` and `7` all find the
same fleet, while a leading `0` is not a number at all.

Two details are easy to get wrong and both are kept here:

* the number typed is the number **shown**, which is the stored id **plus one**
  — see below — so the original subtracts one before looking the fleet up;
* the word `Fleet` is five letters but the original skips **six** characters,
  taking the following space with it. Type `Fleet7` with no space and the `7`
  goes too, and nothing is found.

## How a fleet is named

`PszGetFleetName` (`util.c`) — worth having, because this project had it wrong
in three panes before reading it:

```
"%s%s #%d"   owner prefix, class name, (id & 0x1ff) + 1
```

A fleet the player has renamed shows that name instead. Otherwise it is named
for its **primary design** — the one it has most of — with a **`+`** appended
when it carries more than one design, and the number the player sees is the
stored id **plus one**: a fleet stored as 0 is `Long Range Scout #1` on screen.
A fleet with no design at all falls back to the word `Fleet`, and somebody
else's fleet is prefixed with their race name.

## What else the window does

Everything the scanner does is now reproduced or recorded above.

## What this project does

`crates/stars-ui/src/views/galaxy.rs`.

Reproduced: the nine zoom steps with the original's shift arithmetic; the y
flip; all six views and their names; the names, scanner coverage, mine fields,
fleet paths, ship counts and idle-fleets overlays; the **orbit rings**, in the
game's own three colours and narrowed by the ship filters as the original
narrows them; the **fleet arrows**, in all eight directions and from both
sources; **Player Colors** and the two things it colours, with the counts
gathered per location as the original gathers them; click-to-select, and
**clicking the same spot again** to walk what is on it; and
**waypoint dragging** — adding a leg, moving one, dropping one, and the warp the
client suggests, both halves of it. Every edit writes the order record the real
client writes, so a host replaying the log reaches the same orders. And the
**measuring tape**, with its snapping, its Shift, its status bar and the
original's wording of a distance down to the missing leading zero. **Find**, in
the original's search order and with its fleet-number grammar, and fleet names
written the way `PszGetFleetName` writes them.

Not reproduced in the warp rule: the push *up* for a comfortable leg to
somebody else's planet, which needs the fuel model applied leg by leg; the
ram-scoop and stargate special cases; and the AI's own ceiling.

Not reproduced: the artwork — the original draws planets, fleets and objects as
bitmaps where this draws dots and marks, and the mineral views as small wedges
where this colours the dot by whichever mineral reads highest; scrolling with
`xScanTop`/`yScanTop` (the map is fitted to the panel and zoomed about its
centre); the design and enemy-class filters; the status bar's sunken cells and
its two-row layout, which is one line here; scrolling the map to what Find
found, since the map is always fully fitted; and cycling through objects at one
point. Find is a box on the toolbar rather than the original's modal dialog.
