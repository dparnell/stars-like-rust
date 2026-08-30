# RNG notes

Reconstruction of the original Stars! PRNG, created from
`../templates/rng-notes-template.md`:

- `prng.md` — the core generator (algorithm, seeding, consumption points)

The **encryption** PRNG has already been recovered (Step 2) and is documented in
`../formats/blocks.md` and implemented as `stars_formats::StarsRng`
(`crates/stars-formats/src/crypt.rs`): a subtractive combination of two
Park–Miller LCGs, seeded from a 128-entry primes table. The gameplay RNG used in
Step 3 is expected to be the same generator (possibly seeded differently); when
confirmed, `crates/stars-core`'s `Rng` should reuse or mirror `StarsRng`, and a
reference-sequence vector goes under `../vectors/`.
