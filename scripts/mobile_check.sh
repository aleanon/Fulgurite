#!/usr/bin/env bash
# `just mobile-check` -- M54/T54.0, R12-F6 (docs/plan-a11y-platform.md
# T54.8): does each crate CHECK for Android? `cargo check` type-checks
# and does not link, so it needs no NDK linker -- which is the claim
# docs/mobile-decision.md made about it and got wrong.
#
# One line per configuration, with cargo's exit status, and the first
# error line when it failed. Manual, like the GPU recipes: it builds the
# whole graph for another target (minutes, cold). Exits non-zero if any
# configuration marked `must` failed.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
TARGET=${MOBILE_TARGET:-aarch64-linux-android}
log=$(mktemp); trap 'rm -f "$log"' EXIT
bad=0

# NAME | MUST (1 = a failure fails the recipe) | cargo arguments
CONFIGS=(
    "lumen-static|1|-p lumen-static"
    "lumen-static-widgets|1|-p lumen-static-widgets"
    "lumen-static-shell|1|-p lumen-static-shell"
    "lumen-static-winit (no defaults, android-native-activity)|1|-p lumen-static-winit --no-default-features --features winit/android-native-activity"
    "lumen-static-winit (+ reload, hot, agent)|1|-p lumen-static-winit --no-default-features --features winit/android-native-activity,reload,hot,agent"
    "lumen-static-winit (default features)|0|-p lumen-static-winit --features winit/android-native-activity"
)

for c in "${CONFIGS[@]}"; do
    name=${c%%|*}; rest=${c#*|}; must=${rest%%|*}; args=${rest#*|}
    # shellcheck disable=SC2086
    cargo check -q --target "$TARGET" $args >"$log" 2>&1
    rc=$?
    if [ "$rc" -eq 0 ]; then
        echo "mobile-check: $name: ok (exit 0)"
    else
        first=$(grep -m1 -E '^error' "$log" | cut -c1-140)
        note=""; [ "$must" = 1 ] || note=" -- expected, not gated"
        echo "mobile-check: $name: FAILED (exit $rc)$note: ${first:-see cargo}"
        [ "$must" = 1 ] && bad=$((bad + 1))
    fi
done
echo "mobile-check: $TARGET -- $bad required configuration(s) failed"
[ "$bad" -eq 0 ]
