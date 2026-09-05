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

## What else the window does

Named here because they are the scanner's, and are not reproduced:
`FAddWayPoint` and `FHandleWayPointDrag` (building a fleet's orders by dragging
on the map), `FHandleMeasuringTape` (measuring a distance), `FGetNextObjHere`
(clicking the same spot again to cycle through everything on it), `FindDlg`
(the Find dialog), `DrawScannerSBar` (the scale bar), `DrawLockLight`, and
`GetScanFleetOrientation` (which way a fleet's arrow points).

## What this project does

`crates/stars-ui/src/views/galaxy.rs`.

Reproduced: the nine zoom steps with the original's shift arithmetic; the y
flip; all six views and their names; the names, scanner coverage, mine fields,
fleet paths, ship counts and idle-fleets overlays; and click-to-select.

Not reproduced: the artwork — the original draws planets, fleets and objects as
bitmaps where this draws dots and marks, and the mineral views as small wedges
where this colours the dot by whichever mineral reads highest; scrolling with
`xScanTop`/`yScanTop` (the map is fitted to the panel and zoomed about its
centre); the design and enemy-class filters; the waypoint dragging; the
measuring tape; the Find dialog; the scale bar; and cycling through objects at
one point.
