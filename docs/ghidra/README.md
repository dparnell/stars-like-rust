# Ghidra ↔ stars-asm symbol bridge

This folder makes the Ghidra project (`ghidra/Stars`, program `stars.2.7j.exe`)
navigable by the **real names and types** from the game's own debug symbols, so
Ghidra analysis lines up with the authoritative sources in `tmp/stars-asm`.

There are three bridges, all driven by the same verified address mapping:

- **functions** — `apply_symbols.py` names the 846 internal routines;
- **types & globals** — `apply_types.py` imports the 110 game structs and 74
  enums and lays down the 613 typed, named global variables;
- **function signatures** — `apply_signatures.py` types every function's
  prototype (return type + each parameter) for all 846 routines.

## Why this is needed

`binary/stars.2.7j.exe` is the **debug build** of Stars! 2.7j and carries
CodeView **NB09** symbols (function names, structs, globals). Ghidra's NE loader
does **not** parse NB09, so every internal routine loads as an anonymous
`FUN_<selector>_<offset>`. Only the NE **export** table entry points
(`RACEWIZARDDLG1`, `TRANSFERDLG`, `ABOUT`, …) get names automatically.

`binary/stars.2.7j.exe` is byte-identical to the input analysed by
[`sirgwain/stars-asm`](https://github.com/sirgwain/stars-asm)
(`tmp/stars-asm/dasm/input/stars.exe` — identical SHA-256), which *does* parse
NB09. This bridge exports the 846 named functions from stars-asm and maps them
onto Ghidra addresses.

## The address mapping (verified)

stars-asm reports addresses as **1-based NE logical `segment:offset`**
(e.g. `0001:0014`). Ghidra loads each NE segment as a block with a synthetic
protected-mode selector, assigned linearly:

```
ghidra_selector = 0x1000 + (ne_segment - 1) * 8
ghidra_offset   = ne_offset            (unchanged)
```

So NE `0001:0014` → Ghidra `1000:0014`, NE `000d:0918` → `1060:0918`,
NE `0023:1a96` → `1110:1a96`. Verified against this exact binary: the mapped
addresses land on the corresponding `FUN_*` entry points with matching bodies
across low, mid and high segments.

## Files

### Functions (names)

- `stars-symbols.csv` — `name,ghidra_addr,ne_addr` for all 846 NB09 functions.
- `apply_symbols.py` — a Ghidra (Jython) script that reads the CSV and renames
  each function (creating a function/label if none exists yet).

### Types & globals (structs, enums, global variables)

- `stars-types.prelude.h` — a small hand-written prelude: fixed-width integer
  typedefs (sized for the 16-bit target: `int32_t`→`long`=4, handles→2-byte
  words) plus the few Win16 shims the game types need (`POINT`, `RECT`, the
  `Hxxx` handles, `COLORREF`).
- `stars-types.h` — **generated**: the prelude + forward declarations + the 74
  game enums (`enums.h`) + the 110 game structs (`structs.h`), stitched into one
  self-contained header that parses on its own. The prelude declares
  `#pragma pack(1)` so the byte-packed 16-bit game structs import at their true
  size (e.g. `THING`=18, not 24). Regenerate with `gen-types.sh`.
- `stars-globals.csv` — `name,ghidra_addr,ne_addr,type` for all 613 NB09
  globals. Regenerate with `gen-globals.sh`.
- `stars-struct-sizes.csv` — `name,size` for the 110 structs, used by the script
  to verify the imported layouts. Regenerate with `gen-globals.sh`.
- `apply_types.py` — a Ghidra Python script (runs under both Jython and
  PyGhidra) that parses `stars-types.h` into the program's data-type manager,
  normalises the four struct-embedded enums to their real 2-byte width, verifies
  every struct size against `stars-struct-sizes.csv`, and lays down all 613
  globals (typed + named) at their addresses. It drives Ghidra's `CParser` with
  `storeDataType=True` (see the gotcha under **Notes** below) so the parsed
  types are actually committed to the program.
- `gen-types.sh`, `gen-globals.sh` — the generators (need the `stars-asm`
  checkout in `tmp/stars-asm`; `gen-globals.sh` also needs its built CLI).

### Function signatures (return type + parameter types)

- `stars-signatures.csv` — `name,ghidra_addr,ne_addr,signature` for all 846 NB09
  functions. `signature` is the full C prototype (e.g.
  `void GetIniWinRc(char *szSection, char *szIniFile, StringId ids, WN *pwn)`)
  and is **quoted** because it contains commas. Every type it names is defined in
  `stars-types.h`. Regenerate with `gen-signatures.sh`.
- `apply_signatures.py` — a Ghidra Python script that, for each row, parses the
  prototype with Ghidra's `FunctionSignatureParser` (resolving the named types
  against the program's data-type manager) and applies it with
  `ApplyFunctionSignatureCmd`, **preserving the recovered calling convention**
  (`__cdecl16far`/`__pascal16far`) and leaving the (already-applied) NB09 name
  untouched. Run `apply_symbols.py` and `apply_types.py` first.
- `gen-signatures.sh` — the generator (needs the built `stars-asm` CLI).

## Applying the names and types

In the CodeBrowser that has `stars.2.7j.exe` open:

1. **Window → Script Manager**, click **Manage Script Directories**, add this
   `docs/ghidra` folder, and refresh.
2. Run **`apply_symbols.py`** and, when prompted, pick `stars-symbols.csv`. It
   prints a summary (renamed / created / labelled). The project now shows real
   names, e.g. `FLoadLogFile`, `SetFileXorStream`, `LphuldefFromId`.
3. Run **`apply_types.py`** and, when prompted, pick `stars-types.h` (the two
   CSVs beside it are found automatically). It parses the header into the
   data-type manager, prints the struct-size verification, and reports how many
   globals were typed/named (e.g. `game`, `rghuldef`, `lpPlanets`).
4. Run **`apply_signatures.py`** and, when prompted, pick `stars-signatures.csv`.
   It types every function's return value and parameters and prints a summary
   (`typed (return + params)`, `typed (callback->ptr)`, …). `GetIniWinRc` then
   reads `void GetIniWinRc(char *szSection, char *szIniFile, StringId ids, WN
   *pwn)` instead of `undefined2`/`int` parameters.

Run the three scripts **in order** — `apply_symbols.py` (names) →
`apply_types.py` (types + globals) → `apply_signatures.py` (prototypes) — so the
functions exist and every named type is resolvable before the prototypes are
applied. All three scripts are idempotent and safe to re-run. They never edit the
volatile Ghidra database in git (that stays ignored per the project guidelines);
only the CSV/header/script inputs are versioned.

## Regenerating the inputs

All inputs are regenerable from the `stars-asm` checkout in `tmp/stars-asm`
(needs Go to build its CLI once):

```sh
cd tmp/stars-asm && go build -o ./dist/stars-asm .   # once
```

**Functions** (`stars-symbols.csv`):

```sh
cd tmp/stars-asm
{ echo "name,ghidra_addr,ne_addr"; \
  ./dist/stars-asm symbols functions \
  | perl -ne 'if(/^\s*([0-9a-f]{4}):([0-9a-f]{4})\s+(\S+)/){
        my($s,$o,$n)=(hex($1),$2,$3); my $sel=0x1000+($s-1)*8;
        printf "%s,%04x:%s,%04x:%s\n",$n,$sel,$o,$s,$o; }'; \
} > ../../docs/ghidra/stars-symbols.csv
```

**Types & globals** (from the repo root):

```sh
bash docs/ghidra/gen-types.sh       # -> stars-types.h
bash docs/ghidra/gen-globals.sh     # -> stars-globals.csv, stars-struct-sizes.csv
bash docs/ghidra/gen-signatures.sh  # -> stars-signatures.csv
```

`gen-types.sh` also leaves the header valid, single-namespace C; you can
sanity-check it with `clang -std=c11 -fsyntax-only docs/ghidra/stars-types.h`
(sizes still come from Ghidra's 16-bit data organisation, not the host clang).

## Notes & follow-ups

- **`CParser` gotcha (why an early run imported nothing):** Ghidra's
  single-argument `CParser(dtm)` constructor delegates to
  `new CParser(dtm, /*storeDataType=*/false, null)`, and the parser only calls
  `dtMgr.addDataType(...)` when `storeDataType` is true. So `CParser(dtm).parse(...)`
  *succeeds* but commits **nothing** — every struct shows up MISSING and all
  struct-typed globals become untypable. `apply_types.py` therefore always uses
  the three-argument `CParser(dtm, True, None)` form (the same one
  `CParserUtils.parseHeaderFiles` uses for "Parse C Source → program").
  Verified on Ghidra 12.1.3: 1-arg → 0 structs; 3-arg → 124 structs / 72 enums.
- The header maps `int32_t`/`uint32_t` onto `long` (4 bytes) and Win16 handles
  onto 2-byte words, and declares `#pragma pack(1)`; this matches the program's
  Ghidra data organisation (`long`=4, `short`=2) with no padding, so the
  org-invariant record structs import at their exact sizes (`GAME`=64,
  `PLANET`=56, `FLEET`=124, `THING`=18, …), which `apply_types.py` re-checks per
  struct.
- `enums.h`'s standalone Windows `MessageBoxResult` enum and a duplicate
  `VictoryCondition` are dropped by `gen-types.sh` because they collide in C's
  single enumerator namespace; every game enum is retained.
- **`DRAWITEMSTRUCT`:** three owner-draw routines take a `DRAWITEMSTRUCT *`, a
  standard Win16 type that is not one of the game's own NB09 structs. The
  prelude therefore declares a minimal 26-byte Win16 layout for it (matching the
  game's own `cbDRAWITEMSTRUCT = 26` enum constant); it is the only non-game
  type any signature names.
- **Function-pointer parameters:** four enumerator functions (`EnumLogRts`,
  `LpflFindClosestEnum`, `LpplFindClosestEnum`, `LpplFindBestEnum`) take a
  callback argument (e.g. `int16_t (**pfn)(FLEET *, FLEET *)`) that Ghidra's
  `FunctionSignatureParser` cannot express. `apply_signatures.py` collapses just
  that argument to a same-depth `void *` pointer so the return type and the
  other parameters are still typed faithfully; these are reported as
  `typed (callback->ptr)`.
- Verified offline against Ghidra 12.1.3 (PyGhidra): `stars-types.h` parses into
  a data-type manager and all **846** prototypes parse via
  `FunctionSignatureParser` (842 directly + the 4 callback functions via the
  `void *` fallback).
- `symbols publics` (additional public labels) is not yet imported — a possible
  future addition using the same selector mapping.
