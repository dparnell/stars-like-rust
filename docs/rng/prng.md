# Stars! core PRNG (`FUN_1038_8a58`)

**Status:** recovered and confirmed directly from `STARS!.EXE` (Ghidra), and
already implemented byte-for-byte as `stars_formats::StarsRng`
(`crates/stars-formats/src/crypt.rs`). This is the single generator that drives
both the file **encryption keystream** and (expected) the gameplay simulation.

## Where it lives in the binary

| function | address (`seg:off`) | role | our code |
|----------|---------------------|------|----------|
| `FUN_1038_8a58` | `1038:8a58` | the PRNG step: advance both sub-generators, return `s1 - s2` (32-bit, `dx:ax`) | `StarsRng::next_u32` |
| `FUN_1038_89e4` | `1038:89e4` | seed the two sub-generators from a table and prime the pump | `FileHeader::init_rng` / `StarsRng::new` |
| `FUN_1038_8b20` | `1038:8b20` | XOR stream cipher over a block payload | `StarsRng::apply` |
| seed table | `DS:0x7fe0` | 64-word seed/prime table, **built at runtime** (lies past the data segment's file image, so it is *not* readable statically) | `crypt::PRIMES` |

DS is the auto data segment (NE segment 36, selector `0x1118`); its globals
appear as `DAT_1118_*`. The two 32-bit generator-state globals are
`DAT_1118_237e` (`s1`) and `DAT_1118_2382` (`s2`).

## The algorithm (decompiled)

`FUN_1038_8a58` decompiles to:

```c
int FUN_1038_8a58(void) {
    // s1 = DAT_1118_237e, s2 = DAT_1118_2382 (both 32-bit)
    long a = (s1 / -0xd1a4) * 0x7fffffab + s1 * 0x9c4e;   // -0xd1a4 = -53668
    if (a < 0) a += 0x7fffffab;                            //  0x7fffffab = 2147483563
    long b = (s2 / -0xce26) * 0x7fffff07 + s2 * 0x9ef4;   // -0xce26 = -52774
    if (b < 0) b += 0x7fffff07;                            //  0x7fffff07 = 2147483399
    s1 = a; s2 = b;
    return (int)a - (int)b;                                // full 32-bit dx:ax
}
```

This is **L'Ecuyer's combined multiple-recursive generator** (CACM 1988), the
two-component form also known from `ran2`. Each component is a Lehmer
(Park–Miller) generator evaluated with **Schrage's method**:

```
s' = a*(s % q) - r*(s / q);   if (s' < 0) s' += m
```

with `m = a*q + r`. The binary writes the algebraically-identical
`s*a - (s/q)*m` (because `a*(s%q) - r*(s/q) = a*s - (a*q+r)*(s/q)`), and the
runtime uses 32-bit wrap plus the `+= m` correction as the modulo.

| component | `a` | `q` | `r` | `m` |
|-----------|-----|-----|-----|-----|
| 1 (`s1`)  | `40014` (`0x9c4e`) | `53668` (`0xd1a4`) | `12211` | `2147483563` (`0x7fffffab`) |
| 2 (`s2`)  | `40692` (`0x9ef4`) | `52774` (`0xce26`) | `3791`  | `2147483399` (`0x7fffff07`) |

`r` is not spelled out in the decompilation; it is recovered from
`r = m - a*q` (`2147483563 - 40014*53668 = 12211`,
`2147483399 - 40692*52774 = 3791`) and matches
`crates/stars-formats/src/crypt.rs` exactly.

The combined output is `s1 - s2` as a wrapping 32-bit value. The cipher consumes
the whole 32-bit word (`ax` = low 16 bits, `dx` = high 16 bits); the simulation
typically uses only the low bits / a modulo of the result (to be confirmed in
Step 3).

## Seeding (`FUN_1038_89e4`)

Called with header-derived arguments (see the writer/reader below):

```
FUN_1038_89e4(lidGameLo, salt = word0x0c >> 5, turnWord, iPlayer = word0x0c & 0x1f, crippled = (byte0x0f & 0x10) >> 4)
```

Inside:

```c
lo = turnWord & 0x1f;  hi = (turnWord & 0x3e0) >> 5;
if ((turnWord & 0x400) == 0) hi += 0x20; else lo += 0x20;   // pick which half of the table
s1 = (int16)seedTable[lo];   // sign-extended to 32 bits
s2 = (int16)seedTable[hi];
rounds = ((lidGameLo & 3) + 1) * ((crippled & 3) + 1) * ((iPlayer & 3) + 1) + <arg6>;
while (rounds-- > 0) FUN_1038_8a58();   // warm-up / prime the pump
```

So the two seeds are picked from the runtime-built table by two 5-bit indices,
and the generator is advanced a header-dependent number of warm-up rounds before
first use. `crates/stars-formats/src/crypt.rs` + `header.rs` implement an
equivalent seeding (indexing `PRIMES`) that reproduces real files **byte-for-byte**
across every fixture and turn; the exact runtime table contents are only
observable at run time (the table is in BSS at `DS:0x7fe0`, past the segment's
file image).

## Consumption points

- **File cipher** — `FUN_1038_8b20` (`StarsRng::apply`): the keystream runs
  continuously across all encrypted blocks of a file, seeded once from the
  header (`rtBOF`, type 8). See `../formats/blocks.md` and `file-io-map.md`.
- **Gameplay (Step 3, pending)** — the same `FUN_1038_8a58` is expected to drive
  minerals, movement, combat, and events. When the call sites are mapped,
  `crates/stars-core`'s RNG should reuse/mirror `StarsRng` and a reference
  sequence vector goes under `../vectors/`.
