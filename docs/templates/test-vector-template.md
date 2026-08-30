# Test vector: <name>

- **Covers:** <format spec / formula spec / rng doc it validates>
- **Consumed by:** `crates/<crate>/tests/<file>.rs`
- **Data file:** `../vectors/<name>.json`

## Purpose

<One sentence: what invariant or calculation this vector pins down.>

## Format of the data file

Describe the JSON/CSV/hex schema so the test loader and future editors agree.
Keep it minimal and stable. Example:

```json
{
  "seed": 305419896,
  "inputs": { "population": 25000, "growth_rate_pct": 15 },
  "expected": { "next_population": 28750 }
}
```

## Provenance

<Where the expected values came from: the formula spec's worked example, a
byte-for-byte round-trip of a real file, or a reference run of the original
binary under emulation/Wine. Cite Ghidra addresses / manual pages / filenames.>
