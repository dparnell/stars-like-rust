# Subsystem: Scanner Ranges

- **Status:** verified — combination rule, planetary and ship ranges, what the passes reveal and the detail levels; the space-object marks transcribed from the decompiled passes
- **Ghidra routine(s):** `1038:4c02` `GetPlanetScannerRange`, `1038:50d0` `GetShdefScannerRange`, `1038:4fb8` `GetFleetScannerRange`, `1008:60be` `LookupBestPlanetaryScanner`
- **Manual reference:** `MANUAL.PDF` p. 17-2 ("Scanners are Additive"), p. 20-13 (No Advanced Scanners)
- **Uses RNG:** no
- **Implemented in:** `crates/stars-core/src/scanning.rs`

Stars! tracks two ranges per scanner. The **normal** range finds fleets in deep
space; the **penetrating** range additionally sees through planets, revealing
orbiting fleets and sub-surface mineral concentrations.

## The combination rule

Multiple scanners do not simply add. The manual states it directly:

> The formula for calculating a ship's scanner range is the 4th root of the sum
> of each scanner to the 4th power. Let's say you have a ship design with two
> 100 light year scanners and one 60 light year scanner:
> `(100^4 + 100^4 + 60^4) ^ ¼ = 120 light years`

```
range = (sum of range_i^4) ^ (1/4)
```

The same rule combines penetrating ranges, separately from normal ranges.

## Planetary scanners

```
if race is Alternate Reality:
    range = sqrt(population * 10)             # AR planets scan from the starbase
    if NoAdvancedScanners: range = range * 1412 / 1000; penetrating = 0
    else: a sufficiently advanced starbase penetrates at half range
else:
    if planet has no scanner: range = 0
    else:
        raw = best researched planetary scanner's stored range
        penetrating = (-raw) / 2 if raw < 0 else 0
        range = abs(raw)
        if NoAdvancedScanners: range *= 2; penetrating = 0
```

A **negative** stored range is how the parts table marks a penetrating scanner:
its magnitude is the normal range and half its magnitude is the penetrating
range.

The `1412/1000` factor in the Alternate Reality branch is the fourth root of 4
(1.4142…), i.e. the "doubled range" of No Advanced Scanners expressed under the
fourth-power law — doubling a range means quadrupling its fourth power, so
scaling a combined range by `2^(1/2)` is the consistent way to express it.

## What a scanner reveals (`SetVisPFPlanets` `1070:abde`, `SetVisPFFleets` `1070:a100`)

The host writes each player's turn file from two passes, one over the
player's planets and one over their fleets, and between them they are the
fog of war. Transcribed as `stars_core::visibility`, and checked in
`crates/stars-core/tests/visibility.rs`:

* a **fleet** of another player is seen within a scanner's **normal** range,
  but a fleet **in orbit** only within its **penetrating** range — both cut
  by the fleet's cloak, below;
* a **planet** is learned only within **penetrating** range — the loop over
  planets sits inside `if (penetrating > 0)` in both passes — or by a fleet
  of the player's standing at it. The Scoper 150, which does not penetrate,
  reveals no planet at all;
* what has once been learned stays learned: the client keeps the planet in
  its history file. The **client** also writes itself a "you have found a
  planet" message for every record flagged first-year it reads
  (`file.c`; ids `0xaa`…`0xae` and `0x15d`, by whether the planet is
  occupied, habitable, terraformable, hostile or known only from afar).

A ship's penetrating range is not stored with the scanner part
(`GetShdefScannerRange`, `1038:50d0`): it comes from the part's ability
class — 50, 100 and 200 for classes 1, 2 and 3 (Ferret, Dolphin, Elephant)
— with the Chameleon at 45, the Robber Baron at 120 and the Pick Pocket at
none. A **Jack of All Trades** race's Scout, Destroyer and Frigate hulls
carry a scanner of their own, `20 × Electronics` normal and
`10 × Electronics` penetrating, summed with the fitted ones by fourth powers
— and in the **tutorial** (`fTutorial`) a fixed 40 and 20 instead, the
doubles at `1120:1cd2` and `1120:1cda`.

The tutorial's files pin all of this: its Armed Probe (a Rhino, no
penetration of its own, plus the built-in 20) learns Hiho at seventeen light
years in 2403 and not at forty-three the year before, none of the scouts
learn their first planets from twenty-odd light years out in 2401, and
`tutorial.h1` in 2403 knows exactly home, 90210, Prune, Alexander and Hiho.

### How much is seen (`det`)

Each planet and fleet on the map is written at a **detail level**
(`enums.h`: `detMinimal` 1, `detObscure` 2, `detSome` 3, `detMore` 4,
`detAll` 7), which `MarkPlanet` (`1070:8adc`) and `MarkFleet` (`1070:885e`)
raise but never lower. Transcribed as `stars_core::visibility::Detail`:

| Level | Planet | Fleet |
|---|---|---|
| Minimal (1) | one of the player's fleets sits at it with no scanner aboard (`SetVisPFInit`, `1070:9654`): id, owner, starbase — nothing else | — |
| Obscure (2) | within penetrating range, but its cloaked starbase keeps it out of the cut-down reach (`1070:ab3c`); written as 3 with `fInclude` clear | — |
| Some (3) | scanned: environment, concentrations, the owner's population and defence guesses | scanned: ships, heading, warp, mass |
| More (4) | a Robber Baron in orbit (`iSteal & 2`), or an unowned planet the player is remote-mining from a fleet that stayed all year: the surface minerals too | a Pick Pocket at the same spot (`iSteal & 1`): the minerals aboard too |
| Full (7) | the player's own | the player's own |

A fleet of somebody else's **at one of the player's planets** is seen in
some detail whatever the planet's scanner (`1070:a100`). An **Interstellar
Traveler** sees, in some detail, every planet with a stargate within the
range of each of their own gates — all of them from an unlimited gate —
cut by the target starbase's cloak like a penetrating scan
(`SetVisPFPlanets`' second pass, `1070:abde`).

### The space objects

The same passes settle which minefields, packets and wormholes a player
sees, and leave marks on the objects that the host file keeps:

* a **mineral packet** within a scanner's normal range; a **Packet
  Physics** race sees every packet in flight (`SetVisPFInit`, `1070:9654`);
* a **Mystery Trader** always (`1070:9654`);
* a **wormhole** end within the normal range once seen before
  (`THWORM.grbitPlr`), else within the penetrating range; seeing it sets
  the bit, which a jump clears;
* a **minefield** within the normal range once detected before
  (`THMINE.grbitPlr`), within the penetrating range regardless, and always
  from inside it (the squared distance to its centre no more than its
  mine count). A planet's scanner considers only the fields within its
  normal range (`1070:abde`), a fleet's every field (`1070:a100`). Seeing
  a field sets `grbitPlr`, for good, and `grbitPlrNow`, which
  `UnmarkMineFields` (`10b8:7638`) clears at the start of every turn; the
  player's file carries every field with `grbitPlr` set, and its owner
  is known to them while `grbitPlrNow` is;
* a **Space Demolition** race's own fields scan to their radius, normal
  and penetrating alike (`SetVisPFThings`, `1070:b9ee`).

The turn engine runs the passes for every player at the year's end
(`turn::detect_things`) so the marks are made whether or not a file is
written. Tests: `crates/stars-core/tests/player_files.rs`.

## Cloaking

`PctCloakFromHuldef` (`1048:88c0`), `PctCloakFromLpfl` (`1080:2d5e`),
`CPtsCloakFromLphs` (`1080:3170`); `MANUAL.PDF` ch. 24. Implemented as
`ShipDesign::cloak_points`, `ShipDesign::cloak_pct`, `Fleet::cloak_pct` and
`design::cloak_pct_of_points`, applied in `stars_core::visibility`.

**Points.** Each part that cloaks gives so many points per copy, and a
design's points are the sum over its slots:

| part | points | | part | points |
|------|-------:|-|------|-------:|
| Transport Cloaking | 300 | | Chameleon Scanner | 40 |
| Stealth Cloak | 70 | | Shadow Shield | 70 |
| Super-Stealth Cloak | 140 | | Langston Shell | 20 |
| Ultra-Stealth Cloak | 540 | | Depleted Neutronium | 50 |
| Multi Function Pod | 20 | | Mega Poly Shell | 40 |
| Enigma Pulsar | 20 | | Multi Contained Munition | 20 |
| Alien Miner | 60 | | Orbital Adjuster | 50 |

The cloaks give their table ability; the rest are fixed in the routine. A
**Super Stealth** race adds 300 to every ship and starbase; an **Improved
Starbases** race adds 40 to a starbase (`1048:88ec`).

**Percent.** Points per kiloton become a percentage by the table the manual
prints on p. 24-3 and the binary has from `1048:8a75`: half the points up to
100 (50%), then an eighth of the rest to 300 (75%), a twenty-fourth to 612
(88%), a sixty-fourth to 1,124 (96%), 97 short of 1,612 and 98 beyond.
Nothing, or more than 25,000 (the routine's overflow guard), is 0.

**A fleet's cloak** is its designs' points weighted by the mass of their
ships and spread over the whole fleet's mass, **cargo included** unless the
race is Super Stealth — so cargo dilutes a cloak and an uncloaked ship in the
fleet counts as cargo, as the manual says. Ship by ship the figure is the
design's alone: a Stealth Cloak on an empty ship is 70 points, 35%.

**What it does.** `SetVisPFFleets` (`1070:a1d0`): a fleet is seen when

```
d² ≤ range² × (100 − cloak) / 100 × (100 − cloak) / 100
```

the two divisions in that order, where `cloak` is the fleet's percentage
cut to the scanning fleet's Tachyon Detectors' share: `95%` to the power of
the square root of the detectors fitted, tabulated at `1038:50be` — 100,
95, 93, 91, 90, 89, 88, … 81 for none to seventeen (the best design in the
fleet counts; a planet's scanner has none). A **planet** with a cloaked
starbase is learned only within `range² × (100 − cloak)² / 10,000`
(`1070:ab3c`), the squared figure being cached on the starbase design at the
top of every turn (`10b0:120d`).

So the tutorial's Berserkers, a Super Stealth race, are 75% cloaked with no
cloak fitted at all: the Armed Probe's fifty-four light years reach them at
thirteen, and Stove Top's Scoper 150 at thirty-seven.
`tests/visibility.rs` checks a Stealth Cloak at 35%, the range it cuts, the
dilution by cargo, a detector's five per cent, and the manual's table.

## Edge cases & clamps

- No Advanced Scanners doubles conventional ranges and removes penetration
  entirely (`MANUAL.PDF` p. 20-13).
- Some racial traits turn other objects into scanners (`SetVisPFThings`,
  `1070:b9ee`): a **Packet Physics** race's packets in flight scan,
  penetrating, to the **square of their warp** — fleets (cloak counted, no
  detectors), planets (starbase cloak counted) and space objects alike; a
  **Space Demolition** race's minefields show every fleet **loose** inside
  them — not one in orbit — a cloaked one on a `Random(100) >= cloak` roll,
  the one place the visibility pass draws on the generator. Both are in
  `stars_core::visibility` (`tests/visibility.rs`); the roll is seeded from
  the game seed, the year and the player so a frontend recomputing the view
  sees the same answer all year, where the host rolls the game's own once.

## Worked example (becomes a test vector)

Two 100 ly scanners and one 60 ly scanner:

- `100^4 + 100^4 + 60^4 = 100000000 + 100000000 + 12960000 = 212960000`
- `212960000 ^ 0.25 = 120.85…`, truncated to `120` ly

Captured at: `../vectors/planetary-economy.json` (`scanning`).

## Open questions

- `GetShdefScannerRange` is the routine that walks a design's slots and applies
  the fourth-power law; it is a stub in the reconstructed NB09 sources, so the
  per-design path must be read from our binary directly when ship designs land
  in Step 5. The combination rule itself is confirmed by the manual and is
  implemented.
- ~~The stored ranges of individual scanner parts come from the components
  table, which is not yet decoded.~~ **Resolved in Step 4:** the scanner and
  planetary tables are transcribed in `components.md`, and
  `scanning::planet_scanner_range_for_tech` looks the best planetary scanner up
  from a player's technology levels.
