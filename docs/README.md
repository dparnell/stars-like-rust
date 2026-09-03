# stars-re-docs — Reverse-Engineering Knowledge Base

This directory is the durable knowledge base for the Stars! reimplementation.
It captures everything recovered from the original `STARS!.EXE` (via Ghidra)
and from `documentation/MANUAL.PDF`, so that the Rust implementation is
**spec-driven and reproducible** rather than a machine translation.

## How this fits the project

```
Ghidra RE of STARS!.EXE  ─┐
                          ├─►  docs/ (specs + test vectors)  ─►  crates/*  ─►  cargo test
MANUAL.PDF (game rules)  ─┘                                     (impl)        (verification)
```

Every non-obvious constant, formula, byte layout, or RNG step used in the code
should trace back to a document here.

## Layout

| Path                      | Purpose                                                            |
|---------------------------|-------------------------------------------------------------------|
| `templates/`              | Copy-me starting points for each kind of RE artifact.             |
| `formats/`                | One spec per on-disk file format (`.xy`, `.mN`, `.hN`, …).        |
| `formulas/`               | One spec per simulation subsystem (production, growth, combat…).  |
| `rng/`                    | Notes and reconstruction of the original PRNG.                    |
| `vectors/`                | Golden test vectors (inputs → expected outputs) consumed by tests.|
| `ghidra-triage.md`        | Map of the ~1350 functions into UI / simulation / file-I/O areas. |

## Authoring rules

1. **Cite the source.** Every fact records where it came from: a Ghidra
   address (e.g. `1048:34e0`), a manual page, or a sample-file offset.
2. **Prefer worked examples.** A formula spec is not done until it has at least
   one worked numeric example that becomes a test vector.
3. **Status header.** Each spec starts with a status line
   (`not started` / `in progress` / `verified`) so progress is visible.
4. **One artifact, one file.** Keep specs small and focused; link between them.
5. **Machine-readable vectors.** Store golden vectors as small, stable text
   (JSON/CSV/hex) under `vectors/` so `stars-core` / `stars-formats` tests can
   load them directly.

## Status

- **Step 2 (formats)** — complete: `formats/` documents every on-disk record
  layout, verified by round-trip tests against real games.
- **Step 3 (planetary economy)** — complete: `formulas/` documents
  habitability, population, mining, resources, scanning and movement, with
  golden vectors in `vectors/planetary-economy.json`.
- **Step 4** — in progress: production, combat, research and terraforming specs
  are still to be written.
