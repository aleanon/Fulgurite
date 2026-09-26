#!/usr/bin/env bash
# Open the tally window, run every live check against it, close it.
# Exit code is the verdict. Needs a display; skips with exit 77 without
# one (M43 §2).
#
# The window is launched by `scripts/live/launch.py` (M42/T42.2): the
# binary itself, not `cargo run`, on a free port (never the dev window's),
# refused if the port already answers, and checked -- socket owner and
# `app.diagnostics`' process block -- to be the pid it launched. The
# checks below run as the launcher's drive: this script again, with
# `--drive`, and LUMEN_AGENT_ADDR pointing at that window.
#
# **The app's exit status is part of the verdict** (M42/T42.3, R18-F9).
# After the checks the launcher reads the window's log (`app log: 0
# panics`), then asks it to quit over `app.quit` and waits: anything but
# `app exit status: 0` fails the run as `app exited N`. This ended in
# `wait "$pid" || true` until then, so a panic on the way out -- the
# review's I6 -- was discarded and the gate stayed green.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ "${1:-}" != --drive ]; then
    LAUNCH="python3 scripts/live/launch.py --script live-gate"
    $LAUNCH --display-only || exit $?
    cargo build -q -p counter-window
    exec $LAUNCH --bin target/debug/counter-window --stop quit \
        --expect-strategy Boundaries --expect-features "" \
        -- "$0" --drive
fi

# M39/T39.1 (R14-F1): a browser's no-cors POST, byte for byte, must not
# move the tally -- and the first click below must (the accept half).
python3 scripts/browser_replay.py "$LUMEN_AGENT_ADDR" \
    crates/lumen-static-agent/tests/browser/firefox156-fetch-no-cors.bin '#value' \
    --pid "$LUMEN_LIVE_PID"
before=$(python3 scripts/agent_client.py call ui.resolve '{"selector":"#value"}' | python3 -c 'import json,sys; print(json.load(sys.stdin)["value"])')
python3 scripts/agent_client.py click '#inc' >/dev/null
after=$(python3 scripts/agent_client.py call ui.resolve '{"selector":"#value"}' | python3 -c 'import json,sys; print(json.load(sys.stdin)["value"])')
echo "live-gate: an ordinary click moves the tally: #value $before -> $after"
[ "$before" != "$after" ] || { echo "live-gate: the accept click did not land" >&2; exit 1; }
# Warm the clocks: the first frames after launch run ~4x slower.
for _ in $(seq 20); do python3 scripts/agent_client.py click '#inc' >/dev/null; done
python3 scripts/ui_check.py all
