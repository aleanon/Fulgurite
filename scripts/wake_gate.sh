#!/usr/bin/env bash
# M28/T28.1: a completion wakes an idle window. The live half.
#
# `crates/lumen-static/tests/inbox_wake.rs` asserts that sending asks for
# a wake, with no window. This asserts the ask reaches a real winit loop
# sleeping in `ControlFlow::Wait` and that a frame follows.
#
# The fixture asserts itself and owns its own timeout -- see its crate
# docs for why this cannot be a `cargo test`. Skips (exit 0) without a
# display, which is the configuration CI runs in, exactly as
# scripts/agent_drive.sh and scripts/live_gate.sh do.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh  # live windows on the monitor the user is not reading

if [ -z "${DISPLAY:-}" ]; then
    echo "wake-gate: no X display — skipped"
    exit 0
fi

cargo build -q -p wake-gate
# The fixture exits non-zero on its own deadline, so a hang here is a
# bug in the fixture rather than the thing under test; `timeout` bounds
# that too, and 30s is six times the fixture's own 5s deadline.
timeout 30 ./target/debug/wake-gate
