#!/usr/bin/env bash
# M48/T48.9 (R12-F3): the widgets' `observe` is not silently optional.
#
# A widget's `observe` impl is gated on lumen-static-widgets' own feature,
# not on core's, so a build that turns on core's `observe` (every agent
# build does) without the widgets' reports every crate widget as
# `GenericContainer` -- a working agent that lies about roles, with no
# warning (docs/results-widget-crate.md, "One feature seam").
# `lumen-static-widgets` now refuses to compile in that graph.
#
# Both directions, in one run: the seam must FAIL to build, naming the
# feature to enable, and the matched build must succeed.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

log=$(mktemp)
trap 'rm -f "$log"' EXIT
if cargo check -q -p lumen-static-widgets --features lumen-static/observe >"$log" 2>&1; then
    echo "feature-seam: core observe without the widgets' observe: build succeeded / expected refusal" >&2
    exit 1
fi
msg=$(grep -m1 -oE 'enable `lumen-static-widgets/observe`[^"]*' "$log" || true)
if [ -z "$msg" ]; then
    echo "feature-seam: the build failed, but not with the seam's message:" >&2
    tail -20 "$log" >&2
    exit 1
fi
echo "feature-seam: refused: $msg"
if ! cargo check -q -p lumen-static-widgets --features observe >"$log" 2>&1; then
    echo "feature-seam: the matched build (--features observe) failed:" >&2
    tail -20 "$log" >&2
    exit 1
fi
echo "feature-seam: --features observe builds"
