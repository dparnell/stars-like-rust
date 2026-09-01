# Apply the Stars! 2.7j NB09 function signatures (return type + typed parameters)
# to the open Ghidra program.
#
# The retail STARS!.EXE is stripped; the shipped 2.7j debug build carries
# CodeView NB09 symbols that Ghidra's NE loader does NOT import. The other two
# bridges name the routines (apply_symbols.py) and import the structs/enums +
# typed globals (apply_types.py). This script is the third bridge: it types every
# function's *prototype* — its return type and each parameter — from the
# signatures recovered by sirgwain's `stars-asm` NB09 parser
# (see docs/ghidra/README.md and docs/ghidra/stars-signatures.csv).
#
# Run apply_symbols.py (names) and apply_types.py (types) FIRST: this script
# needs the functions to exist and needs the game structs/enums to be present in
# the program's data-type manager so the prototype strings resolve.
#
# Usage (in the CodeBrowser that has stars.2.7j.exe open):
#   1. Window -> Script Manager -> Manage Script Directories -> add docs/ghidra
#   2. Refresh, then run "apply_signatures.py".
#   3. When prompted, pick docs/ghidra/stars-signatures.csv.
#
# CSV columns: name,ghidra_addr,ne_addr,signature  (signature is QUOTED because
# it contains commas). ghidra_addr = "selector:offset"; the address map is the
# same verified one used by the other scripts: NE logical segment N -> Ghidra
# selector 0x1000 + (N-1)*8, offset unchanged. The script is idempotent and safe
# to re-run.
#
# @category Stars
# @menupath Tools.Stars.Apply NB09 Function Signatures

import csv
import re

from ghidra.app.util.parser import FunctionSignatureParser
from ghidra.app.cmd.function import ApplyFunctionSignatureCmd
from ghidra.program.model.symbol import SourceType

# A function-pointer *parameter* (e.g. `int16_t (**pfn)(FLEET *, FLEET *)`) is
# something Ghidra's FunctionSignatureParser cannot express, so those prototypes
# fail to parse. Only four functions take such a callback; for them we rewrite
# the callback argument to a plain pointer of the same depth (`void **pfn`) so
# the return type and the *other* parameters still get typed faithfully. The
# inner argument list never contains nested parentheses, so `[^()]*` is enough.
_FUNCPTR_PARAM = re.compile(
    r"[A-Za-z_]\w*\s*\(\s*(\*+)\s*([A-Za-z_]\w*)\s*\)\s*\([^()]*\)"
)


def simplify_funcptr_params(sig):
    """Collapse any function-pointer parameter to a same-depth `void *` pointer."""
    return _FUNCPTR_PARAM.sub(lambda m: "void " + m.group(1) + m.group(2), sig)


def build_parser():
    """A FunctionSignatureParser bound to the program's data-type manager.

    The second argument is a DataTypeManagerService used to search *other* open
    archives; passing None restricts resolution to the program DTM, which is
    exactly what we want (apply_types.py has already put every game struct/enum
    there, so nothing should need an external archive)."""
    return FunctionSignatureParser(currentProgram.getDataTypeManager(), None)


def apply_one(parser, addr, sig_text):
    """Parse `sig_text` and apply it to the function at `addr`.

    Returns one of: 'applied', 'simplified', 'nofunc', 'parsefail', 'applyfail'."""
    fm = currentProgram.getFunctionManager()
    func = fm.getFunctionAt(addr)
    if func is None:
        return "nofunc"

    # Parse the C prototype string into a FunctionDefinitionDataType, resolving
    # every named type against the program DTM. Passing the function's current
    # signature as the "original" lets the parser fall back to existing details
    # for anything the text omits (it never omits anything here).
    status = "applied"
    try:
        fdef = parser.parse(func.getSignature(), sig_text)
    except Exception:
        fdef = None
    if fdef is None:
        # Retry with any function-pointer parameter collapsed to a plain pointer.
        simplified = simplify_funcptr_params(sig_text)
        if simplified != sig_text:
            try:
                fdef = parser.parse(func.getSignature(), simplified)
                status = "simplified"
            except Exception:
                fdef = None
    if fdef is None:
        return "parsefail"

    # Preserve the calling convention Ghidra recovered for the NE function
    # (e.g. __cdecl16far / __pascal16far); we are only asserting the data types.
    # forceName=False so the (already-applied) NB09 name is left untouched.
    try:
        cmd = ApplyFunctionSignatureCmd(
            addr, fdef, SourceType.IMPORTED, True, False
        )
    except TypeError:
        # Older API: 3-arg constructor. Remember + restore the convention.
        cc = func.getCallingConventionName()
        cmd = ApplyFunctionSignatureCmd(addr, fdef, SourceType.IMPORTED)
        ok = cmd.applyTo(currentProgram)
        if ok and cc is not None:
            try:
                func.setCallingConvention(cc)
            except Exception:
                pass
        return status if ok else "applyfail"

    return status if cmd.applyTo(currentProgram) else "applyfail"


def run():
    csv_file = askFile("Stars! NB09 signatures CSV", "Choose stars-signatures.csv")
    af = currentProgram.getAddressFactory()
    parser = build_parser()

    applied = 0
    simplified = 0
    nofunc = 0
    parsefail = 0
    applyfail = 0
    badaddr = 0

    f = open(csv_file.getAbsolutePath(), "r")
    try:
        reader = csv.reader(f)
        first = True
        for row in reader:
            if not row or row[0].startswith("#"):
                continue
            if first:
                first = False
                if row and row[0].strip() == "name":
                    continue  # header
            if len(row) < 4:
                continue
            addr_text = row[1].strip()
            sig_text = row[3].strip()
            if ":" not in addr_text or not sig_text:
                continue

            try:
                addr = af.getAddress(addr_text)
            except Exception:
                addr = None
            if addr is None:
                badaddr += 1
                continue

            result = apply_one(parser, addr, sig_text)
            if result == "applied":
                applied += 1
            elif result == "simplified":
                simplified += 1
            elif result == "nofunc":
                nofunc += 1
                println("  no function at %s (%s)" % (addr_text, row[0].strip()))
            elif result == "parsefail":
                parsefail += 1
                println("  could not parse: %s" % sig_text)
            else:
                applyfail += 1
                println("  could not apply: %s" % sig_text)
    finally:
        f.close()

    println("Stars! NB09 function signatures applied:")
    println("  typed (return + params) : %d" % applied)
    println("  typed (callback->ptr)   : %d" % simplified)
    println("  no function at address  : %d" % nofunc)
    println("  prototype parse failed  : %d" % parsefail)
    println("  apply failed            : %d" % applyfail)
    println("  unresolvable addresses  : %d" % badaddr)


run()
