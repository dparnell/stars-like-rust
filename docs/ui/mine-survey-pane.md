# The mine survey pane

Status: **layout and text recovered**; the planet, fleet and space-object
summaries are
reimplemented, the space objects are specified but not drawn.

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

## A planet

An unscanned planet is drawn as the "unknown planet" bitmap and nothing else.
Otherwise:

| row | text |
|-----|------|
| value | `Value:` (`Val:` when the pane is narrow) and the desirability percentage; when there is room, the *optimum* value the race could terraform it to as well |
| population | `Population:` / `Pop:` — comma-grouped for a planet the player owns, an estimate (`%c%ld00`) for one merely scanned, and `???` for one never surveyed; `Uninhabited` when nobody holds it |
| owner | the owning race's plural name |
| age | `Report is current`, or `Report is %d year(s) old` |

Then the six bars the pane exists for.

**Three environment bars** — gravity, temperature, radiation. Each is the
variable's name, the reading through `PszCalcEnvVar` (`1.24`, `-12°C`, `47mR`),
a bar carrying the **race's habitable band**, a diamond at the planet's own
value, and a hatched extension showing how far terraforming could move it
(`FCanTerraformLppl`).

**Three mineral bars** — ironium, boranium, germanium, each named in its own
colour. The bar shows what is on the surface against a shared scale, with the
**concentration** behind it, and a `+` when the amount runs past the end of the
scale. The scale's ticks are chosen to round numbers from `cMinGrafMax`. For a
planet the player owns, `EstMineralsMined` adds what this year's mining will
bring; for an unowned one, what the player's remote miners in orbit would.

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

## What this project does

`crates/stars-ui/src/views/survey.rs`, over methods on `App` that build the rows
and bars so the text can be tested without drawing it.

Reproduced: the title, including `Deep Space`; the planet's value, population,
owner and the rows that follow; the environment bars with the race's habitable
band and the planet's marker; the mineral bars with surface stock against
concentration; and the fleet's ships, mass, cargo, waypoint, task and speed.

All four **space objects** are reproduced, now that the scanner's right-click
menu can select one (`scanner.md`): the minefield's four rows and its `Field:
%d of %d` for one's own, the packet's warp, destination and load, the
wormhole's location, far end and stability, and the Mystery Trader's notice and
speed. Three of them are worth a note:

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

Not reproduced: the planet and fleet pictures and race emblems; the terraforming
extension on the environment bars; the mineral scale's tick labels and the
mining estimate added to the bars; the fuel and cargo *gauges* (the figures are
given as text); and the detonate checkbox.

One gap is in the model rather than the pane: the original prints how old a
planet's report is from `PLANET.turn`, and this engine does not keep that stamp.
A planet the player owns is reported as current; for anything else the row is
left out rather than guessed at.
