---
name: ghidra-bridge
description: Apply or regenerate the Ghidra ↔ stars-asm NB09 bridge (function names, struct/enum types, global variables, function prototypes) for binary/stars.2.7j.exe. Use before any Ghidra analysis of Stars!, when functions show up as FUN_xxxx_yyyy or globals/params are untyped, or when the CSV/header inputs under docs/ghidra need regenerating.
---

# Ghidra ↔ stars-asm NB09 bridge

`binary/stars.2.7j.exe` is the **debug build** of Stars! 2.7j and carries
CodeView **NB09** symbols, but Ghidra's NE loader does not parse them — so
without this bridge every internal routine is an anonymous `FUN_<sel>_<off>`
and nothing is typed. Full background: `docs/ghidra/README.md`.

## Address mapping

stars-asm reports 1-based NE logical `segment:offset`; Ghidra assigns synthetic
selectors linearly:

```
ghidra_selector = 0x1000 + (ne_segment - 1) * 8
ghidra_offset   = ne_offset            (unchanged)
```

So NE `0001:0014` → Ghidra `1000:0014`, NE `0023:1a96` → `1110:1a96`.

## Applying (run in this order — later scripts depend on earlier ones)

In the CodeBrowser with `stars.2.7j.exe` open: **Window → Script Manager →
Manage Script Directories**, add `docs/ghidra`, refresh, then run:

1. `apply_symbols.py` (pick `stars-symbols.csv`) — names the 846 NB09 functions.
2. `apply_types.py` (pick `stars-types.h`) — imports 110 structs + 74 enums,
   verifies every struct size, lays down the 613 typed/named globals.
3. `apply_signatures.py` (pick `stars-signatures.csv`) — types return values and
   parameters for all 846 functions, preserving the recovered calling
   convention (`__cdecl16far` / `__pascal16far`).

All three are idempotent and safe to re-run. They only touch the (git-ignored)
Ghidra database; the versioned inputs live in `docs/ghidra/`.

Quick check that the bridge is applied: a known routine such as `FLoadLogFile`
or `GetIniWinRc` should be named and fully typed rather than `FUN_*` with
`undefined2` parameters.

## Regenerating the inputs

Needs the `sirgwain/stars-asm` checkout in `tmp/stars-asm` and Go once:

```sh
cd tmp/stars-asm && go build -o ./dist/stars-asm .
```

Then from the repo root:

```sh
bash docs/ghidra/gen-types.sh       # -> stars-types.h
bash docs/ghidra/gen-globals.sh     # -> stars-globals.csv, stars-struct-sizes.csv
bash docs/ghidra/gen-signatures.sh  # -> stars-signatures.csv
```

`stars-symbols.csv` is regenerated with the `stars-asm symbols functions` +
perl pipeline in `docs/ghidra/README.md`. Sanity-check the header with
`clang -std=c11 -fsyntax-only docs/ghidra/stars-types.h`.

## Traps that have already bitten this project

- **`CParser(dtm)` commits nothing.** The 1-argument constructor defaults
  `storeDataType=false`, so parsing "succeeds" while adding zero types and every
  struct-typed global becomes untypable. Always use `CParser(dtm, True, None)`.
- **Bitfields abort decompilation.** Ghidra's decompiler refuses to form a
  pointer whose referent is a bitfield (`Pointer reference data-type may not be
  a bitfield`) and aborts the whole function. `gen-types.sh` therefore runs
  `strip-bitfields.py` to collapse every bitfield into a plain scalar of the
  same storage unit, keeping the layout in a comment. Never re-introduce raw
  bitfields into `stars-types.h`.
- **Enum widths.** Four struct-embedded enums must be normalised to 2 bytes;
  `apply_types.py` does this and then verifies each struct against
  `stars-struct-sizes.csv` (`GAME`=64, `PLANET`=56, `FLEET`=124, `THING`=18, …).
- **Callback parameters.** Four enumerator functions take a function-pointer
  argument Ghidra's `FunctionSignatureParser` cannot express;
  `apply_signatures.py` degrades just that argument to a same-depth `void *`
  and reports it as `typed (callback->ptr)`. That is expected, not a failure.

## Recording what you find

Findings from Ghidra go into `docs/` (see the `format-spec` skill for format
work), always citing the Ghidra address and, where relevant, the NB09 struct
field name — NB09 names outrank older tool-derived names from TotalHost,
starsapi or stars-4x.
