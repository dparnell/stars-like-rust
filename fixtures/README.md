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

## Status

Empty at scaffolding time (delivery Step 1). Add fixtures as Step 2 begins so
the round-trip tests have real data to assert against.
