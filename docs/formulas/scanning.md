# Subsystem: Scanner Ranges

- **Status:** in progress — combination rule and planetary ranges verified; per-design ship scanners need the parts table
- **Ghidra routine(s):** `1038:4c02` `GetPlanetScannerRange`, `1038:50d0` `GetShdefScannerRange`, `1038:4fb8` `GetFleetScannerRange`, `1008:60be` `LookupBestPlanetaryScanner`
- **Manual reference:** `MANUAL.PDF` p. 17-2 ("Scanners are Additive"), p. 20-13 (No Advanced Scanners)
- **Uses RNG:** no
- **Implemented in:** `crates/stars-core/src/scanning.rs`

Stars! tracks two ranges per scanner. The **normal** range finds fleets in deep
space; the **penetrating** range additionally sees through planets, revealing
orbiting fleets and sub-surface mineral concentrations.

## The combination rule

Multiple scanners do not simply add. The manual states it directly:

> The formula for calculating a ship's scanner range is the 4th root of the sum
> of each scanner to the 4th power. Let's say you have a ship design with two
> 100 light year scanners and one 60 light year scanner:
> `(100^4 + 100^4 + 60^4) ^ ¼ = 120 light years`

```
range = (sum of range_i^4) ^ (1/4)
```

The same rule combines penetrating ranges, separately from normal ranges.

## Planetary scanners

```
if race is Alternate Reality:
    range = sqrt(population * 10)             # AR planets scan from the starbase
    if NoAdvancedScanners: range = range * 1412 / 1000; penetrating = 0
    else: a sufficiently advanced starbase penetrates at half range
else:
    if planet has no scanner: range = 0
    else:
        raw = best researched planetary scanner's stored range
        penetrating = (-raw) / 2 if raw < 0 else 0
        range = abs(raw)
        if NoAdvancedScanners: range *= 2; penetrating = 0
```

A **negative** stored range is how the parts table marks a penetrating scanner:
its magnitude is the normal range and half its magnitude is the penetrating
range.

The `1412/1000` factor in the Alternate Reality branch is the fourth root of 4
(1.4142…), i.e. the "doubled range" of No Advanced Scanners expressed under the
fourth-power law — doubling a range means quadrupling its fourth power, so
scaling a combined range by `2^(1/2)` is the consistent way to express it.

## Edge cases & clamps

- No Advanced Scanners doubles conventional ranges and removes penetration
  entirely (`MANUAL.PDF` p. 20-13).
- Some racial traits turn other objects into scanners: Packet Physics makes
  mineral packets penetrating scanners, and Space Demolition makes minefields
  normal (non-penetrating) scanners. Neither is implemented yet.

## Worked example (becomes a test vector)

Two 100 ly scanners and one 60 ly scanner:

- `100^4 + 100^4 + 60^4 = 100000000 + 100000000 + 12960000 = 212960000`
- `212960000 ^ 0.25 = 120.85…`, truncated to `120` ly

Captured at: `../vectors/planetary-economy.json` (`scanning`).

## Open questions

- `GetShdefScannerRange` is the routine that walks a design's slots and applies
  the fourth-power law; it is a stub in the reconstructed NB09 sources, so the
  per-design path must be read from our binary directly when ship designs land
  in Step 5. The combination rule itself is confirmed by the manual and is
  implemented.
- The stored ranges of individual scanner parts come from the components table,
  which is not yet decoded.
