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
# **Each leg names the process it drove** (M42/T42.2,
# docs/plan-drive-identity.md): `scripts/live/launch.py` refuses a port
# that already answers, launches the binary itself, and checks the socket's
# owner and the process's own `app.diagnostics` against the leg's declared
# strategy and features before the drive below runs.
#
# Needs a display. Skips with exit 77 without one (M43 §2).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
LAUNCH="python3 scripts/live/launch.py --script agent-drive"
$LAUNCH --display-only || exit $?

# **Both invalidation strategies** — M34/T34.0. This drive ran under
# `Everything` only (the window's default until M34), so it said nothing
# about the frames a `Boundaries` window draws; the other drive scripts
# already ran both legs. The body is not indented: its heredoc's
# terminator must start the line. The heredoc is the launcher's stdin,
# which it hands to the drive.
#
# run_one LEG CARGO-FEATURES EXPECTED-FEATURES -- the expected features are
# declared apart from the build's, so a leg built wrong is refused rather
# than re-described. LEG is the strategy the process must report, with a
# `+oracle` suffix on the leg that builds the shadow oracle in (M41/T41.2b).
#
# After the drive, `scripts/live/shadow.py` asks the running window's
# oracle the leg's question (M42/T42.4, one copy for every drive): a leg
# that declares `shadow-oracle` asserts `supported`, then `compared > 0`,
# then no divergence; any other leg asserts `supported: false`.
run_one() {
cargo build --release -p showcase --features "$2"
echo "== $1 =="
$LAUNCH --leg "$1" --exe target/release/showcase \
    --expect-strategy "${1%%+*}" --expect-features "$3" \
    -- bash -c 'python3 - "$1"; r=$?; python3 scripts/live/shadow.py agent-drive || exit 1; exit $r' \
       _ "{port}" <<'PY'
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

# `select-trigger` and `select-menu` are the ids `Select` stamps on its
# own nodes -- a composite is more than one node and the derive names only
# a field, so a widget that is several has to publish its parts or nothing
# can address them: not a stylesheet, not this script.
TRIGGER = "#select-trigger"
MENU = "#select-menu"

def trigger_label():
    for node in call("ui.getTree", {"depth": 4})["nodes"]:
        if node.get("wid") == "select-trigger":
            return node.get("label", "")
    sys.exit(f"agent-drive: no {TRIGGER} in the tree")

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

before = trigger_label()
assert menu_box() is None, "agent-drive: the menu is already open before anything was clicked"
call("input.click", {"selector": TRIGGER})
box = menu_box()
assert box, "agent-drive: the dropdown did not open"

# **Pick the row by the text it shows** (M27/T27.0). This was
# `{"y": box[1] + ROW_H * 8 + ROW_H / 2}` until 2026-09-18 -- arithmetic
# over a resolved box, because every action method refused a repeater row
# and said so: "selector resolved to virtualized content; pass x/y
# instead". Taking that advice is the first vacuous shape AGENT.md names:
# reorder the catalogue and row 8 is a different widget, silently, with
# the assertion below still green because it only ever checked that
# *something* changed.
call("input.click", {"selector": "~Switch"})
after = trigger_label()
assert after != before, f"agent-drive: picking a row changed nothing ({before!r})"
assert "Switch" in after, f"agent-drive: expected Switch, got {after!r}"
assert menu_box() is None, "agent-drive: picking a row left the menu open"

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
def header_bg():
    return call("ui.getAppliedStyles", {"selector": "#header"})["background"]
prev_bg, prev_shot = header_bg(), shot()
for name in ("dark", "light"):
    r = call("app.setPalette", {"name": name})
    assert r.get("applied") is True, f"agent-drive: the window did not take the {name} palette: {r!r}"
    bg, frame = header_bg(), shot()
    print(f"agent-drive: palette {name}: header background {prev_bg} -> {bg}")
    assert bg != prev_bg, f"agent-drive: the {name} palette did not re-resolve the header's role"
    assert frame != prev_shot, f"agent-drive: the {name} palette left the frame's pixels unchanged"
    prev_bg, prev_shot = bg, frame
print(f"agent-drive: {before.split()[0]} -> {after.split()[0]}, "
      f"menu {int(box[2]-box[0])}x{int(box[3]-box[1])} px, press observable")
PY
}

# **Three legs** (M41/T41.2b, docs/plan-boundaries-repair.md §0 rule 2).
# Until 2026-09-26 the only `Boundaries` leg built the shadow oracle in,
# and the oracle's second pass lays the live widgets out again after the
# frame it judges -- so a defect was reported once and then repaired, and
# no drive judged `Boundaries` as it ships. The plain leg is that build;
# the oracle leg is a separate binary, which the launcher's feature check
# proves (a plain leg that reports `shadow-oracle` is refused).
run_one Everything agent,everything everything
run_one Boundaries agent ""
run_one Boundaries+oracle agent,shadow-oracle shadow-oracle
echo "agent-drive: both strategies, and Boundaries with and without the shadow oracle"
