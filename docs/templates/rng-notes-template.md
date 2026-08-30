# PRNG: <name / where used>

- **Status:** not started | in progress | verified
- **Ghidra routine(s):** `<seg:off>` `<name>` (seed), `<seg:off>` `<name>` (next)
- **State size:** <e.g. 32-bit>
- **Implemented in:** `crates/stars-core/src/rng.rs` (`Rng`)

## Algorithm

<Describe the exact update step recovered from the disassembly: multiplier,
increment, modulus/mask, shifts, and which bits are returned. Mirror the
original's integer width and overflow (wrapping) behaviour precisely.>

```
# pseudocode
state = (state * A + C) & MASK
return <bits of state>
```

## Seeding

<How the game derives the initial seed (from the game file? time? a fixed
constant?) and where reseeding happens during a turn.>

## Consumption points

| Subsystem | What it draws | Notes |
|-----------|---------------|-------|
| combat    |               |       |
| minerals  |               |       |
| events    |               |       |

## Reference sequence (becomes a test vector)

Seed = `<hex>` → first N outputs:

```
0: ...
1: ...
```

Captured at: `../vectors/rng-<seed>.json`
