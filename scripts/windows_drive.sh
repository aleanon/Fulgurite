#!/usr/bin/env bash
# M89 (docs/plan-windows.md): more than one window, through ONE agent
# port -- `just windows-drive`. Each phase is `scripts/windows_drive.py`'s.
#
# Phase 1 (T89.2) is one window, examples/counter-window, under both
# strategies: `app.windows` lists [1], a request naming `"window": 7` is
# -32602 naming `app.windows`, a click naming `"window": 1` lands and its
# reply names window 1, and `app.quit` exits 0 in under 2 s (the
# launcher's `--stop quit`). It inverts T89.0's phase 0, which pinned the
# one-window router.
#
# **Each leg names the process it drove** (M42/T42.2): through
# `scripts/live/launch.py`. Needs a display; skips with exit 77 without
# one (M43 §2). Opens one window per leg (scripts/drive_window.sh places
# it on the monitor the user is not reading, without focus).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh  # live windows on the monitor the user is not reading
LAUNCH="python3 scripts/live/launch.py --script windows-drive"
$LAUNCH --display-only || exit $?

for leg in everything boundaries; do
    echo "== phase 1: counter-window, one window, $leg =="
    case "$leg" in
    everything)
        cargo build --release -q -p counter-window --features everything
        strategy=Everything ;;
    boundaries)
        cargo build --release -q -p counter-window
        strategy=Boundaries ;;
    esac
    $LAUNCH --leg "phase1-$leg" --exe target/release/counter-window \
        --expect-strategy "$strategy" --expect-features "" --stop quit \
        -- python3 scripts/windows_drive.py {port} 1
done

echo "windows-drive: phase 1 passed under both strategies (app.windows [1], \"window\": 7 refused, \"window\": 1 clicks, app.quit exits 0)"
