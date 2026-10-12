#!/usr/bin/env bash
# M0 gate: the release core contains no `unsafe` at all
# (docs/plan-architecture-v2.md, docs/results-safe-prototype.md).
#
# **The compiler is the gate; this script is the early warning.** Since
# M43/T43.1 every crate under crates/ except fulgurite-reload carries
# `#![forbid(unsafe_code)]`, which no inner `allow` can override and which
# covers what a text scan cannot see: a `#[path]` module outside `src/`,
# and code a build script writes into OUT_DIR and the crate `include!`s
# (R4-F4 -- both bypassed this script at 1717808). This script runs before
# any compile, so a stray `unsafe` costs a second to find rather than a
# build, and it checks that the attribute is PRESENT, because removing it
# is the one edit the compiler cannot refuse.
#
# One limit: `forbid` in fulgurite-macros covers the macro crate's own
# code, not the tokens its derives emit. Emitted code is checked by the
# `forbid` of whichever crate uses the derive -- for the core's own tests,
# the core.
#
# The scan is literal-aware (scripts/lib/rustsrc.py): string, raw-string
# and char literals are blanked BEFORE comments are stripped. The previous
# version stripped from the first `//` on a line, so
# `let s = "http://x"; unsafe { .. }` hid its `unsafe` (R17-F2).
#
# Two exclusions, both because the code *is* the FFI boundary:
#   - fixtures/hot-host: dlopen requires unsafe, and it is a fixture (it is
#     not under crates/, so neither the scan nor the presence check reads it).
#   - crates/fulgurite-reload: the `extern "C"` shims a reloadable app
#     exports. They exist in their own crate precisely so this check can stay
#     absolute over the core rather than growing a cfg-gated exception --
#     which is how a gate stops meaning anything.
# Nothing else is excluded, and adding one means editing this list in a commit
# that has to say why.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

EXEMPT=fulgurite-reload
status=0

# --- 1. the attribute is present on every crate but the FFI one ---------
crates=0; have=0
for lib in crates/*/src/lib.rs; do
    c=$(basename "$(dirname "$(dirname "$lib")")")
    [ "$c" = "$EXEMPT" ] && continue
    crates=$((crates + 1))
    if [ -n "$(python3 scripts/lib/rustsrc.py grep '^\s*#!\[forbid\(unsafe_code\)\]' "$lib")" ]; then
        have=$((have + 1))
    else
        echo "$c: $lib does not carry #![forbid(unsafe_code)]" >&2
        status=1
    fi
done

# --- 2. no `unsafe` token in code, literals and comments blanked --------
mapfile -t files < <(find crates -path '*/src/*' -name '*.rs' -not -path "crates/$EXEMPT/*" | sort)
hits=$(python3 scripts/lib/rustsrc.py grep '\bunsafe\b' "${files[@]}")
if [ -n "$hits" ]; then
    echo "$hits" | sed 's/^\([^:]*:[0-9]*\):/\1: unsafe: /' >&2
    status=1
fi
n=${#files[@]}

if [ "$status" -ne 0 ]; then
    echo "" >&2
    echo "release core must contain no unsafe, and every crate but $EXEMPT must forbid it (see above)" >&2
    exit 1
fi
echo "no-unsafe: clean across $n files in crates/*/src (except $EXEMPT, the FFI crate); forbid: $have/$crates crates"
