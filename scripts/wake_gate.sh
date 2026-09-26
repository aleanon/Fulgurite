#!/usr/bin/env bash
# M28/T28.1: a completion wakes an idle window. The live half.
#
# `crates/lumen-static/tests/inbox_wake.rs` asserts that sending asks for
# a wake, with no window. This asserts the ask reaches a real winit loop
# sleeping in `ControlFlow::Wait` and that a frame follows.
#
# The fixture asserts itself and owns its own timeout -- see its crate
# docs for why this cannot be a `cargo test`. It has no agent endpoint,
# so `scripts/live/launch.py` (M42/T42.2) runs it with `--no-agent`: the
# display check, the per-worktree log, and the fixture's own exit status
# as the verdict. Skips with exit 77 without a display (M43 §2).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
LAUNCH="python3 scripts/live/launch.py --script wake-gate"
$LAUNCH --display-only || exit $?

cargo build -q -p wake-gate
# The fixture exits non-zero on its own deadline, so a hang here is a
# bug in the fixture rather than the thing under test; the launcher's
# 30 s bound covers that too, six times the fixture's own 5 s deadline.
$LAUNCH --no-agent --exe target/debug/wake-gate --stop wait --timeout 30
