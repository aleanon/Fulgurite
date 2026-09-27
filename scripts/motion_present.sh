#!/usr/bin/env bash
# M53/T53.0's live row and T53.4's gate (R7-F6, docs/plan-frame-cost.md):
# a pointer moving over a region it changes nothing in must present
# nothing. Real pointer motion, through XTest -- the agent's own input
# path pumps and presents on its own, so it cannot ask this question --
# over examples/showcase, under both invalidation strategies. Per leg,
# scripts/motion_present.py prints
#
#   presents N for M motions (bound 2)
#
# and checks the accepting cases: a motion that crosses a Button's edge
# presents exactly one frame, and a window covered and uncovered by
# another shows its own pixels on the surface afterwards.
#
# Needs a display and python3-xlib; skips with exit 77 without a display
# (M43 §2). Launched through scripts/live/launch.py (M42/T42.2).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
LAUNCH="python3 scripts/live/launch.py --script motion-present"
$LAUNCH --display-only || exit $?
python3 -c 'import Xlib.ext.xtest' 2>/dev/null || { echo "motion-present: python3-xlib (XTest) is not installed"; exit 77; }

# **A private X server** when Xephyr exists: real pointer motion needs
# the pointer to be over this window, and on a shared display it may not
# be -- another agent's window, or a locked screen's saver, sits on top
# and every motion goes to it (found on the first run: 0 redraws for 200
# motions, under cinnamon-screensaver). Xephyr is a display of its own,
# with no window manager and nothing else on it. Without Xephyr the
# current display is used, and the drive refuses to count unless the
# pointer is over the window.
if command -v Xephyr >/dev/null; then
    for n in $(seq 71 99); do [ -e /tmp/.X11-unix/X$n ] || break; done
    Xephyr :$n -screen 1280x900 -ac -nolisten tcp >target/live/motion-present-xephyr.log 2>&1 &
    xephyr=$!
    trap 'kill $xephyr 2>/dev/null || true' EXIT
    for _ in $(seq 50); do [ -e /tmp/.X11-unix/X$n ] && break; sleep 0.1; done
    [ -e /tmp/.X11-unix/X$n ] || { echo "motion-present: Xephyr :$n did not start"; exit 1; }
    export DISPLAY=:$n
    unset WAYLAND_DISPLAY
    echo "motion-present: on a private Xephyr display :$n"
fi

run_one() { # LEG CARGO-FEATURES EXPECTED-FEATURES
    cargo build --release -q -p showcase --features "$2"
    echo "== $1 =="
    $LAUNCH --leg "$1" --exe target/release/showcase \
        --expect-strategy "$1" --expect-features "$3" \
        -- python3 scripts/motion_present.py {port} "$1"
}
run_one Everything agent,everything everything
run_one Boundaries agent ""
echo "motion-present: both strategies"
