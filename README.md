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
docs/                 # reverse-engineering knowledge base (specs, RNG notes, test vectors, Ghidra bridge)
docs/plans/           # delivery plan
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

# Open a real saved game and see what the engine makes of it:
cargo run -p stars-desktop -- fixtures/games/exodus/2424/exodus.m6
cargo run -p stars-desktop -- fixtures/games/exodus/2424/exodus.m6 --turn
```

The second of those prints something like:

```text
fixtures/games/exodus/2424/exodus.m6
  Turn file, game 0x012e2128, year 2424
  19 planets simulated, 3 known only at a distance, 13 designs
  player 5: 19 planets, 818200 colonists, tech [3, 8, 6, 5, 5, 3], 30% to research

generated year 2425
  1012 kT mined, population +274 (in hundreds)
  player 5 put 1912 into research, gaining 1 levels
  not simulated: [Orders, FleetMovement, Things, Combat, Terraforming, RandomEvents, Scores]
```

The last line is deliberate: the engine reports the parts of a turn it does not
yet simulate rather than quietly leaving them out.

## Development status

The file-format layer (Step 2) is decoded and round-trip tested against real
games, and the deterministic planetary economy (Step 3) — habitability,
population, mining, resources, scanning and fleet movement — is recovered,
specified and verified against real save files.

Step 4 is well advanced: research, the production queue, the turn pipeline and
most of combat are implemented. A real saved game now loads and generates a
turn, and a whole-turn replay against the next year's file reproduces **87% of
planet populations and 86% of mineral concentrations** exactly. The AI players
are the main piece not yet started.

See `docs/plans/stars-re-reimplementation.md` for the full delivery plan,
`docs/formats/README.md` and `docs/formulas/README.md` for the specs, and
`docs/ghidra-triage.md` for the map of the original binary.

## Contributing

`CLAUDE.md` holds the working agreements (commands, commit conventions, code
and documentation rules) for both humans and Claude Code. Repo-specific Claude
Code skills and commands live under `.claude/`.

## License

MIT (see crate manifests). Original **Stars!** assets under `binary/` and
`documentation/` are the property of their respective owners and are included
here only to support reverse engineering.
