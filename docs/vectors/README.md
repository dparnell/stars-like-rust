# Golden test vectors

Small, stable data files (JSON/CSV/hex) that pin down exact expected outputs.
They are loaded directly by tests in `stars-formats` and `stars-core`, so the
docs and the code cannot drift apart.

- Each vector is described by a doc created from
  `../templates/test-vector-template.md`.
- Keep files tiny and deterministic; record their provenance.

## Contents

| File | Covers | Provenance |
|------|--------|------------|
| `planetary-economy.json` | Maximum population, growth and death, resources, mining and concentration decay, scanner combination, warp travel | Every case is a rule stated in `documentation/MANUAL.PDF`; each section names the page and the Ghidra routine it exercises |

`planetary-economy.json` is deliberately sourced from the **manual** rather than
from our own output, so a failure means the implementation has drifted from the
documented game rather than merely from its own earlier behaviour. It is
consumed by `crates/stars-core/tests/golden_vectors.rs`.

Populated from delivery Step 2 onward.
