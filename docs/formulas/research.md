# Subsystem: Research

- **Status:** verified
- **Ghidra routine(s):** `10d8:1dba` `GetTechLevelCost`, `10b8:80fe` `UpdateResearchStatus`, `10d8:65ae` `ProjectedResearchSpending`
- **Manual reference:** `MANUAL.PDF` pp. 8-4..8-6 ("The Cost of Research"), p. 8-6 sidebar (Generalized Research)
- **Uses RNG:** no
- **Implemented in:** `crates/stars-core/src/research.rs`

## Inputs

| Name | Type | Range / units | Source |
|------|------|---------------|--------|
| `player.rgTech[6]` | `int8_t` | level per field, `0..=26` | `PLAYER` +0x1A, disk offset 26 |
| `player.rgResSpent[6]` | `uint32_t` | resources banked toward the next level | `PLAYER` +0x20, disk offset 32 |
| `player.pctResearch` | `char` | share of resources for research | `PLAYER` +0x38, disk offset 56 |
| `player.iTechCur` | `char` | low nibble = current field, high nibble = next-field policy | disk offset 57 |
| `player.lResLastYear` | `int32_t` | resources research received this turn | `PLAYER` +0x3A, disk offset 58 |
| `race.rgAttr[8..14]` | `char` | per-field cost setting | `PLAYER` +0x3E+8, disk offset 70 |

## The cost table

`rglTechCost[27]` at `10d8:1d4e`, verified byte-for-byte against our binary:

```
0, 50, 80, 130, 210, 340, 550, 890, 1440, 2330, 3770, 6100, 9870, 13850,
18040, 22440, 27050, 31870, 36900, 42140, 47590, 53250, 59120, 65200,
71490, 77990, 84700
```

The manual calls this "increasing in a Fibonacci type series" (p. 8-5), which
holds well up to about level 12 (890 + 1440 = 2330 exactly) and then flattens
into roughly even steps.

## Formula

```
cost = (sum of all six tech levels) * 10 + rglTechCost[level]

setting = race.rgAttr[rsTechBonus1 + field]
if setting == 0: cost = cost * 2 - cost / 4     # costs 75% extra
if setting == 2: cost = cost / 2                # costs 50% less

if game.fSlowTech: cost *= 2
```

The `sum of levels * 10` term is the manual's "added cost of 10 resources per
field of study per level you've already achieved" (p. 8-5) — the reason
researching everything ends up costing the same however you order it.

### The per-field setting is stored the opposite way round to how it reads

A stored `0` makes a field **more** expensive and a stored `2` makes it
cheaper. This corrects `docs/formats/race-r.md`, which had the mapping
inverted. Confirmed twice over: the branch at `10d8:1dba` multiplies by 1.75
when the value is below 1 and halves it when above; and the Exodus fixture's
War Monger stores `2` for Weapons and `0` for all five other fields, which is
exactly the trade a War Monger makes.

## The annual advance

```
research budget = sum over the player's planets of the Produce skim
                  (see production.md)

if Generalized Research:
    points[current] += (budget + 1) / 2                 # half, rounded up
    for every other field: points[f] += (budget*3 + 19) / 20   # 15%, rounded up
else:
    points[current] += budget

while some field can afford its next level:
    points[field] -= cost(field, level+1)
    level += 1
    if field is the current field and the player asked for another next:
        move the remaining points to the new field and switch to it
```

Note that Generalized Research spends 125% of the budget in total (50% + five
lots of 15%), which matches the manual's description of the trait: half the
resources go to the current field and 15% of the total to each other field.

Super Stealth races additionally steal `spent / living_players / 2` in every
field anyone spent in, provided that comes to more than 1.

## What a level brings (`UpdateResearchStatus`, `10b8:80fe`)

Each level gained sends `idmScientistsHaveCompletedResearch…` (`0x50`, or
`0x136` for Generalized Research) with the Research dialog for its Goto,
and then a message for **each part the level has just made buildable**.
The scan walks the component categories from the engines up — one
`hst` bit at a time, `hstEngine`, `hstScanner`, … `hstHull`, `hstPlanetary`
— and every item in each; a part counts when `FLookupPart` now says
*available* and its requirement **in the field that advanced equals the
new level**, so a part that was waiting on another field is reported when
that field catches up, and a part already reachable is not reported
twice. A Total Terraforming race skips terraforming items 8, 12 and 16.
The message is one of five:

| category | message | object |
|----------|---------|--------|
| a starbase hull | `0xd0` `idmRecentBreakthroughHasAlsoGivenHullDesign` | `-3`, the Ship Design dialog |
| a ship hull | `0x78` `idmRecentBreakthroughHasAlsoGivenHullType` | `-3` |
| planetary items 9–13 (the defences) | `0x145` `idmRecentBreakthroughHasAlsoTaughtHowBuild` | the part word |
| planetary items 0–8 (the scanners) | `0x157` `idmRecentBreakthroughHasAlsoTaughtHowBuild2` | the part word |
| anything else | `0x5f` `idmRecentBreakthroughHasAlsoGivenBenefit` | the part word |

The part word is `0xc000 | category index << 8 | item`, which Goto opens
the Technology Browser on; the parameters are `[field, category bits,
item]`. The tutorial's 2413 is the worked example: Construction 4 brings
the Robo-Miner (`0x5f`, mining robots item 2) and the Privateer hull
(`0x78`, hull 11), in that order, and pages 41 and 44 read them.

## Edge cases & clamps

- Levels stop at 26.
- Players flagged crippled or cheating cannot advance past level 9.
- The next-field policy is `6` to stay put and `7` for "whichever field is
  lowest"; anything else is a field index.
- `rgResSpent` is stored at twice its logical value when the slow-tech option
  is on, which cancels against the doubled cost.

## Worked example (becomes a test vector)

The Exodus fixture's player 5 at year 2435, researching Weapons with levels
`[3, 10, 7, 7, 5, 4]` and Weapons set to "costs 50% less":

- sum of levels = 36, so the added cost is 360
- `rglTechCost[11]` = 6100, so the base is 6460
- halved for the cheap field: **3230**

The engine consumed exactly 3230 resources taking Weapons from 10 to 11.

Captured at: `../vectors/research.json`.

## Verification

`crates/stars-core/tests/differential_research.rs` replays the 40-turn Exodus
game: 11 years where nothing was researched confirm that the banked points grow
by exactly the year's allocation, and 5 breakthroughs confirm the cost formula
to the resource. Seven further years change research field (where leftover
resources move with the player and the banked balance cannot be read directly)
and four advance a field nobody was researching or one the player could not
have afforded — technology gifts, which the game grants through Mystery
Traders and captured or scrapped ships.

## Open questions

- Technology gifts (Mystery Trader, ship capture and scrapping) are not
  modelled, and are the reason four Exodus advances cannot be priced.
- The newly-available-parts announcement after each breakthrough needs the
  components table.
