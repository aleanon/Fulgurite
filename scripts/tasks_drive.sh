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
# M47/T47.3 (R7-F5) adds a seventh: a tick must land while a 400 ms job
# occupies one of the pool's workers. M88/T88.1 adds a sipper: a click on
# `#sip` streams 10,000 values through `Sender::progress`, the label
# must reach `sip 9999`, and the distinct labels read must not outnumber
# the frames `app.presents` counted across the run.
#
# M88/T88.2 adds a 16x16 swatch at the window's origin and a GPU leg:
# `ui.screenshot`, a click on `#swatch`, a second `ui.screenshot`, and
# the swatch's `ui.regionHash` must differ between them -- the readback
# is per call. The software legs run the same phase (their hashes must
# differ too), and the GPU leg also asks `source: "surface"`, which must
# stay "not checkable" there. The GPU leg exits 77 with no adapter, as
# `just gpu-parity` does.
#
# M88/T88.3 adds screenshot REQUESTS on every leg, one per drain site:
# `#shot` pushes one from its handler (answered after the agent call that
# clicked it; the label must name this click's colour), and `#shot-timer`
# from two timer ticks (answered by the redraw drain alone, tick 2's on a
# frame that presented nothing; one read after an unobserved 1.5 s, both
# answers tick 1's colour, and under Boundaries exactly 3 presented and
# >= 1 skipped). On a GPU leg whose display is asleep the presents bound
# is printed as not checkable (the answers still are).
#
# M88/T88.4 adds the CLIPBOARD on the software legs: the drive puts `A`
# on the clipboard and `B` on the primary selection through the app's
# own `ClipboardWrite` requests (each must answer `Written`), reads both
# back through `ClipboardRead`, round-trips an HTML clip byte for byte,
# and checks the primary selection with an independent
# `xclip -o -selection primary`; a second write (`C`) must leave primary
# at `B`. **It writes the desktop's clipboard and primary selection.**
#
# M88/T88.5 adds an IMAGE leg: tasks-demo built with `clipboard-image`
# writes an 8x4 image through `Clip::Image` and reads it back as
# `ClipFormat::Image`; it must read 8x4 with the written pixels' hash.
# The software legs, built without the feature, must answer both
# `Refused(Unsupported)` -- not `Refused(Empty)`.
#
# **Required tool: `xclip`** (owner decision D7,
# docs/plan-iced-roadmap.md §3.4) -- install it with
#     sudo apt install xclip
# It is a prerequisite, not a capability: the primary leg calls
# `scripts/probe.sh xclip` first and propagates its exit 1, so a missing
# xclip FAILS the drive with that line and never reports skipped. The
# drive's own 77 stays for a missing display only (M43 §2). Both
# directions run every time: the preflight is run once with xclip taken
# off PATH, and must exit non-zero, non-77, with the install line.
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
. scripts/drive_window.sh  # live windows on the monitor the user is not reading
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
# M88/T88.4, D7's refuse direction: the primary leg's preflight with
# xclip off PATH (a PATH holding only what probe.sh itself needs) must
# FAIL -- non-zero, non-77 -- and print the install line. The accept
# direction is every leg's clipboard phase below, with xclip present.
shadow=$(mktemp -d)
ln -s "$(command -v bash)" "$shadow/bash"
ln -s "$(command -v dirname)" "$shadow/dirname"
pre_rc=0
pre_out=$(PATH="$shadow" "$(command -v python3)" scripts/tasks_drive.py --xclip-preflight 2>&1) || pre_rc=$?
rm -rf "$shadow"
echo "$pre_out" | sed 's/^/  (xclip off PATH) /'
if [ "$pre_rc" = 0 ]; then
    echo "tasks-drive: the primary leg's preflight passed with xclip off PATH -- it checks nothing"; exit 1
elif [ "$pre_rc" = 77 ]; then
    echo "tasks-drive: with xclip off PATH the primary leg exited 77, a skip -- want non-77 (D7: a missing prerequisite fails, never skips)"; exit 1
elif ! grep -q 'sudo apt install xclip' <<<"$pre_out"; then
    echo "tasks-drive: with xclip off PATH the primary leg failed (exit $pre_rc) without the install line"; exit 1
fi
echo "  ok  refuse direction: xclip off PATH -> exit $pre_rc (non-zero, non-77) with the install line"

for leg in everything boundaries boundaries+oracle; do
    echo "== $leg =="
    # `Boundaries` is the default since M34/T34.2; `everything` is the
    # diagnostic feature that selects the whole-tree walk.
    case "$leg" in
    everything)
        cargo build --release -q -p tasks-demo --features everything
        expect=(--expect-strategy Everything --expect-features "") ;;
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

# M88/T88.5: the image round trip, on a build with `clipboard-image`.
echo "== clipboard-image =="
cargo build --release -q -p tasks-demo --features clipboard-image
$LAUNCH --leg clipboard-image --exe target/release/tasks-demo \
    --expect-strategy Boundaries --expect-features "" \
    -- python3 scripts/tasks_drive.py {port} boundaries image

# M88/T88.2: the pixel phases on wgpu. One strategy: what is checked is
# the backend's readback, which the strategy does not reach.
echo "== gpu =="
cargo build --release -q -p tasks-demo --features gpu
rc=0
$LAUNCH --leg gpu --exe target/release/tasks-demo --env LUMEN_BACKEND=gpu \
    --expect-strategy Boundaries --expect-features "" \
    -- python3 scripts/tasks_drive.py {port} boundaries gpu || rc=$?
if [ "$rc" = 77 ]; then
    echo "tasks-drive: no adapter -- the GPU leg skipped (77)"
    exit 77
elif [ "$rc" != 0 ]; then
    exit "$rc"
fi

echo "tasks-drive: all five capabilities landed under both strategies, and under the shadow oracle; the clipboard round-tripped text, HTML, primary and an image; the GPU answered ui.screenshot per call"
