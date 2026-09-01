#!/usr/bin/env bash
#
# gen-signatures.sh — (re)generate docs/ghidra/stars-signatures.csv, the
# per-function C prototypes (return type + typed parameters) recovered from the
# Stars! 2.7j NB09 debug symbols, keyed by Ghidra address (see apply_signatures.py
# / README.md).
#
# Output columns:  name,ghidra_addr,ne_addr,signature
#   - signature is the full C prototype exactly as stars-asm prints it, e.g.
#       void GetIniWinRc(char *szSection, char *szIniFile, StringId ids, WN *pwn)
#     and is QUOTED (it contains commas). Every type it names is defined in
#     stars-types.h, so apply_signatures.py can resolve it against the program's
#     data-type manager (run apply_types.py first).
#
# The NE logical `segment:offset` addresses map to Ghidra exactly as the other
# bridges do (see README.md): ghidra_selector = 0x1000 + (seg-1)*8, offset
# unchanged. Source of truth: the sirgwain/stars-asm NB09 symbol database via the
# built `stars-asm` CLI (expected at tmp/stars-asm/dist/stars-asm).
#
# Run from the repo root:
#
#     bash docs/ghidra/gen-signatures.sh
#
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
asm="$repo/tmp/stars-asm"
bin="$asm/dist/stars-asm"

if [[ ! -x "$bin" ]]; then
  echo "error: expected the built stars-asm CLI at $bin" >&2
  echo "       build it with:  (cd $asm && go build -o ./dist/stars-asm .)" >&2
  exit 1
fi

# Enumerate function names from the (never-wrapped) NAME column of the table,
# then ask for each one's clean single-line prototype via the detail view
# (`--name`), which prints unwrapped `addr:` and `sig:` lines. Emit an
# intermediate TSV of `ne_addr <TAB> name <TAB> signature` and let a final awk
# pass compute the Ghidra selector and quote the signature into CSV.
names="$("$bin" --input-path "$asm/dasm/input" symbols functions \
        | awk 'NR>2 && $1 ~ /^[0-9a-fA-F]{4}:[0-9a-fA-F]{4}$/ {print $2}')"

{
  echo "name,ghidra_addr,ne_addr,signature"
  for n in $names; do
    "$bin" --input-path "$asm/dasm/input" symbols functions --name "$n" \
    | awk -v name="$n" '
        /^[[:space:]]*addr:/ { ne=$2 }
        /^[[:space:]]*sig:/  { sub(/^[[:space:]]*sig:[[:space:]]*/,""); sig=$0 }
        END { if (ne != "" && sig != "") printf "%s\t%s\t%s\n", ne, name, sig }'
  done \
  | awk -F'\t' '
      function h2d(s,  n,i,c,v){ s=tolower(s); n=0;
        for(i=1;i<=length(s);i++){ c=substr(s,i,1); v=index("0123456789abcdef",c)-1; n=n*16+v } return n }
      function d2h4(n,  s,d){ s=""; if(n==0) s="0";
        while(n>0){ d=n%16; s=substr("0123456789abcdef",d+1,1) s; n=int(n/16) }
        while(length(s)<4) s="0" s; return s }
      $1 ~ /^[0-9a-fA-F]{4}:[0-9a-fA-F]{4}$/ {
        split($1, a, ":");
        sel = 4096 + (h2d(a[1]) - 1) * 8;   # 4096 = 0x1000 (BSD awk has no hex literals)
        printf "%s,%s:%s,%s,\"%s\"\n", $2, d2h4(sel), a[2], $1, $3;
      }'
} > "$here/stars-signatures.csv"

echo "wrote $here/stars-signatures.csv ($(($(wc -l < "$here/stars-signatures.csv") - 1)) functions)"
