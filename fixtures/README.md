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
  star-names.txt   # master planet-name table (index -> name)
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

## `games/`

Complete games (all files for one game together), one directory per game:

```
fixtures/games/
  tutorial/   # the shipped "Tutorial Game": tutorial.{xy,hst,m1,m2,h1,x1}
  exodus/     # one player's 50-turn history: exodus.xy + <year>/exodus.m6 + Races/
```

`tutorial/` is a **second, independent game** (a different `game_id` from the
`incoming/` sample: `0x008cef49` vs `0x2a031dd8`). It is a 2-player game in a
tiny 24-planet universe ("Tutorial Game"), partly played: the `.hst`/`.m2` are
at turn 0 while player 1's `.m1`/`.h1`/`.x1` were saved at **turn 3**. Because
nothing in the format code is game-specific, every framed file decodes and
re-encodes byte-for-byte unchanged, which is the strongest evidence yet that the
container and cipher seeding are game- and turn-agnostic.

`exodus/` is a **third game**, captured as a single player's long-running
history (player 6). Its layout differs from the others:

```
fixtures/games/exodus/
  exodus.xy          # the universe (540 planets, 8 players; standalone)
  <year>/exodus.m6   # player 6's turn file at ~40 game years (2400 .. 2450)
  Races/*.R1         # the seven shipped AI expansion races
```

The per-year folders each hold one `exodus.m6` snapshot; the folder name is the
in-game year (the user's label) while the authoritative turn number is the
monotonic counter inside each header. All ~40 snapshots decode and re-encode
byte-for-byte, exercising the cipher across many consecutive turns of one game.
The `Races/` files (`BIGPRO`, `DEFENDER`, `ECOBOOM`, `FLEXIBLE`, `JUMPERS`,
`OFFENDER`, `SNEAK`) are the first race fixtures with **Lesser Racial Traits
enabled**, which pinned down the LRT bitfield (see `docs/formats/race-r.md`).

## Status

`incoming/` holds the first real game at two turns. `encode(decode(bytes)) ==
bytes` is verified byte-for-byte for every fully-framed file across both turns
— `.hst`/`.mN` (turn 0 and turn 1), plus the turn-1 `.hN` history and `.xN`
orders — and the turn-1 files confirm the seeding math on non-zero turns.

`xy/` now holds four **standalone universe** files of assorted sizes
(`02ca32d8.xy`, `across.xy`, `e8dda8f7.xy`, `dancing.xy` — 160/160/360/540
planets). Together with the in-game `.xy` from `incoming/` and the 24-planet
`games/tutorial/tutorial.xy`, they let the `.xy` parser be verified across
**six** different universes (24–540 planets): the planet count is read from the
game-info block, each planet record decodes to a unique absolute position and
resolved name, and the whole file (including its trailer) **round-trips
byte-for-byte** — see `docs/formats/xy.md`,
`tests/real_files.rs::xy_dir_universes_round_trip`, and
`tests/real_files.rs::tutorial_xy_universe_round_trips`.

`games/tutorial/` adds a whole second game (see above) whose every framed file
and its `.xy` round-trip byte-for-byte via
`tests/real_files.rs::tutorial_*`.

`star-names.txt` is the **master planet-name table** (999 tab-delimited
`index -> name` rows, `0..=998`) originally embedded in `STARS!.EXE`. A `.xy`
planet record stores only a 10-bit `nameid` index into this list, so the table
is needed to turn indices back into names. It is copied into the crate at
`crates/stars-formats/data/star-names.txt` and used to fully decode `.xy`
planet records — every planet in all **seven** sample universes (the six above
plus `games/exodus/exodus.xy`) resolves to a unique name (see
`docs/formats/xy.md`).

`r/` holds seven exported **race** files (the six built-in default races plus a
"random" race); `games/exodus/Races/` adds the seven shipped AI races. All
round-trip byte-for-byte, and the verified fields (habitability, growth,
research cost, PRT, LRT) are exposed as a typed `RaceRecord`. The AI races were
the key to confirming the **LRT bit layout** (the default races mostly have no
LRTs). See `docs/formats/race-r.md`, `tests/race_files.rs`, and
`tests/exodus_files.rs`.
