# Formula specs

One spec per simulation subsystem, created from
`../templates/formula-spec-template.md`.

Every spec cites the Ghidra address of the routine in `stars.2.7j.exe` it was
recovered from **and** the `MANUAL.PDF` page that documents the same rule. The
manual is a cross-check, not the authority: where the two disagree the binary
wins, because the shipped code is what produced the save files the project
tests against. On the rules below they agree, which is why the manual's worked
examples make good golden vectors.

## Delivery Step 3 — the planetary economy

| Spec | Subsystem | Status |
|------|-----------|--------|
| `habitability.md` | Planet value and maximum population | verified |
| `population.md` | Growth, death, overcrowding, the `iDeltaPop` accumulator | verified |
| `mining.md` | Mineral extraction and concentration decay | verified |
| `resources.md` | Resource output, operable mines and factories | verified |
| `scanning.md` | Scanner ranges and the fourth-power combination rule | in progress |
| `movement.md` | Fleet movement geometry and fuel use | in progress |

The PRNG that mining and the turn pipeline draw from is specified separately in
`../rng/prng.md`; it is the same generator as the file cipher's.

## Delivery Step 4 — turn generation

| Spec | Subsystem | Status |
|------|-----------|--------|
| `turn-order.md` | The order a year happens in | verified |
| `research.md` | Tech costs, the annual advance, Generalized Research | verified |
| `production.md` | Resource/research split; the build queue | in progress |
| `components.md` | The component data tables (engines, weapons, scanners, …) | verified |

| `combat.md` | Board, movement, targeting, weapon accuracy, firing | in progress |
| `bombing.md` | Bombing a planet from orbit | transcribed, unverified |
| `ground.md` | Landing colonists: settling and invasion | in progress |
| `design.md` | Hulls, slots, and the values derived from them | verified |
| `fleet.md` | Ship stacks, cargo, and what derives from them | model and loading done |

Still to come in Step 4:

- the torpedo **accuracy formula**, transcribed but unverified against real fire
- **RNG alignment**, which needs consecutive tutorial-mode turns

## Delivery Step 5 — the frontend

| Spec | Subsystem | Status |
|------|-----------|--------|
| `new-game.md` | Universe generation, homeworlds, starting fleets, advantage points | verified |
| `waypoint-tasks.md` | What a fleet does when it arrives: colonise, transport, merge, scrap, route, and what the rest still need | in progress |

`new-game.md` is checked against `fixtures/incoming/turn0/`, a real turn-0 game
as `GenerateWorld` left it, which pins down almost every stage of the algorithm.

## Verification

Two layers, both in `crates/stars-core/tests/`:

- `golden_vectors.rs` checks the implementation against
  `../vectors/planetary-economy.json`, every case of which is a rule stated in
  the manual — so a failure means drift from the documented game, not merely
  from our own earlier output.
- `differential_growth.rs` replays real save files (the 40-turn Exodus game and
  the three-player sample game) and compares against what the original engine
  actually wrote a year later, down to the fractional-population accumulator.
- `new_game.rs` checks generation against `../vectors/new-game.json` and against
  the turn-0 fixture directly, including a distribution comparison between 600
  real planets and 11,440 generated ones.
