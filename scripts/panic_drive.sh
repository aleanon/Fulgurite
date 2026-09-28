#!/usr/bin/env bash
# M36/T36.3 (docs/plan-panic-recovery.md): a panic in a live window shows
# where it happened and the window survives it — under both invalidation
# strategies, built --release WITH `reload` (rule 3). Then the build that
# must not recover: --release WITHOUT `reload` must still die, so the cfg
# is known to switch both ways. Needs a display; skips without one.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh  # live windows on the monitor the user is not reading
if [ -z "${DISPLAY:-}" ]; then
    echo "panic-drive: no X display — skipped"
    exit 0
fi
PORT="${LUMEN_PANIC_PORT:-7661}"
for strategy in everything boundaries; do
    echo "== $strategy =="
    feats="agent,reload"
    [ "$strategy" = everything ] && feats="$feats,lumen-static-winit/everything"
    cargo build --release -q -p panic-app --features "$feats"
    LUMEN_AGENT="127.0.0.1:$PORT" ./target/release/panic-app --window >"/tmp/panic_drive_$strategy.log" 2>&1 &
    pid=$!
    trap 'kill $pid 2>/dev/null || true' EXIT
    sleep 3
    python3 scripts/panic_drive.py "$PORT" "$strategy"
    kill $pid 2>/dev/null || true
    wait $pid 2>/dev/null || true
done

echo "== release without reload: must still die =="
cargo build --release -q -p panic-app --features agent
LUMEN_AGENT="127.0.0.1:$PORT" ./target/release/panic-app --window >/tmp/panic_drive_norecover.log 2>&1 &
pid=$!
sleep 3
python3 - "$PORT" <<'PY'
import json, socket, sys
s = socket.create_connection(("127.0.0.1", int(sys.argv[1])), timeout=5)
f = s.makefile("rwb")
def call(i, m, p):
    f.write((json.dumps({"jsonrpc": "2.0", "id": i, "method": m, "params": p}) + "\n").encode()); f.flush()
    return f.readline()
r = json.loads(call(1, "app.failure", {}))["result"]
assert r.get("supported") is False, f"a release build without reload claims recovery: {r!r}"
call(2, "input.click", {"selector": "#arm1"})
try:
    call(3, "input.click", {"selector": "#go"})
except Exception:
    pass
PY
sleep 1
if kill -0 $pid 2>/dev/null; then
    kill $pid
    echo "panic-drive: FAIL — a release build without reload survived a panic"
    exit 1
fi
echo "panic-drive: release without reload died, as it must"
