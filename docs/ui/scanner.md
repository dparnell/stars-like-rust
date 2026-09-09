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
| `Add Way Points Mode` | clicking the map adds a waypoint instead of selecting — the same thing shift-click does |

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
overlay is on and the fleet is not marked done: `pt.y - ptD.y/2 - 2`
above an arrow, `pt.y - 7` above a selected-point glyph, and `pt.y - 5 - 2` or
`pt.y - 9 - 2` above an orbit ring depending on which of the two sizes it is.

### The ship counts

`DrawScanFleetCount` (`1058:47d2`) writes **one number per location**, not per
fleet — "the number of ships at a location", as `MANUAL.PDF` p. 5-15 puts it.
It is handed one fleet and walks the **circular list** `LinkFleets`
(`1038:1bb4`) builds out of the fleets sharing a point, adding up what
`CShipsScanVis` counts of each, and on the way round it sets the bit at
`wFlags & 0x800` on every fleet in the ring so that none of the others writes
the same number again; `LinkFleets` clears it when it rebuilds the rings. The
NB09 symbols name that bit **`fDone`** — the map borrows one of the turn
engine's own flags as its "already numbered here" mark.

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

### The fleet paths

`grbitScan & 0x80`, and a **pass of its own** over every fleet, run before the
planets are drawn — so the lines lie under the planet dots and the fleet marks
rather than over them.

The pen is `hpenStarbase`: solid, one pixel, `0x00ff0000`. **Blue for every
fleet**, whoever owns it — a path is not tinted the way an arrow or a name is.
(The pen's name is the cross-check: a starbase's own square on the map is blue
too. See [the colours](#the-colours).)
The line starts at **waypoint 0**, which is where the fleet is, and joins each
waypoint after it in turn.

A fleet is skipped unless all of this holds:

* the view is not **No Player Information** (`uVar8 != 5`), which draws no
  paths at all;
* it is not dead (`fDead`, bit 10 of the word at `FLEET+4`);
* the record's **detail is more than 6** — `(wFlags & 0xff) > 6`, and that low
  byte is the record's detail level, of which 7 means a full one. Only your own
  fleets are described that fully (see `docs/formats/fleet.md`), so **nobody
  else's path is ever drawn**, however much of their course you have worked
  out;
* it has **more than one** waypoint;
* and `CShipsScanVis` counts something of it, or it is the selected fleet — the
  two ship filters narrow the paths exactly as they narrow the arrows.

Not this: `DrawShipScanPath` (`1058:540c`) is a **different** overlay, drawn
for whatever the scanner has selected rather than for every fleet. It XORs a
scale line through the selection along its heading — length `warp² × 5`, with
ticks every tenth and an arrow head — to show a year's travel, and it is
toggled by `fOrdersVis` rather than by `grbitScan`. This project does not draw
it yet.

### What the selection draws for itself

`DrawShipScanPath` (`1058:540c`) is a **second** overlay, and not the same
thing as the fleet paths: it draws for whatever is **selected** rather than for
every fleet, and it draws with `SetROP2(hdc, 7)` — XOR — so that calling it
again rubs the same lines out. `fOrdersVis` remembers which way round it is,
and a drag hides the overlay and puts it back. A window redrawn from scratch
every frame has no use for either, so this project keeps the shapes and drops
the toggling.

It has three arms.

**The scale line.** When the scanner has selected something whose course is
known, a line is run through it along its heading, `warp² × 5` galaxy units
each way — the square of the warp is a year's travel, so the line reaches five
years back and five forward — and each year is marked: a **tick**, a
perpendicular about five pixels long, for each year behind, and an **arrow
head** of two five-pixel barbs at forty-five degrees for each year ahead. The
line scales with the map; the ticks and the barbs are fixed pixel sizes. Three
kinds of object have a course:

* a **fleet**, from the direction and warp stored with the sighting. Only a
  fleet described in part — somebody else's — carries those, so this is the
  answer to "where is that enemy going?" and a fleet of your own gets its
  waypoints instead;
* a **mineral packet**, heading for the planet it was flung at, at the stored
  warp plus four;
* the **Mystery Trader**, heading for its destination.

**The selected fleet's own path**, over the marks rather than under them, in
`hpenShip` green. A leg the fleet travels **twice** — the same two points
again later, in either direction, as a shuttle run has — is drawn **once** in
yellow (or in the stock white pen when the Ship Paths overlay is off) and the
repeat is left out. The original walks its `rgDup` table with the two loops an
index apart, which would colour a leg either side of the doubled one; the
reading that makes them agree, and that matches what the game draws, is that
the entry belongs to the leg **into** waypoint `i`.

### The waypoint markers

There are none — and that is the finding rather than an omission. Nothing in
the program draws a glyph, a box or a dot at a waypoint: the whole of
`DrawScanner`, `DrawShipScanPath` and `DrawScanXorLines` were read for it, and
the only thing any of them does at a waypoint is **take a hole out of the
line**.

`DrawShipScanPath` calls `ExcludeClipRect(pt.x - 5, pt.y - 5, pt.x + 6,
pt.y + 6)` at every waypoint before drawing the selected fleet's path, and
`DrawScanXorLines` (`1058:8af6`) excludes the same box at each corner of the
rubber band a drag pulls about. So the marker is the **11x11 gap centred on the
point**: the line stops short of each waypoint, which picks the waypoint out
and leaves whatever is there — a planet, a fleet — unobscured.

Two details follow from the box being **square**. A diagonal leg loses more of
its length to a hole than a straight one does, about seven pixels rather than
five, because the clip is a Chebyshev radius and not a distance along the line.
And a leg **shorter than the two holes at its ends** vanishes altogether, which
is what a leg between two waypoints eleven pixels apart does in the original
too.

The blue **Ship Paths** overlay, drawn for every fleet, has no holes: it is
drawn before the planets and never touches the clip. So a selected fleet whose
overlay is on shows a continuous blue line with the green one broken over it at
each waypoint.

**A selected planet's lines.** A dark purple one to the planet its starbase's
**mass driver** is aimed at, and a dark green one to the planet it **routes**
new fleets to. A planet with **both** shows both: the `goto` that draws the
purple line jumps into the route loop's body past the assignment that would
stop it, so the loop runs once more and draws the green one too. Each target is
stored **one-based**, zero meaning none, and the driver's line needs the planet
to actually have a starbase.

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

### Scanner coverage

`grbitScan & 0x20`, and the **first** thing `DrawScanner` paints after it
clears the map — so the discs lie under the minefields, the planets and the
fleets alike, which is what makes them read as ground rather than as rings
round things.

Each is a **filled ellipse**, brush and pen the same colour: `hbrRadar`, the
constant `0x0000007f` — a flat **dark red**. The overlapping discs are opaque,
so two coverages look exactly like one. (On the byte order, see
[the colours](#the-colours) below; this project had it as dark blue at first.)

It goes round twice. The **first pass** takes the normal range of

* every **planet** of this player's, from `GetPlanetScannerRange`
  (`1038:4c02`) — a planet whose `iScanner` is 31 has none, and an
  **Alternate Reality** race scans from its starbase by population instead;
* every **fleet** of this player's, from `GetFleetScannerRange` (`1038:4fb8`),
  and only when that range is positive. That routine keeps the **largest**
  range among the fleet's designs: the fourth-root combination applies between
  the scanners **within** one design, never across the designs in a fleet.

The **second pass** swaps the brush for `hbrRadarNear` — `0x00006060` on any
screen deeper than eight colours and `0x00007f7f` on one that is not, dark
yellow either way — and draws the penetrating ranges over the top:

* a **planet**'s is drawn as its **normal range halved**. The routine shifts
  the normal range right by one rather than using the penetrating range it was
  just handed; the two agree for every scanner in the game, since a
  penetrating scanner's deep range is half its normal one by construction. An
  AR planet penetrates only when its starbase hull is better than `0x22`, the
  Space Station;
* a **fleet**'s is the penetrating range itself;
* and for a **Packet Physics** race, each of this player's **mineral packets**
  under way adds a disc of its own, radius `(stored warp + 4)²`. The stored
  nibble is the warp less four, so that is the square of the real warp — which
  is how `MANUAL.PDF` p. 20-9 puts it: "the radius of the scan is equal to the
  square of the packet's warp speed". A packet whose stored warp is zero is not
  flying and scans nothing.

**The percentage.** Every radius passes through
`MulDiv(range, vpctRadarView, 100)` when `vpctRadarView` is under a hundred —
`MulDiv` rounds to nearest, not down. That is the combo in the toolbar: ten
entries from 100% down in tens, and a typed value clamped to `2..=100`. It
lives in `stars.ini` and defaults to 100.

Not reproduced, because it cannot be seen: `DrawRadarCircle` (`1058:4e7c`) is a
**batching** routine. It queues up to 250 circles, drops any that fall outside
the clipping rectangle, drops a new circle wholly inside one already queued
(and kills a queued one wholly inside the new), and when a circle covers all
four corners of the visible rectangle it draws that one immediately and stops
taking any more, since everything else would be under it. All of that is an
optimisation over opaque fills: the picture it produces is the picture you get
by drawing every circle.

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

## The colours

Every colour in this file is a Windows **COLORREF**, `0x00bbggrr`: the low byte
is red and the high one blue. The constants are created in `FCreateStuff`
(`1000:0014`), and reading them the other way round is an easy mistake — this
project made it twice and had to come back and fix the scanner coverage and the
fleet paths.

Two checks settle the order without any guessing. `hbrTooltip` is `0x9fffff`,
which under this reading is the pale yellow Windows paints tooltips with, and
under the other would be a pale blue nobody uses for one. And `rgcrScanMine` —
verified in our own binary at `1058:0026` — is `{0x00ff0000, 0x0000ffff,
0x000000ff}`, which the manual says is **yours blue, a friend's yellow, anybody
else's red**, in that order.

The community reconstruction's `init.c` writes these as `RGB()` calls with the
arguments **reversed** (`hpenStarbase = CreatePen(0, 1, RGB(0xff, 0, 0))` for a
constant that is blue). Its variable *names* are sound — `hbrBlue` really is
`0x7f0000` and `hpenYellow` really is `0x00ffff` — so name and raw constant
agree with each other and disagree with the `RGB()` transcription.

The ones this file uses:

| name | constant | colour |
|---|---|---|
| `hbrRadar` | `0x00007f` | dark red — scanner coverage |
| `hbrRadarNear` | `0x006060` | dark yellow — penetrating coverage |
| `hpenStarbase` | `0xff0000` | blue — the fleet paths |
| `hpenShip` | `0x00ff00` | green — the selected fleet's path |
| `hpenYellow` | `0x00ffff` | yellow — a leg travelled twice |
| `hpenDkGreen` | `0x007f00` | dark green — a planet's route line |
| `hpenDkPurple` | `0x7f007f` | purple — a mass driver's target line |

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

## Giving orders by clicking

`ScannerWndProc` (`1058:0ae1`) sends a left click to `FAddWayPoint`
(`1058:7504`) when the selection is a **fleet** and either of two things is
true:

```c
if (sel.grobj == grobjFleet)
    if ((wParam & MK_SHIFT) || (grbitScan & 0x10))
        FAddWayPoint(x, y, pscan);
```

— so **shift-click** and `Add Way Points Mode` are one code path, not two.
Shift is how you lay a course without leaving select mode, and the mode is how
you lay several without holding a key. Neither works with a planet selected;
you have to have a fleet in hand.

Everything else falls through to the branch below, which is where a drag that
starts on one of the selected fleet's existing waypoints moves it
(`FHandleWayPointDrag`, `1058:8176`, finding the waypoint with
`FNearAWayPoint`, `1058:8074`). Waypoint 0 is where the fleet *is* and cannot
be dragged.

### A waypoint lands on what it is near

A new waypoint is not simply dropped where the pointer was. The click handler
has already asked `FFindNearestObject` (`1038:4070`) what the nearest object
is — with no radius at all, so it always answers something — and
`FAddWayPoint` then measures the click against it:

```c
dx = ptIn.x - pscan->pt.x;  dy = ptIn.y - pscan->pt.y;
r  = ScanToPt(0x14);
if (r*r < dx*dx + dy*dy) {          /* too far: deep space */
    pscan->grobj = grobjOther;  pscan->idpl = -1;  pscan->ifl = -1;
    pscan->pt = ptIn;
}
```

`ScanToPt` (`1058:0fc2`) is the zoom applied backwards, so the reach is
**twenty screen pixels** whatever the zoom — about 20 light years at 100%, 5 at
400%, 80 at 25%. Within it the waypoint takes the object's **own** position;
outside it, the raw point, and the status bar says `Deep Space Waypoint`.

The snap is not decoration. The waypoint stores what it landed on — the id, and
the `grobj` class beside it, because a bare id cannot say whether it means
planet 7 or fleet 7 — and only a waypoint that names an object can be given a
task there. A leg that stops half a light year short of a planet cannot be told
to unload at it.

| lands on | class | id stored |
|----------|-------|-----------|
| a planet | 1 | `pscan->idpl` |
| a fleet | 2 | the fleet's own id |
| nothing, or one of this fleet's waypoints | 4 | the waypoint index |
| a `THING` — minefield, packet, wormhole, Trader | 8 | the thing's id |

`FFindNearestObject`'s search order settles the ties: planets first, then
fleets, then things, then the selected fleet's own waypoints. A fleet exactly
on a planet does not displace it, and among fleets at the same point **your
own** wins.

Two clicks are refused outright rather than adding a leg that goes nowhere: one
that resolves to the waypoint the fleet is already at, and one that resolves to
the *next* waypoint along. And a fleet may hold **87** orders including
waypoint 0; the 88th beeps and puts up an alert, whose text says 86 because it
counts the legs rather than the entries.

`FHandleWayPointDrag` snaps the same way while a waypoint is being dragged, but
with a wrinkle: it passes mask `0x4f` normally and `0x8f` while a key is held,
and `0x80` sets the reach to **zero**. The key turns snapping off rather than
widening it.

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

## The status bar

`DrawScannerSBar` (`1058:62d8`) draws **two rows** across the bottom of the
scanner's client area. The scanner keeps them out of the map: the drawing code
starts from `GetClientRect` and immediately sets `rc.top = rc.bottom - dySBar`,
and `dySBar` is

```
dySBar = (dyArial8 + 12) * 2                    FrameWndProc, 1020:0714
```

— two rows, each a line of Arial 8 plus twelve pixels. The bar's own font is
`rghfontArial8[1]`, Arial 8 **bold**.

### How it is drawn

A full redraw fills the strip with `hbrButtonFace` and then lays a one-pixel
`hbrButtonHilite` line down its **left** edge and along its **top**, which is
what lifts it off the map. Each cell is then sunk into that face by
`DrawLockLight` (`1058:6b00`):

```
shadow    1 px down the cell's left edge, 1 px along its top
highlight 1 px down its right edge, 1 px along its bottom
```

On a partial redraw `DrawLockLight` skips the frame and simply fills the cell
less two pixels with the face again, which is how the old text is rubbed out.

Every cell sits four pixels in from its row on all four sides, the next cell
starts four pixels after the one before, and the text goes at `left + 3`,
`top + 2`, clipped to the cell less two pixels.

### The cells

The top row's three left cells are drawn **only when the client area is wider
than 359 pixels** (`if (0x167 < rc.right)`). That is the case `MANUAL.PDF`
p. 5-16 means by "if the scanner is too narrow to display all the status bar
information": below it the name has the whole row.

| cell | width | contents |
| --- | --- | --- |
| id | `"ID #000"` + 6 | `ID #%d` with `idpl + 1` for a planet, `WP #%d` with `iwp` for a waypoint, **nothing** for a fleet or an object |
| x | `"X: 8888"` + 6 | `X: %d`, and only when the coordinate is positive |
| y | the same width again | `Y: %d`, likewise |
| name | the rest of the row | the object's name, or `Deep Space Waypoint` |

The two samples are literals in the data segment (`DS:0x5a2` and `DS:0x5b8`)
and the cells are sized from **them**, not from the text about to go in, so
they do not jump about; the y cell reuses the x cell's measurement.

The manual describes the same split from the outside — "ID#, coordinates and
name of planet", but for fleets, packets and wormholes only "coordinates and
name" — and the binary agrees, because only `grobjPlanet` and `grobjOther` ever
reach the id cell.

The name comes from `sel.scan.grobjFull` rather than `grobj` when the scan is a
waypoint, so a waypoint that has landed on a planet is named for the planet and
a bare one falls back to `idsDeepSpaceWaypoint`. And **a fleet in orbit shows
the planet**: `ChangeScanSel` sets `sel.scan.grobj = grobjPlanet` whenever
`grobjFull` has the planet bit, which is the manual's "when you select a fleet
orbiting a planet, only the planet's information is displayed".

### The bottom row

One cell the whole way across, holding a distance — but only when the bar has
two different points to measure between. It has them in two cases:

* the **measuring tape**, which hands `DrawScannerSBar` an `SBAR` whose `pscan`
  is the anchor's own scan (`FHandleMeasuringTape`, `1058:9b8b`);
* a **waypoint being dragged**, which leaves `pscan` null
  (`FHandleWayPointDrag`, `1058:8551`), and the point measured from is then
  `sel.pt` — the fleet.

A null `pscan` is exactly what adds the `from <name>` clause, so the tape says
`50.0 ly` and a waypoint drag says `100.0 light years from Long Range Scout #1`.
The name is `PszGetLocName` (`1038:3b08`): the planet's, the fleet's or the
object's, `Deep Space` when the point is `(-1, -1)`, and `Space (%d, %d)`
otherwise.

Otherwise `sel.pt` and `sel.scan.pt` are the same point and the row is blank.

### How a distance is worded

`PszGetDistance` (`1038:3f00`) is small and worth transcribing exactly:

```
hundredths = (long)(distance * 100 + 0.5)      round to nearest hundredth
print "%ld.%ld  l.y."  with  hundredths / 100  and  hundredths % 100
```

with the wide form `"%ld.%ld  Light Years"` chosen when `dyArial8 < 15`. Both
formats carry **two** spaces before the unit.

None of which the player ever sees. `PszGetDistance` has exactly one caller,
and that caller throws the unit away: the status bar walks to the **first
space** in the result and overwrites everything after it with `idsLy` (`ly`) or
`idsLightYears` (`light years`), chosen on the **window's width** — wider than
349 pixels gets the long one. So the double space is collapsed to one, the
abbreviation with the full stops never reaches the screen, and the font-height
branch inside `PszGetDistance` is dead.

What does survive is a **quirk that is the game's, and is kept here**: the
remainder is printed with `%ld` and so has **no leading zero**. Three and five
hundredths of a light year reads `3.5`, and twenty and two hundredths reads
`20.2`. The reconstruction drops both the `× 100` and the `+ 0.5`; the binary
has them, in two loaded constants at `DS:0x1cc2` and `DS:0x1cba`.

### Clicking it — the pop-up summary

A press in the **upper** row puts up a pop-up summary of what the bar is
naming (`ScannerWndProc`, `1058:043a`). It is not gated on the window's width,
though the manual (p. 5-16) only mentions it as the way round a scanner too
narrow to show the cells.

It is a **press-and-hold**, not a click. `Popup` (`10c0:0c7c`) creates a window
of the class `starspopup` and takes the mouse capture; `PopupWndProc`
(`10c0:0000`) destroys it on the next `WM_LBUTTONUP` or `WM_RBUTTONUP` and
clears `GlobalPD.grPopup`. The class is registered in `InitMDIApp`
(`1020:01ca`) with `CS_SAVEBITS | CS_NOCLOSE` and
`GetStockObject(WHITE_BRUSH)`, and the window style adds `WS_BORDER` — so it is
white with a one-pixel black frame. Its **bottom-right corner** goes at the
pointer (`x -= width; y -= height`), clamped to `SM_CXSCREEN` and
`SM_CYSCREEN`.

Which of the fifteen `grPopup` kinds appears here:

* `grPopupFleet` (3) when `sel.scan.grobj` is a fleet — which, since
  `ChangeScanSel` turns the scan into the planet whenever the point has one,
  means a fleet out in **open space**;
* `grPopupUnknownObj` (4) otherwise;
* and **nothing at all** unless `sel.scan.grobjFull` has the planet or the
  fleet bit, so a space object's press does nothing.

**`grPopupUnknownObj`** is four rows, `dyArial8` apart from `y = 4`: the labels
`Planet: `, `ID: `, `X: ` and `Y: ` right-aligned in bold, and the planet's
name, `idpl + 1`, `sel.scan.pt.x` and `sel.scan.pt.y` left-aligned from the
same x. The label column is the widest label plus **eight** when the window is
sized and plus **four** when it is drawn, so there are four pixels of slack on
the right; the value column is the widest of the values against `idsN9999`
(`9999`). Height is `dyArial8 * 4 + 8`.

**`grPopupFleet`** is a header row and one row per design slot the fleet holds
any of, walked 0 to 15 — so the rows are in design order, not stack order.
`Ship Name` sits at `x = 4` in bold, `#` is right-aligned at
`right - 4 - dxDamage`, and the design names and counts line up under them.
Width is `names + counts + 16 + dxDamage`, height `dyArial8 + 8` plus a line
per row. A fleet holding nothing says `None` and stops.

`dxDamage` is the damage column, and it appears only when two things line up:
`POPUPDATA.fRedDamage` is on **and** the sizing pass found a damaged stack.
When it does, a `Damage` header joins the row, a one-pixel `BLACKNESS` rule is
drawn under the header, and each damaged design's row goes **red** and gains

```
"%d@%d%%"   ships = pctSh * count / 100 , at least 1
            pct   = w / 640             , at least 1
```

where `w` is the fleet record's packed damage word, `pctSh = w & 0x7f` and
`pctDp = (w >> 7) & 0x1ff`. Dividing the **whole word** by 640 is the game's
own shortcut for `pctDp / 5`, and it is exact for every value either field can
hold — `pctDp` is in 500ths, so a fifth of it is the percentage.

There is a loose end in the original worth writing down. `fRedDamage` and the
hull-type filter beside it live in the same union as the fleet pointer, and the
**scanner sets neither** — only the Selection Summary's own ship tile does
(`MineClick`, `1028:3e7b`, which sets `fRedDamage` for a fleet seen in full
detail and the filter to `0xff`, meaning no filter). So from the status bar
they carry whatever the last pop-up left there. This reproduces the cold-start
reading: no damage column and no filter.

`DecorateHullName` (`10c0:…`) is what names a row. For your own designs it is
just the design's name; for **another player's** it appends a Roman numeral in
brackets when several of their designs share a name, numbered by slot order.
That cannot arise here, because a player's file holds only that player's own
designs.

### How a space object is named

`PszGetThingName` (`1038:26de`), which the name cell, the right-click menu and
the Selection Summary's title all go through:

| kind | format |
| --- | --- |
| minefield | `"%s%s Mine Field"` (`idsSSMineField`) — owner, then `Standard`, `Heavy` or `Speed Bump` |
| packet | `"%sMineral Packet"` (`idsSmineralPacket`), or `Salvage ` (`idsSalvage`) when it is aimed at no planet |
| wormhole | `Wormhole` |
| trader | `Mystery Trader` |
| anything else | `Mystery Object` |

The three minefield kinds are literals at `DS:0x4d8` reached through the
pointer table at `DS:0x4f2`, and the **same table** feeds the Mine Survey pane's
`Field Type:  %s` (`DrawMineSurvey`, `1028:1c8a`) — so the kind is the adjective
alone and the words `Mine Field` are not part of it. The owner prefix is
`"%s "` (`DS:0x518`) and is left off your own objects, exactly as a fleet's name
leaves it off.

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

`crates/stars-ui/src/views/galaxy.rs`, with the status bar in
`crates/stars-ui/src/statusbar.rs` and `crates/stars-ui/src/views/statusbar.rs`
and the pop-up summary in `crates/stars-ui/src/popup.rs` and
`crates/stars-ui/src/views/popup.rs`.

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
**measuring tape**, with its snapping, its Shift and its two-unit threshold.

The **status bar** is reproduced whole: the strip taken out of the map rather
than drawn over it, the two rows, the sunken cells and their fixed widths, the
three cells that appear only past 359 pixels, `ID #` on a planet and `WP #` on a
waypoint and nothing on a fleet, the planet that a fleet in orbit is named for,
the distance from the tape and from a waypoint drag with the `from <name>`
clause on exactly one of them, and the unit chosen by the window's width — down
to the missing leading zero in the figure. Space objects are named the way
`PszGetThingName` names them, owner prefix and minefield kind and all. And the
**pop-up summary** a press in its upper row raises: press-and-hold, white with
a one-pixel frame, its bottom-right corner at the pointer and clamped to the
screen, the planet's four labelled rows or the fleet's ship list, the damage
column and its `%d@%d%%` where the original would draw one, and nothing at all
for a space object.

**Find**, in the original's search order and with its fleet-number grammar, and
fleet names written the way `PszGetFleetName` writes them.

Not reproduced in the warp rule: the push *up* for a comfortable leg to
somebody else's planet, which needs the fuel model applied leg by leg; the
ram-scoop and stargate special cases; and the AI's own ceiling.

Not reproduced: the artwork — the original draws planets, fleets and objects as
bitmaps where this draws dots and marks, and the mineral views as small wedges
where this colours the dot by whichever mineral reads highest; scrolling with
`xScanTop`/`yScanTop` (the map is fitted to the panel and zoomed about its
centre); the design and enemy-class filters; the thirteen other `grPopup`
kinds, which belong to the panes that raise them; scrolling the map to what
Find found, since the map is always fully fitted; and cycling through objects
at one point. Find is a box on the toolbar rather than the original's modal
dialog.
