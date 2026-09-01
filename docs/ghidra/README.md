# Ghidra ↔ stars-asm symbol bridge

This folder makes the Ghidra project (`ghidra/Stars`, program `stars.2.7j.exe`)
navigable by the **real names and types** from the game's own debug symbols, so
Ghidra analysis lines up with the authoritative sources in `tmp/stars-asm`.

There are two bridges, both driven by the same verified address mapping:

- **functions** — `apply_symbols.py` names the 846 internal routines;
- **types & globals** — `apply_types.py` imports the 110 game structs and 74
  enums and lays down the 613 typed, named global variables.

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
  self-contained header that parses on its own. Regenerate with `gen-types.sh`.
- `stars-globals.csv` — `name,ghidra_addr,ne_addr,type` for all 613 NB09
  globals. Regenerate with `gen-globals.sh`.
- `stars-struct-sizes.csv` — `name,size` for the 110 structs, used by the script
  to verify the imported layouts. Regenerate with `gen-globals.sh`.
- `apply_types.py` — a Ghidra (Jython) script that parses `stars-types.h` into
  the program's data-type manager, normalises the four struct-embedded enums to
  their real 2-byte width, verifies every struct size against
  `stars-struct-sizes.csv`, and lays down all 613 globals (typed + named) at
  their addresses.
- `gen-types.sh`, `gen-globals.sh` — the generators (need the `stars-asm`
  checkout in `tmp/stars-asm`; `gen-globals.sh` also needs its built CLI).

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

Run `apply_symbols.py` **before** `apply_types.py` so the globals that live in
mixed code/data segments attach cleanly. Both scripts are idempotent and safe to
re-run. They never edit the volatile Ghidra database in git (that stays ignored
per the project guidelines); only the CSV/header/script inputs are versioned.

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
bash docs/ghidra/gen-types.sh     # -> stars-types.h
bash docs/ghidra/gen-globals.sh   # -> stars-globals.csv, stars-struct-sizes.csv
```

`gen-types.sh` also leaves the header valid, single-namespace C; you can
sanity-check it with `clang -std=c11 -fsyntax-only docs/ghidra/stars-types.h`
(sizes still come from Ghidra's 16-bit data organisation, not the host clang).

## Notes & follow-ups

- The header maps `int32_t`/`uint32_t` onto `long` (4 bytes) and Win16 handles
  onto 2-byte words; this matches the program's Ghidra data organisation
  (`long`=4, pointer=4, `short`=2), which `apply_types.py` re-checks per struct.
- `enums.h`'s standalone Windows `MessageBoxResult` enum and a duplicate
  `VictoryCondition` are dropped by `gen-types.sh` because they collide in C's
  single enumerator namespace; every game enum is retained.
- `symbols publics` (additional public labels) is not yet imported — a possible
  future addition using the same selector mapping.
