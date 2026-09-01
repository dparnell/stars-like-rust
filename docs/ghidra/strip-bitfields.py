#!/usr/bin/env python3
"""strip-bitfields.py — collapse C bitfield declarations into plain scalar fields.

Ghidra's decompiler is bitfield-unaware (it always renders bitfield access as
shift/mask, never as `s->member`), and — worse — it *aborts* a whole function
whenever it has to form a pointer whose referent is a bitfield, e.g.

    Pointer reference data-type may not be a bitfield: uint16_t:11

This happens for any function that dereferences a struct/union pointer at an
offset the applied game types describe with a bitfield (for example
`CAdvantagePoints`, which reads the `PLAYER` flags word at +0x54, a union whose
first members are the `fDead..unused:11` bitfields). Applying the NB09 game
types therefore *broke* decompilation of those functions.

This filter rewrites every bitfield declaration in a generated Stars! header
into a single plain field of the *same base type*, preserving the bit layout as
a trailing comment. It is size-preserving for these headers because every Stars!
bitfield declaration occupies exactly one storage unit of its base type
(uint8_t / uint16_t / uint32_t) — verified against the `size=` annotations and
the `@bitN` running offsets in the generated header (no declaration's bits
exceed its base type's width). Examples:

    uint16_t id : 11, iPlayer : 5;        ->  uint16_t id; /* bitfield: id:11, iPlayer:5 */
    union { uint16_t fDead:1, ... unused:11; uint16_t wFlags; };
        ->  union { uint16_t fDead; /* bitfield: ... */ uint16_t wFlags; };

The decompiler renders these identically to how it already handled bitfields
(shift/mask on the plain word), but no `Pointer reference ... bitfield` abort can
occur any more, so the affected functions decompile again. The human-readable
bit breakdown lives on in the comment (and in docs/formats/*).

Usage (filter):  python3 strip-bitfields.py < in.h > out.h
"""
import re
import sys

# A single bitfield declaration statement:
#   <indent> <base-type> <first-member> : <width> [ , <more> ... ] ;
# `[^{}]*?` spans newlines (multi-line declarations) but never crosses a brace,
# so it cannot swallow a following/enclosing struct or union. Only declarations
# that actually contain `name : <digits>` match, so plain fields are untouched.
_BITFIELD = re.compile(
    r"^([ \t]*)"                                   # 1: indentation
    r"((?:unsigned +|signed +)?[A-Za-z_]\w*)[ \t]+"  # 2: base type
    r"([A-Za-z_]\w*[ \t]*:[ \t]*\d+[^{}]*?)"       # 3: member list (has a bitfield)
    r"[ \t]*;"
    r"(?:[ \t]*/\*[^\n]*?\*/)?",              # trailing same-line comment (dropped)
    re.MULTILINE,
)

_PAIR = re.compile(r"([A-Za-z_]\w*)\s*:\s*(\d+)")


def _replace(m):
    indent, base, body = m.group(1), m.group(2), m.group(3)
    body_no_comments = re.sub(r"/\*.*?\*/", "", body, flags=re.DOTALL)
    pairs = _PAIR.findall(body_no_comments)
    if not pairs:  # defensive: shouldn't happen given the pattern
        return m.group(0)
    first = pairs[0][0]
    summary = ", ".join("%s:%s" % (n, w) for n, w in pairs)
    return "%s%s %s; /* bitfield: %s */" % (indent, base, first, summary)


def strip_bitfields(text):
    return _BITFIELD.sub(_replace, text)


def main():
    sys.stdout.write(strip_bitfields(sys.stdin.read()))


if __name__ == "__main__":
    main()
