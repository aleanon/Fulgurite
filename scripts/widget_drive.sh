#!/usr/bin/env bash
# M27/T27.3: every widget, driven through every action it accepts,
# against the live window -- under BOTH invalidation strategies.
#
# `cargo test` drives a `Host` headlessly and `just agent-drive` proves
# the socket reaches a real window. Neither asks the question this does:
# for each widget, is every action a user can perform actually wired, does
# it do the specific thing it claims, and does the same input just outside
# the widget do nothing?
#
# Twice, because M26 found five defects and **four were invisible under
# the default `Everything`**: a clean subtree is skipped before a wrong
# answer can be observed. A protocol written after M26 that runs only the
# default has ignored M26's finding.
#
# Needs a display. Skips -- exit 0 -- without one, like every other live
# gate here.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh  # live windows on the monitor the user is not reading

if [ -z "${DISPLAY:-}" ]; then
    echo "widget-drive: no X display — skipped"
    exit 0
fi

ONLY="${1:-}"
PORT="${LUMEN_AGENT_PORT:-7656}"
status=0

run_one() {
    local strategy="$1" features="$2"
    cargo build --release -p showcase --features "$features" >/dev/null
    RUST_BACKTRACE=1 LUMEN_AGENT="127.0.0.1:$PORT" \
        ./target/release/showcase >"/tmp/widget_drive_$strategy.log" 2>&1 &
    local pid=$!
    # Wait for the socket rather than sleeping a guess: a fixed sleep is
    # how this script would start reporting "connection refused" the day
    # the machine is busy.
    LUMEN_AGENT_ADDR="127.0.0.1:$PORT" python3 scripts/agent_client.py wait-port --timeout 60
    if LUMEN_AGENT_ADDR="127.0.0.1:$PORT" LUMEN_DRIVE_STRATEGY="$strategy" \
        python3 scripts/drive/run.py "$ONLY"; then
        :
    else
        status=1
    fi
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
}

run_one Everything  agent,everything
run_one Boundaries  agent
# M57/D6: a third pass at scale 2, where fractional logical edges land on
# physical pixels, and every action's damaged frame is compared with a
# full repaint of the same state.
WINIT_X11_SCALE_FACTOR=2 LUMEN_DRIVE_FULLCHECK=1 run_one Scale2 agent

exit "$status"
