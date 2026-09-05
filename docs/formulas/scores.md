# Player scores

Status: **verified** — every one of the 1,826 comparable score rows in the
fixtures comes out exactly right: the score itself, and the planet, starbase
and tech-level counts beside it. The victory conditions are implemented too,
and checked as far as the fixtures allow (below). Implemented in
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

## Victory

The same routine decides who has **won**, and
[`stars_core::victory`](../../crates/stars-core/src/victory.rs) follows it.

A game is set up with some of ten conditions switched on. Each is one byte of
`GAME.rgvc` in the **universe file** — not in the save — where bit 7 says the
game is playing for it (`GetVCCheck`, `1078:b60c`) and the low seven bits are a
**slider position**, which `GetVCVal` (`1078:b710`) turns into the threshold:

| # | condition | threshold from slider `n` |
|--:|-----------|---------------------------|
| 0 | owns a share of all planets | `n × 5 + 20` percent |
| 1 | attains a tech level | `n + 8` |
| 2 | …in that many fields | `n + 2` |
| 3 | exceeds a score | `n × 1000 + 1000` |
| 4 | exceeds second place by | `n × 10 + 20` percent |
| 5 | produces resources a year | `n × 10 + 10` thousand |
| 6 | owns capital ships | `n × 10 + 10` |
| 7 | holds the highest score after | `n × 10 + 30` years |
| 8 | how many must be met | `n`, capped by how many are switched on |
| 9 | the earliest year a win counts | `n × 10 + 30` |

Three things about how they are applied are worth stating, because none of them
is obvious:

- **A met condition is flagged whether or not the game is playing for it.** The
  original sets the scoreboard bit as soon as the threshold is passed and only
  then asks `GetVCCheck` before counting it, so a scoreboard can show a
  condition met in a game nobody can win that way.
- **The two comparative conditions are the sole leader's alone.** "Exceeds
  second place" and "holds the highest score" are not offered to a tie.
- **The last player standing wins**, whatever the game was set up for and
  however early it is.

The capital-ship count is compared after a round trip through the scoreboard's
[packing](#the-score-calcplayerscore-103858a6), so a large fleet is compared
roughly rather than exactly.

### What the fixtures could and could not settle

The five conditions a player meets on their own — planets, tech, score,
production, capital ships — are checked against 1,787 real scoreboard rows, and
none is ever claimed falsely. None of those rows has one *set*, though, so the
check is one-sided: the games in the fixtures are young.

Two things stop it going further. The comparative conditions cannot be checked
from a player file at all, because that file describes one player and everybody
else's score computes as zero — which would make its owner the runaway leader of
every game — and **no `.hst` in the fixtures carries a scoreboard**. And the
exodus turns, the only ones whose scoreboards *do* show conditions met, come
with a universe file in which every condition is switched off; the settings
those games were played with are not in the corpus, so their rows are skipped
rather than explained away.

Who has won is worked out; **telling the players is not**, since messages are
not modelled.

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
