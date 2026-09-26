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
# **Each leg names the process it drove** (M42/T42.2): through
# `scripts/live/launch.py`, which refuses a port that already answers and
# checks the socket's owner and the process's own `app.diagnostics`
# against the leg's strategy. That makes the legs *identifiable*; it does
# not make the last two phases observe an idle window (R7-F2, M47).
#
# Needs a display. Skips with exit 77 without one (M43 §2).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
LAUNCH="python3 scripts/live/launch.py --script tasks-drive"
$LAUNCH --display-only || exit $?

# The strategy is a COMPILE-TIME feature of lumen-static-winit, not a
# runtime switch, so this builds twice. Both arms matter: M25/T25.0 and
# M26/T26.1 were both defects that `Everything` hid, because it walks
# the tree regardless of what was marked.
#
# A third leg builds the shadow oracle in (M41/T41.7): it lays the live
# widgets out again after each frame it judges, so it is never the
# `Boundaries` leg itself, only a leg beside it. `scripts/live/shadow.py`
# asks each window's oracle the leg's question after the drive.
for leg in everything boundaries boundaries+oracle; do
    echo "== $leg =="
    # `Boundaries` is the default since M34/T34.2; `everything` is the
    # diagnostic feature that selects the whole-tree walk.
    case "$leg" in
    everything)
        cargo build --release -q -p tasks-demo --features everything
        expect=(--expect-strategy Everything --expect-features everything) ;;
    boundaries)
        cargo build --release -q -p tasks-demo
        expect=(--expect-strategy Boundaries --expect-features "") ;;
    boundaries+oracle)
        cargo build --release -q -p tasks-demo --features lumen-static-winit/shadow-oracle
        expect=(--expect-strategy Boundaries --expect-features shadow-oracle) ;;
    esac
    $LAUNCH --leg "$leg" --exe target/release/tasks-demo "${expect[@]}" \
        -- env LUMEN_TASKS_PID={pid} bash -c \
           'python3 scripts/tasks_drive.py "$1" "$2"; r=$?
            python3 scripts/live/shadow.py tasks-drive || exit 1; exit $r' \
           _ {port} "${leg%%+*}"
done

echo "tasks-drive: all five capabilities landed under both strategies, and under the shadow oracle"
