# Apply the Stars! 2.7j NB09 data types (structs + enums) and typed global
# variables to the open Ghidra program.
#
# The retail STARS!.EXE is stripped; the shipped 2.7j debug build carries
# CodeView NB09 symbols (functions, structs, enums, globals) that Ghidra's NE
# loader does NOT import. apply_symbols.py names the functions; this script adds
# the *types* and the *globals*, recovered by sirgwain's `stars-asm` NB09 parser:
#
#   1. Parses docs/ghidra/stars-types.h (the 74 game enums + 110 game structs,
#      with a small fixed-width / Win16 shim prelude) into the program's
#      data-type manager via Ghidra's C parser.
#   2. Normalises the four struct-embedded enums to their real 2-byte width.
#   3. Verifies every struct's size against docs/ghidra/stars-struct-sizes.csv
#      and reports any mismatches.
#   4. Places each of the 613 globals from docs/ghidra/stars-globals.csv at its
#      address (selector:offset), giving it the recovered type + name.
#
# Usage (in the CodeBrowser that has stars.2.7j.exe open):
#   1. Window -> Script Manager -> Manage Script Directories -> add docs/ghidra
#   2. Refresh, then run "apply_types.py".
#   3. When prompted, pick docs/ghidra/stars-types.h. The two CSVs are read from
#      the same folder automatically.
#
# The address map is the same one verified for apply_symbols.py: NE logical
# segment N -> Ghidra selector 0x1000 + (N-1)*8, offset unchanged. The script is
# idempotent and safe to re-run.
#
# @category Stars
# @menupath Tools.Stars.Apply NB09 Types and Globals

import re

from java.io import File
from java.util import ArrayList

from ghidra.program.model.symbol import SourceType
from ghidra.program.model.data import (
    ArrayDataType,
    DoubleDataType,
    Enum,
    EnumDataType,
    PointerDataType,
    VoidDataType,
)

# The four enums that appear as struct fields are 2 bytes wide in the game
# (HullSlotType reaches 0x8000, the others are small but still stored as WORDs).
# Ghidra's C parser may give a freshly-parsed enum a different default width, so
# we normalise them here to keep the embedding structs the right size.
ENUM_WIDTHS = {
    "GrPopupType": 2,
    "GrobjClass": 2,
    "HulDef": 2,
    "HullSlotType": 2,
}


def find_type(dtm, name):
    """Resolve a base type name to a DataType in the program DTM."""
    l = ArrayList()
    dtm.findDataTypes(name, l)
    if not l.isEmpty():
        return l.get(0)
    if name == "double":
        return DoubleDataType()
    if name == "void":
        return VoidDataType()
    return None


def parse_modifiers(rest):
    """Tokenise the pointer/array suffix of a type string into '*' and ints."""
    toks = []
    i = 0
    n = len(rest)
    while i < n:
        c = rest[i]
        if c == "*":
            toks.append("*")
            i += 1
        elif c == "[":
            j = rest.index("]", i)
            toks.append(int(rest[i + 1:j]))
            i = j + 1
        else:
            i += 1  # skip spaces / stray chars
    return toks


def resolve_type(dtm, type_str):
    """Turn a stars-asm type string (e.g. 'SHDEF *[16]') into a DataType."""
    s = type_str.strip()
    if s == "":
        return None
    # Function / function-pointer globals (e.g. 'int32_t() *') -> a 4-byte ptr.
    if "(" in s:
        return PointerDataType(VoidDataType(), dtm)
    m = re.match(r"^([A-Za-z_][A-Za-z0-9_]*)", s)
    if m is None:
        return None
    base = m.group(1)
    rest = s[m.end():]
    dt = find_type(dtm, base)
    if dt is None:
        # Unknown base (e.g. jmp_buf): if it is ultimately a pointer, a generic
        # pointer is the right size; otherwise we can't type it.
        if "*" in rest:
            return PointerDataType(VoidDataType(), dtm)
        return None
    for tok in parse_modifiers(rest):
        if tok == "*":
            dt = PointerDataType(dt, dtm)
        else:
            dt = ArrayDataType(dt, tok, dt.getLength())
    return dt


def parse_c_header(dtm, text):
    """Parse the consolidated header into the DTM via Ghidra's C parser."""
    from ghidra.app.util.cparser.C import CParser

    try:
        parser = CParser(dtm)
    except TypeError:
        parser = CParser(dtm, True, None)
    parser.parse(text)
    try:
        msgs = parser.getParseMessages()
        if msgs:
            println("C parser messages:\n" + msgs)
    except Exception:
        pass


def normalise_enums(dtm):
    fixed = 0
    for name, width in ENUM_WIDTHS.items():
        dt = find_type(dtm, name)
        if isinstance(dt, Enum) and dt.getLength() != width:
            ne = EnumDataType(dt.getCategoryPath(), name, width)
            for vn in dt.getNames():
                ne.add(vn, dt.getValue(vn))
            dtm.replaceDataType(dt, ne, True)
            fixed += 1
    return fixed


def read_csv(path):
    rows = []
    f = open(path, "r")
    try:
        header = True
        for raw in f:
            line = raw.rstrip("\n").rstrip("\r")
            if not line or line.startswith("#"):
                continue
            if header:
                header = False
                continue
            rows.append(line.split(","))
    finally:
        f.close()
    return rows


def verify_sizes(dtm, sizes_path):
    ok = 0
    mismatch = 0
    missing = 0
    for parts in read_csv(sizes_path):
        if len(parts) < 2:
            continue
        name = parts[0].strip()
        want = int(parts[1].strip())
        dt = find_type(dtm, name)
        if dt is None:
            missing += 1
            println("  size: MISSING type %s" % name)
        elif dt.getLength() != want:
            mismatch += 1
            println("  size: %-16s expected %d, got %d" % (name, want, dt.getLength()))
        else:
            ok += 1
    println("Struct sizes: %d ok, %d mismatched, %d missing" % (ok, mismatch, missing))
    return ok, mismatch, missing


def apply_globals(dtm, globals_path):
    af = currentProgram.getAddressFactory()
    listing = currentProgram.getListing()

    typed = 0
    labeled_only = 0
    untypable = 0
    bad_addr = 0

    for parts in read_csv(globals_path):
        # name, ghidra_addr, ne_addr, type   (type may itself have contained a
        # comma? it never does in practice, but be defensive by re-joining)
        if len(parts) < 4:
            continue
        name = parts[0].strip()
        addr_text = parts[1].strip()
        type_str = ",".join(parts[3:]).strip()

        try:
            addr = af.getAddress(addr_text)
        except Exception:
            addr = None
        if addr is None:
            bad_addr += 1
            continue

        dt = resolve_type(dtm, type_str)
        if dt is not None and dt.getLength() > 0:
            try:
                end = addr.add(dt.getLength() - 1)
                listing.clearCodeUnits(addr, end, False)
                listing.createData(addr, dt)
                typed += 1
            except Exception:
                # Could not lay the type down (e.g. it would overlap a function
                # in a mixed code/data segment) -- still give the address a name.
                labeled_only += 1
        else:
            untypable += 1

        try:
            createLabel(addr, name, True, SourceType.USER_DEFINED)
        except Exception:
            pass

    println(
        "Globals: %d typed, %d labelled-only, %d untypable-type, %d bad-address"
        % (typed, labeled_only, untypable, bad_addr)
    )


def run():
    hdr = askFile("Stars! NB09 types header", "Choose stars-types.h")
    base = hdr.getParentFile()
    globals_csv = File(base, "stars-globals.csv")
    sizes_csv = File(base, "stars-struct-sizes.csv")

    dtm = currentProgram.getDataTypeManager()

    f = open(hdr.getAbsolutePath(), "r")
    try:
        text = f.read()
    finally:
        f.close()

    println("Parsing %s ..." % hdr.getName())
    parse_c_header(dtm, text)

    fixed = normalise_enums(dtm)
    println("Normalised %d embedded enum width(s)." % fixed)

    if sizes_csv.exists():
        verify_sizes(dtm, sizes_csv.getAbsolutePath())
    else:
        println("(no stars-struct-sizes.csv next to the header; skipping size check)")

    if globals_csv.exists():
        apply_globals(dtm, globals_csv.getAbsolutePath())
    else:
        println("(no stars-globals.csv next to the header; skipping globals)")

    println("Done.")


run()
