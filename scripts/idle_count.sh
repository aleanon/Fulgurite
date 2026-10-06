#!/usr/bin/env bash
# M47/T47.8: the Idle section of docs/results-frame-cost.md -- what a still
# window and a window with a 120 ms timer cost over SECONDS (default 60),
# in frames, wakes, context switches and threads. See scripts/idle_count.py.
# M89/T89.6 adds the daemon leg (K89d, docs/plan-windows.md): examples/daemon,
# no window, idle with no deadline and with its 120 ms one.
# Needs a display; skips with exit 77 without one (M43 §2).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
LAUNCH="python3 scripts/live/launch.py --script idle-count"
$LAUNCH --display-only || exit $?
cargo build --release -q -p tasks-demo
$LAUNCH --leg boundaries --exe target/release/tasks-demo \
    --expect-strategy Boundaries --expect-features "" \
    -- python3 scripts/idle_count.py {port} "${1:-60}"
cargo build --release -q -p daemon
$LAUNCH --leg daemon-still --exe target/release/daemon --arg=--still \
    --expect-strategy Boundaries --expect-features "" \
    -- python3 scripts/idle_count.py {port} "${1:-60}" daemon-still
$LAUNCH --leg daemon-timer --exe target/release/daemon --arg=--steady \
    --expect-strategy Boundaries --expect-features "" \
    -- python3 scripts/idle_count.py {port} "${1:-60}" daemon-timer
