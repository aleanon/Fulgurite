#!/usr/bin/env bash
# M36/T36.3 (docs/plan-panic-recovery.md): a panic in a live window shows
# where it happened and the window survives it — under both invalidation
# strategies, built --release WITH `reload` (rule 3). Then the build that
# must not recover: --release WITHOUT `reload` must still die, so the cfg
# is known to switch both ways. Needs a display; skips with exit 77
# without one (M43 §2).
#
# **Each leg names the process it drove** (M42/T42.2): through
# `scripts/live/launch.py`, which refuses a port that already answers and
# checks the socket's owner and the process's own `app.diagnostics` --
# strategy, and `reload` present exactly where the leg says it is. The
# last leg is expected to die, and the launcher asserts that it did.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh  # live windows on the monitor the user is not reading
LAUNCH="python3 scripts/live/launch.py --script panic-drive"
$LAUNCH --display-only || exit $?
for strategy in everything boundaries; do
    echo "== $strategy =="
    feats="agent,reload"
    expect=(--expect-strategy Boundaries --expect-features reload)
    if [ "$strategy" = everything ]; then
        feats="$feats,everything"
        expect=(--expect-strategy Everything --expect-features reload)
    fi
    cargo build --release -q -p panic-app --features "$feats"
    # Twelve panics, all provoked: the eleven sites (site 8 since M80/T80.5,
    # site 9 -- App::wake_in_ms -- since M84/T84.3, site 10 -- App::images
    # -- since M84/T84.6, site 11 -- App::answered -- since M88/T88.3), and
    # site 1 again for the timer step. Exact, so a thirteenth is a failure
    # (M42/T42.3).
    $LAUNCH --leg "$strategy" --exe target/release/panic-app --arg=--window "${expect[@]}" --expect-panics 12 \
        -- python3 scripts/panic_drive.py {port} "$strategy"
done

echo "== release without reload: must still die =="
cargo build --release -q -p panic-app --features agent
$LAUNCH --leg norecover --exe target/release/panic-app --arg=--window \
    --expect-strategy Boundaries --expect-features "" --expect-exit nonzero \
    -- python3 - {port} <<'PY'
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
echo "panic-drive: release without reload died, as it must"
