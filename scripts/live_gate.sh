#!/usr/bin/env bash
# Open the tally window, run every live check against it, close it.
# Exit code is the verdict. Needs a display (or xvfb-run).
set -euo pipefail
cd "$(dirname "$0")/.."
export LUMEN_AGENT_ADDR="${LUMEN_AGENT_ADDR:-127.0.0.1:9231}"   # not the dev window's port

cargo build -q -p counter-window
# The binary itself, not `cargo run`: the replay below checks that the
# port belongs to the pid this script launched, and `cargo run`'s pid is
# cargo's.
./target/debug/counter-window &
pid=$!
trap 'kill "$pid" 2>/dev/null || true' EXIT

python3 scripts/agent_client.py wait-port --timeout 60 >/dev/null
# M39/T39.0 (R14-F1): a browser's no-cors POST, byte for byte, must not
# move the tally. Red until T39.1 — the endpoint executes the body.
python3 scripts/browser_replay.py "$LUMEN_AGENT_ADDR" \
    crates/lumen-static-agent/tests/browser/firefox156-fetch-no-cors.bin '#value' \
    --pid "$pid" --xfail T39.1
# Warm the clocks: the first frames after launch run ~4x slower.
for _ in $(seq 20); do python3 scripts/agent_client.py click '#inc' >/dev/null; done
python3 scripts/ui_check.py all
python3 scripts/agent_client.py quit >/dev/null
wait "$pid" || true
