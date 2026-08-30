# Format: block framing — the shared container of every Stars! file

- **Status:** framing **implemented & round-trip tested**; payload encryption + per-format records **pending**
- **Original files analysed:** none yet (no real fixtures captured — see `fixtures/README.md`)
- **Ghidra reader routine(s):** file bytes are read with the Win16 `_LREAD` import (present in `STARS!.EXE`); the in-memory block loop is not yet pinned to a `seg:off` (see *RE status* below)
- **Ghidra writer routine(s):** Win16 `_LWRITE` import present; block writer not yet pinned
- **Encoding/compression:** framing is plaintext; non-header block payloads are obfuscated with the Stars! stream cipher (**not yet recovered**)
- **Checksum/CRC:** unknown / to be confirmed

## Overview

Every Stars! on-disk file (`.xy`, `.mN`, `.hN`, `.xN`, `.rN`, `.hst`) is a flat
sequence of **blocks**. A block is a 16-bit little-endian header word followed
by its payload. The header word packs a 6-bit *type id* and a 10-bit *payload
size*. The game reads the whole file into memory (via `_LREAD`), walks the block
list, and decrypts/decodes each payload; saving is the reverse (`_LWRITE`).

This spec covers the **framing** (fully implemented in `stars-formats::block`).
The payload **encryption** and the **per-format record layouts** are separate,
still-open sub-tasks tracked here and in the per-format specs.

## Top-level layout (per block)

| Offset | Size     | Type     | Name    | Notes / source                                  |
|-------:|---------:|----------|---------|-------------------------------------------------|
| 0x0000 | 2        | u16 (LE) | header  | `size = header & 0x03FF`, `type = header >> 10` |
| 0x0002 | `size`   | u8[]     | payload | verbatim bytes; encrypted for non-header blocks |

> All multi-byte integers are little-endian (16-bit x86 origin).

A file is simply blocks concatenated back-to-back until EOF; there is no
top-level count. The framing therefore round-trips byte-for-byte:
`join_blocks(split_blocks(bytes)) == bytes`, which holds for **real** files too
because payloads are copied verbatim without decoding.

### Header word bit layout

```
 bit: 15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
      +--------------+  +-----------------------------+
      |   type id    |  |        payload size         |
      |   (6 bits)   |  |          (10 bits)          |
      +--------------+  +-----------------------------+
```

- `type id` range: `0..=63`
- `payload size` range: `0..=1023` bytes

Worked example: a header block (`type = 8`) with a 4-byte payload encodes as
`(8 << 10) | 4 = 0x2004`, written little-endian as `04 20`, then the 4 payload
bytes. (Covered by the `header_bit_layout_is_type_hi6_size_lo10` unit test.)

## Records / blocks

The **first** block is always the file-header block, [`type id 8`], and is the
one block whose payload is stored **unencrypted**. It carries the values needed
to seed the stream cipher for every following block. Its exact field layout is
documented in `player-m.md` / `xy.md` once recovered; the well-known shape is:

- magic bytes `"J3J3"`
- game id
- file version (packed major/minor)
- turn number
- player number + flags
- salt used to derive the cipher seed

> ⚠️ The header field offsets above are the community-documented shape and are
> **not yet verified** against this binary or a real file. Do not rely on them
> until confirmed.

## Bitfields

| Field        | Bits | Meaning                              |
|--------------|------|--------------------------------------|
| header.type  | 15–10| block type id (`0..=63`)             |
| header.size  | 9–0  | payload length in bytes (`0..=1023`) |

## Annotated hex (synthetic; real-file capture pending)

```
offset    bytes                     meaning
00000000  04 20                     header: type=8 (FileHeader), size=4
00000002  4A 33 4A 33               payload: "J3J3"
```

## Encryption (pending recovery)

Non-header block payloads are XOR-obfuscated with a keystream from the Stars!
PRNG, seeded from the file-header block's game id / turn / player / salt. This
same PRNG family also drives gameplay RNG (Step 3), so recovering it unlocks
both. Recovery approach:

1. Pin the in-memory block loop in Ghidra and the routine that transforms a
   block payload right after `_LREAD` (constants, shift amounts, seed mixing).
2. Capture a real `.m1`/`.xy` file as a fixture; confirm the file-header layout
   and salt derivation against its bytes.
3. Implement `decrypt_blocks` / `encrypt_blocks` and assert they are mutual
   inverses **and** that a real file decrypts to sane records, then re-encrypts
   byte-for-byte.

## RE status / notes

- `STARS!.EXE` imports the Win16 file primitives `_LREAD` and `_LWRITE`,
  confirming files are read/written as raw byte buffers and parsed in memory
  (consistent with the block model).
- Ghidra's call graph does **not** resolve callers across the NE far-call
  import thunks, so the loader/decrypt routine could not be reached by
  caller-tracing this pass; it needs to be located by other means (e.g. finding
  the code that compares the `"J3J3"` magic / references the header constants).

## Open questions

- Exact file-header block field offsets and the salt → seed derivation.
- The PRNG constants and keystream application (per-byte vs per-word; whether the
  size field or type participates in seeding).
- Whether any block or the whole file carries a checksum/CRC.
- The full block-type registry (only `8` = file header is anchored so far).

## Derived test vectors

- Framing: covered by unit tests in `stars-formats::block` (round-trip,
  max-size payload, truncated-header/payload errors).
- File-level vectors await a real fixture in `fixtures/` (see
  `../vectors/README.md`).
