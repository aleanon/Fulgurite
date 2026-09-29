#!/usr/bin/env bash
# Kill rule K3 of docs/plan-lss.md (M61): **a release window without
# `reload` links no `.lss` reader.**
#
# A `.lss` sheet is `const` rules in every build (`include_lss!`), and the
# reader -- `lumen_static_lss`'s parser and lowering, reached through
# `lumen_static_shell::lss` -- is for re-reading the file while a debug or
# `reload` window runs. The window crate calls `Program::live_lss` under
# its own `cfg(any(debug_assertions, feature = "reload"))`, and nothing
# else names the reader, so a shipped window should drop it at link time.
#
# This builds the tally -- the first program that styles itself from a
# `.lss` (`Program::style_lss_by`) -- in release twice and counts the
# symbols whose demangled name contains `lumen_static_lss`:
#
#   * default features (no `reload`): the count must be 0;
#   * `--features lumen-static-winit/reload`: the count must be ABOVE 0,
#     so the gating is shown switching both ways. A 0 there would mean the
#     pattern matches nothing -- a gate that could not have gone red.
#
# Printed beside `.text`, and the binary is the one cargo reports having
# built (scripts/lib/firmware.sh says why).
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

PATTERN='lumen_static_lss'

build() {
    cargo build --release -q -p counter-window "$@" --message-format=json | python3 -c '
import json, sys
exe = None
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") == "compiler-artifact" and m.get("target", {}).get("name") == "counter-window" \
            and m.get("executable"):
        exe = m["executable"]
if not exe:
    sys.exit("lss-release: cargo reported no executable for counter-window")
print(exe)
'
}

# count BIN -> "<symbols> <text>"
count() {
    local n text
    n=$(nm -C "$1" | grep -cF "$PATTERN" || true)
    text=$(size -A "$1" | awk '$1 == ".text" {print $2}')
    echo "$n $text"
}

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
off=$(build) || exit 1
cp "$off" "$tmp/off" && off="$tmp/off"
on=$(build --features lumen-static-winit/reload) || exit 1

read -r n_off t_off <<<"$(count "$off")"
read -r n_on t_on <<<"$(count "$on")"
echo "lss-release: counter-window release, no reload: $n_off $PATTERN symbols (bound 0), .text $t_off B"
echo "lss-release: counter-window release, reload:    $n_on $PATTERN symbols (bound > 0), .text $t_on B"

rc=0
if [ "$n_off" -ne 0 ]; then
    echo "lss-release: K3 fails -- a release window without reload links the .lss reader:" >&2
    nm -C "$off" | grep -F "$PATTERN" | head -5 | sed 's/^/    /' >&2
    rc=1
fi
if [ "$n_on" -eq 0 ]; then
    echo "lss-release: the reload build links no $PATTERN symbol either -- the pattern or the build is wrong, so the 0 above proves nothing" >&2
    rc=1
fi
exit "$rc"
