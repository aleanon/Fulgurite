#!/usr/bin/env bash
# firmware_delta.sh BASE [BIN=B ...] [print] -- what one task costs the
# firmware (docs/plan-iced-roadmap.md K4; M80/T80.0).
#
# Builds the four firmware binaries (scripts/lib/firmware.sh) at BASE, in
# a detached worktree under target/firmware-delta/, and in the work tree,
# then prints `.text` and `.rodata` for each side and the delta. A
# `BIN=B` word bounds that binary's `.text` delta at B bytes and fails by
# name above it. Bounds are per binary: with no `BIN=B` word the default
# is K4's per-task cap, `firmware_interactive=1000`, and the other three
# are printed unbounded. `print` never fails: a task that only records
# its growth says so.
#
# Why this exists: `just firmware-size` only prints, and
# `scripts/check_text.sh` compares the paint-only headline with a 1%
# tolerance -- about 280 B -- so neither can say "this task cost 0 B".
# This is the one comparator the plans cite; none of them adds another.
#
# Arguments are plain words, not `--` flags, because `just` reads
# `NAME=VALUE` as a variable override only before the recipe name, so
# the recipe passes them positionally:
#   just firmware-delta <sha> firmware=0 firmware_interactive=256
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

[ $# -ge 1 ] || { echo "usage: $0 BASE [BIN=B ...] [print]" >&2; exit 2; }
BASE=$1; shift

# shellcheck source=lib/firmware.sh
source scripts/lib/firmware.sh

declare -A BOUND=()
PRINT_ONLY=0
for w in "$@"; do
    case $w in
        print) PRINT_ONLY=1 ;;
        *=*)
            b=${w%%=*}; v=${w#*=}
            case " $FIRMWARE_BINS " in *" $b "*) ;; *)
                echo "firmware-delta: unknown binary '$b' (known: $FIRMWARE_BINS)" >&2; exit 2 ;;
            esac
            [[ $v =~ ^[0-9]+$ ]] || { echo "firmware-delta: bound for $b is not a byte count: '$v'" >&2; exit 2; }
            BOUND[$b]=$v ;;
        *) echo "firmware-delta: unknown argument '$w'" >&2; exit 2 ;;
    esac
done
if [ "$PRINT_ONLY" -eq 0 ] && [ ${#BOUND[@]} -eq 0 ]; then
    BOUND[firmware_interactive]=1000
fi

SHA=$(git rev-parse --verify -q "$BASE^{commit}") || {
    echo "firmware-delta: '$BASE' is not a commit" >&2; exit 2; }

# sections NAME_PATHS -- "name text rodata" per binary, from the last build.
sections() {
    local name path
    while read -r name path; do
        size -A "$path" | awk -v n="$name" '
            $1 == ".text" {t = $2} $1 == ".rodata" {r = $2}
            END {print n, t + 0, r + 0}'
    done <<<"$FIRMWARE_PATHS"
}

# The base side, built once per commit and kept: a worktree at BASE with
# its own target dir, so the work tree's incremental build is untouched.
WT=target/firmware-delta/$SHA
if [ ! -f "$WT/.fwd-built" ]; then
    if [ ! -d "$WT" ]; then
        mkdir -p target/firmware-delta
        git worktree add -q --detach "$WT" "$SHA" || {
            echo "firmware-delta: cannot create a worktree at $SHA" >&2; exit 1; }
    fi
    (cd "$WT" && source scripts/lib/firmware.sh && firmware_build \
        && printf '%s\n' "$FIRMWARE_PATHS" > .fwd-paths) || {
        echo "firmware-delta: the firmware does not build at $SHA" >&2; exit 1; }
    touch "$WT/.fwd-built"
fi
FIRMWARE_PATHS=$(cat "$WT/.fwd-paths")
# The base worktree may predate a binary this tree builds; read what it has.
base=$(sections)

firmware_build || exit 1
work=$(sections)

rc=0
echo "firmware-delta: base ${SHA:0:12}, work tree $(git rev-parse --short HEAD)$(git diff --quiet HEAD -- . || echo '+dirty')"
for name in $FIRMWARE_BINS; do
    read -r _ bt br <<<"$(printf '%s\n' "$base" | awk -v n="$name" '$1 == n')"
    read -r _ wt wr <<<"$(printf '%s\n' "$work" | awk -v n="$name" '$1 == n')"
    if [ -z "${bt:-}" ] || [ -z "${wt:-}" ]; then
        echo "firmware-delta: $name: missing on one side (base '${bt:-}', work '${wt:-}')" >&2
        rc=1; continue
    fi
    dt=$((wt - bt)); dr=$((wr - br))
    bound=${BOUND[$name]:-}
    line=$(printf '%-22s .text %6d -> %6d (%+d)  .rodata %6d -> %6d (%+d)' \
        "$name" "$bt" "$wt" "$dt" "$br" "$wr" "$dr")
    if [ -n "$bound" ] && [ "$PRINT_ONLY" -eq 0 ]; then
        if [ "$dt" -gt "$bound" ]; then
            echo "firmware-delta: $line  bound $bound -- FAIL: $name grew $dt B, over its $bound B bound" >&2
            rc=1
        else
            echo "firmware-delta: $line  bound $bound"
        fi
    else
        echo "firmware-delta: $line  (unbounded)"
    fi
    bt=; wt=
done
exit $rc
