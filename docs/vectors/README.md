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
| `new-game.json` | Universe generation: planet counts, coordinate bands, environment and mineral shape, homeworlds, advantage points, starting technology, designs and fuel | Read out of `fixtures/incoming/turn0/`, a real three-player game exactly as `GenerateWorld` left it |
| `order-attr-nib.json` | The `.xN` waypoint-task operation (record type 11): what each payload decodes to and whether the host accepts it | Read out of the replay arm at `1048:c3f0`. The one operation no fixture can supply, because nothing in the client writes it |

`planetary-economy.json` is deliberately sourced from the **manual** rather than
from our own output, so a failure means the implementation has drifted from the
documented game rather than merely from its own earlier behaviour. It is
consumed by `crates/stars-core/tests/golden_vectors.rs`.

Populated from delivery Step 2 onward.
