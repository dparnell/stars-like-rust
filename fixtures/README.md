# Fixtures — real Stars! sample files

This folder holds real (anonymized) game files used by the differential
round-trip tests in `stars-formats` and the semantic/turn tests in
`stars-core`.

## What to put here

Real files produced by the original game, grouped by extension:

```
fixtures/
  xy/     # .xy   universe files
  m/      # .mN   player state files
  h/      # .hN   player history files
  x/      # .xN   player orders files
  r/      # .rN   race files
  hst/    # .hst  host files
```

## Conventions

- Prefer **small** universes so tests stay fast.
- **Anonymize** player names/passwords where present before committing.
- Name files by scenario, e.g. `xy/tiny-2p.xy`, `m/tiny-2p.m1`.
- Tests locate fixtures relative to the workspace root (`fixtures/...`), so
  they run the same locally and in CI.

## `incoming/`

Real sample files provided by the user live in `fixtures/incoming/`, grouped by
the turn at which they were captured (one 3-player game, "A Barefoot JayWalk"):

```
fixtures/incoming/
  turn0/   # fresh game, turn 0:  Game.{xy,m1,m2,m3,hst}
  turn1/   # after 1 turn:       Game.{xy,m1,m2,m3,hst,h1,h2,h3,x1}
```

The turn-1 capture adds the previously-missing file types — player history
(`.hN`), player orders (`.xN`) — and, being turn 1, exercises the
turn-dependent cipher seeding. The differential tests in `stars-formats`
(`tests/real_files.rs`) read directly from these folders and skip gracefully if
they are absent.

## Status

`incoming/` holds the first real game at two turns. `encode(decode(bytes)) ==
bytes` is verified byte-for-byte for every fully-framed file across both turns
— `.hst`/`.mN` (turn 0 and turn 1), plus the turn-1 `.hN` history and `.xN`
orders — and the turn-1 files confirm the seeding math on non-zero turns.

`xy/` now holds four **standalone universe** files of assorted sizes
(`02ca32d8.xy`, `across.xy`, `e8dda8f7.xy`, `dancing.xy` — 160/160/360/540
planets). Together with the in-game `.xy` from `incoming/`, they let the `.xy`
parser be verified across five different universes: the planet count is read
from the game-info block and the whole file (including its trailing player-count
bytes) **round-trips byte-for-byte** — see `docs/formats/xy.md` and
`tests/real_files.rs::xy_dir_universes_round_trip`.

`r/` now holds seven exported **race** files (the six built-in default races
plus a "random" race). They round-trip byte-for-byte and the race record is
largely decoded — see `docs/formats/race-r.md` and
`crates/stars-formats/tests/race_files.rs`.
