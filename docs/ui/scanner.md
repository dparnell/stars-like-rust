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
| `Mine Fields Overlay` | the fields, as circles |
| `Fleet Paths Overlay` | each fleet's waypoints, leg by leg |
| `Ship Counts Overlay` | how many ships in each fleet |
| `Idle Fleets Filter` | show only the fleets with nothing to do |
| `Ship Design Filter`, `Enemy Ship Class Filter` | show only certain designs or classes |
| `Add Way Points Mode` | clicking the map adds a waypoint instead of selecting |

There is also a `Scanner Effective %` slider (`vpctRadarView`) and a
`Zoom Menu`.

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

Named here because they are the scanner's, and are not reproduced:
`FGetNextObjHere` (clicking the same spot again to cycle through everything on
it), and `GetScanFleetOrientation` (which way a fleet's arrow points).

## What this project does

`crates/stars-ui/src/views/galaxy.rs`.

Reproduced: the nine zoom steps with the original's shift arithmetic; the y
flip; all six views and their names; the names, scanner coverage, mine fields,
fleet paths, ship counts and idle-fleets overlays; click-to-select; and
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
