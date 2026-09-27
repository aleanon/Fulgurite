#!/usr/bin/env bash
# M48/T48.9 (R12-F3), extended to `a11y` by M54/T54.1: the widgets'
# accessibility impls are not silently optional.
#
# A widget's `observe` impl is gated on lumen-static-widgets' own feature,
# not on core's, so a build that turns on core's feature without the
# widgets' reports every crate widget as `GenericContainer` -- a working
# agent, or screen reader, that lies about roles, with no warning
# (docs/results-widget-crate.md, "One feature seam"). Since M54 the impls
# are behind `a11y`, which `observe` implies, so there are two seams:
# core's `observe` without the widgets' `observe`, and core's `a11y`
# without the widgets' `a11y`. `lumen-static-widgets` refuses to compile
# in either graph.
#
# Both directions per feature, in one run: the seam must FAIL to build,
# naming the feature to enable, and the matched build must succeed.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

log=$(mktemp)
trap 'rm -f "$log"' EXIT
for f in observe a11y; do
    if cargo check -q -p lumen-static-widgets --features "lumen-static/$f" >"$log" 2>&1; then
        echo "feature-seam: core $f without the widgets' $f: build succeeded / expected refusal" >&2
        exit 1
    fi
    msg=$(grep -m1 -oE "enable \`lumen-static-widgets/$f\`[^\"]*" "$log" || true)
    if [ -z "$msg" ]; then
        echo "feature-seam: core $f alone: the build failed, but not with the seam's message:" >&2
        tail -20 "$log" >&2
        exit 1
    fi
    echo "feature-seam: core $f alone refused: $msg"
    if ! cargo check -q -p lumen-static-widgets --features "$f" >"$log" 2>&1; then
        echo "feature-seam: the matched build (--features $f) failed:" >&2
        tail -20 "$log" >&2
        exit 1
    fi
    echo "feature-seam: --features $f builds"
done
