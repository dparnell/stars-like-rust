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

Still to come in Step 4:

- `combat.md` — battle resolution
- `terraforming.md` — auto and remote terraforming
- the production **build queue**, which needs the components table
- the AI players

## Verification

Two layers, both in `crates/stars-core/tests/`:

- `golden_vectors.rs` checks the implementation against
  `../vectors/planetary-economy.json`, every case of which is a rule stated in
  the manual — so a failure means drift from the documented game, not merely
  from our own earlier output.
- `differential_growth.rs` replays real save files (the 40-turn Exodus game and
  the three-player sample game) and compares against what the original engine
  actually wrote a year later, down to the fractional-population accumulator.
