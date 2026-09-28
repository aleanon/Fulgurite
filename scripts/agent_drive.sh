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
. scripts/drive_window.sh  # live windows on the monitor the user is not reading
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
# then `skipped > 0` (M53/T53.3: the idle fast path was judged too),
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
# That press was a whole click on the trigger, so the menu is open again.
# Close it with Escape: left open, the next click anywhere is the outside
# press that dismisses it, and a sidebar pick lands on nothing.
assert menu_box(), "agent-drive: the press and release on the trigger did not open its menu"
call("input.key", {"key": "Escape"})
assert menu_box() is None, "agent-drive: Escape left the Select menu open"

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
# A hover, alone on its frame (M41/T41.7). Every step above is a click, a
# press or a palette swap, and each of those marks the node it touches
# for a reason of its own -- so no frame of this drive depended on the
# precise hover mark, and T34.1's own injection (skip that mark) left the
# oracle leg quiet over 30 frames. An unconsumed pointer move over a
# button whose hover shade differs from rest is the frame a hover-mark
# defect can show on, and the oracle leg judges it. (The trigger was
# tried first and stayed quiet under the injection: the menu rows'
# `hover_dim(0.0)` reaches it, so a stale hover there is invisible.)
# Whether the shade *shows* is `just widget-drive`'s Button check; this
# drive only supplies the frame.
pick("Button")
assert wid_text("title") == "Button", f"agent-drive: setup: expected the Button page, got {wid_text('title')!r}"
#
# Up to three tries: the window is on a real display, and a real pointer
# moving over it posts its own `PointerMove`, which moves the hover
# somewhere else between this drive's move and its read (seen on :0 on
# 2026-09-26, at ~700 frames/s while someone used the mouse).
b = call("ui.resolve", {"selector": "#d_button_a"})["bounds"]
for attempt in range(3):
    r = call("input.hover", {"x": (b[0] + b[2]) / 2, "y": (b[1] + b[3]) / 2})
    st = call("ui.getAppliedStyles", {"selector": "#d_button_a"})["state"]
    print(f"agent-drive: hover over #d_button_a: handled={r['handled']}, hovered={st['hovered']}")
    if st["hovered"]:
        break
    call("input.hover", {"x": 2, "y": 2})
assert r["handled"] is False, f"agent-drive: setup: the hover was consumed, so it marks for another reason: {r!r}"
assert st["hovered"], "agent-drive: setup: the hover did not land on #d_button_a"
call("input.hover", {"x": 2, "y": 2})

# ---- input.compose: an IME's frames, live (M48/T48.13) -----------------
# "abc" by preedit, then committed, through `input.compose` -- the same
# `Host::compose` the window routes `Ime::*` to. Each step is a frame the
# oracle leg judges (R1-F2's shape, which M41/T41.1 closed: a compose that
# marked nothing painted over a stale layout under `Boundaries`), and the
# field's value is read back after each. `scripts/live/shadow.py` then
# asserts, on the oracle leg, that no judged frame diverged.
pick("TextInput")
assert wid_text("title") == "TextInput", f"agent-drive: setup: expected the TextInput page, got {wid_text('title')!r}"
FIELD = "#d_textinput_a"
call("input.fill", {"selector": FIELD, "text": ""})
def shadow_compared():
    sh = call("app.shadow")
    return sh.get("compared") if sh.get("supported") else None
c0 = shadow_compared()
for params, want in [({"text": "a"}, "a"), ({"text": "ab"}, "ab"), ({"text": "abc"}, "abc"),
                     ({"text": "abc", "commit": True}, "abc")]:
    r = call("input.compose", params)
    got = call("ui.resolve", {"selector": FIELD}).get("value")
    print(f"agent-drive: compose {params}: changed {r['changed']}, field {got!r} (want {want!r})")
    assert r["changed"] is True, f"agent-drive: the compose step {params} changed nothing"
    assert got == want, f"agent-drive: after compose {params} the field reads {got!r}, want {want!r}"
c1 = shadow_compared()
if c0 is None:
    print("agent-drive: compose frames judged: no oracle in this leg")
else:
    print(f"agent-drive: compose frames judged {c1 - c0} / bound >= 2")
    assert c1 - c0 >= 2, f"agent-drive: the oracle judged {c1 - c0} compose frames, bound 2"
print(f"agent-drive: {before} -> {after} by the sidebar; Select chose {chosen}, "
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
run_one Everything agent,everything ""
run_one Boundaries agent ""
run_one Boundaries+oracle agent,shadow-oracle shadow-oracle
echo "agent-drive: both strategies, and Boundaries with and without the shadow oracle"
