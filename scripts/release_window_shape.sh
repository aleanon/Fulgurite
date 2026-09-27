#!/usr/bin/env bash
# `just release-window-shape`: a shipped release window links no panic-
# recovery code -- M52 (docs/plan-embedded-footprint.md §2.2, F-NEW-3).
#
# M36's rule 3 (docs/plan-panic-recovery.md) says recovery is there in debug
# builds and in release builds with `lumen-static-winit/reload`, and that a
# release build without it keeps the default behaviour. The behaviour was
# right: recovery was off, no hook installed, nothing caught. The LINK was
# not: the window crate chose the policy as a `const` and handed it to a
# runtime field, every guarded arm tested the field, and so every
# `lumen_static_shell::recover::` function -- the failure screen, the
# report, the panic hook -- was compiled into every release window. No gate
# priced a release window binary: fixtures/binsize depends on neither the
# shell nor the window crate.
#
# So this builds `examples/minimal` in release twice and counts the
# `lumen_static_shell::recover::` symbols in each, printed beside `.text`:
#
#   * default features (no `reload`): the count must be 0;
#   * `--features lumen-static-winit/reload`: the count must be ABOVE 0,
#     so the `cfg` is shown switching both ways. A count of 0 there means
#     the pattern matches nothing -- a gate that could not have gone red.
#
# The binary is the one cargo reports having built, as in
# scripts/lib/firmware.sh, so a CARGO_TARGET_DIR cannot leave it judging a
# stale ./target.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

PATTERN='lumen_static_shell::recover::'
# Symbols DEFINED in the recover module: the demangled name begins with
# its path (or `<` and its path, for an impl). Not `core::ptr::drop_in_place
# <Option<lumen_static_shell::recover::PanicReport>>` -- the drop glue of
# `Host`'s always-`None` failure field, 3 x 118 B, which is data every
# host carries under `panic = "unwind"` and not recovery code. Recorded in
# docs/results-embedded.md (M52/T52.7), not hidden: it prints below.
DEFINED_RE='^[0-9a-f]+ [0-9a-f]+ [A-Za-z] <?lumen_static_shell::recover::'

# **Expected red until M52/T52.7**, which makes recovery a type. While this
# is set, the bound-0 check going red is reported and passes, and it going
# GREEN fails -- so the marker cannot outlive the fix it waits for. It
# covers that one check only: a reload build with no recovery symbol is
# red whatever this says.
XFAIL=""

# Build `minimal` in release with extra cargo args; print the executable.
build() {
    cargo build --release -q -p minimal "$@" --message-format=json | python3 -c '
import json, sys
exe = None
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") == "compiler-artifact" and m.get("target", {}).get("name") == "minimal" \
            and m.get("executable"):
        exe = m["executable"]
if not exe:
    sys.exit("release-window-shape: cargo reported no executable for minimal")
print(exe)
'
}

# count BIN -> "<symbols> <bytes> <text>"
count() {
    local syms n bytes text
    syms=$(nm -C --size-sort -S --radix=d "$1") || { echo "release-window-shape: nm failed on $1" >&2; return 1; }
    n=$(printf '%s\n' "$syms" | grep -cE "$DEFINED_RE" || true)
    bytes=$(printf '%s\n' "$syms" | grep -E "$DEFINED_RE" | awk '{s += $2} END {print s + 0}')
    glue=$(printf '%s\n' "$syms" | grep -F "$PATTERN" | grep -vcE "$DEFINED_RE" || true)
    text=$(size -A "$1" | awk '$1 == ".text" {print $2}')
    echo "$n $bytes $text $glue"
}

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
off=$(build) || exit 1
# Both builds write the same path: keep this one to name its symbols.
cp "$off" "$tmp/minimal-off" && off="$tmp/minimal-off"
read -r n_off b_off t_off g_off <<<"$(count "$off")" || exit 1
echo "release-window-shape: without reload: $n_off recover symbols ($b_off B) (bound 0), .text $t_off B; $g_off drop-glue symbols naming a recover type (not code; recorded)"

on=$(build --features lumen-static-winit/reload) || exit 1
read -r n_on b_on t_on g_on <<<"$(count "$on")" || exit 1
echo "release-window-shape: with reload:    $n_on recover symbols ($b_on B) (bound > 0), .text $t_on B"

fail=0
if [ "$n_on" -eq 0 ]; then
    echo "release-window-shape: a reload build links no '$PATTERN' symbol -- the pattern" >&2
    echo "matches nothing, so the bound of 0 below proves nothing" >&2
    fail=1
fi
if [ -n "$XFAIL" ]; then
    if [ "$n_off" -ne 0 ]; then
        echo "release-window-shape: expected red -- $n_off recover symbols without reload ($XFAIL)"
    else
        echo "release-window-shape: expected red ($XFAIL), but the count is 0 -- remove XFAIL" >&2
        fail=1
    fi
elif [ "$n_off" -ne 0 ]; then
    echo "release-window-shape: a release window without reload links $n_off recovery symbols:" >&2
    nm -C --size-sort -S --radix=d "$off" | grep -E "$DEFINED_RE" | sort -k2 -n -r | head -5 | sed 's/^/    /' >&2
    echo "" >&2
    echo "M36's rule 3, as amended by M52/T52.7: a release build without" >&2
    echo "\`lumen-static-winit/reload\` links no recover:: symbol." >&2
    fail=1
fi
exit "$fail"
