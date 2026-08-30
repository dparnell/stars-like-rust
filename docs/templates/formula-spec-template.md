# Subsystem: <name> (e.g. Mineral Production)

- **Status:** not started | in progress | verified
- **Ghidra routine(s):** `<seg:off>` `<name>`
- **Manual reference:** `MANUAL.PDF` p.<page(s)>
- **Uses RNG:** no | yes (see `../rng/prng.md`)
- **Implemented in:** `crates/stars-core/src/<module>.rs`

## Inputs

| Name | Type | Range / units | Source |
|------|------|---------------|--------|
|      |      |               |        |

## Outputs

| Name | Type | Range / units |
|------|------|---------------|
|      |      |               |

## Formula

<State the exact calculation, step by step, including integer truncation,
rounding, saturation, and 16-bit overflow behaviour recovered from the
disassembly. Note the order of operations precisely — it matters for fidelity.>

```
# pseudocode mirroring the original's integer math
result = ...
```

## Edge cases & clamps

- <min/max clamps, division-by-zero handling, negative handling, overflow wrap>

## Worked example (becomes a test vector)

Given:
- input_a = …
- input_b = …

Then:
- expected_output = …

Captured at: `../vectors/<name>.json`

## Open questions

- <ambiguities to resolve, values that don't yet match reference output>
