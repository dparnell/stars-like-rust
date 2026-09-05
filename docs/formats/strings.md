# Format: Stars! packed strings

- **Status:** **decoded & verified** against real files; read-only decoder
  implemented in `stars-formats::strings` (`decode_field` / `decode_packed`).
- **Source of the algorithm:** Rick Steeves' TotalHost `StarsBlock.pm`
  (`decodeBytesForStarsString` + `nibbleToChar`), referenced by the user. The
  scheme was then confirmed by decoding the built-in race names from the real
  `fixtures/r/*.r1` files.
- **Used by:** race singular/plural names (see `race-r.md`); the same encoding is
  used for other user-supplied text (player/fleet names, messages) in `.mN` /
  `.hN` / `.xN`, which are not yet wired up.

## Overview

Stars! does **not** store user text as plain ASCII. To save space it packs the
most common letters into a single 4-bit **nibble** and escapes everything else.
A string is a length-prefixed blob:

```
[ len:u8 ][ len bytes of packed data ]
```

The `len` byte counts the packed **bytes** that follow (not the number of
decoded characters), and is not itself part of the text. The packed bytes are
expanded into a stream of nibbles — **high nibble first** for each byte — and
the nibble stream is decoded left-to-right.

## Nibble decoding

Read the leading nibble `n` at the current position:

| leading nibble `n` | action                                                                  | nibbles consumed |
|--------------------|-------------------------------------------------------------------------|:----------------:|
| `0x0 .. 0xA`       | emit `ENCODES_ONE[n]`                                                    | 1                |
| `0xB`              | emit `ENCODES_B[next]`                                                   | 2                |
| `0xC`              | emit `ENCODES_C[next]`                                                   | 2                |
| `0xD`              | emit `ENCODES_D[next]`                                                   | 2                |
| `0xE`              | emit `ENCODES_E[next]`                                                   | 2                |
| `0xF`              | emit a **literal byte** `= (nibble[t+2] << 4) | nibble[t+1]` (low first) | 3                |

Where `next` is the immediately following nibble used as a 0–15 table index.

### Lookup tables

```
ENCODES_ONE = " aehilnorst"        # index 0..=10 (space + 10 most common letters)
ENCODES_B   = "ABCDEFGHIJKLMNOP"   # 16 entries
ENCODES_C   = "QRSTUVWXYZ012345"   # 16 entries
ENCODES_D   = "6789bcdfgjkmpquv"   # 16 entries
ENCODES_E   = "wxyz+-,!.?:;'*%$"   # 16 entries
```

So the whole printable set is reachable: the ten most common lowercase letters
(and space) cost one nibble; the rest of the alphabet, digits and punctuation
cost two nibbles via the `B`–`E` tables; and any other byte falls back to the
`F` literal-byte escape (three nibbles).

### Boundary handling

The nibble stream always has an even length (two nibbles per byte), so a
two-nibble (`B`–`E`) or three-nibble (`F`) sequence can run off the end. The
reference treats an out-of-range nibble read as `0`; the Rust port matches this
(`nibbles.get(i).unwrap_or(0)`), and only emits an `F` literal when at least the
two following nibbles are notionally present, mirroring `StarsBlock.pm`'s
`unless ($t+2 > length)` guard.

## Worked example (Humanoid singular name)

The decrypted Humanoid race record ends with the singular name field:

```
06 B7 DE DB 16 74 D6      # len=6, then 6 packed bytes
```

Skipping the length byte and expanding the six bytes to nibbles gives the stream
`B 7 D E D B 1 6 7 4 D 6`, decoded left-to-right:

- `B 7` → `ENCODES_B[7]` = `H`
- `D E` → `ENCODES_D[14]` = `u`
- `D B` → `ENCODES_D[11]` = `m`
- `1`   → `ENCODES_ONE[1]` = `a`
- `6`   → `ENCODES_ONE[6]` = `n`
- `7`   → `ENCODES_ONE[7]` = `o`
- `4`   → `ENCODES_ONE[4]` = `i`
- `D 6` → `ENCODES_D[6]` = `d`

→ **"Humanoid"**. The plural field is the same with a trailing `s`.

## Encoding

Not implemented. Round-tripping a whole file goes through the byte-exact cipher
container in `stars-formats::file`, so re-encoding individual strings is never
needed for write-back. (TotalHost's own `charToNibble` is marked *untested*.) If
a text editor is ever needed, an encoder can be added and validated by
re-encrypting and comparing against the original bytes.

## Derived test vectors

- `crates/stars-formats/src/strings.rs` — unit tests for the one-nibble table,
  the `B` escape table, and the `F` literal-byte escape.
- `crates/stars-formats/tests/race_files.rs::default_races_decode_names` —
  decodes the singular/plural names of all six built-in default races from real
  `.rN` files and asserts the exact text.

## User strings: the literal escape

A name the **player typed** — a fleet's name, and the name in a `.xN` rename
order — is written by `WriteRtString` (`1070:87b4`), which uses the same
`[length][packed bytes]` field with one escape.

The packed form is given a budget of 31 bytes. When it does not fit, the game
writes a **length byte of `0`** and then the string itself, NUL-terminated. So
a reader has to check for that byte before decoding nibbles, and a writer has
to make the same choice:

```c
cOut = 0x1f;
if (FCompressUserString(lpsz, rgb + 1, &cOut) == 0) {
    strcpy(rgb + 1, lpsz);        // did not fit
    rgb[0] = 0;
    cOut = strlen(lpsz) + 1;      // including the NUL
} else {
    rgb[0] = (uint8_t)cOut;
}
WriteRt(rtString, cOut + 1, rgb);
```

`decode_user_string` and `encode_user_string` implement exactly that. The buffer
is 33 bytes, so a literal name holds at most 31 characters.

Nothing in the fixtures uses the escape — no captured game has a renamed fleet —
so it is recovered from the binary rather than fixture-verified. The packed half
is the same codec verified on 230 real names; see `fleet.md`.
