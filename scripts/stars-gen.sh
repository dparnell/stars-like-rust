#!/usr/bin/env bash
#
# stars-gen.sh - batch turn generation for a Stars! host game.
#
# For each turn it:
#   1. reads the current game year out of the .HST file
#   2. creates <gamedir>/<year>/ and copies the current game files into it
#   3. generates the turn with stars.exe (via wine)
#   4. confirms the year advanced, then repeats
#
# Usage:  stars-gen.sh [options] <game.hst> <turns>
# Try:    stars-gen.sh -h
#
set -Eeuo pipefail

readonly PROG=${0##*/}
readonly VERSION=1.0

# ---------------------------------------------------------------- defaults ---

# Path to stars.exe. Either a unix path, or a Windows path wine understands
# (e.g. 'C:\stars\stars.exe'). Override with -e or $STARS_EXE.
STARS_EXE=${STARS_EXE:-$HOME/.wine/drive_c/stars/stars.exe}
WINE=${WINE:-wine}
TURN_TIMEOUT=${TURN_TIMEOUT:-300}   # seconds to allow one generation
USE_XVFB=0                          # -X : run wine under xvfb-run (headless)
FORCE=0                             # -f : overwrite an existing year dir
DRY_RUN=0                           # -n : show what would happen, generate nothing
KEEP_GOING=0                        # -k : don't abort the run on a failed turn
QUIET=0
VERBOSE=0                           # -v : echo stars.exe output every turn
LOGFILE=""

# A Stars! file is a sequence of blocks. Each block starts with a 2-byte
# little-endian word: the top 6 bits are the block type, the low 10 bits the
# payload size. The first block is always the file header (type 8, 16 bytes)
# and it is not encrypted:
#
#   file  payload
#   0..1     -     block header: (8 << 10) | 16 == 0x2010
#   2..5     0..3  magic "J3J3"
#   6..9     4..7  game id
#   10..11   8..9  file version
#   12..13  10..11 turn counter   <-- year = 2400 + turn
#   14..15  12..13 player (low 5 bits) + salt (high 11 bits)
#   16..17  14..15 flags / file type
#
HST_MAGIC="J3J3"
HDR_BLOCK_TYPE=8            # file header block
HDR_BLOCK_SIZE=16           # payload bytes
HDR_PAYLOAD_OFFSET=2        # payload starts after the 2-byte block header
HST_TURN_OFFSET=10          # of the turn counter *within the payload*
BASE_YEAR=2400

# ---------------------------------------------------------------- plumbing ---

c_reset=""; c_bold=""; c_red=""; c_yellow=""; c_green=""
if [[ -t 2 ]]; then
    c_reset=$'\e[0m'; c_bold=$'\e[1m'; c_red=$'\e[31m'
    c_yellow=$'\e[33m'; c_green=$'\e[32m'
fi

log()  { (( QUIET )) || printf '%s\n' "$*" >&2; _tolog "$*"; }
info() { (( QUIET )) || printf '%s%s%s\n' "$c_bold" "$*" "$c_reset" >&2; _tolog "$*"; }
ok()   { (( QUIET )) || printf '%s%s%s\n' "$c_green" "$*" "$c_reset" >&2; _tolog "$*"; }
warn() { printf '%s%s: warning:%s %s\n' "$c_yellow" "$PROG" "$c_reset" "$*" >&2; _tolog "warning: $*"; }
err()  { printf '%s%s: error:%s %s\n'   "$c_red"    "$PROG" "$c_reset" "$*" >&2; _tolog "error: $*"; }
die()  { err "$*"; exit 1; }

_tolog() {
    [[ -n $LOGFILE ]] || return 0
    printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*" >>"$LOGFILE"
}

usage() {
    cat <<EOF
$PROG $VERSION - batch turn generation for Stars!

Usage:
  $PROG [options] <game.hst> <turns>

Arguments:
  game.hst    the host file of the game (e.g. /srv/stars/games/foo/FOO.HST)
  turns       how many turns to generate

Options:
  -e PATH     path to stars.exe            (default: \$STARS_EXE)
  -w CMD      wine command                 (default: \$WINE, currently '$WINE')
  -X          run wine under xvfb-run, for a headless host
  -p STYLE    how to pass the .HST to stars.exe: win (default, a full
              drive-letter path via winepath), unix, or base (bare filename).
              If you get "invalid universe definition", try another style.
  -t SECS     timeout for one generation   (default: $TURN_TIMEOUT)
  -y YEAR     assume this year instead of reading it from the .HST
  -d DELAY    sleep DELAY seconds between turns
  -f          overwrite an existing year directory
  -k          keep going if a turn fails (default: stop)
  -n          dry run: snapshot nothing, generate nothing, just report
  -i          print the .HST header (year, game id, block layout) and exit
  -l FILE     append a timestamped log to FILE
  -v          echo stars.exe output after every turn
  -q          quiet
  -h          this help

Environment:
  STARS_EXE, WINE, TURN_TIMEOUT are read as defaults for -e, -w and -t.

Examples:
  $PROG -i ~/games/andromeda/AND.HST
  $PROG ~/games/andromeda/AND.HST 5
  $PROG -X -e 'C:\\stars\\stars.exe' -d 2 /srv/stars/AND.HST 10
EOF
}

# ------------------------------------------------------------ .HST parsing ---

# byte_to_char <n> -> the printable character for byte n, or '.'
byte_to_char() {
    if (( $1 >= 32 && $1 < 127 )); then
        printf '%b' "\\$(printf '%03o' "$1")"
    else
        printf '.'
    fi
}

# parse_header <file>
#   -> prints: turn gameid version playersalt flags blocktype blocksize
parse_header() {
    local file=$1 i p blk btype bsize magic=""
    local -a b=()

    [[ -f $file ]] || die "no such file: $file"
    [[ -r $file ]] || die "cannot read: $file"

    local raw
    raw=$(od -An -tu1 -v -N18 "$file")
    read -r -a b <<<"${raw//$'\n'/ }"
    (( ${#b[@]} >= 18 )) || die "$file is too short to be a Stars! file"

    blk=$(( b[0] + b[1] * 256 ))
    btype=$(( blk >> 10 ))
    bsize=$(( blk & 0x3FF ))
    (( btype == HDR_BLOCK_TYPE )) ||
        die "$file does not start with a Stars! file-header block" \
            "(expected block type $HDR_BLOCK_TYPE, got $btype)"
    (( bsize == HDR_BLOCK_SIZE )) ||
        warn "unexpected file-header size in $file ($bsize bytes, expected $HDR_BLOCK_SIZE)"

    p=$HDR_PAYLOAD_OFFSET
    for (( i = 0; i < 4; i++ )); do magic+=$(byte_to_char "${b[p+i]}"); done
    [[ $magic == "$HST_MAGIC" ]] ||
        die "$file does not look like a Stars! file (expected magic '$HST_MAGIC', got '$magic')"

    printf '%d %d %d %d %d %d %d\n' \
        "$(( b[p+HST_TURN_OFFSET] + b[p+HST_TURN_OFFSET+1] * 256 ))" \
        "$(( b[p+4] + b[p+5]*256 + b[p+6]*65536 + b[p+7]*16777216 ))" \
        "$(( b[p+8]  + b[p+9]  * 256 ))" \
        "$(( b[p+12] + b[p+13] * 256 ))" \
        "$(( b[p+14] + b[p+15] * 256 ))" \
        "$btype" "$bsize"
}

# read_turn <file> -> prints the turn counter (year - 2400)
read_turn() {
    local file=$1 fields turn
    fields=$(parse_header "$file") || return 1
    turn=${fields%% *}

    # a game that has run for 500+ years means we are reading the wrong offset
    if (( turn > 500 )); then
        die "implausible turn counter ($turn) in $file;" \
            "the file may be corrupt, or from a Stars! version with a different header." \
            "Run '$PROG -i $file' to inspect it, or use -y to set the year manually."
    fi
    printf '%s\n' "$turn"
}

# hst_info <file> -> human-readable dump of the header, for diagnosis
hst_info() {
    local file=$1 turn gameid version playersalt flags btype bsize
    local size off=0 n=0 lo hi w t s

    read -r turn gameid version playersalt flags btype bsize < <(parse_header "$file")

    printf 'file:         %s\n' "$file"
    printf 'header block: type %d, %d byte payload\n' "$btype" "$bsize"
    printf 'magic:        %s\n' "$HST_MAGIC"
    printf 'game id:      0x%08X\n' "$gameid"
    printf 'version:      0x%04X\n' "$version"
    printf 'turn:         %d  ->  year %d\n' "$turn" "$(( BASE_YEAR + turn ))"
    printf 'player:       %d%s\n' "$(( playersalt & 0x1F ))" \
        "$( (( (playersalt & 0x1F) == 31 )) && echo '  (31 = host/all)' )"
    printf 'salt:         %d\n' "$(( playersalt >> 5 ))"
    printf 'flags:        0x%04X\n' "$flags"

    size=$(stat -c '%s' "$file" 2>/dev/null || stat -f '%z' "$file")
    printf 'blocks:\n'
    while (( n < 8 && off + 2 <= size )); do
        lo=""; hi=""
        read -r lo hi <<<"$(od -An -tu1 -v -j"$off" -N2 "$file")"
        [[ -n $hi ]] || break
        w=$(( lo + hi * 256 )); t=$(( w >> 10 )); s=$(( w & 0x3FF ))
        printf '  @%-6d type %2d, %4d bytes\n' "$off" "$t" "$s"
        off=$(( off + 2 + s ))
        n=$(( n + 1 ))
    done
    (( off < size )) && printf '  ... (%d bytes total)\n' "$size"
    return 0
}

read_year() {
    local turn
    turn=$(read_turn "$1") || return 1
    printf '%s\n' $(( BASE_YEAR + turn ))
}

# --------------------------------------------------------------- snapshot ----

# game_files <dir> <base> -> prints the game's files, one per line
game_files() {
    local dir=$1 base=$2 f
    local -a found=()
    shopt -s nullglob nocaseglob
    for f in "$dir/$base".*; do
        [[ -f $f ]] || continue          # skip the year directories
        found+=("$f")
    done
    shopt -u nullglob nocaseglob
    (( ${#found[@]} )) && printf '%s\n' "${found[@]}"
}

# snapshot <dir> <base> <year>
snapshot() {
    local dir=$1 base=$2 year=$3
    local target="$dir/$year"
    local -a files=()

    mapfile -t files < <(game_files "$dir" "$base")
    (( ${#files[@]} )) || die "found no '$base.*' files in $dir"

    if [[ -e $target ]]; then
        [[ -d $target ]] || die "$target exists and is not a directory"
        if (( FORCE )); then
            warn "overwriting existing snapshot $target"
        else
            die "snapshot directory already exists: $target (use -f to overwrite)"
        fi
    fi

    info "  snapshot -> $target (${#files[@]} files)"
    if (( DRY_RUN )); then
        local f
        for f in "${files[@]}"; do log "    would copy ${f##*/}"; done
        return 0
    fi

    mkdir -p -- "$target"
    cp -p -- "${files[@]}" "$target/"
}

# -------------------------------------------------------------- generation ---

# Stars! derives the names of the game's companion files - the .XY universe
# definition above all - from the path it was handed on the command line. Give
# it a bare filename and it resolves them against whatever it thinks the
# current directory is, which under wine is not necessarily the directory the
# shell was in; the symptom is "invalid universe definition" even though the
# .XY is sitting right next to the .HST. So hand it a full drive-letter path,
# the way the file dialog does. -p selects the style if the default misbehaves.
PATH_STYLE=win

# win_path <unix path> -> the equivalent Windows path, if wine can tell us
win_path() {
    local p=$1 out
    if command -v winepath >/dev/null 2>&1; then
        out=$(winepath -w "$p" 2>/dev/null) || out=""
        [[ -n $out ]] && { printf '%s\n' "$out"; return 0; }
    fi
    warn "winepath unavailable; passing the unix path to stars.exe"
    printf '%s\n' "$p"
}

# hst_argument <dir> <hstname> -> what to pass to stars.exe after -a
hst_argument() {
    local dir=$1 hst=$2
    case $PATH_STYLE in
        win)  win_path "$dir/$hst" ;;
        unix) printf '%s\n' "$dir/$hst" ;;
        base) printf '%s\n' "$hst" ;;
        *)    die "unknown path style '$PATH_STYLE' (use win, unix or base)" ;;
    esac
}

STARS_OUTPUT=""

# generate <dir> <hstname>
generate() {
    local dir=$1 hst=$2 arg
    local -a cmd=()

    arg=$(hst_argument "$dir" "$hst")

    (( USE_XVFB )) && cmd+=(xvfb-run -a)
    cmd+=("$WINE" "$STARS_EXE" -g "$arg")
    [[ $TURN_TIMEOUT == 0 ]] || cmd=(timeout --foreground "$TURN_TIMEOUT" "${cmd[@]}")

    info "  generating: ${cmd[*]}"
    if (( DRY_RUN )); then
        return 0
    fi

    local rc=0
    STARS_OUTPUT=""
    # still run from the game directory, so that a relative lookup also lands
    # in the right place
    STARS_OUTPUT=$( cd "$dir" && "${cmd[@]}" 2>&1 ) || rc=$?
    [[ -n $STARS_OUTPUT ]] && _tolog "$STARS_OUTPUT"
    (( VERBOSE )) && [[ -n $STARS_OUTPUT ]] && printf '%s\n' "$STARS_OUTPUT" >&2

    if (( rc == 124 )); then
        err "generation timed out after ${TURN_TIMEOUT}s"
        [[ -n $STARS_OUTPUT ]] && printf '%s\n' "$STARS_OUTPUT" >&2
        return 1
    fi
    if (( rc != 0 )); then
        err "stars.exe exited with status $rc"
        [[ -n $STARS_OUTPUT ]] && printf '%s\n' "$STARS_OUTPUT" >&2
        return 1
    fi
    return 0
}

# ------------------------------------------------------------------- main ----

delay=0
year_override=""
info_only=0

while getopts ':e:w:t:y:d:l:p:XfiknqvhV' opt; do
    case $opt in
        e) STARS_EXE=$OPTARG ;;
        w) WINE=$OPTARG ;;
        t) TURN_TIMEOUT=$OPTARG ;;
        y) year_override=$OPTARG ;;
        d) delay=$OPTARG ;;
        l) LOGFILE=$OPTARG ;;
        p) PATH_STYLE=$OPTARG ;;
        v) VERBOSE=1 ;;
        X) USE_XVFB=1 ;;
        f) FORCE=1 ;;
        i) info_only=1 ;;
        k) KEEP_GOING=1 ;;
        n) DRY_RUN=1 ;;
        q) QUIET=1 ;;
        h) usage; exit 0 ;;
        V) printf '%s %s\n' "$PROG" "$VERSION"; exit 0 ;;
        :) die "option -$OPTARG requires an argument (try -h)" ;;
        \?) die "unknown option -$OPTARG (try -h)" ;;
    esac
done
shift $(( OPTIND - 1 ))

if (( info_only )); then
    (( $# >= 1 )) || { usage >&2; exit 2; }
    [[ -f $1 ]] || die "no such file: $1"
    hst_info "$1"
    exit 0
fi

(( $# == 2 )) || { usage >&2; exit 2; }

hst_path=$1
turns=$2

[[ $turns =~ ^[0-9]+$ && $turns -gt 0 ]] || die "turns must be a positive integer, got '$turns'"
[[ $delay =~ ^[0-9]+$ ]] || die "delay must be an integer number of seconds"
[[ $TURN_TIMEOUT =~ ^[0-9]+$ ]] || die "timeout must be an integer number of seconds"
[[ -z $year_override || $year_override =~ ^[0-9]{4}$ ]] || die "year must be four digits"

[[ -f $hst_path ]] || die "no such host file: $hst_path"
game_dir=$(cd -- "$(dirname -- "$hst_path")" && pwd) || die "cannot resolve directory of $hst_path"
hst_name=${hst_path##*/}
game_base=${hst_name%.*}
[[ -w $game_dir ]] || die "game directory is not writable: $game_dir"

case $PATH_STYLE in win|unix|base) ;; *) die "-p must be win, unix or base (got '$PATH_STYLE')" ;; esac

# stars.exe needs the universe definition beside the host file; without it the
# generation fails with "invalid universe definition"
shopt -s nullglob nocaseglob
xy_files=( "$game_dir/$game_base".[x][y] )     # bracket globs so nullglob applies
shopt -u nullglob nocaseglob
(( ${#xy_files[@]} )) ||
    die "no $game_base.XY universe file in $game_dir - stars.exe cannot generate without it"

# Linux is case sensitive where Windows is not: stars.exe builds the .XY name
# from the .HST name, so a stem that differs in case can be invisible to it.
xy_stem=${xy_files[0]##*/}; xy_stem=${xy_stem%.*}
[[ $xy_stem == "$game_base" ]] ||
    warn "universe file is '${xy_files[0]##*/}' but the host file is '$hst_name';" \
         "stars.exe looks for '$game_base.XY' and may not find it - consider renaming"

# stars.exe may be given as a Windows path, which we cannot stat - only
# complain when it looks like a unix path and isn't there.
if [[ $STARS_EXE == /* && ! -f $STARS_EXE ]]; then
    if (( DRY_RUN )); then
        warn "stars.exe not found at '$STARS_EXE' (set it with -e or \$STARS_EXE)"
    else
        die "stars.exe not found at '$STARS_EXE' (set it with -e or \$STARS_EXE)"
    fi
fi
if ! (( DRY_RUN )); then
    command -v "${WINE%% *}" >/dev/null 2>&1 || die "wine command not found: $WINE"
    (( USE_XVFB )) && { command -v xvfb-run >/dev/null 2>&1 || die "xvfb-run not found"; }
    [[ $TURN_TIMEOUT == 0 ]] || command -v timeout >/dev/null 2>&1 ||
        die "'timeout' not found; use -t 0 to disable the turn timeout"
fi

if [[ -n $LOGFILE ]]; then
    : >>"$LOGFILE" || die "cannot write log file: $LOGFILE"
fi

info "$PROG: game '$game_base' in $game_dir"
info "generating $turns turn(s)$( (( DRY_RUN )) && echo ' [dry run]')"

failed=0
generated=0

for (( i = 1; i <= turns; i++ )); do
    if [[ -n $year_override ]]; then
        year=$year_override
    else
        year=$(read_year "$game_dir/$hst_name") ||
            die "cannot determine the game year from $hst_name (use -y to set it manually)"
    fi

    info "[$i/$turns] year $year"

    snapshot "$game_dir" "$game_base" "$year"

    if ! generate "$game_dir" "$hst_name"; then
        failed=1
        (( KEEP_GOING )) || die "aborting at year $year (use -k to continue past failures)"
        warn "continuing after failed turn at year $year"
        continue
    fi

    if (( DRY_RUN )); then
        ok "  would generate year $year -> $(( year + 1 ))"
        [[ -n $year_override ]] && year_override=$(( year_override + 1 ))
        generated=$(( generated + 1 ))
        continue
    fi

    # the generation is only real if the host file moved on
    new_year=$(read_year "$game_dir/$hst_name") ||
        die "cannot re-read the game year after generating year $year"
    if (( new_year <= year )); then
        err "the host file is still at year $new_year - the turn was not generated"
        [[ -n $STARS_OUTPUT ]] && printf '%s\n' "$STARS_OUTPUT" >&2
        failed=1
        (( KEEP_GOING )) || exit 1
        continue
    fi
    (( new_year == year + 1 )) || warn "year jumped from $year to $new_year"

    [[ -n $year_override ]] && year_override=$new_year
    generated=$(( generated + 1 ))
    ok "  generated -> year $new_year"

    if (( delay > 0 && i < turns )); then
        sleep "$delay"
    fi
done

if (( DRY_RUN )); then
    info "dry run finished."
elif (( failed )); then
    warn "finished with errors: $generated of $turns turn(s) generated."
    exit 1
else
    final_year=$(read_year "$game_dir/$hst_name") || final_year="unknown"
    ok "done: $generated turn(s) generated; game is now at year $final_year."
fi
