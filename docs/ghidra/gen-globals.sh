#!/usr/bin/env bash
#
# gen-globals.sh — (re)generate the data files consumed by apply_types.py:
#
#   stars-globals.csv       name,ghidra_addr,ne_addr,type   (613 globals)
#   stars-struct-sizes.csv  name,size                       (110 structs)
#
# Both are derived from the sirgwain/stars-asm NB09 symbol database via the
# `stars-asm` CLI (built binary expected at tmp/stars-asm/dist/stars-asm).
#
# The NE logical `segment:offset` addresses map to Ghidra exactly as the
# function bridge does (see README.md): ghidra_selector = 0x1000 + (seg-1)*8,
# offset unchanged. Run from the repo root:
#
#     bash docs/ghidra/gen-globals.sh
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

# ---- globals -------------------------------------------------------------
# Table columns are: ADDRESS  NAME  TYPE...  MODULE. TYPE can contain spaces
# (e.g. "SHDEF *[16]", "FLEET * *"), so we take addr=$1, name=$2, module=$NF
# and join the remaining middle fields as the type.
{
  echo "name,ghidra_addr,ne_addr,type"
  ( cd "$asm" && ./dist/stars-asm symbols globals ) \
  | awk '
      function h2d(s,  n,i,c,v){ s=tolower(s); n=0;
        for(i=1;i<=length(s);i++){ c=substr(s,i,1); v=index("0123456789abcdef",c)-1; n=n*16+v } return n }
      function d2h4(n,  s,d){ s=""; if(n==0) s="0";
        while(n>0){ d=n%16; s=substr("0123456789abcdef",d+1,1) s; n=int(n/16) }
        while(length(s)<4) s="0" s; return s }
      $1 ~ /^[0-9a-fA-F]{4}:[0-9a-fA-F]{4}$/ {
        split($1, a, ":");
        sel = 4096 + (h2d(a[1]) - 1) * 8;   # 4096 = 0x1000 (BSD awk has no hex literals)
        name = $2;
        type = "";
        for (i = 3; i < NF; i++) type = (type == "" ? $i : type " " $i);
        printf "%s,%s:%s,%s,%s\n", name, d2h4(sel), a[2], $1, type;
      }
    '
} > "$here/stars-globals.csv"

# ---- struct sizes (for post-import verification) -------------------------
{
  echo "name,size"
  ( cd "$asm" && ./dist/stars-asm symbols structs ) \
  | awk '
      function h2d(s,  n,i,c,v){ s=tolower(s); sub(/^0x/,"",s); n=0;
        for(i=1;i<=length(s);i++){ c=substr(s,i,1); v=index("0123456789abcdef",c)-1; n=n*16+v } return n }
      $1 ~ /^[A-Za-z_][A-Za-z0-9_]*$/ && $2 ~ /^0x[0-9a-fA-F]+$/ {
        printf "%s,%d\n", $1, h2d($2);
      }
    '
} > "$here/stars-struct-sizes.csv"

echo "wrote $here/stars-globals.csv      ($(($(wc -l < "$here/stars-globals.csv") - 1)) globals)"
echo "wrote $here/stars-struct-sizes.csv ($(($(wc -l < "$here/stars-struct-sizes.csv") - 1)) structs)"
