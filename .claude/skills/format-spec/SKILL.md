---
name: format-spec
description: Workflow for reverse-engineering a Stars! on-disk record or block layout and landing it as a docs/formats spec, a typed stars-formats module, and fixture-backed round-trip tests. Use when decoding a new record/block type or extending an existing .xy/.mN/.hN/.xN/.rN/.hst format.
---

# Decoding a Stars! file format

The formats layer is the project's first correctness anchor. Every layout is
written down in `docs/formats/` **before or alongside** the Rust, and every
claim is anchored to a real fixture.

## Sources, in order of authority

1. **NB09 debug symbols** via `sirgwain/stars-asm` — the game's own struct,
   field and enum names. `docs/formats/nb09-structs.md` and
   `docs/formats/record-types.md` hold what has been captured.
2. **`sirgwain/stars-decompile`** — reconstructed C (`types.h`, `enums.h`,
   `file.c`, `save.c`).
3. **Ghidra** on `stars.2.7j.exe`, with the bridge applied (see the
   `ghidra-bridge` skill). Cite addresses as `selector:offset`, e.g. `1048:34e0`.
4. **`documentation/MANUAL.PDF`** for semantics and value ranges.
5. Older community tools (TotalHost, starsapi, StarsBlock.pm, stars-4x) — useful
   cross-checks, but they lose to NB09 names on any conflict.

## Steps

1. **Locate the record.** Every file is a flat sequence of blocks: a 16-bit
   type/size header word plus payload (`stars-formats::block`), with payloads
   encrypted by the Stars! PRNG stream cipher (`crypt`, seeded from the
   plaintext header block). Identify the block type in
   `docs/formats/record-types.md`.
2. **Write the spec** from `docs/templates/format-layout-template.md` into
   `docs/formats/<name>.md`. Start it with a status line
   (`not started` / `in progress` / `verified`). Give a field table with byte
   offsets, widths, bit ranges and the NB09 field name, and include annotated
   hex from a real fixture with its file and offset.
3. **Implement** a typed record in a new `crates/stars-formats/src/<name>.rs`
   module: a struct plus `read`/`write` (or `decode`/`encode`) that round-trips
   exactly. Data only — no game logic, no filesystem, no `unsafe`. Export it
   from `lib.rs` and update the format/status tables in the `lib.rs` doc comment.
4. **Test against real files** in `crates/stars-formats/tests/<name>_files.rs`,
   following the existing tests: assert `write(read(bytes)) == bytes`, then
   assert semantic values that a human verified (planet counts, fleet
   positions, race traits). Fixtures live in `fixtures/` — `incoming/turn0`,
   `incoming/turn1`, `games/tutorial` and `games/exodus` (40 turns) are the
   independent games; using two different `game_id`s proves the cipher seeding
   is game-independent. **Skip, don't fail, when a fixture is missing.**
5. **Capture vectors** for anything formula-like under `docs/vectors/` as small
   stable text the tests can load.
6. **Verify and commit.** `cargo fmt --all`, `cargo clippy --workspace
   --all-targets` (CI is `-D warnings`), `cargo test --workspace`. Commit with a
   `WIP:` prefix while the layout is still partly unknown; drop the prefix once
   the spec says `verified` and the round-trip test passes.

## Done means

The spec's status is `verified`, its fields all cite a source, the module
round-trips every applicable fixture byte-for-byte, and
`docs/formats/README.md` plus the `lib.rs` status table list the new format.
