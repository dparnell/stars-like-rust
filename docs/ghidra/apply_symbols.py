# Apply Stars! 2.7j NB09 function names to the open Ghidra program.
#
# The retail STARS!.EXE is stripped and the shipped 2.7j debug build carries
# CodeView NB09 symbols that Ghidra's NE loader does NOT import, so every
# internal routine loads as an anonymous FUN_seg_off. This script names them
# from the map recovered by sirgwain's `stars-asm` NB09 parser
# (see docs/ghidra/README.md and docs/ghidra/stars-symbols.csv).
#
# Usage (in the CodeBrowser that has stars.2.7j.exe open):
#   1. Window -> Script Manager -> Manage Script Directories -> add docs/ghidra
#   2. Refresh, then run "apply_symbols.py".
#   3. When prompted, pick docs/ghidra/stars-symbols.csv.
#
# The CSV columns are: name,ghidra_addr,ne_addr  (ghidra_addr = "selector:off").
# The map was verified against this exact binary: NE logical segment N maps to
# Ghidra selector 0x1000 + (N-1)*8, offset unchanged.
#
# @category Stars
# @menupath Tools.Stars.Apply NB09 Symbols

from ghidra.program.model.symbol import SourceType

FORCE = True  # rename even if the function already has a non-default name


def run():
    csv_file = askFile("Stars! NB09 symbol CSV", "Choose stars-symbols.csv")
    fm = currentProgram.getFunctionManager()
    af = currentProgram.getAddressFactory()

    renamed = 0
    created = 0
    labeled = 0
    skipped = 0
    missing = 0

    f = open(csv_file.getAbsolutePath(), "r")
    try:
        for raw in f:
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split(",")
            if len(parts) < 2:
                continue
            name = parts[0].strip()
            addr_text = parts[1].strip()
            if name == "name" or ":" not in addr_text:
                continue  # header / malformed

            try:
                addr = af.getAddress(addr_text)
            except Exception:
                addr = None
            if addr is None:
                missing += 1
                continue

            func = fm.getFunctionAt(addr)
            if func is not None:
                cur = func.getName()
                if (not FORCE) and (not cur.startswith("FUN_")):
                    skipped += 1
                    continue
                func.setName(name, SourceType.USER_DEFINED)
                renamed += 1
            else:
                nf = createFunction(addr, name)
                if nf is not None:
                    created += 1
                else:
                    createLabel(addr, name, True, SourceType.USER_DEFINED)
                    labeled += 1
    finally:
        f.close()

    println("Stars! NB09 symbols applied:")
    println("  renamed existing functions : %d" % renamed)
    println("  created new functions       : %d" % created)
    println("  fell back to labels         : %d" % labeled)
    println("  skipped (already named)     : %d" % skipped)
    println("  unresolvable addresses      : %d" % missing)


run()
