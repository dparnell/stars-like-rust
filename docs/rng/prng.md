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
| gameplay | `lRandSeed1` / `lRandSeed2` | `Random` (`1040:16d2`) | `Randomize` **and `Randomize2`** — see below; the two write the same state |

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

### But the state is only fourteen bits wide, so it can be searched

The paragraph above is right that the state is not recorded, and an earlier
revision stopped there. It overstates the difficulty. The startup seeding is
`Randomize2((uint32_t)GetTickCount())` in `WinMain`, and **`Randomize2` writes
the same `lRandSeed1`/`lRandSeed2` as `Randomize`** — they are two seedings of
one generator, not two generators:

```c
void Randomize2(uint32_t dw) {
    b = (dw & 0x7F) ^ 0x35;   a = ((dw >> 7) & 0x7F) ^ 0x5C;
    if (b == a) a = (a + 1) & 0x7F;
    lRandSeed1 = rgPrimes[b];  lRandSeed2 = rgPrimes[a];
}
```

Both indices are seven bits into a 128-entry table, so however arbitrary the
tick count, the generator starts in one of at most **128 x 127 = 16,256
states**. The tick count itself is irrelevant; only its low fourteen bits reach
the generator. The real unknown is not the state but the *offset*: how many
draws the host consumed between seeding and the point of interest.

That makes alignment a search rather than an impossibility, and mining supplies
the constraints. `MineMinerals` walks every planet in id order and
`EstMineralsMined` draws exactly one `Random(100)` per mineral whose hundredths
remainder is non-zero — nothing else in that loop draws, and an unowned or
unpopulated planet returns before drawing at all. On a planet whose surface
change is only what it mined, the change says whether that draw rounded up. One
year pair of `all-computer-players` yields 556 draws in a known order, 242 of
them constrained.

### The search, and what it found

`cargo run --release -p stars-core --example rng_search -- <game dir> <span>
[tolerance] [selftest]` tries every seeding against every offset up to `span`.

**It found nothing.** Two passes, both negative:

| pass | span | tolerance | best score | what luck reaches |
|------|-----:|----------:|-----------:|------------------:|
| exact prefix | 50,000 | 0 | run of 39 | about 52 |
| tolerant | 3,000 | 60 wrong | 197 of 242 | about 207 |

Both are *inside* what chance produces, so neither is a signal. The comparison
matters: constraints are not coin flips — a draw with a remainder of 90 rounds
up nine times in ten — so a random stream already satisfies 68% of them, and
over 5x10^7 tries the luckiest reaches 197 on its own.

The search is not vacuous, and that is checked rather than assumed. Run it as
`... 20000 0 selftest` and it replaces the observed outcomes with ones generated
from a known seed and offset; it recovers them exactly — `best run 556 (seeds
37,71 offset 12345)`, a full match.

So the negative is real within those bounds. Three things could explain it, and
this corpus cannot separate them:

- the offset is larger than the spans searched, though the steps between seeding
  and mining — the player shuffle, thing movement, fleet movement, decay — should
  draw only tens of times;
- the draw sequence differs from the model, most likely in `CMinesOperating`,
  where being one mine out changes a remainder and so the constraint;
- too many of the 242 observed outcomes are wrong. Only 44% of the draws could
  be constrained at all, and a planet that also spent minerals can still land
  one kilotonne from the mined figure by coincidence.

**The conclusion stands, but for a sharper reason than "the state is not
stored":** it is not stored, the space it lives in is small enough to enumerate,
and enumerating it does not recover the stream from these files. Consecutive
turns from a tutorial-mode game remain the fixture that would settle it, because
they remove the offset problem entirely — the generator is re-seeded to a known
constant at the head of every turn.

### The exception: tutorial mode

`FGenerateTurn` re-seeds when bit 11 of the word at `DS:0x7ca` is set
(`10b0:0036`):

```asm
MOV  CX, 0xb
MOV  AX, [0x7ca]
SHR  AX, CL
AND  AX, 0x1
JZ   skip
MOV  AX, 0x2d2 ; MOV DX, 0x4996     ; 0x499602d2
CALLF 1040:15ea                     ; Randomize
```

**That bit is tutorial mode.** `StartTutor` (`10f8:0748`) sets it, and nothing
else does:

```asm
MOV AX, [0x7ca] ; AND AX, 0xf7ff ; OR AX, 0x800 ; MOV [0x7ca], AX
```

`[0x7ca]` is a **runtime mode word**, not a saved game setting. Its 265
references are `StartTutor` and `EndTutor`, `DestroyCurGame`, `FLoadGame`,
`BattleVCR` and `VCRDlg`, `InitProduction` and `FinishProduction`,
`NewGameWizard`, `BringUpHostDlg` and the dialogs — the things a session is
currently doing. `ReadIniSettings` and `WriteIniSettings` persist a *different*
word at `DS:0x588`, which holds the toolbar and scanner display options.

`stars-core` exposes the constant as `Rng::for_deterministic_turn`.

### Why this makes sense

Tutorial mode explains all four behaviours the bit controls, which no
game-option reading did:

| routine | effect when set |
|---------|-----------------|
| `FGenerateTurn` | re-seed the RNG from a constant every turn |
| `InitRandomPlanetList` | do not shuffle the AI's planet list |
| `FFillProdMinesAndFactories` | charge factories all three minerals |
| `QueueAiStarbases` | a different guard on whether to build |

A tutorial has to play out the same way every time it is run, which is exactly
what a fixed seed and an unshuffled planet list buy. The two production changes
are tutorial simplifications.

### What this means for fixtures

`fixtures/games/tutorial` was produced in tutorial mode, so **its turns were
generated with the RNG seeded from `0x499602d2`** — they are reproducible. It
holds only a single turn state, though, with no consecutive pair to replay
across, so it cannot be used as it stands.

The fixture worth acquiring is therefore narrower and more obtainable than
previously recorded: **consecutive turns played through the tutorial**. Anyone
with the original executable can produce them by starting the tutorial and
generating turns. That would make the whole-turn replay exact rather than
statistical, and it is the prerequisite for torpedo combat resolution.

### Two corrections

Earlier revisions of this document got this wrong twice, and the mistakes are
worth recording because both were the same kind.

First it described the bit as living in the *game flags* and being set by a
checkbox in `ExecuteButton` (`1068:14ea`). That instruction does set bit 11 —
but of `[0x588]`, the INI/UI settings word, which has nothing to do with turn
generation. The two were conflated because Ghidra names the tested global `gd`
and the assumption was never checked against the disassembly.

Second it concluded that `StartTutor` wrote "a different word" and that the
tutorial therefore was not this flag. That had the two addresses the wrong way
round.

Both were fixed by reading the actual operands rather than trusting a symbol
name, and by listing every reference to each address to see what kind of state
it holds.
