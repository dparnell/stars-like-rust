# Subsystem: New game creation (universe, homeworlds, starting fleets)

- **Status:** verified (against a real turn-0 game; the exceptions are listed
  below, and one of them — `CAdvantagePoints` for a race with several lesser
  traits — is a known discrepancy rather than an untested claim)
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
copied from planet 0; mineral concentrations are copied too, floored at 30. The
environment is set to the **exact middle of the race's habitable band** on every
axis, `min + (max - min) / 2`, or a random `1 + Random(99)` on an axis the race
is immune to.

**Verified**: the fixture's three homeworlds all read 10/10/10, population 250,
no artifact, concentrations `86/30/67` where planet 0 reads `86/8/67`, and
environments `50/50/50` and `62/33/61` — exactly the midpoints of their owners'
bands.

### Leftover advantage points

`min(50, CAdvantagePoints(race))` points are spent on the homeworld, in whatever
currency `rsUseLeftover` names:

| Setting | What it buys |
|---------|--------------|
| Surface minerals | `points * 10`, a quarter to each mineral and the remainder plus another quarter to the scarcest |
| Mineral concentrations | half the points to the scarcest, a quarter to all three |
| Mines | `points / 2` |
| Factories | `points / 5` |
| Defences | `(points + 5) / 10` |

**The rule is verified; the point values are only half verified.** The
fixture's homeworlds hold `399/399/432` (player 0) and `462/462/556` (players 1
and 2). Both come from the same planet-0 stock by the rule above, and the only
stock and point pair that produces both is `[337, 337, 306]` with 25 and 50 —
and 50 is the cap, so the second says only "50 or more".

Our `CAdvantagePoints` returns exactly **25** for the Jack of All Trades. For
the two computer players it returns **13**, not 50 or more. See the open
question below. `../vectors/new-game.json` records both, and the test drives the
spending rule from the value the file implies rather than the one we compute, so
the rule and the pricing are checked separately.

`CAdvantagePoints` itself is a long sum over habitability, growth rate, economy,
primary and lesser traits and research costs, divided by three at the end. Its
largest term is `LInnateRaceHabitability`, which integrates the race's planet
value over an 11×11×11 grid across its habitable bands at three levels of
terraforming (0%, 5% and 15% — 8% and 17% with Total Terraforming), weighting
the three levels 7/5/6.

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

- **Seed-identical universes.** The original sorts its scratch array with the C
  library's `qsort`, whose permutation of equal x coordinates is unspecified,
  and every later draw indexes that array. Draw order and distributions are
  faithful; a given seed is not.
- **Wormholes.** `vrgWormholeMin = {0,1,1,3,4}` and
  `vrgWormholeVar = {3,3,5,4,5}` by universe size, placed by
  `IValidateWormholePos`; they live in the `THING` list, which `GameState` does
  not model yet.
- **`CreateRandomRace`**, so a computer player is given whatever race the caller
  supplies rather than a generated one.
- **Battle plans and victory conditions**, which the generated `GameState` does
  not carry.

## Open questions

- **`CAdvantagePoints` is wrong for a race with several lesser traits.** The
  transcription prices the stock Humanoid at exactly 25, which the fixture
  confirms twice over. It prices `Turindrones, Standard` at **13**, and that
  game's homeworlds were stocked as a race of **50 or more**. The gap is at
  least 111 points on the internal scale (the function divides by three at the
  end), so it is not rounding and not the single Cheap Factories deduction.
  What the two races differ in is the terms only the second exercises: four
  lesser traits, an off-centre and lopsided habitable band, a growth rate of 14,
  and a mine-operation figure below the baseline. One of those coefficients is
  wrong or missing. Nothing else in this spec depends on it: the homeworld it
  produces is stocked a little more thinly than the original's.

  An earlier revision of this note had it the other way round, and said the
  fixture proved the built-in opponents do *not* have Cheap Factories. That was
  wrong, and writing the file formats disproved it: a race record stores that
  trait and "expensive tech starts at level 3" **outside** the sixteen-bit
  lesser-trait field, as bits 7 and 5 of the checkbox byte at offset 81 — and
  the fixture's computer players carry the Cheap Factories checkbox exactly as
  `vrgplrComp`'s bit 31 says. The reconstructed table is right; the pricing
  function is what does not add up.
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
