"""M47/T47.7's live step. See scripts/resume_drive.sh.

One screenshot before the window's LUMEN_TEST_RESUME round trip, one
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
# window back off the X server, for a backend that cannot hand its
# pixels back (the GPU one answers "does not render" to `ui.screenshot`).
source = sys.argv[4] if len(sys.argv) > 4 else "canvas"
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
# The hook fires at LUMEN_TEST_RESUME (4000 ms after the window starts);
# the second read comes well after the frame the resume asks for.
time.sleep(max(0.0, 7.0 - (time.time() - t0)))
b = shot()

# The setup must have happened: the hook fired once, and a second backend
# was made -- or, on the still leg, neither. Read from the app's own log,
# which the launcher writes per script and leg.
log = open(f"target/live/resume-drive-{leg}.log", errors="replace").read()
hooks = log.count("LUMEN_TEST_RESUME: suspend and resume")
backends = sum(1 for l in log.splitlines() if l.startswith("lumen-static: ") and "(" in l
               and "LUMEN_TEST_RESUME" not in l)
print(f"  ..  round trips {hooks} (want {int(resume)}), backends made {backends} "
      f"(want {1 + int(resume)})")
if (hooks, backends) != (int(resume), 1 + int(resume)):
    sys.exit(f"resume-drive[{leg}]: setup: the round trip did not happen as asked")
if custom:
    named = [l.split("lumen-static: ", 1)[1] for l in log.splitlines()
             if l.startswith("lumen-static: ") and "(" in l and "LUMEN_TEST_RESUME" not in l]
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
