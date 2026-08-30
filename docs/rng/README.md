# RNG notes

Reconstruction of the original Stars! PRNG, created from
`../templates/rng-notes-template.md`:

- `prng.md` — the core generator (algorithm, seeding, consumption points),
  **confirmed from the binary** (`FUN_1038_8a58` / `FUN_1038_89e4` in
  `STARS!.EXE`).

The **encryption** PRNG has been recovered (Step 2) and, as of the Ghidra pass,
confirmed against the shipped binary. It is documented in `prng.md` and
`../formats/blocks.md`, and implemented as `stars_formats::StarsRng`
(`crates/stars-formats/src/crypt.rs`): **L'Ecuyer's combined LCG** — two
Park–Miller sub-generators (Schrage's method) combined by subtraction, seeded
from a table indexed by header fields. It reproduces real files byte-for-byte.

The gameplay RNG used in Step 3 is the **same generator** (`FUN_1038_8a58`,
possibly seeded differently at the call sites); `crates/stars-core`'s `Rng`
should reuse or mirror `StarsRng`, and a reference-sequence vector goes under
`../vectors/`.
