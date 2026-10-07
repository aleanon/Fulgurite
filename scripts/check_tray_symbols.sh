#!/usr/bin/env bash
# check_tray_symbols.sh -- M90/T90.7 (docs/plan-desktop-integration.md; K2
# and K90c, docs/plan-iced-roadmap.md): a window built without `tray`
# links no `ksni`, and **K90c's size clause**, the stripped bytes the
# feature costs. `scripts/check_tray_cost.sh` is K90c's crate clause, which
# needs no compile and so runs in `just static`; this one builds.
#
# Modelled on scripts/check_clipboard_image_symbols.sh. Every build is
# examples/showcase, stripped with GNU strip on a copy as
# docs/results-a11y-platform.md measured `a11y`:
#
# - **K90c's measurement, as written: release profile, without `a11y`**
#   (`--no-default-features`: the showcase's `a11y` is its one default
#   feature, so the D-Bus stack `a11y` already links is not in both
#   sides), plain and with `lumen-static-winit/tray`. Bound
#   **3,300,000 B** -- the owner's re-budget of 2026-10-07 (option 1),
#   which replaced K2's 3 MB for plain `tray` when the first build measured
#   +3,286,360 B (docs/plan-desktop-integration.md, K90c and "What building
#   found"). Printed beside it, not bounded: the same pair with `a11y`.
# - `tray-no-tracing` (opt-in: it compiles every `tracing` call in the
#   whole release build out, the application's included), both pairs,
#   informational.
# - The size recipe (`book/src/platform.md`, "The tray"): the workspace's
#   `release-small` profile, which is `release` plus `opt-level = "s"` for
#   zbus, zvariant and ksni, both pairs, informational. An application gets
#   it only by writing the three lines in its own top-level manifest. And
#   the two together, the smallest tray an application can choose.
#
# `nm -C` counts `ksni::` symbols in the default showcase (bound 0) and in
# the tray build, the positive control, which must not be 0 or the count is
# blind (B7's falsifiable form of "default-off is free"). The tray is
# reachable from every window -- `Config::tray` is a run-time answer -- so
# the linker keeps it whether or not the program declares one.
#
# Twelve release builds, so it is not in `static`; `just tray-symbols` runs
# it, and `check-extra` includes it. `LUMEN_TRAY_BOUND` overrides the size
# bound, for the gate's own red run only. Numbers:
# docs/results-desktop-integration.md T90.7.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
out=target/tray-symbols
mkdir -p "$out"
bound=${LUMEN_TRAY_BOUND:-3300000}
count() { nm -C "$1" 2>/dev/null | grep -cE '(^|[ <(&*,])ksni::' || true; }
size() { strip -o "$1.stripped" "$1" && stat -c %s "$1.stripped"; }
build() { # name profile cargo-args...
    local name=$1 profile=$2
    shift 2
    cargo build -q --profile "$profile" -p showcase "$@"
    cp "target/$profile/showcase" "$out/$name"
}
build plain release --no-default-features
build tray release --no-default-features --features lumen-static-winit/tray
build notrace release --no-default-features --features lumen-static-winit/tray-no-tracing
build a11y release
build a11y-tray release --features lumen-static-winit/tray
build a11y-notrace release --features lumen-static-winit/tray-no-tracing
build small release-small --no-default-features
build small-tray release-small --no-default-features --features lumen-static-winit/tray
build small-a11y release-small
build small-a11y-tray release-small --features lumen-static-winit/tray
build small-notrace release-small --no-default-features --features lumen-static-winit/tray-no-tracing
build small-a11y-notrace release-small --features lumen-static-winit/tray-no-tracing
declare -A s
for f in plain tray notrace a11y a11y-tray a11y-notrace small small-tray small-a11y small-a11y-tray small-notrace small-a11y-notrace; do
    s[$f]=$(size "$out/$f")
done
plain_k=$(count "$out/a11y"); plain_n=$(count "$out/plain"); tray_k=$(count "$out/tray")
d=$((s[tray] - s[plain]))
echo "tray-symbols: ksni:: symbols in the default showcase: $plain_k (without a11y: $plain_n) / bound 0 (tray build: $tray_k, must not be 0)"
echo "tray-symbols: K90c, without a11y: stripped showcase ${s[plain]} B -> ${s[tray]} B with tray, delta $d B / bound $bound B"
echo "tray-symbols: with a11y (printed, not bounded): ${s[a11y]} B -> ${s[a11y-tray]} B, delta $((s[a11y-tray] - s[a11y])) B"
echo "tray-symbols: tray-no-tracing (opt-in; silences the application's tracing too): without a11y delta $((s[notrace] - s[plain])) B, with a11y $((s[a11y-notrace] - s[a11y])) B"
echo "tray-symbols: the recipe (release-small: opt-level \"s\" for zbus, zvariant, ksni; informational): without a11y ${s[small]} B -> ${s[small-tray]} B, delta $((s[small-tray] - s[small])) B; with a11y delta $((s[small-a11y-tray] - s[small-a11y])) B"
echo "tray-symbols: the recipe and tray-no-tracing together (informational): without a11y delta $((s[small-notrace] - s[small])) B, with a11y $((s[small-a11y-notrace] - s[small-a11y])) B"
fail=0
if [ "$tray_k" -eq 0 ]; then
    echo "tray-symbols: the tray build links no ksni:: symbols -- the count is blind" >&2; fail=1
fi
if [ "$plain_k" -ne 0 ] || [ "$plain_n" -ne 0 ]; then
    echo "tray-symbols: a build without tray links ksni" >&2; fail=1
fi
if [ "$d" -gt "$bound" ]; then
    echo "tray-symbols: K90c fires -- the tray costs $d B stripped without a11y, over the $bound B budget" >&2; fail=1
fi
exit "$fail"
