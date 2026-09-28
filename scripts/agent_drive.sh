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
. scripts/drive_window.sh  # live windows on the monitor the user is not reading

if [ -z "${DISPLAY:-}" ]; then
    echo "agent-drive: no X display — skipped"
    exit 0
fi

# **Both invalidation strategies** — M34/T34.0. This drive ran under
# `Everything` only (the window's default until M34), so it said nothing
# about the frames a `Boundaries` window draws; the other drive scripts
# already ran both legs. The body is not indented: its heredoc's
# terminator must start the line.
run_one() {
PORT="${LUMEN_AGENT_PORT:-7655}"
cargo build --release -p showcase --features "$2"
RUST_BACKTRACE=1 LUMEN_AGENT="127.0.0.1:$PORT" ./target/release/showcase >/tmp/agent_drive.log 2>&1 &
pid=$!
trap 'kill $pid 2>/dev/null || true' EXIT
echo "== $1 =="
sleep 4

python3 - "$PORT" <<'PY'
import socket, json, sys, base64, hashlib
port = int(sys.argv[1])
s = socket.create_connection(("127.0.0.1", port), timeout=15)
f = s.makefile("rwb")
n = [0]
def try_call(method, params=None):
    """A request whose error is an answer, not a failure."""
    n[0] += 1
    f.write((json.dumps({"jsonrpc": "2.0", "id": n[0],
                         "method": method, "params": params or {}}) + "\n").encode())
    f.flush()
    return json.loads(f.readline())
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

# `select-trigger` and `select-menu` are the ids `Select` stamps on its
# own nodes -- a composite is more than one node and the derive names only
# a field, so a widget that is several has to publish its parts or nothing
# can address them: not a stylesheet, not this script. Since M56 the only
# `Select` in the showcase is its own demo, reached through the sidebar.
TRIGGER = "#select-trigger"
MENU = "#select-menu"

def wid_text(wid):
    for node in call("ui.getTree", {"depth": 12})["nodes"]:
        if node.get("wid") == wid:
            return node.get("value") or node.get("label") or ""
    sys.exit(f"agent-drive: no #{wid} in the tree")

def menu_box():
    """The menu's bounds, or None when it is closed."""
    r = call("ui.resolve", {"selector": MENU})
    b = r.get("bounds")
    if not b or b[2] - b[0] <= 0 or b[3] - b[1] <= 0:
        return None
    return b

def shot():
    r = call("ui.screenshot", {})
    return hashlib.sha256(base64.b64decode(r["image_base64"])).hexdigest()

# **Pick an entry by the text its sidebar row shows** (M27/T27.0, M56).
# `~=` is exact: `~Select` would also match a row whose name only starts
# with it. Reorder the catalogue and this still picks the Switch.
before = wid_text("title")
call("input.click", {"selector": "~=Switch"})
after = wid_text("title")
assert after != before, f"agent-drive: picking a sidebar row changed nothing ({before!r})"
assert after == "Switch", f"agent-drive: expected the Switch page, got {after!r}"

def pick(name):
    """Scroll the sidebar until `name`'s row is announced, then click it.
    A repeater announces only the rows in its viewport (F4.7), so a row
    below the fold has to be scrolled to -- as a user would."""
    nav = call("ui.resolve", {"selector": "#nav"})["bounds"]
    mid = ((nav[0] + nav[2]) / 2, (nav[1] + nav[3]) / 2)
    for _ in range(40):
        r = try_call("ui.resolve", {"selector": f"~={name}"})
        b = r.get("result", {}).get("bounds") if "result" in r else None
        if b and b[1] >= nav[1] and b[3] <= nav[3]:
            call("input.click", {"selector": f"~={name}"})
            return
        call("input.scroll", {"x": mid[0], "y": mid[1], "dy": -64.0})
    sys.exit(f"agent-drive: the {name} row never scrolled into the sidebar")

# The dropdown, as a demo now: open it, choose a row by its text, and the
# trigger and the readout follow while the menu closes.
pick("Select")
assert wid_text("title") == "Select", f"agent-drive: the Select page did not open: {wid_text('title')!r}"
assert menu_box() is None, "agent-drive: the Select demo's menu is open before anything was clicked"
call("input.click", {"selector": TRIGGER})
box = menu_box()
assert box, "agent-drive: the dropdown did not open"
call("input.click", {"selector": "~=Cherry"})
chosen, readout = wid_text("select-trigger"), wid_text("readout")
assert chosen == "Cherry", f"agent-drive: the trigger reads {chosen!r} after choosing Cherry"
assert readout == "selected: Cherry", f"agent-drive: the readout reads {readout!r}"
assert menu_box() is None, "agent-drive: choosing a row left the menu open"

# A press, held. The frame while held must differ from the released one --
# which is the claim a screenshot taken after a click cannot make.
b = call("ui.resolve", {"selector": TRIGGER})["bounds"]
cx, cy = (b[0] + b[2]) / 2, (b[1] + b[3]) / 2
rest = shot()
call("input.pointerDown", {"x": cx, "y": cy})
held = shot()
call("input.pointerUp", {"x": cx, "y": cy})
assert held != rest, "agent-drive: a held press looks identical to the resting frame"

# M25/T25.3 and T25.11: the running window takes a palette, and the
# showcase's sheet names roles, so each swap re-resolves them. Asserted on
# the header's resolved background *and* on the frame's pixels: `applied`
# alone was the check before any rule named a role, and it would pass for a
# host that stored the palette and re-cascaded nothing.
# The sidebar's surface since M56: the header no longer has a ground.
def header_bg():
    return call("ui.getAppliedStyles", {"selector": "#sidebar"})["background"]
prev_bg, prev_shot = header_bg(), shot()
for name in ("dark", "light"):
    r = call("app.setPalette", {"name": name})
    assert r.get("applied") is True, f"agent-drive: the window did not take the {name} palette: {r!r}"
    bg, frame = header_bg(), shot()
    print(f"agent-drive: palette {name}: sidebar background {prev_bg} -> {bg}")
    assert bg != prev_bg, f"agent-drive: the {name} palette did not re-resolve the sidebar's role"
    assert frame != prev_shot, f"agent-drive: the {name} palette left the frame's pixels unchanged"
    prev_bg, prev_shot = bg, frame
print(f"agent-drive: {before} -> {after} by the sidebar; Select chose {chosen}, "
      f"menu {int(box[2]-box[0])}x{int(box[3]-box[1])} px, press observable")
# M34/T34.1: the shadow oracle, when this build has it. The count is
# asserted before the silence, because a quiet oracle that compared
# nothing proves nothing.
sh = call("app.shadow")
if sh.get("supported"):
    assert sh["compared"] > 0, f"agent-drive: the shadow oracle never ran: {sh!r}"
    assert sh["first"] is None, f"agent-drive: a bounded frame diverged from the whole-tree one: {sh['first']!r}"
    print(f"agent-drive: shadow oracle compared {sh['compared']} frames, none diverged")
PY
kill $pid 2>/dev/null || true
wait $pid 2>/dev/null || true
}

run_one Everything agent,everything
run_one Boundaries agent,shadow-oracle
echo "agent-drive: both strategies"
