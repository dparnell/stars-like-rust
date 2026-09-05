# Player scores

Status: **located, not implemented** — the formula is read out below from
`CalcPlayerScore` (`1038:58a6`) and `UpdatePlayerScores` (`10b8:6258`), but one
piece of it is not yet pinned down, so nothing computes a score yet. The score
**block** it produces is decoded and verified: see
[`score.md`](../formats/score.md).

This is the last visible number the engine does not produce. It is written down
here rather than guessed at in code, and the fixtures can settle it when it is
implemented: every `.mN` carries its player's own row and every `.hN` a row per
turn, so a computed score can be compared against what Stars! itself wrote —
the same differential test the rest of the project uses.

## The score (`CalcPlayerScore`)

```
score = 0
for each planet the player owns:
    planets += 1
    score += min((population + 999) / 1000, 6)   # population is in hundreds,
                                                 # so a point per 100,000 people,
                                                 # six at most
    if the planet has a starbase whose hull has cargo space:
        starbases += 1
    resources += CResourcesAtPlanet(planet)

score += resources / 30
score += starbases * 3

if the player is not dead:
    for each of the six tech fields, at level L:
        techs += L
        score += L                 if L < 4
               | L * 2 - 3         if L < 7
               | L * 3 - 9         if L < 10
               | L * 4 - 18        otherwise
```

Then the ships, which is where it gets interesting. Every design is classified
by `LComputePower` (`1038:0b32`) into **unarmed** (no power), **escort** (a
little) or **capital** (more), the ships of each class are counted across the
player's fleets, and:

```
unarmed  = min(unarmed, planets)      # you cannot score more ships than planets
escorts  = min(escorts, planets)
score += unarmed / 2
score += escorts                      # ... the exact weighting still to confirm
score += capital ships × planets × k / (capital ships + planets)
```

The capping by planet count is the part worth knowing: a huge fleet over a
small empire scores as though it were a small one.

## What is not pinned down

- **`LComputePower`'s scale**, and with it the two thresholds that separate
  unarmed from escort from capital. The routine is
  `Σ beam (dp × count × (range + 3) / 4)` — a sapper counting a third of that —
  plus `Σ torpedo (dp × count × (range - 2) / 2)`, plus
  `Σ bomb ((kill + installations killed) × count × 2)`, with the beam total
  scaled by a capacitor multiplier and by the design's speed
  (`+ beams × (speed - 4) / 10`). The thresholds are constants in the compare
  at `1038:5a...`, which have not been read out yet.
- The exact multiplier `k` in the capital-ship term.

## Ranking and victory (`UpdatePlayerScores`)

Ranking is simply `rank = 1 + the number of players scoring higher`. The rest
of that routine is the **victory conditions**: each is a `GetVCVal` threshold
(planets held, tech levels, total score, twice the second player's score,
production, capital ships, a turn count) and a `GetVCCheck` saying whether the
game is using it; a player meeting enough of them wins, and everybody is told.
None of that is modelled either.

## Source

- `CalcPlayerScore` `1038:58a6`, `UpdatePlayerScores` `10b8:6258`,
  `LComputePower` `1038:0b32`, `CResourcesAtPlanet` `1048:788e`.
- The block it fills: `docs/formats/score.md` (type 45), decoded and verified.
