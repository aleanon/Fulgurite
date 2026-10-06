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
#   window costs. Until M89b/T89.10 every GPU window makes a context of
#   its own; K89b (`docs/plan-windows.md`) compares T89.10's shared
#   second window with this separate-context delta, bound < 50%.
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
            --env LUMEN_BACKEND="$backend" \
            --expect-strategy Boundaries --expect-features shaping --stop quit \
            -- python3 scripts/windows_rss.py {port} {pid} "$backend" | tee -a "$out"
        # Setup: the leg ran on the backend it names.
        line=$(grep -m1 -E '^lumen-static: (gpu \(|software)' "target/live/windows-rss-$backend-$run.log" || true)
        echo "  ..  setup: backend line '$line'"
        case "$backend:$line" in
        gpu:"lumen-static: gpu ("*|software:"lumen-static: software"*) ;;
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
print(f"windows-rss: the second window's GPU share (gpu delta - software delta) "
      f"{mb(g[2] - sw[2]):.1f} MB; K89b's half of each: {mb(g[2]) / 2:.1f} MB raw, "
      f"{mb(g[2] - sw[2]) / 2:.1f} MB GPU share")
PY
