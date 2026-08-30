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

Real sample files provided by the user live in `fixtures/incoming/`
(`Game.{xy,m1,m2,m3,hst}` — a fresh 3-player game, turn 0). The differential
tests in `stars-formats` (`tests/real_files.rs`) read directly from there and
skip gracefully if the directory is absent.

## Status

`incoming/` holds the first real game. `encode(decode(bytes)) == bytes` is
verified byte-for-byte for `.hst`/`.m1`/`.m2`/`.m3`; `.xy` header + game-info
are decoded (planet array pending — see `docs/formats/xy.md`). Additional files
that would help: a `.rN` race file and a `.hN`/`.xN` history/orders file to
anchor those record layouts, and a later-turn save to confirm the seeding math
on non-zero turns.
