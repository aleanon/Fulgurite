#!/usr/bin/env bash
# M35/T35.0 (docs/plan-observe-cost.md): does compiling `observe` in stop
# a row widget's `layout` and `paint` being inlined into its container?
#
# The symbol table is the gate: it answers yes or no, on any machine, with
# or without `perf`. Two containers — `List` and `Repeat`, the two whose
# observe walks reach statically typed rows — each built plain and with
# `observe`, UNSTRIPPED (the harness profile strips by default). Any
# standalone `Label<S> as Node>::{layout,paint}` symbol fails the gate,
# with its size printed.
#
#   check_observe_inlining.sh            the gate
#   check_observe_inlining.sh --aa       both builds plain — must pass
#   check_observe_inlining.sh --clock    also record the frame arm's clock
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)/benches-competitive"
mode="${1:-gate}"
second="--features observe"
[ "$mode" = --aa ] && second=""
export CARGO_PROFILE_RELEASE_STRIP=false
CARGO_TARGET_DIR=target/probe-plain cargo build -q --release -p observe-probe
# shellcheck disable=SC2086
CARGO_TARGET_DIR=target/probe-second cargo build -q --release -p observe-probe $second
fail=0
for build in plain second; do
    for bin in list repeat; do
        syms=$(nm -C -S "target/probe-$build/release/$bin" \
            | grep -E 'Label<.*> as lumen_static::widget::Node>::(layout|paint)$' || true)
        if [ -n "$syms" ]; then
            echo "FAIL $build/$bin: a row widget's layout/paint is not inlined:"
            echo "$syms" | awk '{n=$4; for(i=5;i<=NF;i++) n=n" "$i; printf "    %s (0x%s bytes)\n", n, $2}'
            fail=1
        else
            echo "ok   $build/$bin: row layout and paint inlined"
        fi
    done
done
if [ "$mode" = --clock ]; then
    # The statistic (plan T35.0): the delta between the two builds'
    # fast-cluster minima, beside the plain build's fast-cluster width.
    # The fast cluster is every run within 5% of that build's minimum —
    # a rule in the script, not a judgement by eye.
    CARGO_TARGET_DIR=target/clock-plain cargo build -q --release -p lumen-static-frame
    CARGO_TARGET_DIR=target/clock-obs cargo build -q --release -p lumen-static-frame --features lumen-static-widgets/observe
    for i in 1 2 3 4 5 6 7 8; do
        for b in plain obs; do
            printf "%s " $b
            "target/clock-$b/release/lumen-static-frame" 3000 200 \
                | awk '/\/layout / && !/ALL/{printf "%s ", $(NF-3)} /\/\+paint / && !/ALL|culled/{printf "%s", $(NF-3)}'
            echo
        done
    done | python3 -c '
import sys
runs = {"plain": [], "obs": []}
for line in sys.stdin:
    b, lay, paint = line.split()
    runs[b].append((float(lay), float(paint)))
for k, name in ((0, "layout"), (1, "+paint")):
    fast = {}
    for b, v in runs.items():
        xs = sorted(x[k] for x in v)
        fast[b] = [x for x in xs if x <= xs[0] * 1.05]
    p, o = fast["plain"], fast["obs"]
    width, delta = p[-1] - p[0], o[0] - p[0]
    print("clock %s: plain %.2f-%.2f us, observe %.2f-%.2f us; "
          "delta of minima %+.2f us beside a plain width of %.2f us"
          % (name, p[0], p[-1], o[0], o[-1], delta, width))
'
fi
exit $fail
