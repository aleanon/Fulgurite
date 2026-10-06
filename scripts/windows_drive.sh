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
# Phase 2 (T89.3) is examples/two-windows, under both strategies: a row
# opens a detail window answered inside the drain, the detail's counter is
# reached by the agent's `window` member and by a real XTest click, the
# detail paints its RTL label with its own program's stack, the process
# listens on one port, a panic in the detail is shown there while the list
# answers, three kinds of close each leave the list answering, and closing
# the list ends the process with status 0. Built with `reload` so a
# release window catches the detail's panic (M36), and `shaping` for the
# detail's `Shaping::Complex`; XTest needs python3-xlib, a prerequisite.
#
# Phase 3 (T89.4) is examples/two-windows again, under both strategies:
# each row's detail shows the row posted to it (`Request::Post`), a post
# reaches an idle detail and is drawn on its next frame with no input to
# it, and a post to a closed window is dropped and counted.
#
# Phase 4 (T89.5) is examples/two-windows again, under both strategies: a
# request is answered by the window it was drained from -- the detail's
# screenshot is the detail's pixels, not the list's, and `app.imeArea`
# answers each window's own caret.
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

for leg in everything boundaries; do
    echo "== phase 2: two-windows, $leg =="
    case "$leg" in
    everything)
        cargo build --release -q -p two-windows --features reload,everything
        strategy=Everything ;;
    boundaries)
        cargo build --release -q -p two-windows --features reload
        strategy=Boundaries ;;
    esac
    # The drive's last step closes the primary, so the process exits by
    # itself (`--expect-exit 0`); its one panic is the detail's, on purpose.
    $LAUNCH --leg "phase2-$leg" --exe target/release/two-windows \
        --expect-strategy "$strategy" --expect-features shaping,reload \
        --expect-exit 0 --expect-panics 1 \
        -- python3 scripts/windows_drive.py {port} 2
done

for leg in everything boundaries; do
    echo "== phase 3: two-windows, posts, $leg =="
    case "$leg" in
    everything)
        cargo build --release -q -p two-windows --features reload,everything
        strategy=Everything ;;
    boundaries)
        cargo build --release -q -p two-windows --features reload
        strategy=Boundaries ;;
    esac
    $LAUNCH --leg "phase3-$leg" --exe target/release/two-windows \
        --expect-strategy "$strategy" --expect-features shaping,reload --stop quit \
        -- python3 scripts/windows_drive.py {port} 3
done

for leg in everything boundaries; do
    echo "== phase 4: two-windows, per-window answers, $leg =="
    case "$leg" in
    everything)
        cargo build --release -q -p two-windows --features reload,everything
        strategy=Everything ;;
    boundaries)
        cargo build --release -q -p two-windows --features reload
        strategy=Boundaries ;;
    esac
    $LAUNCH --leg "phase4-$leg" --exe target/release/two-windows \
        --expect-strategy "$strategy" --expect-features shaping,reload --stop quit \
        -- python3 scripts/windows_drive.py {port} 4
done

echo "windows-drive: phase 1 passed under both strategies (app.windows [1], \"window\": 7 refused, \"window\": 1 clicks, app.quit exits 0); phase 2 passed under both (open, route, shape, one port, panic, close); phase 3 passed under both (posts: the clicked row, an idle target, a closed target); phase 4 passed under both (a screenshot and an IME area answered by their own window)"
