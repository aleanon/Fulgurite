#!/usr/bin/env bash
# Every library under crates/, tested ALONE (M43/T43.6,
# docs/plan-gate-hygiene.md §2.3).
#
# `cargo test --workspace` unifies features across the whole selection, so
# a crate whose manifest forgets a feature its tests need still passes
# there, because some neighbour turns the feature on. That is how
# `fulgurite-agent`'s tests passed while `cargo test -p
# fulgurite-agent` failed 19 of them (R13-F7): core's dev-dependency
# enabled the widgets' `observe`, and the agent's own manifest did not.
#
# Only crates/*: those are the published libraries, whose manifests someone
# else will build alone. Examples and fixtures are only ever built by this
# workspace (plan §2.3 weighs the cost).
#
#   scripts/test_crates.sh            every crate
#   scripts/test_crates.sh NAME...    just those
#
# Prints `crate: P passed, F failed` per crate (doctests included) and a
# total, and exits non-zero if any crate failed or did not build.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

if [ "$#" -gt 0 ]; then
    crates=("$@")
else
    mapfile -t crates < <(for d in crates/*/; do basename "$d"; done)
fi

t0=$(date +%s)
tp=0; tf=0; bad=0
log=$(mktemp); trap 'rm -f "$log"' EXIT
for c in "${crates[@]}"; do
    s=$(date +%s)
    cargo test -p "$c" --no-fail-fast > "$log" 2>&1
    rc=$?
    p=$(grep -oE 'test result: [A-Za-z]+\. [0-9]+ passed; [0-9]+ failed' "$log" \
        | awk '{p += $4; f += $6} END {print p + 0, f + 0}')
    read -r np nf <<< "$p"
    tp=$((tp + np)); tf=$((tf + nf))
    note=""
    if [ "$rc" -ne 0 ] && [ "$nf" -eq 0 ]; then
        note=" -- cargo exited $rc with no failed test (did not build?)"
        grep -m3 -E '^error' "$log" | sed 's/^/    /'
    fi
    [ "$rc" -ne 0 ] && bad=$((bad + 1))
    echo "$c: $np passed, $nf failed$note ($(( $(date +%s) - s )) s)"
    if [ "$nf" -gt 0 ]; then
        grep -E '^    [a-z_0-9:]+$|^test .* FAILED$' "$log" | head -5 | sed 's/^/    /'
    fi
done
echo "test-crates: ${#crates[@]} crates alone -- $tp passed, $tf failed, $bad crate(s) red ($(( $(date +%s) - t0 )) s)"
[ "$bad" -eq 0 ]
