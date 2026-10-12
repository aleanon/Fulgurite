"""M47/T47.7's live step. See scripts/resume_drive.sh.

One screenshot before the window's FULGURITE_TEST_RESUME round trip, one
after, and the fraction of pixels that differ printed beside its bound.
Reads are spaced by sleeps, not polled: the hook fires on a clock of its
own, and the reads are what this compares.
"""
import base64, io, json, socket, sys, time

from PIL import Image

port, leg, resume = int(sys.argv[1]), sys.argv[2], sys.argv[3] == "resume"
# M83/T83.2: the `custom-resume` leg runs glasswall's `custom-backend`
# build, whose window backend comes from `Program::backend_with`: a
# counter around `Software` named `counting (software) #n`.
custom = leg.startswith("custom")
# A pixel glasswall's wallpaper covers and no panel does (the gap between
# the status bar and the first row of icons, between the first two
# columns), in the window's 460 x 997 physical pixels, and the page colour
# (`PAGE`) the window clears to -- what that pixel shows if the backend
# dropped the image source.
WALL_PX = (112, 105)
PAGE = (0x08, 0x0A, 0x12)
# `canvas` asks the agent for what the renderer drew; `xwd` reads the
# window back off the X server, which the GPU leg keeps for its own
# before/after diff -- so that diff never trusts the readback it checks.
source = sys.argv[4] if len(sys.argv) > 4 else "canvas"
# M88/T88.2: the GPU answers `ui.screenshot` by reading its target back
# (it answered "does not render" until then -- T88.0's pin, flipped
# here). On the GPU leg the canvas is compared with the `xwd` readback of
# the same still frame, worst channel delta against `just gpu-parity`'s
# bound of 1; on the software still leg the same comparison must read 0.
COMPARE = {"gpu-resume": 1, "software-still": 0}
BOUND = 0.001  # 0.1% of the window

s = socket.create_connection(("127.0.0.1", port), timeout=60)
f = s.makefile("rwb")
n = [0]


def call(method, params=None):
    n[0] += 1
    f.write((json.dumps({"jsonrpc": "2.0", "id": n[0], "method": method,
                         "params": params or {}}) + "\n").encode())
    f.flush()
    r = json.loads(f.readline())
    if "error" in r:
        sys.exit(f"resume-drive[{leg}]: {method} failed: {r['error']}")
    return r["result"]


def shot():
    if source == "xwd":
        return xwd_shot()
    r = call("ui.screenshot")
    return Image.open(io.BytesIO(base64.b64decode(r["image_base64"]))).convert("RGB")


def xwd_shot():
    """The window, read back off the X server -- `scripts/gpu_parity.sh`'s reader."""
    import struct, subprocess, tempfile
    call("app.diagnostics")  # the process still answers
    with tempfile.NamedTemporaryFile(suffix=".xwd") as t:
        subprocess.run(["xwd", "-name", "glasswall", "-out", t.name], check=True,
                       stderr=subprocess.DEVNULL)
        d = open(t.name, "rb").read()
    h = struct.unpack(">25I", d[:100])
    hdr, w, ht, bpp, bpl, ncolors = h[0], h[4], h[5], h[11], h[12], h[19]
    px, step = d[hdr + ncolors * 12:], bpp // 8
    img = Image.new("RGB", (w, ht))
    img.putdata([(px[y * bpl + x * step + 2], px[y * bpl + x * step + 1], px[y * bpl + x * step])
                 for y in range(ht) for x in range(w)])
    return img


t0 = time.time()
time.sleep(2.0)  # the first frame, and a margin
a = shot()

if leg in COMPARE:
    # Setup landed: the backend the leg names is the one the window made.
    started = [l for l in open(f"target/live/resume-drive-{leg}.log", errors="replace").read().splitlines()
               if l.startswith("fulgurite: ") and "(" in l and "FULGURITE_TEST_RESUME" not in l]
    want = "fulgurite: gpu (" if source == "xwd" else "fulgurite: software"
    print(f"  ..  setup: startup line {started[:1]} (want {want!r})")
    if not started or not started[0].startswith(want):
        sys.exit(f"resume-drive[{leg}]: setup: the window did not start on {want!r}: {started[:1]}")
    # Two reads of one still frame, back to back: the agent's canvas and
    # the X server's copy of the window.
    r = call("ui.screenshot")
    canvas = Image.open(io.BytesIO(base64.b64decode(r["image_base64"]))).convert("RGB")
    shown = xwd_shot()
    print(f"  ..  setup: ui.screenshot answered {canvas.size} (source {r.get('source')}), "
          f"xwd {shown.size}")
    if canvas.size != shown.size:
        sys.exit(f"resume-drive[{leg}]: the canvas is {canvas.size} and the window {shown.size}")
    pc, ps = canvas.tobytes(), shown.tobytes()
    worst = max(abs(x - y) for x, y in zip(pc, ps))
    over = sum(1 for i in range(0, len(pc), 3)
               if max(abs(pc[i + k] - ps[i + k]) for k in range(3)) > COMPARE[leg])
    print(f"  ..  canvas vs xwd: worst channel delta {worst} (bound {COMPARE[leg]}), "
          f"{over} px over it")
    if worst > COMPARE[leg]:
        sys.exit(f"resume-drive[{leg}]: ui.screenshot differs from the window by {worst} on a "
                 f"channel (bound {COMPARE[leg]}, {over} px over): the canvas is not what was drawn")
    print(f"  ok  canvas = window within {COMPARE[leg]} ({leg})")

if custom:
    log = open(f"target/live/resume-drive-{leg}.log", errors="replace").read()
    first = sum(1 for l in log.splitlines() if "counting (software)" in l)
    print(f"  ..  setup: factory backends before the resume {first} (want >= 1)")
    if first < 1:
        sys.exit(f"resume-drive[{leg}]: setup: the program's factory made no backend before the resume")
    # The "before" frame must hold the images, or a counter that drops the
    # source loses the wallpaper from both captures and the diff reads 0.
    wall = a.getpixel(WALL_PX)
    print(f"  ..  setup: wallpaper pixel {WALL_PX} is {wall}, page colour {PAGE}")
    if max(abs(x - y) for x, y in zip(wall, PAGE)) <= 8:
        sys.exit(f"resume-drive[{leg}]: setup: the first capture shows the page colour at {WALL_PX} "
                 f"-- the factory's backend drew no wallpaper (set_images dropped?)")
# The hook fires at FULGURITE_TEST_RESUME (4000 ms after the window starts);
# the second read comes well after the frame the resume asks for.
time.sleep(max(0.0, 7.0 - (time.time() - t0)))
b = shot()

# The setup must have happened: the hook fired once, and a second backend
# was made -- or, on the still leg, neither. Read from the app's own log,
# which the launcher writes per script and leg.
log = open(f"target/live/resume-drive-{leg}.log", errors="replace").read()
hooks = log.count("FULGURITE_TEST_RESUME: suspend and resume")
backends = sum(1 for l in log.splitlines() if l.startswith("fulgurite: ") and "(" in l
               and "FULGURITE_TEST_RESUME" not in l)
print(f"  ..  round trips {hooks} (want {int(resume)}), backends made {backends} "
      f"(want {1 + int(resume)})")
if (hooks, backends) != (int(resume), 1 + int(resume)):
    sys.exit(f"resume-drive[{leg}]: setup: the round trip did not happen as asked")
if custom:
    named = [l.split("fulgurite: ", 1)[1] for l in log.splitlines()
             if l.startswith("fulgurite: ") and "(" in l and "FULGURITE_TEST_RESUME" not in l]
    counted = [n for n in named if n.startswith("counting (software)")]
    second = named[1] if len(named) > 1 else None
    print(f"  ..  factory backends {len(counted)} (bound: exactly 2); backend #2 is {second!r}")
    if len(counted) != 2 or second != "counting (software) #2":
        sys.exit(f"resume-drive[{leg}]: the program's factory made {len(counted)} backends (want 2, "
                 f"one per resumed), and backend #2 is {second!r}")

if a.size != b.size:
    sys.exit(f"resume-drive[{leg}]: the window changed size {a.size} -> {b.size}")
pa, pb = a.tobytes(), b.tobytes()
total = a.size[0] * a.size[1]
diff = sum(1 for i in range(0, len(pa), 3) if pa[i:i + 3] != pb[i:i + 3])
ca, cb = len(set(a.getdata())), len(set(b.getdata()))
print(f"  ..  differing pixels: {diff} / {total} ({diff / total:.4%}, bound {BOUND:.1%}); "
      f"distinct colours before {ca}, after {cb}; resumed: {resume}; read from {source}")
if diff > BOUND * total:
    sys.exit(f"resume-drive[{leg}]: the window differs on {diff / total:.1%} of its pixels "
             f"after a suspend/resume round trip (bound {BOUND:.1%}): the images did not come back")
print(f"  ok  {leg}: {diff} differing pixels (bound {int(BOUND * total)})")
