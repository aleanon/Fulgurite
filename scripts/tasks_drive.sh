#!/usr/bin/env bash
# M28/T28.12: drive examples/tasks-demo through the agent endpoint and
# check that each of spawn / batch / chain / cancel / subscribe landed
# AND had its consequence -- under BOTH invalidation strategies.
#
# AGENT.md, after M24: an end-to-end assembly is the only thing that can
# find a capability which is present, correct in isolation, and
# unreachable. Every piece of M28 has a passing test; none of them had
# been used together by an application through a real window.
#
# M47/T47.1 (R7-F1, R7-F2): the last two phases are UNOBSERVED -- one
# read after a sleep with no RPC traffic. Every agent reply ends in a
# redraw request, so the earlier phases watch a window that is never
# idle, and they stayed green with the waker uninstalled and with the
# deadline ignored. A check of a wake or a deadline must not poll.
#
# Needs a display. Skips -- exit 0 -- without one, like the other live
# gates.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

if [ -z "${DISPLAY:-}" ]; then
    echo "tasks-drive: no X display -- skipped"
    exit 0
fi

PORT="${LUMEN_TASKS_PORT:-7658}"

# The strategy is a COMPILE-TIME feature of lumen-static-winit, not a
# runtime switch, so this builds twice. Both arms matter: M25/T25.0 and
# M26/T26.1 were both defects that `Everything` hid, because it walks
# the tree regardless of what was marked.
for strategy in everything boundaries; do
    echo "== $strategy =="
    # `Boundaries` is the default since M34/T34.2; `everything` is the
    # diagnostic feature that selects the whole-tree walk.
    if [ "$strategy" = everything ]; then
        cargo build --release -q -p tasks-demo --features everything
    else
        cargo build --release -q -p tasks-demo
    fi
    LUMEN_AGENT="127.0.0.1:$PORT" ./target/release/tasks-demo \
        >"/tmp/tasks_drive_$strategy.log" 2>&1 &
    pid=$!
    trap 'kill $pid 2>/dev/null || true' EXIT
    sleep 3
    python3 scripts/tasks_drive.py "$PORT" "$strategy"
    kill $pid 2>/dev/null || true
    wait $pid 2>/dev/null || true
    PORT=$((PORT + 1))
done

echo "tasks-drive: all five capabilities landed under both strategies"
