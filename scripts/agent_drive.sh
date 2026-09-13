#!/usr/bin/env bash
# M23/T23.1c: the agent endpoint, driving a *live* window.
#
# `cargo test` drives a `Host` headlessly, which is the gate for layout and
# handlers. This drives the window through the whole stack -- socket thread,
# event-loop proxy, backend, present -- because "the tree changed" and "the
# window shows it" are different claims, and this project has been caught by
# the gap between them before (crates/lumen-static-winit/src/surface.rs, and
# again in examples/glasswall, where a wallpaper rendered headless and was
# missing on screen).
#
# What it proves, in one run of one process:
#   * the dropdown opens on a synthesized click,
#   * picking a row swaps the demo,
#   * a press is *observable while held* -- the frame differs from the
#     released one, which is the thing a screenshot after the fact cannot show.
#
# Needs a display. Skips -- exit 0 -- without one, because that is the
# configuration CI runs in.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

if [ -z "${DISPLAY:-}" ]; then
    echo "agent-drive: no X display — skipped"
    exit 0
fi

PORT="${LUMEN_AGENT_PORT:-7655}"
cargo build --release -p showcase --features agent
LUMEN_AGENT="127.0.0.1:$PORT" ./target/release/showcase >/tmp/agent_drive.log 2>&1 &
pid=$!
trap 'kill $pid 2>/dev/null || true' EXIT
sleep 4

python3 - "$PORT" <<'PY'
import socket, json, sys, base64, hashlib
port = int(sys.argv[1])
s = socket.create_connection(("127.0.0.1", port), timeout=15)
f = s.makefile("rwb")
n = [0]
def call(method, params=None):
    n[0] += 1
    f.write((json.dumps({"jsonrpc": "2.0", "id": n[0],
                         "method": method, "params": params or {}}) + "\n").encode())
    f.flush()
    r = json.loads(f.readline())
    if "error" in r:
        sys.exit(f"agent-drive: {method} failed: {r['error']}")
    if "result" not in r:
        sys.exit(f"agent-drive: {method} answered without a result: {r}")
    res = r["result"]
    # A reply that is not the shape the method promises is the failure
    # mode of a stubbed endpoint -- it answers, and answers nothing.
    if method == "ui.getTree" and "nodes" not in res:
        sys.exit(f"agent-drive: ui.getTree answered {res!r}, not a tree")
    return res

def trigger_label():
    for node in call("ui.getTree", {"depth": 3})["nodes"]:
        if node.get("wid") == "trigger":
            return node.get("label", "")
    sys.exit("agent-drive: no #trigger in the tree")

def shot():
    r = call("ui.screenshot", {})
    return hashlib.sha256(base64.b64decode(r["image_base64"])).hexdigest()

before = trigger_label()
call("input.click", {"selector": "#trigger"})
rows = [x for x in call("ui.getTree", {"depth": 3})["nodes"] if x.get("wid") == "opt"]
assert rows, "agent-drive: the dropdown did not open"
# Row 8 is the Switch. Rows start at y = 118 and are 30 px apart.
call("input.click", {"x": 178, "y": 118 + 30 * 8 + 15})
after = trigger_label()
assert after != before, f"agent-drive: picking a row changed nothing ({before!r})"
assert "Switch" in after, f"agent-drive: expected Switch, got {after!r}"

# A press, held. The frame while held must differ from the released one --
# which is the claim a screenshot taken after a click cannot make.
b = call("ui.resolve", {"selector": "#trigger"})["bounds"]
cx, cy = (b[0] + b[2]) / 2, (b[1] + b[3]) / 2
rest = shot()
call("input.pointerDown", {"x": cx, "y": cy})
held = shot()
call("input.pointerUp", {"x": cx, "y": cy})
assert held != rest, "agent-drive: a held press looks identical to the resting frame"
print(f"agent-drive: {before.split()[0]} -> {after.split()[0]}, {len(rows)} rows, press observable")
PY
