# Format: `<.ext>` — <short name>

- **Status:** not started | in progress | verified
- **Original files analysed:** `<fixture filenames>`
- **Ghidra reader routine(s):** `<seg:off>` `<name>`
- **Ghidra writer routine(s):** `<seg:off>` `<name>`
- **Encoding/compression:** <none | RLE | LZ-like | XOR-obfuscation | …>
- **Checksum/CRC:** <none | algorithm + covered range>

## Overview

<One paragraph: what this file stores and when the game reads/writes it.>

## Top-level layout

| Offset | Size | Type      | Name        | Notes / source                     |
|-------:|-----:|-----------|-------------|------------------------------------|
| 0x0000 | 4    | u8[4]     | magic       | expected `?? ?? ?? ??`             |
| 0x0004 | 2    | u16 (LE)  | version     |                                    |
| …      | …    | …         | …           |                                    |

> All multi-byte integers are little-endian unless noted (16-bit x86 origin).

## Records / blocks

<Describe repeated record structures, their count field, and per-record layout
as a nested table. Note bitfields with a bit map, e.g. `bit0=has_starbase`.>

## Bitfields

| Field | Bits | Meaning |
|-------|------|---------|
|       |      |         |

## Annotated hex (from a real file)

```
offset    bytes                     meaning
00000000  4A 12 ...                 magic
```

## Open questions

- <unresolved fields, suspected meanings, things to confirm against another file>

## Derived test vectors

- Link to `../vectors/<name>.*` capturing at least one real file's parsed model.
