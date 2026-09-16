# Dump to Text File — File ▸ Dump to Text File ▸ …

Status: **the three dumps recovered**; a few columns are approximated
where this engine keeps less than the original (noted below).

The File menu's submenu has three items, each one `CommandHandler` arm
and one routine in segment `1108`:

| id | item | routine | file |
|----|------|---------|------|
| `0x55` | `&Universe Definition` | `DumpUniverse` (`1108:851e`) | `<base>.map` |
| `0x54` | `&Planet Information` | `DumpPlanets` (`1108:86e4`) | `<base>.pla`, or `<base>.p<N>` |
| `0x53` | `&Fleet Information` | `DumpFleets` (`1108:9530`) | `<base>.fle`, or `<base>.f<N>` |

Each refuses without a game and a player (`game.lid == 0 || idPlayer ==
-1`), opens the file with `StreamOpen` (`szBase` with the extension,
formats `%s.map` at `1120:1633`, `%s.pla`/`%s.p%d` at `165c`/`1655`,
`%s.fle`/`%s.f%d` at `1682`), streams a header and a row per object with
`RgToStream`, and puts up a box — `idsUniverseDefinitionHasSuccessfully
WrittenSMap` (`0x4d6`) and its five siblings — saying where it went or
that it could not. Rows are tab-separated and end in `szCRLF`.

`gd.fPerPlayerDumps` — bit 5 of the settings word at `gd + 4` — is a
switch this project has not found a dialog for. Set, the planet and
fleet dumps take a **third header string** and a third block of columns,
and are named by player (`.p1`, `.f1` for player one). `App::
per_player_dumps` carries it, off by default.

## The headers

String-table entries with `*` for each tab (`CchGetString`, then every
`*` becomes `\t`):

- planets: `0x4dc` *Planet Name, Owner, Starbase Type, Report Age,
  Population, Value, Production Queue, Mines, Factories, Def %, S Iron, S
  Bora, S Germ*; `0x4dd` *Iron MR, Bora MR, Germ MR, Iron MC, Bora MC,
  Germ MC, Resources*; `0x4de` (per-player) *Grav, Temp, Rad, GravOrig,
  TempOrig, RadOrig, Terra, Cap, Scan, Pen, Driver, Warp, Route,
  GateRange, GateMass, PctDmg*.
- fleets: `0x4df` *Fleet Name, X, Y, Planet, Destination, Battle Plan*;
  `0x4e0` *Ship Cnt, Iron, Bora, Germ, Col, Fuel*; `0x4e1` (per-player)
  *Owner, ETA, Warp, Mass, Cloak, Scan, Pen, Task, Mining, Sweep, Laying,
  Terra, Unarmed, Scout, Warship, Utility, Bomber*.

The universe's header is a plain data-segment string, `#\tX\tY\tName`
(`1120:163a`), and its rows `%d\t%d\t%d\t%s` (`1647`): the planet's
number from one, its x and y, its name — every planet of `cPlanMax`,
from `rgptPlan` and `rgidPlan`.

## The planet rows

One per planet in the player's list (`lpPlanets`, `cPlanet` of them —
everything the player has a record of), with the columns filled in as
the record's `det` allows: `< 3` a bare sighting, `3`..`6` scanned, `7`
the player's own.

| column | when | what |
|--------|------|------|
| Planet Name | always | `PszGetCompressedPlanet` |
| Owner | owned | `PszPlayerName(owner, 1, 0, 0, 0)` |
| Starbase Type | owned, `fStarbase` | the base's design |
| Report Age | always | `game.turn − PLANET.turn` |
| Population | `det == 7` | `pop × 100`; else, owned and scanned, the estimate `uPopGuess × 400` |
| Value | `det ≥ 3` | `PctPlanetDesirability`, `%d%%` |
| Production Queue | `det == 7` | the queue's first line as `FillPlanetProdLB` words it |
| Mines, Factories, Def % | `det == 7` | `%ld\t%ld\t%d.%d%%`, the coverage from `CalcPctSurvive` as `1 − pct`; else three blanks — and, per-player, an estimated `\t%d.%d%%` **appended as a fourth field** when the scan gave one (`uPopGuess >> 12`) |
| S Iron/Bora/Germ | `det ≥ 3` | the surface minerals |
| Iron/Bora/Germ MR | `det ≥ 4` | `EstMineralsMined` |
| Iron/Bora/Germ MC | `det ≥ 3` | the concentrations |
| Resources | `det == 7` | `CResourcesAtPlanet` |
| Grav, Temp, Rad; …Orig | per-player, `det ≥ 3` | `PszCalcEnvVar` of the environment and the original environment; seven blanks below `det 3` |
| Terra | per-player, `det ≥ 3` | `PctPlanetOptValue` with `%` |
| Cap | per-player, `det == 7` | `PctPlanetCapacity` |
| Scan, Pen | per-player, `det == 7` | `GetPlanetScannerRange`'s two ranges |
| Driver, Warp | per-player, `det == 7` | the mass driver's target planet and `((word >> 10) & 0xf) + 4`, or blank and `0` |
| Route | per-player, `det == 7` | the route's planet, or blank |
| GateRange, GateMass | per-player, `det == 7` | the base's stargate part, `+0x36` and `+0x34`; `\t0\t0` without one |
| PctDmg | per-player, `det == 7` | `STARBASE.pctDamage >> 4`, `0` without a base |

## The fleet rows

One per fleet in `rglpfl` — every fleet on the player's map — with the
name from `PszGetFleetName` called with **`idPlayer` set to −1**, so the
owner's race leads every name, the player's own included.

| column | when | what |
|--------|------|------|
| X, Y | always | the fleet's position |
| Planet | orbiting | its name |
| Destination | `det == 7` | `PszGetDestName` — the next waypoint's place; per-player, another player's fleet with a known heading gets `dx.dy` |
| Battle Plan | the plan exists | its name |
| Ship Cnt | always | the sum of the sixteen stacks |
| Iron, Bora, Germ, Col, Fuel | always | the five holds |
| Owner | per-player | `iPlayer + 1` |
| ETA | per-player | `PszGetETA` to the next waypoint; `0` with none |
| Warp | per-player | the next leg's; a scanned fleet's seen speed; else `0` |
| Mass | per-player | `WtFromLpfl` |
| Cloak | per-player | `PctCloakFromLpfl` |
| Scan, Pen | per-player | `GetFleetScannerRange`, `-1` printed as `0` |
| Task | per-player, `det == 7` | `PszGetTaskName` |
| Mining, Sweep, Laying, Terra | per-player | `CMineFromLpfl`, `CMineSweepFromLpfl`, `CLayMinesFromLpfl(−1, −1)`, `PctTerraFromLpfl` |
| Unarmed, Scout, Warship, Utility, Bomber | per-player | ships by hull class (`huldef.init >> 10 & 0xf`): everything outside 2..5, then 2, 3, 4, 5 |

## What this project does

`App::dump_universe`, `dump_planets` and `dump_fleets` (`stars-ui/src/
dump.rs`) build the same text — the game's own header strings when a
copy of the executable is at hand (`resources::text`), this project's
column names otherwise — and the desktop's submenu writes it beside the
game and reports the file (or the failure) in its notice bar. The rows
draw on the same figures the summary pane shows: `pct_planet_
desirability`, `pct_survive`, `minerals_mined`, `resources_at_planet`,
`env_text`, `pct_planet_opt_value`, `planet_scanner_range_for_tech`,
`fleet.mass`, `cloak_pct`, `fleet_scan`, `remote_mines`, `fleet_sweep`,
`mines_laid`, `orbital_adjusters`, `ship_class`.

Approximated, because this engine keeps less than the original:

* **Report Age** is `0` for the player's own planets and blank for the
  rest — there is no `PLANET.turn` stamp here.
* **Population** of somebody else's planet is the estimate as the
  summary pane holds it, in hundreds.
* **Def %** of somebody else's planet: the appended estimate uses the
  scanned defence count.
* **Cap** is the population over `calc_planet_max_pop`, in percent.
* **Destination** of another player's fleet is left blank.
* **ETA** is the leg's distance over `warp²`, rounded up.
* **GateRange/GateMass** are read off the part's name, `any` as `−1`.

`tests/dump.rs` checks the three on the tutorial's world: the universe
lists every planet from the `.xy`, the planet and fleet tables have as
many fields in every row as in their header (20 and 12, or 36 and 29
per-player), the files are named for the width, and the headers are
the game's own with the executable.
