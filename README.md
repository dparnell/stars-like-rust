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

# The game itself: a window with nothing open, or a saved game.
cargo run -p stars-desktop
cargo run -p stars-desktop -- fixtures/games/exodus/2424/exodus.m6

# Text tools on a saved game: a summary, or a year generated from it.
cargo run -p stars-desktop -- fixtures/games/exodus/2424/exodus.m6 --summary
cargo run -p stars-desktop -- fixtures/games/exodus/2424/exodus.m6 --turn

# The tutorial, playing itself in a window at a human pace (release: the
# frames are drawn on the CPU). STARS_AUTOPLAY_DELAY_MS sets the pace.
cargo run --release -p stars-desktop --example autoplay_tutorial
```

The window reproduces the original's screen: its panes, tiles, dialogs,
reports and menus are laid out from the original's own dialog templates
and the pixel offsets its code places things at, and its pictures and
text are read from a copy of `stars.exe` at run time — one under
`binary/`, beside the game being opened, or named by `STARS_EXE` — so
nothing of the original's art is in this repository. The bitmaps, icons
and their masks are decoded straight out of the executable's resource
table (`crates/stars-formats/src/resources/`), including the colour-table
rewrites the original makes at start-up.

`--turn` prints something like:

```text
fixtures/games/exodus/2424/exodus.m6
  Turn file, game 0x012e2128, year 2424
  19 planets simulated, 3 known only at a distance, 13 designs, 33 fleets
  player 5: 19 planets, 818200 colonists, 30 fleets (38 ships), tech [3, 8, 6, 5, 5, 3], 30% to research

  player 5: replayed 11 orders (2 cargo, 3 waypoint, 1 queue, 0 research, 0 design, 0 routing)

generated year 2425
  1012 kT mined, population +329 (in hundreds)
  planet 187 built 1 of item 8, 3 of item 0
  ...
  player 5 put 571 into research, gaining 1 levels
```

Had the engine skipped a step, a last line `not simulated: [...]` would name
it: the engine reports the parts of a turn it does not simulate rather than
quietly leaving them out.

## Development status

The file-format layer (Step 2) is decoded and round-trip tested against real
games; the deterministic planetary economy (Step 3) — habitability,
population, mining, resources, scanning and fleet movement — is recovered,
specified and verified against real save files.

Step 4, turn generation, is nearly whole. The pipeline runs the original's
steps in the original's order (`docs/formulas/turn-order.md`): the order
logs each player submitted are replayed, fleets move and burn fuel through
minefields, packets, wormholes and the Mystery Trader move, planets mine,
build, grow and research, fleets carry out their tasks on arrival —
transport, colonise, remote mine, lay mines, scrap, patrol, route —
**battles** are fought on the original's board and recorded for the VCR
and their wreckage picked over for technology, planets are **bombed**,
mineral **packets** are thrown, caught and felt, minefields hurt what flies
into them, cloaks hide fleets from scanners, terraforming runs, the year's
**random events** strike, and the scores are kept. What is still missing
is a handful of corners named in the specs.

The **computer players** are in: `DoAiTurn` and the TurinDrone personality
— its research plan, its ship designs from the AI part tables, its queue
pass, its scouts, colony ships, miners, haulers, mine layers and armadas,
and its housekeeping (`docs/formulas/ai.md`). All seven personalities
play: each researches by its own recovered plan and share; the Maid's
turn is its own, so is the Robotoid's — its Frigate mine layers, Meta
Morph and Battleship warships, Nubian armadas and its hunt for the other
players' fleets — and so is the Automitron's, which colonises by Medium
Freighter and whose scouts chase the enemy — and so is the Cybertron's,
which spreads its people by freighter, builds warship groups and throws
mineral packets at its enemies — and the Rototill's, which never designs
a ship and lives on what it started with — and the Macinti's, the
Alternate Reality opponent that moves its people between its starbases
by the resources they would make there. All seven are transcribed
(`docs/formulas/ai.md`, *The seven personalities*) and checked against the
two sixteen-player corpus games, each player's turn run on its own files:
research exact over 3,200 player-years, the designs by slot and hull,
the colony ships' targets in nineteen legs of twenty, the queues wherever
no random draw gates them (*What the corpus confirms of the
personalities*).

Step 5, the frontend, is a playable game: the scanner with its six views
and its overlays, scrolled with its own bars past 100% as the original's
window is; the toolbar with the game's own pictures; the planet and fleet
panes and their tiles at the sizes the original's tables give them; the
production dialog with its templates; the **Ship and Starbase Designer**
on the original's 610-by-450 client, its slots wearing the component
pictures and the empty-slot sheet, parts dragged from the palette under
the original's rules (`IDropPart`: what stacks, what fills, what is
refused); research, the technology browser, cargo and ship transfer
between a fleet and its planet or two fleets standing together,
battle plans, player relations, the four reports, the score sheet, host
mode, and a new-game wizard with the six-page race designer. The
**Battle VCR** plays the recordings back on the original's board — the
ships' pictures with their owners' emblems, the beams as `AnimateAttack`
draws them, the torpedoes flying frame by frame from the game's own icons
and the bursts landing where they hit. The **tutorial** is wired up — the
original's tutor machine, all eighty of its pages and its world — and a
test plays it through the panes from the first page to the last, pressing
what each page names where the pane drew it; where this engine's game
parts from the original's (the ships come out under other numbers) the
run does what the page asks and says so (`docs/ui/tutorial.md`, *The run,
to the end*). Every dialog and pane has been rasterised through the test
harness's own software renderer and looked over for overlaps; the same
renderer films the tutorial (`docs/ui/tutorial.md`).

Verification is differential where a fixture allows it: replaying a year
of a real game against the file the original engine wrote reproduces the
economy to the unit on the tutorial's fixtures and the bulk of the
sixteen-player corpus, and the computer players' turns are run on the
corpus players' own files and compared with what the original wrote;
combat is transcribed from the binary and unit-tested, and is not yet
checked against a recorded battle. The frontend is tested through egui's
own input — the tests press, drag and click what the panes drew, as a
hand would — and the workspace's suite stands at about 120 test binaries
that run on every push.

What is still not the original's, and says so in its spec: the Help
buttons, and
the dialogs' colours — the shell is on a dark theme where the original is
button-face grey, though the tiles keep the original's black-on-grey.

See `docs/plans/stars-re-reimplementation.md` for the delivery plan,
`docs/formats/README.md`, `docs/formulas/README.md` and `docs/ui/` for
the specs (two dozen each of format, formula and screen specs, every
fact with its Ghidra address, manual page or fixture offset), and
`docs/ghidra-triage.md` for the map of the original binary.

## Contributing

`CLAUDE.md` holds the working agreements (commands, commit conventions, code
and documentation rules) for both humans and Claude Code. Repo-specific Claude
Code skills and commands live under `.claude/`.

## License

MIT (see crate manifests). Original **Stars!** assets under `binary/` and
`documentation/` are the property of their respective owners and are included
here only to support reverse engineering.
