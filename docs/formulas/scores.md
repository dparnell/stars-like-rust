# Player scores

Status: **verified** — every one of the 1,826 comparable score rows in the
fixtures comes out exactly right: the score itself, and the planet, starbase
and tech-level counts beside it. Implemented in
[`stars_core::score`](../../crates/stars-core/src/score.rs) and checked by
`crates/stars-core/tests/scores.rs` against the scoreboards Stars! wrote.

The block the score goes into is decoded separately: see
[`score.md`](../formats/score.md).

## The score (`CalcPlayerScore`, `1038:58a6`)

```
score = 0
for each planet the player owns:
    planets += 1
    score += min((population + 999) / 1000, 6)   # population is in hundreds,
                                                 # so a point per 100,000
                                                 # colonists, six at most
    if the planet has a starbase whose hull has cargo space:
        starbases += 1                           # an Orbital Fort has none
    resources += CResourcesAtPlanet(planet)

score += resources / 30
score += starbases * 3

if the player is not dead:
    for each of the six tech fields, at level L:
        score += L            if L < 4
               | L * 2 - 3    if L < 7
               | L * 3 - 9    if L < 10
               | L * 4 - 18   otherwise
```

The planet itself is worth nothing: only its people count, and only in hundred
thousands.

Then the ships. Every design is classified once by its power (below) as
**unarmed**, **escort** or **capital**, the ships of each class are counted
across the player's fleets, and — this is the part worth knowing — the counts
are **capped by the number of planets**, so a huge fleet over a small empire
scores as though it were a small one:

```
score += min(unarmed, planets) / 2
score += min(escorts, planets) * 2
score += 8 * capital * planets / (capital + planets)
```

## A design's power (`LComputePower`, `1038:0b32`)

```
beams     = Σ beam slots:    damage × count × (range + 3) / 4   # a sapper: a third of that
torpedoes = Σ torpedo slots: damage × count × (range - 2) / 2
bombs     = Σ bomb slots:    (colonists killed + installations destroyed) × count × 2
capacitors: a running 1000, multiplied by (100 + ability) / 100 for each one fitted
            (the Energy and Flux Capacitors, items 12 and 13 of the electrical specials)
if any capacitor: beams = beams × min(capacitors / 10, 255) / 100
power = bombs + beams + torpedoes
```

`power <= 0` is unarmed, `power < 2000` an escort, and the rest are capital
ships (`1038:5b14`, `1038:5b4a`).

The original adds one more term — `beams × (speed - 4) / 10`, the speed coming
from `SpdOfShip`, which is not recovered yet. Leaving it out changed **none** of
the 1,826 rows, so no design in the fixtures sits close enough to 2000 for it to
matter; a design that did could be classified one step low.

## Ranking (`UpdatePlayerScores`, `10b8:6258`)

`rank = 1 + the number of players scoring higher`, so a tie shares a rank.

That routine also decides who has **won**: each victory condition is a
`GetVCVal` threshold — planets held, tech levels, total score, twice the second
player's score, production, capital ships, a turn count — and a `GetVCCheck`
saying whether the game uses it. A player meeting enough of them wins and
everybody is told. None of that is modelled.

It is also where a player is **marked dead**: the score is computed first, and a
player with nothing left is marked afterwards. That ordering shows in the files
— a dead player's last scoreboard row still counts the tech levels the next
year's row will not — and it is why the differential test skips a player the
file marks dead.

## What the corpus settled

Two bugs in the **loader** turned up while making these numbers agree, both
found by the scoreboard rather than by any test of their own:

- A planet with **no installations at all** — a colony settled that year — has
  the `fIncImp` flag clear and no installations field, and the loader was
  rejecting it as "not fully described", demoting it to a scanned sighting and
  losing its population. That cost a point of score per such planet, and would
  have cost far more than that everywhere else.
- A **foreign design** — one player's design that another has merely seen,
  stored without its slots — was being skipped without spending its owner's
  design count, so the next player's first design was attributed to whoever the
  foreign one belonged to. A design moved between players, and the ships built
  to it stopped being counted.

## Source

- `CalcPlayerScore` `1038:58a6`, `UpdatePlayerScores` `10b8:6258`,
  `LComputePower` `1038:0b32`, `WPackLong` `1038:4ba2`,
  `CResourcesAtPlanet` `1048:788e`.
- The block it fills: `docs/formats/score.md` (type 45).
