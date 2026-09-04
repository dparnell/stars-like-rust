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


## Two generators, and why a turn cannot be replayed

There are **two** independent generators running the same algorithm on
different state.

| generator | state | driven by | seeded by |
|-----------|-------|-----------|-----------|
| file cipher | `DAT_1118_237e` / `DAT_1118_2382` | `FUN_1038_8a58` | the file header, on every block stream |
| gameplay | `lRandSeed1` / `lRandSeed2` | `Random` (`1040:16d2`) | `Randomize` |

The cipher's seeding is fully recovered — it is why the fixtures decode at all.
The gameplay generator is the one every simulation formula draws from, and its
seeding is the problem.

### `Randomize` is not called per turn

`Randomize` has seven callers: `CommandHandler` (program start),
`GenNewGameFromFile` and `CreateTutorWorld` (game creation),
`FSerialAndEnvFromSz` and `FormatSerialAndEnv` (serial-number handling),
`RandomSeedDlg`, and `FGenerateTurn`.

`FGenerateTurn` (`10b0:0000`) calls it in one place only, at the very top:

```c
DestroyCurGame();
if ((gd.flags >> 0xb & 1) != 0) {
    Randomize(0x499602d2);
}
```

So in an ordinary game **the gameplay generator is never re-seeded when a turn
is generated**. Its state at the head of any turn is whatever the host process
happened to leave it at, which depends on every draw taken since the program
started — including draws made while the host was doing something else
entirely.

**That state is not written to any save file.** It follows that a recorded turn
cannot be replayed draw-for-draw from the fixtures, however completely the
formulas are recovered. This is a property of the game, not a gap in the
reverse engineering.

### The exception: bit 11 of the game flags

A game with bit 11 set restarts the generator from the fixed constant
`0x499602d2` at the head of every turn, which makes turns fully reproducible.
`stars-core` exposes this as `Rng::for_deterministic_turn`.

The same flag turns up in three other places already recovered, which is a
useful cross-check on what it means: `InitRandomPlanetList` skips the AI's
planet shuffle when it is set, `FFillProdMinesAndFactories` charges factories
all three minerals rather than germanium alone, and `QueueAiStarbases` changes
its guard. It reads as a "reproducible / test game" mode.

### What sets it

Exactly one instruction in the binary writes bit 11 into the game flags word at
`DS:0x588`, in `ExecuteButton` (`1068:14ea`):

```asm
SHL  AX, CL                      ; AX = 1 << (checkbox index)
XOR  word ptr [0x51aa], AX       ; toggle that checkbox
MOV  AX, [0x588]
AND  AX, 0x800
JNZ  done                        ; already set: nothing to do
MOV  AX, 1 ; SHL AX, CL
AND  AX, word ptr [0x51aa]
JZ   done                        ; the checkbox went *off*: nothing to do
OR   word ptr [0x588], 0x800     ; set it
CALLF 14f8:0274                  ; and tell the user
```

So it is a **checkbox in a dialog**. Ticking it sets the flag and pops a
message; the flag is **never cleared anywhere in the binary**, so the choice is
one-way once made. Which checkbox it is has not been identified — the handler
works from a bit index in a local, so the label is not reachable from this code
alone.

`StartTutor` (`10f8:074e`) also ORs `0x800`, but into `[0x7ca]`, a different
word — the settings/INI flags, not the game flags. The tutorial is therefore
*not* this flag, and `fixtures/games/tutorial` does not supply what is needed.

### What that means for the fixtures

None of the fixture games sets it, and the `.xy` game-info block does not
obviously carry it either: the 64-byte block differs in shape between the
tutorial and the played games, and no word in it has bit 11 set in any fixture.

A game created with the checkbox ticked would be the single most valuable
fixture this project could acquire. It would make the whole-turn replay exact
rather than statistical, and it is the prerequisite for torpedo combat
resolution, which needs the RNG in the right state to score at all.

### A caution on naming it

An earlier note here called this a "reproducible / test game" mode. That reads
well against three of its four known effects — fixed RNG seed per turn, no AI
planet shuffle, an altered `QueueAiStarbases` guard — but the fourth, charging
factories all three minerals rather than germanium alone
(`FFillProdMinesAndFactories`), has nothing to do with determinism. The flag is
better described by what it does than by a guessed name.
