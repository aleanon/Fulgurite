"""M53/T53.0's live row and T53.4's gate -- docs/plan-frame-cost.md (R7-F6).

Launched by `scripts/motion_present.sh` against `examples/showcase`, one
strategy per leg. Counts, not clocks: `app.presents` is the window's own
count of frames presented and of redraws that presented nothing, answered
without drawing a frame, so reading it does not move it.

1. **Motion over nothing.** 200 XTest motions over a 40x40 px (or
   smaller) region inside `#blurb`, a label: `presents N for M motions
   (bound 2)`. The redraws those motions asked for are printed beside it
   and must be at least 20, or the motions never reached the window and
   the bound proves nothing (AGENT.md: the setup silently did not
   happen).
2. **Motion across a Button's edge** (accept): from 30 px left of
   `#d_button_a` into it, in 12 steps -- exactly one present, the hover
   shade.
3. **An expose** (accept): an override-redirect window covers this one
   and goes away; the surface read back from the server must then equal
   the canvas. Under a compositor there is no expose at all, and the
   check still holds.

Usage: motion_present.py PORT LEG
"""
import base64, hashlib, json, os, socket, sys, time

from Xlib import X, display
from Xlib.ext import xtest

port, leg = int(sys.argv[1]), sys.argv[2]
pid = int(os.environ["FULGURITE_LIVE_PID"])
s = socket.create_connection(("127.0.0.1", port), timeout=15)
f = s.makefile("rwb")
n = [0]


def call(method, params=None):
    n[0] += 1
    f.write((json.dumps({"jsonrpc": "2.0", "id": n[0], "method": method,
                         "params": params or {}}) + "\n").encode())
    f.flush()
    r = json.loads(f.readline())
    if "result" not in r:
        sys.exit(f"motion-present [{leg}]: {method} failed: {r}")
    return r["result"]


d = display.Display()
root = d.screen().root
PID = d.intern_atom("_NET_WM_PID")


def find(w):
    try:
        p = w.get_full_property(PID, X.AnyPropertyType)
        if p and p.value[0] == pid and w.get_attributes().map_state == X.IsViewable:
            return w
        for c in w.query_tree().children:
            r = find(c)
            if r:
                return r
    except Exception:
        pass
    return None


# A fresh window is not idle: the window manager maps, places and exposes
# it. Wait for it to settle before counting anything.
time.sleep(3.0)
win = find(root)
if win is None:
    sys.exit(f"motion-present [{leg}]: no mapped window with _NET_WM_PID {pid}")
g = win.get_geometry()
o = win.translate_coords(root, 0, 0)
ox, oy = -o.x, -o.y  # the window's origin in root coordinates
scale = call("ui.screenshot", {}).get("scale", 1.0)


def toplevel(w):
    while True:
        parent = w.query_tree().parent
        if parent.id == root.id:
            return w
        w = parent


def raise_window():
    """On a shared display other windows may sit over this one; a motion
    over them never reaches it. Raised, and then checked (`over_us`)."""
    top = toplevel(win)
    top.configure(stack_mode=X.Above)
    d.sync()
    time.sleep(0.3)


def over_us():
    """Whether the pointer is over this window and not another's."""
    w, q = root, root.query_pointer()
    while q.child:
        if q.child.id == win.id:
            return True
        w = q.child
        q = w.query_pointer()
    return w.id == win.id


def to_root(x, y):
    return int(ox + x * scale), int(oy + y * scale)


def move(x, y):
    rx, ry = to_root(x, y)
    xtest.fake_input(d, X.MotionNotify, x=rx, y=ry)
    d.sync()


def presents():
    r = call("app.presents")
    return r["presents"], r["skipped"]


def bounds(sel):
    b = call("ui.resolve", {"selector": sel}).get("bounds")
    if not b or b[2] - b[0] <= 2 or b[3] - b[1] <= 2:
        sys.exit(f"motion-present [{leg}]: {sel} has no usable bounds: {b}")
    return b


# The Button demo, chosen through the agent before anything is counted,
# as `scripts/agent_drive.sh` chooses it: since M56 by its sidebar row,
# found by the exact text it shows (`~=`, so `Button` is not a prefix
# match), and the page title says whether the pick landed.
call("input.click", {"selector": "~=Button"})
titles = [nd.get("value") or nd.get("label") for nd in call("ui.getTree", {"depth": 64})["nodes"]
          if nd.get("wid") == "title"]
assert titles == ["Button"], f"motion-present [{leg}]: setup: expected the Button page, got {titles!r}"
time.sleep(0.5)

# 1. Motion over nothing.
raise_window()
b = bounds("#blurb")
w, h = min(40, b[2] - b[0] - 2), min(40, b[3] - b[1] - 2)
x0, y0 = b[0] + 1, b[1] + 1
move(x0, y0)
time.sleep(0.5)
assert over_us(), f"motion-present [{leg}]: another window is under the pointer at #blurb"
p0, k0 = presents()
M = 200
for i in range(M):
    # A boustrophedon over the region, one step per motion.
    row, col = divmod(i, 20)
    col = col if row % 2 == 0 else 19 - col
    move(x0 + col * w / 19, y0 + (row % 10) * h / 9)
    time.sleep(0.004)
time.sleep(0.8)
p1, k1 = presents()
N, skipped = p1 - p0, k1 - k0
print(f"motion-present [{leg}]: presents {N} for {M} motions (bound 2); "
      f"redraws that presented nothing {skipped} (floor 20); region {w:.0f}x{h:.0f} px in #blurb",
      flush=True)
assert N + skipped >= 20, f"motion-present [{leg}]: only {N + skipped} redraws for {M} motions -- the motions did not reach the window"
assert N <= 2, f"motion-present [{leg}]: {N} presents for {M} motions over a region that changes nothing (bound 2)"

# 2. Across a Button's edge: exactly one present.
b = bounds("#d_button_a")
cy = (b[1] + b[3]) / 2
start, end = b[0] - 30, b[0] + 12
move(start, cy)
time.sleep(0.5)
p0, _ = presents()
for i in range(13):
    move(start + (end - start) * i / 12, cy)
    time.sleep(0.02)
time.sleep(0.6)
p1, _ = presents()
print(f"motion-present [{leg}]: crossing #d_button_a's edge in 12 steps presented {p1 - p0} frame(s) (want 1)",
      flush=True)
assert p1 - p0 == 1, f"motion-present [{leg}]: crossing a button's edge presented {p1 - p0} frames, not 1"

# 3. An expose: cover and uncover, then the surface must equal the canvas.
def sha(src):
    params = {"source": src} if src else {}
    r = call("ui.screenshot", params)
    return hashlib.sha256(base64.b64decode(r["image_base64"])).hexdigest()


control = sha("surface") == sha(None)
print(f"motion-present [{leg}]: before covering, surface {'==' if control else '!='} canvas", flush=True)
time.sleep(0.5)
p0, k0 = presents()
cover = root.create_window(ox, oy, g.width, g.height, 0, d.screen().root_depth,
                           X.InputOutput, X.CopyFromParent, background_pixel=d.screen().black_pixel,
                           override_redirect=True)
cover.map()
d.sync()
time.sleep(0.4)
cover.destroy()
d.sync()
time.sleep(0.8)
p1, k1 = presents()

surface, canvas = sha("surface"), sha(None)
print(f"motion-present [{leg}]: covered and uncovered: {p1 - p0} present(s), {k1 - k0} redraw(s) skipped; "
      f"surface {'==' if surface == canvas else '!='} canvas", flush=True)
assert surface == canvas, f"motion-present [{leg}]: after an expose the surface does not show the window's frame"
print(f"motion-present [{leg}]: pass", flush=True)
