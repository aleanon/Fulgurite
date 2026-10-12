#!/usr/bin/env bash
# check_tray_cost.sh -- K90c, the tray's crate cost (M90/T90.7,
# docs/plan-desktop-integration.md; numbers in
# docs/results-desktop-integration.md T90.7).
#
# K2's command (docs/plan-iced-roadmap.md K2),
#   cargo tree -p fulgurite-winit -e normal --prefix none [--features F] \
#     | sed 's/ (\*)$//' | sort -u | wc -l
# for the default build, `tray`, `a11y` and `a11y,tray`, all printed, and
# three clauses:
#
# 1. `tray` adds no more crates than `a11y` does -- the feature whose
#    D-Bus stack it shares, and the plan's "no more than the most
#    expensive feature" bound (K90c; second review B8). The bound is
#    computed here, from `a11y`'s own count, never written down.
# 2. **Over `a11y`, `tray` adds exactly the four crates the plan names**:
#    `ksni`, `futures-channel`, `pastey` and `task-local` (second review
#    P15). Clause 1 alone could not see `ksni` built with its default
#    `tokio`: that graph is *smaller* than the `async-io` one (tokio
#    replaces the async-io stack rather than joining it), so the count
#    passed while the transport was no longer the one `a11y` already
#    links -- found by injection (6), which left clause 1 green
#    (docs/plan-desktop-integration.md, "What building found", T90.7).
# 3. No `tokio` anywhere in the `tray` graph (P15's reason for the pin).
# 4. `tray-no-tracing` adds no crate over `tray`: `tracing` is already in
#    the graph through zbus, and the feature only sets its
#    `release_max_level_off`.
#
# K90c's other two clauses are elsewhere: the stripped size is
# `scripts/check_tray_symbols.sh` (`just tray-symbols`; it builds, so it
# cannot run here), bound **3,300,000 B** without `a11y` since the owner's
# re-budget of 2026-10-07 (docs/plan-desktop-integration.md, K90c), and
# `unsafe` is `scripts/check_no_unsafe.sh`, also in `just static`.
#
# `--locked --offline`, no compile, so it runs in `just static`. Only
# cargo's stdout is counted (check_default_crates.sh says why).
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

crates() { # features -- the deduplicated crate list, K2's way
    local out
    out=$(cargo tree -p fulgurite-winit -e normal --prefix none --locked --offline ${1:+--features "$1"} 2>/dev/null) || {
        echo "check_tray_cost: cargo tree failed for features '${1:-default}' -- not a pass" >&2
        return 1
    }
    [ -n "$out" ] || { echo "check_tray_cost: cargo tree printed no crates for '${1:-default}'" >&2; return 1; }
    printf '%s\n' "$out" | sed 's/ (\*)$//' | sort -u
}
names() { sed -E 's/ v[0-9].*$//' | sort -u; }

base=$(crates "") || exit 1
tray=$(crates tray) || exit 1
a11y=$(crates a11y) || exit 1
both=$(crates a11y,tray) || exit 1
notrace=$(crates tray-no-tracing) || exit 1
n() { printf '%s\n' "$1" | wc -l; }
nb=$(n "$base"); nt=$(n "$tray"); na=$(n "$a11y"); nab=$(n "$both"); nn=$(n "$notrace")
dt=$((nt - nb)); da=$((na - nb))
over=$(comm -13 <(printf '%s\n' "$a11y" | names) <(printf '%s\n' "$both" | names) | paste -sd' ')
want="futures-channel ksni pastey task-local"
tokio=$(printf '%s\n' "$tray" | names | grep -cx 'tokio' || true)

echo "check_tray_cost: default $nb crates; tray $nt (+$dt); a11y $na (+$da); a11y,tray $nab (+$((nab - na)) over a11y) -- K2's command"
echo "check_tray_cost: (1) tray +$dt / bound a11y's +$da"
echo "check_tray_cost: (2) over a11y, tray adds: [$over] / want exactly [$want]"
echo "check_tray_cost: (3) tokio in the tray graph: $tokio / bound 0"
echo "check_tray_cost: (4) tray-no-tracing $nn crates, +$((nn - nt)) over tray / bound 0"
echo "check_tray_cost: the size clause is just tray-symbols (bound 3300000 B stripped, without a11y)"
fail=0
if [ "$dt" -gt "$da" ]; then
    echo "check_tray_cost: K90c fires -- tray adds $dt crates, more than a11y's $da" >&2; fail=1
fi
if [ "$over" != "$want" ]; then
    echo "check_tray_cost: K90c fires -- over a11y the tray adds [$over], not the four the plan names [$want]: its transport is no longer the stack a11y links" >&2; fail=1
fi
if [ "$tokio" -ne 0 ]; then
    echo "check_tray_cost: K90c fires -- tokio is in the tray's graph (ksni built with its default runtime; P15)" >&2; fail=1
fi
if [ "$nn" -ne "$nt" ]; then
    echo "check_tray_cost: K90c fires -- tray-no-tracing adds $((nn - nt)) crates over tray" >&2; fail=1
fi
exit "$fail"
