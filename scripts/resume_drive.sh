#!/usr/bin/env bash
# M47/T47.7 (R15-F6): a window's images survive suspend and resume.
#
# `resumed` makes a new window and backend every time. Until T47.7 there
# was no `suspended` handler and the image source was taken once, so a
# second `resumed` drew every image as nothing. A desktop never sends
# that pair, so the window runs it on itself: a DEBUG build started with
# LUMEN_TEST_RESUME=4000 suspends and resumes once, 4 s in. The drive
# screenshots `examples/glasswall` (the example with images) before and
# after and prints the differing pixels beside a 0.1% bound.
#
# Legs: software, with the round trip; the same with no round trip (the
# comparison must read 0 there, or it is sensitive to something else);
# a program's own backend from `Program::backend_with` (M83/T83.2), which
# must be made twice, keep the wallpaper and answer `ui.screenshot`;
# and the GPU backend with the round trip, its before/after diff read
# back with `xwd`, which is independent of the readback being checked.
# M88/T88.2: the GPU answers `ui.screenshot` now, and its canvas must
# match the `xwd` readback of the same still frame within 1 per channel
# (the software still leg's must match exactly).
#
# M89b/T89.9: `two-windows-gpu`, a debug examples/two-windows on the GPU
# with both windows open across the round trip, each read back with
# `xwd -id` and its differing pixels printed against 0.1%, and the
# backends counted: two before it and two made by it, one per window
# (scripts/resume_windows.py). K89a reads this leg.
#
# Each leg names the process it drove (M42/T42.2, scripts/live/launch.py).
# Needs a display; skips with exit 77 without one (M43 §2).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
LAUNCH="python3 scripts/live/launch.py --script resume-drive"
$LAUNCH --display-only || exit $?

cargo build -q -p glasswall --features agent
for leg in software-resume software-still; do
    echo "== $leg =="
    env=(--env LUMEN_BACKEND=software)
    mode=still
    if [ "$leg" = software-resume ]; then env+=(--env LUMEN_TEST_RESUME=4000); mode=resume; fi
    $LAUNCH --leg "$leg" --exe target/debug/glasswall "${env[@]}" \
        --expect-strategy Boundaries --expect-features "" \
        -- python3 scripts/resume_drive.py {port} "$leg" "$mode"
done

# M83/T83.2: a program's own backend (`Program::backend_with`), built
# without `gpu`, so `pick_backend`'s `not(gpu)` branch is the one that asks
# the factory. `LUMEN_BACKEND=auto` because `software` skips a factory.
# One leg, not one per strategy: the factory runs in `resumed` before the
# session's first frame, where the host's strategy cannot reach.
echo "== custom-resume =="
cargo build -q -p glasswall --features agent,custom-backend
$LAUNCH --leg custom-resume --exe target/debug/glasswall \
    --env LUMEN_BACKEND=auto --env LUMEN_TEST_RESUME=4000 \
    --expect-strategy Boundaries --expect-features "" \
    -- python3 scripts/resume_drive.py {port} custom-resume resume

echo "== gpu-resume =="
cargo build -q -p glasswall --features agent,gpu
$LAUNCH --leg gpu-resume --exe target/debug/glasswall \
    --env LUMEN_BACKEND=gpu --env LUMEN_TEST_RESUME=4000 \
    --expect-strategy Boundaries --expect-features "" \
    -- python3 scripts/resume_drive.py {port} gpu-resume resume xwd

# M89b/T89.9: two windows, one GPU backend each, across one round trip.
# Placed off the monitor the user reads, without focus, as the other
# multi-window drives are (scripts/drive_window.sh), in a subshell so the
# glasswall legs keep their placement.
echo "== two-windows-gpu =="
cargo build -q -p two-windows --features gpu
(
    . scripts/drive_window.sh
    $LAUNCH --leg two-windows-gpu --exe target/debug/two-windows \
        --env LUMEN_BACKEND=gpu --env LUMEN_TEST_RESUME=8000 \
        --expect-strategy Boundaries --expect-features shaping \
        -- python3 scripts/resume_windows.py {port} {pid} {log}
)

echo "resume-drive: images survived suspend and resume on every leg, and two windows on the GPU came back"
