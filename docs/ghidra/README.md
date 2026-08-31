# Ghidra ↔ stars-asm symbol bridge

This folder makes the Ghidra project (`ghidra/Stars`, program `stars.2.7j.exe`)
navigable by the **real function names** from the game's own debug symbols, so
Ghidra analysis lines up with the authoritative sources in `tmp/stars-asm`.

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

- `stars-symbols.csv` — `name,ghidra_addr,ne_addr` for all 846 NB09 functions.
- `apply_symbols.py` — a Ghidra (Jython) script that reads the CSV and renames
  each function (creating a function/label if none exists yet).

## Applying the names

In the CodeBrowser that has `stars.2.7j.exe` open:

1. **Window → Script Manager**, click **Manage Script Directories**, add this
   `docs/ghidra` folder, and refresh.
2. Run **`apply_symbols.py`** and, when prompted, pick `stars-symbols.csv`.
3. It prints a summary (renamed / created / labelled). The project now shows
   real names, e.g. `FLoadLogFile`, `SetFileXorStream`, `LphuldefFromId`.

The script is idempotent and safe to re-run. It never edits the volatile Ghidra
database in git (that stays ignored per the project guidelines); only the CSV +
script are versioned.

## Regenerating the CSV

Requires Go and the stars-asm checkout in `tmp/stars-asm`:

```sh
cd tmp/stars-asm
go build -o ./dist/stars-asm .
{ echo "name,ghidra_addr,ne_addr"; \
  ./dist/stars-asm symbols functions \
  | perl -ne 'if(/^\s*([0-9a-f]{4}):([0-9a-f]{4})\s+(\S+)/){
        my($s,$o,$n)=(hex($1),$2,$3); my $sel=0x1000+($s-1)*8;
        printf "%s,%04x:%s,%04x:%s\n",$n,$sel,$o,$s,$o; }'; \
} > ../../docs/ghidra/stars-symbols.csv
```

## Follow-ups

- stars-asm also exposes `symbols globals` / `symbols publics`; the same
  selector mapping applies, so data globals can be added to the CSV later.
- Applying struct/enum types (`RTPLANET`, `SHDEF`, …) from stars-asm to the
  Ghidra data-type manager is a further enhancement.
