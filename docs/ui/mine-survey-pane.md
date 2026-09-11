# The Selection Summary pane

Status: **layout, text and colours recovered**; the planet, fleet and
space-object summaries are reimplemented.

The third pane down the left of the frame, and the one that answers *"what is
that?"*. Whatever is selected in the scanner — a planet, a fleet, a minefield, a
packet, a wormhole, the Mystery Trader, or nothing at all — this pane summarises
it.

Source: `MineWndProc` (`1028:0000`), `DrawMineSurvey` (`1028:065a`),
`SetMineralTitleBar` (`1028:47dc`), `HtMineWindow` (`1028:37ac`).

## The title

`"<name> Summary"` — the planet's, the fleet's or the object's name with
`Summary` appended — or **`Deep Space`** when the selection is empty
(`idsDeepSpace`).

The title bar carries one control, and only in one case: a **minefield of the
player's own**, when the player is Space Demolition, gets a checkbox there for
whether the field is armed to detonate.

## Nothing is fixed but the shape

The pane's contents depend entirely on what is selected. There is no tile table
here as there is in the planet pane; `DrawMineSurvey` branches on
`sel.scan.grobj` and lays each case out itself.

## How the planet case is laid out

Four lines of text run across the top and everything left over is split into
**six equal rows** — three environment bars and three mineral bars:

```
dyRow = ((rc.bottom - rc.top) - dyArial8 * 4 - 2) / 6 + 1,  then forced even
```

The columns come from measuring, not from constants:

* the **label** column is the widest of the six labels plus six;
* the **value** column on the right is `idsN999mr` — the literal `999mR` —
  plus six, so a reading never moves the bars about;
* the bars fill what is between them.

If four label columns would not fit across the pane (`labelWidth * 4 >=
rc.right`) the pane switches to its **narrow** form, and every label switches
at once. The pairs are the string table's own:

| wide | narrow |
| --- | --- |
| `Value: ` | `Val: ` |
| `Population:  ` | `Pop:  ` |
| `Gravity`, `Temperature`, `Radiation` | `Grav`, `Temp`, `Rad` |
| `Ironium`, `Boranium`, `Germanium` | the **first four characters** of each |
| `Report is current` | `Current` |

The minerals have no short words of their own: the narrow pane simply draws
each label with `cLen = 4`.

## The colours

All of them are `HbrGet` calls in `FCreateStuff` (`1000:0014`), and a COLORREF
is `0x00bbggrr`, so they do not read the way they look:

| thing | brush | COLORREF | colour |
| --- | --- | --- | --- |
| gravity band | `rghbrPlanetAttr[0][0]` | `0x7f0000` | dark blue |
| gravity marker | `rghbrPlanetAttr[0][1]` | `0xff0000` | blue |
| temperature band | `rghbrPlanetAttr[1][0]` | `0x7f` | dark red |
| temperature marker | `rghbrPlanetAttr[1][1]` | `0xff` | red |
| radiation band | `rghbrPlanetAttr[2][0]` | `0x7f00` | dark green |
| radiation marker | `rghbrPlanetAttr[2][1]` | `0xff00` | green |
| ironium bar | `rghbrMinSum[0][0]` / `[0][1]` | `0xff0000` / `0x7f0000` | blue / dark blue |
| boranium bar | `rghbrMinSum[1][0]` / `[1][1]` | `0xff00` / `0x7f00` | green / dark green |
| germanium bar | `rghbrMinSum[2][0]` / `[2][1]` | `0xffff` / `0x7f7f` | yellow / dark yellow |
| mineral labels | `rgcrMin`, `DS:0x448` | `0xff0000`, `0x7f00`, `0xffff` | blue, **dark** green, yellow |

Note the last row: boranium's *label* is the dark green while its *bar* is the
bright one, so the two do not match.

## The bars themselves

An **environment** row draws the race's habitable band in the dark shade, a
**diamond** at the planet's own value in the bright one, and — when
`FCanTerraformLppl` says the planet can be moved — a one-pixel run from the
value to where terraforming would take it. The diamond is half as tall as a
quarter of the row and never smaller than two pixels:

```
x      = xBars + 2 + MulDiv(value, barWidth, 100)
half   = max(2, dyRow / 4 - 1)
```

A **mineral** row draws two bars over each other against a shared kiloton
scale. The **sum** — the surface stock plus what this year's mining will add —
goes down first in the dark shade, and the **surface stock alone** over it in
the bright one, so the tail that shows is the estimate. A bar that runs past
the end of the scale gets a `+` (`DS:0x507`). A separate marker sits at the
**concentration**, on its own percentage scale rather than the kiloton one
(`MulDiv(conc, barWidth - 11, 100)`).

The mining estimate is `EstMineralsMined`: the planet's own mines for a planet
the player holds, whatever remote miners of theirs are in orbit of one nobody
holds, and nothing at all for somebody else's.

## The mineral scale

Under the three mineral bars, with `kT` (`DS:0x504`) to the left of it. The
step is worked out by measuring the widest figure, seeing how many of those
plus half their own width again would fit, dividing the scale by that, and then
**rounding the step up to a round number** whose size depends on the scale:

| `cMinGrafMax` | rounded up to a multiple of |
| --- | --- |
| under 500 | 10 |
| under 1000 | 50 |
| under 2500 | 100 |
| under 7500 | 250 |
| under 15000 | 500 |
| 15000 and over | 1000 |

`cMinGrafMax` ships at 5000, so the shipped ladder rounds to 250, and the scale
carries `cMinGrafMax / step` ticks.

## A Claim Adjuster sees somebody else's band

The one surprise in the routine. Before it draws anything, it checks
`GetRaceStat(me, rsMajorAdv) == 3` — **Claim Adjuster** — and, if the selected
planet is held by a race this player knows, **copies that owner's nine
habitability bytes over its own** for the length of the drawing and puts them
back afterwards. So a CA player looking at an enemy world is shown the band
*that* race lives in, which is the band the planet would be terraformed
towards, rather than their own.

## A planet

An unscanned planet is drawn as the "unknown planet" bitmap and nothing else.
Otherwise:

| row | text |
|-----|------|
| value | `Value:` (`Val:` when the pane is narrow) and the desirability percentage; when there is room, the *optimum* value the race could terraform it to as well |
| population | `Population:  ` / `Pop:  ` — comma-grouped for a planet the player owns, an estimate for one merely scanned, and `???` for one never surveyed; `Uninhabited` when nobody holds it |
| owner | the owning race's plural name |
| age | `Report is current`, or `Report is %d year(s) old` |

The estimate is `"%c%ld00"` (`idsCLd00`) with `%c` the character **`0xb1`**, a
**±** — so an estimated population reads `±123400`, ungrouped, with the last
two digits supplied by the format rather than by the arithmetic. `???`
(`idsMsg1264`) is what the row falls back to when the estimate comes out at
zero or less.

The report-age line carries a small bug that is the game's. The wide form
builds `Report is %d year` (`idsReportDYear`), appends an `s` when the number
is more than one, and then ` old` (`idsOld`). The narrow form calls `wsprintf`
with `idsOld2` — which is the bare string ` old`, with **no format specifier**
— and passes the age anyway, so a stale report in a narrow pane reads just
` old` with no number in it.

Then the six bars the pane exists for, laid out as above. Each environment bar
is the variable's name, the reading through `PszCalcEnvVar` (`1.24`, `-12°C`,
`47mR`), the race's habitable band, and the diamond; each mineral bar is the
surface stock, the mining estimate behind it, and the concentration marker.

## A fleet

The fleet's picture and its owner's emblem, then:

| row | text |
|-----|------|
| ships | `Ship Count: %ld` |
| fuel | `Fuel:` with a gauge |
| cargo | `Cargo:` with a gauge |
| mass | `Fleet Mass: %ldkT` (`Mass: %ldkT` when narrow) |
| waypoint | `Next Waypoint: %s` (`WP: %s`), or `(none)` |
| task | `Waypoint Task: %s` (`Task: %s`) |
| speed | `Warp Speed: %d`, `Warp Speed: (stopped)`, or `Use Stargate` |
| sweeping | `This fleet can destroy up to %ld mines per year.` when it can |

The fuel and cargo gauges and the order rows are drawn only for a fleet the
player owns; somebody else's shows its ships, mass and — if known — its warp.

## A space object

| kind | rows |
|------|------|
| mineral packet | `Traveling at Warp %d`, `Destination:` and the planet, then the three minerals |
| minefield | `Location: (%d, %d)`, `Field Type: %s`, `Field Radius: %d l.y. (%ld mines)`, `Decay rate: %ld / year`, and for one's own field `Field: %d of %d` |
| wormhole | `Location:` `(%d, %d)`, `Destination:` — the far end, or `Unknown` — and `Stability:` |
| Mystery Trader | the notice asking for a fleet with at least 5,000kT aboard, until this player has traded, then `Trader is traveling at Warp %d.` |

### The plinth and the pictures

All four get the same plinth the fleet gets, at `prc->left + 6,
prc->top + 6`: a 64-pixel bitmap out of `hdibThings` blitted into a
0x42-pixel black square. Which bitmap is a straight index:

| index | picture |
| --- | --- |
| 0, 1, 2 | the three minefield kinds, in `MINEFIELD_KINDS` order |
| 3 | salvage |
| 4 | a mineral packet in flight |
| 5 | a wormhole |
| 6 | the Mystery Trader |

A minefield and a packet have an **owner**, so the second black square and the
32-pixel race emblem from `hdibRaces` follow, exactly as they do for a fleet. A
wormhole and the Mystery Trader belong to nobody: the routine takes a different
branch and draws the picture square alone.

### Where the text goes

A minefield, a packet and the Trader run plain lines down a column `0x28` past
the picture corner — so `prc->left + 6 + 0x28` — a line and two pixels apart.

A **wormhole** is different in three ways: its column starts `0x2f` past the
corner instead, its three rows are a **right-aligned label against a value**
rather than one string apiece, and they are a line and a **half** apart
(`dyArial8 * 3 / 2`) where everything else in the pane is a line and two
pixels. The label column is the widest label plus `0x14`, the labels end two
pixels short of it and the values start on it.

The packet's mineral list has that same two-column shape — `"%s: "`
(`idsS2`) right-aligned against the amount — but at the pane's ordinary row
spacing, and it starts a line and a half below the rows above it.

**Salvage is a packet with nowhere to go.** A packet whose target planet is
zero is salvage: it gets its own picture, and the original skips both the
`Traveling at Warp %d` and the `Destination: ` rows and draws the minerals
alone.

The Mystery Trader's notice is word-wrapped with `DrawText` across what is left
of the pane, and the warp line follows eight pixels below whatever height that
came out at.

**`Field Type:` names the kind alone.** `DrawMineSurvey` (`1028:1c8a`) indexes
the pointer table at `DS:0x4f2` into the literals `Standard`, `Heavy` and
`Speed Bump` at `DS:0x4d8`, so the row reads `Field Type:  Standard`. The same
table is what `PszGetThingName` puts in front of `Mine Field` when it names the
object, which is why the words are not part of the kind — see
`docs/ui/scanner.md`.

**The wormhole's stability is not the `iStable` field.** It is one of seven
words indexed by `PctWormholeMoves` — the chance the end jumps this year, which
`docs/formulas/wanderers.md` derives:

| chance | word |
|--------|------|
| 0 | Rock Solid |
| 1 | Stable |
| 2 | Mostly Stable |
| 3 | Average |
| 4 | Slightly Volatile |
| 5 | Volatile |
| 6 | Extremely Volatile |

So a wormhole that will not move at all this year reads *Rock Solid*, and one at
the six-per-cent cap reads *Extremely Volatile*. The player is being shown the
formula's answer, not the stored field.

### Reading an environment value

`PszCalcEnvVar` turns a click in `0..=100` into what the player sees. Two of the
three are straight lines — temperature is `clicks × 4 − 200` degrees, radiation
is the click itself in mR — and gravity is not:

```
d = |clicks − 50|
value = d < 26 ? d × 4 + 100 : (d − 25) × 24 + 200
if clicks < 50: value = 10000 / value        the bottom half is the reciprocal
print value / 100 "." value % 100
```

So the scale is **two straight pieces measured from the middle**, with the lower
half the reciprocal of the upper: 1.00 in the middle, 0.12 at one end and 8.00
at the other. Note that gravity prints **no unit** — the row's own label carries
it — where temperature and radiation print theirs.

## Rescaling the graph

The scale is a hit area of its own, `htMineScale`, and `MineClick`
(`1028:4020`) answers a click on it — **either button**, since it passes
`fRightBtn` — with a menu of nine:

```
100kT  500kT  1000kT  2500kT  5000kT  7500kT  10000kT  20000kT  30000kT
```

captioned `%dkT`, with a tick on the one in use. Choosing one sets
`cMinGrafMax`, redraws the pane, and redraws the **scanner** as well when
the surface-mineral view is the one showing (`grbitScan & 0xf == 1`) —
which is the manual's "rescaling that graph rescales the bars in this
view" (p. 5-13): one number serves both.

It is kept in `stars.ini` as `[Windows] MineralScale`, written every time.
The reader takes anything from **100 to 30000** — the two ends of that
ladder — and **replaces** anything outside with the shipped 5000 rather
than clamping to the nearer end. A value inside the range but not on the
ladder is kept and simply leaves nothing ticked, which is a thing only a
hand-edited file can arrange.

## What this project does

`crates/stars-ui/src/views/survey.rs`, over methods on `App` that build the rows
and bars so the text can be tested without drawing it.

Reproduced: the title, including `Deep Space`; the planet's value, population,
owner and the rows that follow, in **both** the wide and the narrow wording,
with the terraformed optimum beside the value where the original has room for
it and the `±` estimate for somebody else's planet; the **layout** — four lines
of text and then six equal, even-height rows, the label column measured from
the widest label and the value column from `999mR`, and the narrow switch when
four label columns would not fit; the environment bars in the game's own six
brushes, with the race's band, the diamond and the terraforming reach; the
mineral bars with the surface stock over the mining estimate, each label in
`rgcrMin`'s own colour, the `+` overflow marker and the concentration marker;
the **mineral scale** with its rounding ladder and its `kT`; and the **Claim
Adjuster** band swap.

The **fleet** half is reproduced too: the picture and emblem plinths and the
text column at `0x56`; the two **gauges**, framed and stacked and filled in
`rghbrMineral`'s five colours, with their `%ld of %ldmg` and `%ld of %ldkT`
labels centred on them and dropped when they will not fit; the mass, waypoint,
task and speed rows in both wordings, with the waypoint named by
`PszGetLocName` and `Use Stargate` at the pseudo-warp; the mine-sweeping line;
and the rule about **how much is shown** — the gauges and the order rows are
for a fleet the player commands, and somebody else's gets its ship count, its
mass and, only when it was scanned, its speed.

All four **space objects** are reproduced, now that the scanner's right-click
menu can select one (`scanner.md`): the minefield's four rows and its `Field:
%d of %d` for one's own, the packet's warp, destination and load, the
wormhole's location, far end and stability, and the Mystery Trader's notice and
speed — each with its plinth, its picture index and its owner's emblem square
where the original draws one, the wormhole's two-column shape and wider rows,
the packet's mineral table, and salvage treated as the separate thing it is.
Three of them are worth a note:

* the minefield's **decay rate** is what the field would lose this year, which
  counts the planets inside it and halves for a Space Demolition owner — it is
  `stars_core::minefield::decay_amount`, the same figure the turn applies;
* the wormhole's **stability** is the jump chance as a word, so the stored
  stability and the word run *opposite* ways round. A wormhole stored as `0`,
  the least settled kind, reads `Rock Solid` until it has sat still for ten
  years, while one stored as `3` is restless from the first and reads `Stable`;
* the **Mystery Trader**'s notice appears only until this player has traded
  with it, and the test is `1 << idPlayer & grbitPlr` — the same bit that stops
  them trading twice, which this engine keeps as
  `MysteryTrader::detected_by`. The speed line stays either way. What the
  notice asks for is in `../formulas/wanderers.md`; the wording here is this
  project's own, as the game's message text always is.

Not reproduced: the **bitmaps** themselves — `DrawFleetBitmap`, `hdibRaces`,
`hdibThings` and `hbmpUnknownPlanet` — so the plinths are drawn and left black;
the detonate checkbox; and the clicks the pane takes, which raise six more of
the `Popup` kinds (`MineClick`, `1028:3c0f` onwards). `grPopupFleet`, the one a
click on the ship picture raises, this project already has from the scanner's
status bar instead.

One gap is in the model rather than the pane: the original prints how old a
planet's report is from `PLANET.turn`, and this engine does not keep that stamp.
A planet the player owns is reported as current; for anything else the row is
left out rather than guessed at.
