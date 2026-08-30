# Stars-re

A clean, portable **Rust reimplementation** of the 16-bit Windows 3.1 4X game
**Stars!**, produced by reverse-engineering the original `STARS!.EXE` with
Ghidra and re-expressing its data formats and game logic as specifications and
maintainable code.

Targets native Windows / macOS / Linux, with a WebAssembly build as a stretch
goal. This is an **RE-guided reimplementation** — not a decompilation — so
correctness is anchored in differential file testing and spec-driven test
vectors rather than machine-translated code.

## Repository layout

```
Cargo.toml            # Cargo workspace
crates/
  stars-formats/      # on-disk file formats (.xy/.mN/.hN/.xN/.rN/.hst): read/write + round-trip tests
  stars-core/         # deterministic simulation + turn engine + AI (no I/O, no UI)
  stars-ui/           # shared egui view code (built on stars-core)
  stars-desktop/      # native eframe/winit frontend (binary: `stars`)
  stars-web/          # wasm/eframe frontend (stretch goal)
docs/                 # reverse-engineering knowledge base (specs, RNG notes, test vectors, Ghidra triage)
fixtures/             # real (anonymized) sample game files used by tests
binary/               # original game assets (STARS!.EXE, help, sound, …)
documentation/        # MANUAL.PDF (original game manual)
ghidra/               # Ghidra project used for reverse engineering
```

## Architecture

A **headless core + thin frontends** design: all game logic and file formats
live in platform-agnostic library crates; the desktop and web shells only wire
those to a windowing/rendering backend and file I/O.

```
stars-formats ──► stars-core ──► stars-ui ──► stars-desktop (native)
                                          └──► stars-web (wasm, stretch)
```

The core is strictly deterministic: identical inputs and seed produce identical
results on every platform.

## Building

Requires a stable Rust toolchain (developed against Rust 1.98).

```sh
cargo build --workspace     # compile everything
cargo test  --workspace     # run all tests
cargo run -p stars-desktop  # run the (placeholder) native shell
```

## Development status

Early scaffolding. The workspace builds and its smoke tests pass; the
reverse-engineering knowledge base under `docs/` is set up and ready to be
populated. See `.junie/plans/stars-re-reimplementation.md` for the full
delivery plan and `docs/ghidra-triage.md` for the map of the original binary.

## License

MIT (see crate manifests). Original **Stars!** assets under `binary/` and
`documentation/` are the property of their respective owners and are included
here only to support reverse engineering.
