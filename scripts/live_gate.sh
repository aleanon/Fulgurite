#!/usr/bin/env bash
# Open the tally window, run every live check against it, close it.
# Exit code is the verdict. Needs a display (or xvfb-run).
set -euo pipefail
. "$(dirname "$0")/drive_window.sh"  # live windows on the monitor the user is not reading
cd "$(dirname "$0")/.."
export LUMEN_AGENT_ADDR="${LUMEN_AGENT_ADDR:-127.0.0.1:9231}"   # not the dev window's port

cargo build -q -p counter-window
cargo run -q -p counter-window &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT

python3 scripts/agent_client.py wait-port --timeout 60 >/dev/null
# Warm the clocks: the first frames after launch run ~4x slower.
for _ in $(seq 20); do python3 scripts/agent_client.py click '#inc' >/dev/null; done
python3 scripts/ui_check.py all
python3 scripts/agent_client.py quit >/dev/null
wait "$pid" || true
