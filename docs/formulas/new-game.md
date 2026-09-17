# Subsystem: New game creation (universe, homeworlds, starting fleets)

- **Status:** verified against three real turn-0 games; the exceptions are
  listed below, and none of them is a known discrepancy
- **Ghidra routine(s):** `GenerateWorld`, `CreateStartupShip`, `CAdvantagePoints`
  (`10e0:444c`), `LInnateRaceHabitability` (`10e0:4cb2`)
- **Manual reference:** `MANUAL.PDF` pp. 2-1..2-3 (the New Game wizard) and
  p. 3-2 (the advantage-point budget)
- **Uses RNG:** yes, heavily (see `../rng/prng.md`)
- **Implemented in:** `crates/stars-core/src/newgame.rs`,
  `crates/stars-core/src/advantage.rs`, `crates/stars-core/src/startup.rs`,
  `crates/stars-core/src/opponents.rs`

`GenerateWorld` is the routine behind the New Game wizard. It builds the
universe, chooses homeworlds, sets each player's starting technology, stocks
their homeworld and hands out their first ships, and finally writes the `.xy`
and one data file per player.

## The reference game

`fixtures/incoming/turn0/` is a three-player game exactly as `GenerateWorld`
left it — turn 0, year 2400, nobody has moved. It fixes almost every stage of
the algorithm, and every claim below that says "verified" was checked against
it in `crates/stars-core/tests/new_game.rs`.

Its parameters: small universe (`mdSize` 1), sparse-plus-one density
(`mdDensity` 1), 128 planets, three players, `mdStartDist` 1, single-player
flag set. Player 0 is a person playing a Jack of All Trades; players 1 and 2
are both **Turindrones, Standard**.

## 1. Scatter and thin

```
dGal      = mdSize * 400 + 400                 # 400 .. 2000 light years
cPlanMax  = dGal * dGal / 5000
cPlanMax += (cPlanMax / 4) * (mdDensity - 1)
if mdDensity >= 3: cPlanMax += cPlanMax / 4
cPlanMax  = min(cPlanMax, 999)
```

`iMax = min(cPlanMax + cPlanMax/7, 999)` points are drawn uniformly, x then y,
each as `1000 + 10 + Random(dGal - 19)`; the array is sorted by ascending x.
Any point within `dGalMinDist = 12` light years of an earlier one is struck out
(its y is set to `-100`, which also removes it from every later distance test,
so nothing is struck twice). Points are then struck at random until only
`cPlanMax` remain.

**Verified.** The planet-count formula reproduces `GAME.cPlanMax` exactly for
all nine distinct `.xy` fixtures, covering sizes 0-2 and densities 0-3:

| Universe | size | density | planets |
|----------|-----:|--------:|--------:|
| `tutorial.xy` | 0 | 0 | 24 |
| `incoming/turn0/Game.xy` | 1 | 1 | 128 |
| `across.xy` | 1 | 2 | 160 |
| `all-computer-players` | 2 | 2 | 360 |
| `exodus.xy` | 2 | 3 | 540 |

Coordinates lie in `[1010, 1010 + dGal - 20]` on both axes in every fixture, and
the generated ones do too.

### Clumping

With `fClumping` set, each planet in turn (chosen at random, `cPlanMax` times)
is dragged toward its nearest neighbour: to one third of the way if that
neighbour is more than 40 light years off, a half beyond 25, two thirds beyond
18, and four fifths beyond 12. The array is re-sorted afterwards.

## 2. Names

Names are drawn without replacement from the 999-entry master table (see
`../formats/xy.md`): `Random(999)`, then linear probing upward with wraparound
until an unused index is found.

## 3. Environment and minerals

Per planet:

```
fArtifact       = Random(3) == 0                 # unless fNoRandom
gravity         = 1 + Random(90) + Random(10)    # 1 .. 99
temperature     = 1 + Random(90) + Random(10)    # 1 .. 99
radiation       = 1 + Random(99)                 # 1 .. 99
for each mineral:
    conc = Random(45) + Random(45) + 31          # 31 .. 119
    if radiation >= 90: conc += Random(99 - conc) / 2
iT = Random(27)
if iT < 18:                                      # ~2/3 of planets
    if iT >= 9: one mineral := 1 + Random(30)
    else:       iT += 1; while iT < 16: one mineral := 1 + Random(30); iT *= 2
```

Two things fall out of this that are worth stating, because they are what makes
the rule checkable without a seed:

* **Gravity and temperature are the sum of two draws; radiation is one.** So the
  first two reach their extremes about half as often as the third does.
* **About two thirds of planets have one mineral impoverished** below 31,
  which pulls the mean concentration from 75 down to about 57.

**Verified.** Across the 600 unowned planets the fixtures describe, against
11,440 generated ones:

| Measure | real games | generated |
|---------|-----------:|----------:|
| gravity ≤ 9 | 4.8% | 5.1% |
| temperature ≤ 9 | 3.8% | 5.0% |
| radiation ≤ 9 | 9.2% | 9.3% |
| any mineral < 31 | 66.8% | 66.7% |
| mean concentration | 57.4 | 57.6 |

The 600-planet sample carries about ±2% of noise, so every row agrees, and the
radiation-versus-gravity asymmetry the rule predicts is reproduced almost
exactly.

### Planet 0 is the stock room

After the loop, **planet 0's surface minerals** are rolled — `Random(conc * 10)
+ 10`, topped up by `155 + Random(150)` if that came to less than 200 — and
every homeworld is stocked from them. Planet 0 itself is then zeroed if it ends
up unowned, which is why the stock cannot be read back out of a save.

## 4. Homeworlds

```
floor  = dGal * 6
ideal  = max(0, dGal * dGal / cPlayer - floor) * 9 / 10
ideal  = ideal * mdStartDist / 3 + floor
min²   = ideal * 9 / 10
max²   = ideal * 7 / 6
```

Player 0 gets up to fifty tries at a planet in the middle half of the map, and
otherwise the closest one that came up. Every other player gets fifty random
tries at a planet that is inside a border (a twentieth of the map in from each
edge for five or more players, a tenth for three or four, three twentieths for
one or two), at least `min²` from every player already placed and within `max²`
of at least one of them. If that fails, the whole planet array is walked from
where the search stopped; if *that* fails, the band widens by `ideal/35` at each
end and **every** player is placed again.

Which player gets which planet is then shuffled.

## 5. Setting the players up

### Starting technology

| Primary trait | Starting levels |
|---------------|-----------------|
| Super Stealth | Electronics 5 |
| War Monger | Weapons 6, Propulsion 1, Energy 1 |
| Claim Adjuster | Biotech 6, Construction 2, Energy 1, Weapons 1, Propulsion 1 |
| Space Demolition | Propulsion 2, Biotech 2 |
| Packet Physics | Energy 4 |
| Inner Tech | Propulsion 5, Construction 5 |
| Alternate Reality | Energy 1 |
| Jack of All Trades | 3 in every field |
| Hyper Expansion, Inner Strength | none |

Then "expensive tech starts at level 3" raises every field with no per-field
research setting of its own to 3 (4 for a Jack of All Trades), and Cheap Engines
and Improved Fuel Efficiency each add a level of Propulsion.

**Verified** against the fixture: its Jack of All Trades reads `[3,3,3,3,3,3]`
and its two Super Stealth computer players `[0,0,1,0,5,0]` — Electronics 5 from
the trait, Propulsion 1 from Improved Fuel Efficiency.

### The homeworld

Ten mines, ten factories, ten defences, a starbase, no artifact, and 250
hundreds of colonists (175 with Low Starting Population). Surface minerals are
copied from planet 0; mineral concentrations are copied too, floored at 30
(at 25 while the tutorial runs — bit 11 of the client's `gd` word, tested at
`1078:1db6`; `tutorial.hst` has both home worlds at `[25, 70, 84]` from a
template of 24). The
environment is set to the **exact middle of the race's habitable band** on every
axis, `min + (max - min) / 2`, or a random `1 + Random(99)` on an axis the race
is immune to.

**Verified**: the fixture's three homeworlds all read 10/10/10, population 250,
no artifact, concentrations `86/30/67` where planet 0 reads `86/8/67`, and
environments `50/50/50` and `62/33/61` — exactly the midpoints of their owners'
bands.

### Leftover advantage points

A **person** spends what their race did not: `min(50, CAdvantagePoints(race))`.
A **computer player** spends the full fifty whatever its race costs — which is
one of the ways the built-in opponents are handed an advantage, and why several
of them are priced well over the budget a player is held to. Two more advantages
come with difficulty: from **Tough** (level 2) upward the homeworld's mineral
concentrations are raised as well, even when the race would have spent on
surface minerals; from **Expert** (level 3) upward it starts with a tenth more
colonists.

The points buy whatever `rsUseLeftover` names:

| Setting | What it buys |
|---------|--------------|
| Surface minerals | `points * 10`, a quarter to each mineral and the remainder plus another quarter to the scarcest |
| Mineral concentrations | half the points to the scarcest, a quarter to all three |
| Mines | `points / 2` |
| Factories | `points / 5` |
| Defences | `(points + 5) / 10` |

**Verified against every homeworld in all three real turn-0 games** — 34
homeworlds covering all six computer personalities at all four difficulties,
plus two human players. Every population, mineral concentration and surface
stock is reproduced exactly.

That is checkable without knowing anything the file does not say, because every
homeworld in a game is stocked from the same planet 0: a race that spends its
leftovers on **concentrations** never touches the surface minerals and so shows
the stock directly, and a player below Tough never touches the concentrations
and shows those. From those two readings the other twenty-nine homeworlds
follow.

#### Where the reconstructed source is wrong

`create.c` renders this as

```c
iT = min(50, CAdvantagePoints(pplr));
if ((pplr->fAi != 0) && (pplr->lvlAi > 2)) {
    iT = 50;
    plHome->rgwtMin[3] += plHome->rgwtMin[3] / 10;
}
```

which makes the full fifty conditional on the difficulty. The disassembly shows
two **nested** tests, the outer one on `fAi` alone:

```text
1078:1f48  CALLF CAdvantagePoints        ; iT = min(50, ...)
1078:1f8b  SHR AX, 9 / AND AX, 1         ; fAi
1078:1f93  JNZ  1f98 / JMP 2010          ; a person keeps what it computed
1078:1f98  MOV  [iT], 0x32               ; a computer player gets fifty
1078:1fb0  SHR AX, 10 / AND AX, 7        ; lvlAi
1078:1fb5  CMP AX, 3 / JNC 1fbd          ; only then, the population bonus
1078:1fbd  ...                           ; rgwtMin[3] += rgwtMin[3] / 10
```

and the same shape at `1078:2290`, where the jump into the concentration branch
is taken when `fAi` and `lvlAi >= 2`. The difficulty is a **three**-bit field
(`(word >> 10) & 7`), not two.

This is what the earlier "`CAdvantagePoints` is wrong" note in this spec was
really about: the function is not consulted for a computer player at all, so its
value for the built-in races was never the thing being observed.

#### A note on `uPopGuess`

The owner's own population estimate is a quarter of the starting figure and is
computed **before** the Expert bonus: a Robotoids Expert homeworld reads
population 275 and guess 62, not 68. Packet Physics and Inner Tech recompute
both guesses after the second-planet split, so theirs are a quarter of the split
figure. `GameState` does not model the field, so a written file recomputes it
from the population it has — a one-field difference on an Expert homeworld, and
one the game overwrites on the next turn.

### Starting ships

Templates come from the built-in design tables `rgshdefT` (22 ships) and
`rgshdefSBT` (4 starbases), transcribed in
`crates/stars-core/src/startup.rs`. Which ones a player gets:

| Primary trait | Ships |
|---------------|-------|
| Packet Physics | Long Range Scout, Santa Maria, and a second colonised planet |
| War Monger | Armed Probe (plus Stalwart Defender and Gadfly above Construction 2), Santa Maria |
| Jack of All Trades | Armed Probe, Long Range Scout, Santa Maria, Teamster, Stalwart Defender, Cotton Picker |
| Super Stealth | Smaugarian Peeping Tom (Shadow Sleuth above Energy 1), Shadow Transport if a person is playing, Santa Maria |
| Hyper Expansion | Smaugarian Peeping Tom, three Spore Clouds |
| Inner Tech | Smaugarian Peeping Tom, Mayflower, Stalwart Defender, Swashbuckler, and a second planet |
| Alternate Reality | Smaugarian Peeping Tom, Pinta |
| Space Demolition | Smaugarian Peeping Tom, Santa Maria, Little Hen, Speed Turtle |
| Claim Adjuster | Smaugarian Peeping Tom, Santa Maria, Change of Heart |
| Inner Strength | Smaugarian Peeping Tom, Santa Maria |

Advanced Remote Mining without Only Basic Remote Mining adds two Potato Bugs.
Each ship is its own fleet, orbiting the homeworld with full tanks.

Every template is written against the cheapest components in the game, so a
race that starts with technology has them upgraded in place. The substitution
lists, first match wins:

| Component | Candidates, in order |
|-----------|----------------------|
| Quick Jump 5 | Radiating Hydro-Ram Scoop, Alpha Drive 8, Daddy Long Legs 7, **Long Hump 6, Fuel Mizer** |
| Bat Scanner, Rhino Scanner | Possum, Mole, Rhino |
| Mole-skin Shield / Tritanium | Wolverine Diffuse Shield / Carbonic Armor, then Cow Hide / Crobmnium |
| Laser, X-Ray Laser | Yakimora Light Phaser, X-Ray Laser |
| Alpha Torpedo | Beta Torpedo |
| Robo Midget/Mini Miner | Robo Miner, then the original |

**Verified**: all six of the fixture's design records are reproduced slot for
slot, and so are the six fleets' fuel loads (50, 300, 200, 450, 530, 210 mg).

Two departures from the decompiled source, both forced by that check:

* The reconstructed `create.c` lists **Fuel Mizer before Long Hump 6**. The
  fixture's Jack of All Trades starts at Propulsion 3, which reaches both, and
  its designs carry Long Hump 6. The order above is the one the file shows.
* The mining list falls back to the **original** part rather than the Robo
  Midget Miner. The fixture's Cotton Picker keeps its Robo Mini Miners; the
  decompiled list would have downgraded them.

### Built-in computer players

Six personalities at four difficulties, `vrgplrComp[6][4]`, transcribed in
`crates/stars-core/src/opponents.rs`: Robotoids (Hyper Expansion), Turindrones
(Super Stealth), Automitrons (Inner Strength), Rototills (Claim Adjuster),
Cybertrons (Packet Physics) and Macinti (Alternate Reality), each at Easy,
Standard, Tough or Expert.

**Verified**: the fixture's two computer players are `Turindrones, Standard`
field for field — same economy bytes, same habitable bands, same growth rate,
same lesser traits, including the Cheap Factories checkbox that entry's
`grbitAttr` bit 31 calls for.

## Edge cases & clamps

- `cPlanMax` is capped at `cPlanetAbsMax = 999`, and a Huge/Packed universe
  reaches it.
- The border and centre bounds are computed with `muldiv_i16`, i.e. in 32 bits:
  `2000 * 19` does not fit a 16-bit register.
- If the minimum-distance pass removes more planets than the target cull, the
  universe simply ends up with fewer planets than the formula asks for.
- The homeworld search widens its band and restarts rather than giving up. Our
  implementation caps the restarts at 1000 and accepts the last arrangement, so
  a degenerate universe (one planet, sixteen players) terminates.
- `Random(99 - conc)` is called with a negative argument when a concentration
  already exceeds 99; the generator is still stepped and returns 0.

## What is not reproduced

* **Seed-identical universes** — *this was listed here as impossible and is
  not.* The reasoning was that the original sorts its scratch array with
  `qsort`, whose permutation of equal x coordinates the C standard leaves
  unspecified. True, and beside the point: the runtime is statically linked
  into `stars.exe`, so whatever it does it does the same way every time.

  `crates/stars-core/tests/tutorial_seed.rs` proves it. The tutorial's
  universe is the one real galaxy whose seed can be known —
  `CreateTutorWorld` (`1078:5e5e`) calls `Randomize(0x499602d2)`, a constant
  compiled into the program — and generating from it reproduces
  `fixtures/games/tutorial/tutorial.xy` exactly: 24 planets, same
  coordinates, same names, `0x0c` is Prune and `0x0d` is Stove Top just as
  the tutorial's own pages say.

  No other fixture can check this, and that is why it went untested. A `.xy`
  stores its settings but **not** its seed: `GenNewGameFromFile`
  (`1078:4b0d`) seeds from a game-definition file when there is one, and an
  ordinary new game seeds from the clock. Regenerating a real player's
  galaxy is impossible for want of the seed, not for want of the algorithm.

## 6. Random races (`CreateRandomRace`, `10e0:5b08`)

Run for any player whose race carries `ibitRaceAIPlayer` — the wizard's
"Random", the only shipped race with the bit — in the same loop that shuffles
the homeworlds (`1078:13d0`), just after each player's shuffle draw. The
opponents' own races do not carry it and are left alone.

```
shape = Random(25)
shape < 4:   all three axes immune;                         growth = 2 + Random(4)
shape < 7:   all three axes 0..100;                         growth = 3 + Random(4)
shape < 9:   axes 0, 1: Random(2) == 0 → 0..100, else left as the template
             has it (and growth = 2 + Random(4), overwritten below);
             axis 2: left as it is (the template's centres being equal);
                                                             growth = 2 + Random(5)
else:        each axis: width = 20 + 2 × Random(40); low = Random(101 − width);
             shape < 12: one axis (Random(3)) immune
             shape < 14: one axis 0..100
             shape < 17: one axis low = Random(81), 20 wide;  growth = 7 + Random(9)

k = Random(3): research settings 8..13 all 1 if k == 0, else each Random(3)
primary trait = Random(10)
k = Random(4): lesser traits 0..13 all off if k == 0, else each Random(2)
Expensive Tech Starts at 3 = Random(2); Cheap Factories = Random(2)
k = Random(3): k == 0 → economy = {10,10,10,10,10,5,10} (1120:0de0), leftover = Random(5)
               else each statistic 0..7 = min + Random(max + 1 − min)  (CS:30f4 / CS:3104)
name == "Random" → one of the 24 from idsBerserker (0x56e), Random(24)
```

Then the balancing: while `CAdvantagePoints` is outside `0..=50`, up to 251
tries, each a nudge kept only if it lowers the distance outside the range
(`max(points − 50, −points)`):

| `Random(10)` | nudge |
|---|---|
| < 3 | a research setting (`Random(6)`) down one, else up one |
| < 6 | a lesser trait (`Random(14)`) off, then on |
| < 9 | an economy statistic (`Random(7)`) down one, then up one |
| else, coin | an axis (`Random(3)`): immune → `low = Random(31)`, 70 wide; else immune |
| else | growth down one, then up one |

The 252nd try gives up and copies `vrgplrDef[0]` — the predefined Humanoid,
name aside — over the race. `crates/stars-core/tests/new_game.rs` rolls sixty
of them.

## 6a. Battle plans

`GenerateWorld` gives every player the five stock plans (`InitBattlePlan`,
one per `rgbtlplanT` entry, the player's number on each); in a single-player
game the first — Default — attacks **everyone** rather than enemies.
`crate::default_battle_plans`.

## 7. Victory conditions and the simple dialog's roster

`NewGameWizard` (`1078:6022`) writes the dialog's defaults into `GAME.rgvc`
the moment the simple dialog returns: `88 8e 82 0a 88 09 09 07 01` — owning
60% of the planets, tech 22 in 4 fields and a score twice the second
player's, counting (bit 7); a score of 11,000, 100,000 resources, 100 capital
ships and the highest score after 100 years, set but not counting; one
condition enough. `InitNewGamePlr` (`1078:6e44`) then sets the least years to
`2 × mdSize` — 30 for a tiny universe, ten more a size. `CreateTutorWorld`
sets only `rgvc[7] = 0x80, rgvc[8] = 0x81`, which `tutorial.xy` confirms.
`crate::newgame::default_victory`, `NewGame::victory`.

The simple dialog also chooses the opponents (`InitNewGamePlr`), from the
size and the difficulty (0 easy to 3 expert): the player count by the draws
in `simple_game_opponents`'s doc comment, then the computer players dealt a
type byte by their place — `personality << 2 | 3` with the level in the top
three bits, `0x9b` for one drawn at random — from the tables at `1078:6f9a`
onward, and shuffled with `Random(cPlayer − i − 1)` from the second place
to the second last. `NewGameWizard` reads a personality of 6 as `Random(6)`
and a level past 3 as `Random(4)`. The dialog then sets normal density, a
start distance of 1 (2 at harder and expert), AIs banding together at
expert, and a game name from the string table by difficulty and size.
This project's single-page wizard offers the roster as a button per
difficulty; the density, distance and name stay the player's own.

## 8. Wormholes

After the battle plans, unless `fNoRandom`: `vrgWormholeMin[mdSize] +
Random(vrgWormholeVar[mdSize])` pairs, with `min = {0,1,1,3,4}` (`1078:0000`)
and `var = {3,3,5,4,5}` (`1078:0006`). Each end is a `THING` of kind wormhole
with `iStable = Random(3)`, the second end and the first made each other's
partner, and each placed by up to a hundred tries at `(Random(dGal) + 1000,
Random(dGal) + 1000)` — the first `IValidateWormholePos` scores 0 (see
`wanderers.md`), else the least bad seen. The first end of a pair is scored
before its partner exists.

## Open questions

- **`CAdvantagePoints` has only one independent check.** It returns exactly 25
  for the stock Humanoid, which the turn-0 fixture confirms twice over — and
  that is the only race in the corpus whose price is *observable*, because a
  computer player never consults the function and the only two human players in
  a turn-0 game both play the stock Humanoid. Its values for the built-in
  computer races are large and negative (`Robotoids, Expert` prices at −1161),
  which is what one would expect of races deliberately built over budget, but
  nothing in the fixtures confirms or refutes them. A saved game whose human
  player used a custom race would settle it.

  Two earlier revisions of this note got this wrong in opposite directions. The
  first said the fixture proved the built-in opponents do not have Cheap
  Factories; writing the file formats disproved that — the trait is stored in
  the checkbox byte at offset 81, and the fixture's computer players carry it
  exactly as `vrgplrComp`'s bit 31 says. The second concluded that
  `CAdvantagePoints` must therefore be wrong by at least 111 points; the
  disassembly disproved that too, by showing the function is not consulted for a
  computer player at all. The lesson both times was that the observable was not
  what it looked like.
- **`mdStartDist`'s base.** Only the values 1 and 3 appear in the fixtures, and
  the formula divides by 3, so 1..3 is assumed (Close, Moderate, Distant). A
  fixture with 2 would confirm it and one with 0 would refute it.
- **The `.xy` version word.** `FileHeader::new` writes `0x2A2B`, copied from the
  2.7 fixtures, because the split of the sixteen-bit version into
  major/minor/increment does not produce a sensible version number.

## Worked example (becomes a test vector)

Given the turn-0 reference game's planet-0 stock `[337, 337, 306]`:

- a stock Humanoid (Jack of All Trades, growth 15, all stats baseline) has
  `CAdvantagePoints` = **25**, and its homeworld ends with
  `[399, 399, 432]` kT on the surface;
- `Turindrones, Standard` has `CAdvantagePoints` = **72**, capped to 50, and its
  homeworld ends with `[462, 462, 556]`.

Captured at: `../vectors/new-game.json`.
