#!/usr/bin/env bash
# M89 (docs/plan-windows.md): more than one window, through ONE agent
# port -- `just windows-drive`. Each phase is `scripts/windows_drive.py`'s.
#
# Phase 0 (T89.0) pins today's one-window behaviour on
# examples/counter-window: `app.windows` is -32601, a request carrying
# `"window": 7` is answered by the one window anyway, and `app.quit` exits
# 0 in under 2 s (the launcher's `--stop quit`). Each pin names the task
# that inverts it. One leg: phase 0 asks the window crate's router, which
# the invalidation strategy does not reach, and counter-window has no
# `everything` build; the phases that drive application behaviour (T89.2
# on) run under both strategies.
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

echo "== phase 0: counter-window, one window =="
cargo build --release -q -p counter-window
$LAUNCH --leg phase0 --exe target/release/counter-window \
    --expect-strategy Boundaries --expect-features "" --stop quit \
    -- python3 scripts/windows_drive.py {port} 0

echo "windows-drive: phase 0 pinned (app.windows -32601, \"window\" ignored, app.quit exits 0)"
