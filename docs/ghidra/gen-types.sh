#!/usr/bin/env bash
#
# gen-types.sh — (re)generate docs/ghidra/stars-types.h, the consolidated,
# self-contained C header of the Stars! 2.7j game types (enums + structs) used
# to populate the Ghidra data-type manager (see apply_types.py / README.md).
#
# It concatenates, in order:
#   1. stars-types.prelude.h      (hand-written fixed-width + Win16 shims)
#   2. forward typedefs           (one per game struct, so field references that
#                                  appear textually before a struct's definition
#                                  still resolve — makes the header order-independent)
#   3. enums.h (curated)          (the 74 game enums, with two collisions removed
#                                  so the result is valid, single-namespace C:
#                                    - the accidental *second* VictoryCondition
#                                      definition (an exact duplicate), and
#                                    - the standalone Windows MessageBoxResult
#                                      enum, whose IDOK/IDCANCEL members collide
#                                      with the game's ControlId enum.)
#   4. structs.h (stripped)       (the 110 game structs; #include / include-guard
#                                  lines removed)
#
# Source of truth: the sirgwain/stars-asm NB09 decompiled headers under
# tmp/stars-asm/decompiled/. Run from the repo root:
#
#     bash docs/ghidra/gen-types.sh
#
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
src="$repo/tmp/stars-asm/decompiled"
out="$here/stars-types.h"

if [[ ! -f "$src/enums.h" || ! -f "$src/structs.h" ]]; then
  echo "error: expected the stars-asm decompiled headers at $src" >&2
  echo "       (clone https://github.com/sirgwain/stars-asm into tmp/stars-asm)" >&2
  exit 1
fi

{
  # 1. prelude
  cat "$here/stars-types.prelude.h"

  # 2. forward typedefs for every top-level game struct/union
  printf '\n/* ---- forward declarations (make the header order-independent) ---- */\n\n'
  awk '
    /^typedef (struct|union) [A-Za-z_][A-Za-z0-9_]* \{/ { kind=$2; tag=$3; next }
    /^} [A-Za-z0-9_]+;/ {
      name=$2; sub(/;.*/,"",name);
      if (tag!="") { printf "typedef %s %s %s;\n", kind, tag, name; tag="" }
      next
    }
  ' "$src/structs.h"

  # 3. curated enums
  printf '\n/* ---- enums (tmp/stars-asm/decompiled/enums.h, curated) ---- */\n\n'
  awk '
    function flush(name,   keep) {
      keep=1
      if (name=="VictoryCondition") { vc++; if (vc>1) keep=0 }
      if (name=="MessageBoxResult") keep=0
      if (keep) printf "%s", buf
      buf=""
    }
    BEGIN { inblk=0; buf="" }
    {
      if (inblk) {
        buf = buf $0 "\n"
        if ($0 ~ /^}[ \t]*[A-Za-z0-9_]*[ \t]*;/) {
          name=$0; sub(/^}[ \t]*/,"",name); sub(/[ \t]*;.*/,"",name)
          flush(name); inblk=0
        }
        next
      }
      if ($0 ~ /^typedef enum/) {
        buf=$0 "\n"
        if ($0 ~ /}[ \t]*[A-Za-z0-9_]*[ \t]*;/) {
          name=$0; sub(/.*}[ \t]*/,"",name); sub(/[ \t]*;.*/,"",name)
          flush(name)
        } else { inblk=1 }
        next
      }
      print
    }
  ' "$src/enums.h"

  # 4. structs (include / guard lines stripped)
  printf '\n/* ---- structs (tmp/stars-asm/decompiled/structs.h) ---- */\n\n'
  sed -e '/^#ifndef STARS_DECOMPILED_STRUCTS_H/d' \
      -e '/^#define STARS_DECOMPILED_STRUCTS_H/d' \
      -e '/^#include <stdint.h>/d' \
      -e '/^#include <windows.h>/d' \
      -e '/^#endif/d' \
      "$src/structs.h"

  printf '\n/* ======================== END generated game types ========================= */\n'
} > "$out"

echo "wrote $out ($(wc -l < "$out") lines)"
