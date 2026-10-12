#!/usr/bin/env bash
# M89b/T89.9: what each window costs resident -- `just windows-rss`.
#
# `examples/two-windows`, release, `VmRSS` 4 s after the process started
# with one window and 4 s after its second window opened, median of three
# launches (`docs/results-gpu.md` §5's method), on the GPU and on the
# software canvas. Prints the medians and each window's delta:
#
# - the first GPU window over the software one: what opening a context
#   costs (§5's 174 MB, re-read here on this example);
# - the second window over the first, on each backend: what one more
#   window costs. Since T89.10 the GPU windows share one context, and
#   K89b (`docs/plan-windows.md`) compares this delta with T89.9's
#   separate-context arm, printed both ways: the raw `VmRSS` delta (K89b
#   as first written) and the GPU share (GPU delta less the software
#   delta of the same launches), which is the quantity the owner judged
#   K89b on, 2026-10-07 (`docs/results-windows.md` T89.10).
#
# The GPU legs need a real adapter, so this runs on the user's display,
# off the monitor they read and without focus (scripts/drive_window.sh);
# a display with no adapter (Xephyr) falls back to the software canvas,
# which the setup check refuses. Each launch goes through
# scripts/live/launch.py and quits over `app.quit`. Skips (77) with no
# display.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh
LAUNCH="python3 scripts/live/launch.py --script windows-rss"
$LAUNCH --display-only || exit $?

cargo build --release -q -p two-windows --features gpu
out=$(mktemp)
trap 'rm -f "$out"' EXIT
for backend in gpu software; do
    for run in 1 2 3; do
        echo "== $backend, launch $run =="
        $LAUNCH --leg "$backend-$run" --exe target/release/two-windows \
            --env FULGURITE_BACKEND="$backend" \
            --expect-strategy Boundaries --expect-features shaping --stop quit \
            -- python3 scripts/windows_rss.py {port} {pid} "$backend" | tee -a "$out"
        # Setup: the leg ran on the backend it names.
        line=$(grep -m1 -E '^fulgurite: (gpu \(|software)' "target/live/windows-rss-$backend-$run.log" || true)
        echo "  ..  setup: backend line '$line'"
        case "$backend:$line" in
        gpu:"fulgurite: gpu ("*|software:"fulgurite: software"*) ;;
        *) echo "windows-rss: FAIL -- the $backend leg started on '$line'"; exit 1 ;;
        esac
    done
done

python3 - "$out" <<'PY'
import statistics, sys
rows = {}
for line in open(sys.argv[1]):
    if line.startswith("rss: "):
        _, leg, one, two = line.split()
        rows.setdefault(leg, []).append((int(one), int(two)))
med = {leg: (statistics.median(o for o, _ in v), statistics.median(t for _, t in v),
             statistics.median(t - o for o, t in v)) for leg, v in rows.items()}
for leg, v in rows.items():
    print(f"  ..  {leg}: one window {[o for o, _ in v]} kB, two {[t for _, t in v]} kB")
g, sw = med["gpu"], med["software"]
mb = lambda kb: kb / 1024
print(f"windows-rss: median of 3 -- software {mb(sw[0]):.1f} -> {mb(sw[1]):.1f} MB, "
      f"gpu {mb(g[0]):.1f} -> {mb(g[1]):.1f} MB")
print(f"windows-rss: first GPU window over software {mb(g[0] - sw[0]):.1f} MB; "
      f"second window: gpu +{mb(g[2]):.1f} MB, software +{mb(sw[2]):.1f} MB")
# The second window's delta holds what its own program costs on either
# backend -- two-windows' detail lays Arabic out, which loads fallback
# faces -- so the GPU's share of it is the difference of the two medians.
share = g[2] - sw[2]
print(f"windows-rss: the second window's GPU share (gpu delta - software delta) {mb(share):.1f} MB")
# K89b: against T89.9's separate-context arm (docs/results-windows.md
# T89.9, median of three: GPU +91.9 MB, of which +17.5 MB the GPU's share).
# Judged on the GPU share (owner, 2026-10-07); the raw ratio is printed
# beside it, and cannot fall below 74.5 / 91.9 = 81% (the fonts both
# backends pay).
SEP_RAW, SEP_SHARE = 91.9, 17.5
raw_ratio, share_ratio = mb(g[2]) / SEP_RAW, mb(share) / SEP_SHARE
print(f"windows-rss: K89b raw {mb(g[2]):.1f} / {SEP_RAW} MB = {raw_ratio:.0%} (bound < 50%, not judged); "
      f"GPU share {mb(share):.1f} / {SEP_SHARE} MB = {share_ratio:.0%} (bound < 50%: "
      f"{'holds' if share_ratio < 0.5 else 'FIRES'})")
PY
