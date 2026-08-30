# Golden test vectors

Small, stable data files (JSON/CSV/hex) that pin down exact expected outputs.
They are loaded directly by tests in `stars-formats` and `stars-core`, so the
docs and the code cannot drift apart.

- Each vector is described by a doc created from
  `../templates/test-vector-template.md`.
- Keep files tiny and deterministic; record their provenance.

Populated from delivery Step 2 onward.
