# Stars-re — working agreements

RE-guided clean **Rust reimplementation** of the 16-bit Windows 3.1 game
**Stars!**. The original `STARS!.EXE` is reverse-engineered in Ghidra; what we
recover is written down as specs in `docs/` and then reimplemented cleanly in
Rust. This is **not** a decompilation — correctness comes from differential file
tests and spec-driven test vectors.

Delivery plan (scope, architecture decisions, staged steps):
`docs/plans/stars-re-reimplementation.md`. Steps 1–3 are done (file formats,
and the deterministic planetary economy); Step 4 (turn generation, combat,
research, AI) is in progress.

## Layout

```
crates/stars-formats/   on-disk formats (.xy/.mN/.hN/.xN/.rN/.hst): read/write + round-trip tests
crates/stars-core/      deterministic simulation + turn engine + AI (no I/O, no UI)
crates/stars-ui/        shared egui views (built on stars-core)
crates/stars-desktop/   native eframe/winit frontend (binary: `stars`)
crates/stars-web/       wasm frontend (stretch goal)
docs/                   RE knowledge base: format specs, formulas, RNG, vectors, Ghidra bridge
fixtures/               real sample game files consumed by the tests
binary/ documentation/  original game assets + MANUAL.PDF
ghidra/                 live Ghidra project (git-ignored, regenerable)
tmp/stars-asm/          optional checkout of sirgwain/stars-asm (NB09 symbols source)
```

## Commands

```sh
cargo build --workspace --all-targets
cargo test  --workspace
cargo test  -p stars-formats --test real_files      # one integration test binary
cargo fmt --all
cargo clippy --workspace --all-targets              # CI runs with -D warnings
cargo run -p stars-desktop
```

CI (`.github/workflows/ci.yml`) runs fmt-check, clippy, build and test on Linux,
macOS and Windows with `RUSTFLAGS=-D warnings`. Run fmt + clippy before
committing.

## Version control

- **Commit often** — whenever a task or a meaningful chunk of work is done; do
  not batch many unrelated changes into one large commit.
- **Commit work in progress too** — prefer an unfinished commit over leaving
  work uncommitted.
- **Prefix in-progress commits with `WIP:`** (e.g.
  `WIP: reverse-engineer .xy header layout`). Completed, self-contained work
  uses a normal message.
- **Never version Ghidra working files.** `ghidra/*.gpr`, `*.rep/`, `*.lock` are
  a volatile, regenerable database and are git-ignored. Only the original assets
  under `binary/` and the bridge inputs under `docs/ghidra/` are versioned.

## Code conventions

- `stars-formats` is **data only**: typed models plus `read_`/`write_` pairs
  over `&[u8]`. No game logic, no filesystem access, no platform APIs. It is
  `#![forbid(unsafe_code)]`.
- `stars-core` is **pure and deterministic**: no I/O, no rendering, no platform
  code. Same inputs + seed → same output on every target, native and wasm.
- Round-trip contract for every format: `write(read(bytes)) == bytes` on real
  fixtures.
- Tests that need fixtures **skip** rather than fail when the fixture is absent,
  so a checkout without sample games keeps CI green.
- Every non-obvious constant, offset or formula in the code cites its source in
  a comment — a Ghidra address (e.g. `1048:34e0`), an NB09 struct field, a
  `MANUAL.PDF` page, or a fixture offset.

## Documentation rules (`docs/`)

1. **Cite the source** for every fact (Ghidra address, manual page, file offset).
2. **Prefer worked examples** — a formula spec is not done until it has a
   numeric example that becomes a test vector under `docs/vectors/`.
3. **Status header** on each spec: `not started` / `in progress` / `verified`.
4. **One artifact, one file**; link between specs instead of duplicating.
5. **Machine-readable vectors** (JSON/CSV/hex) so tests load them directly.

The authoritative source for struct/field names is the CodeView **NB09** debug
symbols via [`sirgwain/stars-asm`](https://github.com/sirgwain/stars-asm); see
`docs/formats/nb09-structs.md`. They outrank older tool-derived names.
[`sirgwain/stars-decompile`](https://github.com/sirgwain/stars-decompile)
reconstructs the C source and is the fastest way to read a routine, but it
covers a slightly different build and some functions are still stubs, so treat
it as a cross-check and confirm anything load-bearing against our own binary in
Ghidra. Clone either into `tmp/` (git-ignored).

**Game formulas are also documented in `documentation/MANUAL.PDF`.** Consult it
whenever you recover one: it states the designers' intent in plain language and
supplies the player-facing units, which makes it far easier to tell a correct
transcription from a plausible one. The binary still wins on any disagreement —
cite both in the spec.

## Ghidra

The `ghidra` MCP server (configured in `.mcp.json`) drives the live Ghidra
instance with `stars.2.7j.exe` open. Before analysing, make sure the NB09 name /
type / signature bridge has been applied — see the `ghidra-bridge` skill and
`docs/ghidra/README.md`.
