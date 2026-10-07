"""M89b/T89.9: `just resume-drive`'s `two-windows-gpu` leg. See
scripts/resume_drive.sh.

`examples/two-windows`, a DEBUG build on the GPU with
LUMEN_TEST_RESUME=8000 (not glasswall's 4000: a debug launch opens two
GPU contexts, one per window, in most of 4 s, and the first reads must
land before the round trip -- `docs/plan-windows.md`, What building
found, T89.9): the list (window 1) opens the detail (window 2),
both are read back off the X server with `xwd -id` before the window's
own suspend/resume round trip and again after it, and each window's
differing pixels are printed beside a 0.1% bound. `resumed` makes a new
X window per slot, so each window is found again after the round trip
(by its process and its title) rather than read through a stale id.

The leg also counts the backends made: one per window before the round
trip and one per window again after it, read from the app's own log --
the per-window count, printed (G35). K89a (`docs/plan-windows.md`) reads
this leg since T89.10 shares the device.

Since T89.10 the two windows share one GPU context, and the leg also
holds what must stay per window: each backend line says `shared` and
names its surface format, window 2's is the second surface on the
context, and the pixel at each window's `#swatch` centre is its own
program's `ImageId(0)` colour -- before the round trip and after it.
"""
import json, socket, struct, subprocess, sys, tempfile, time

from PIL import Image

port, pid, log_path = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
LEG = "two-windows-gpu"
BOUND = 0.001  # 0.1% of each window
WINDOWS = {1: "list", 2: "detail"}

s = socket.create_connection(("127.0.0.1", port), timeout=60)
f = s.makefile("rwb")
n = [0]


def fail(msg):
    sys.exit(f"resume-drive[{LEG}]: {msg}")


def call(method, params=None, **top):
    n[0] += 1
    req = {"jsonrpc": "2.0", "id": n[0], "method": method, "params": params or {}}
    req.update(top)
    f.write((json.dumps(req) + "\n").encode())
    f.flush()
    r = json.loads(f.readline())
    if "error" in r:
        fail(f"{method} {top or ''} failed: {r['error']}")
    return r


def log():
    return open(log_path, errors="replace").read()


def backend_lines():
    """The window's one line per backend it made (`open_window`)."""
    return [l for l in log().splitlines()
            if l.startswith("lumen-static: gpu (") or l.startswith("lumen-static: software")]


def x_windows():
    """{key: X id} of this process's two top-level windows, by title."""
    tree = subprocess.run(["xwininfo", "-root", "-tree"], capture_output=True, text=True).stdout
    found = {}
    for line in tree.splitlines():
        parts = line.strip().split(None, 1)
        if len(parts) < 2 or not parts[0].startswith("0x") or "two-windows" not in parts[1]:
            continue
        xid = parts[0]
        name = parts[1].split('":', 1)[0]
        owner = subprocess.run(["xprop", "-id", xid, "_NET_WM_PID"], capture_output=True, text=True).stdout
        if f"= {pid}" not in owner:
            continue
        for key, word in WINDOWS.items():
            if word in name:
                found[key] = xid
    return found


def xwd(xid):
    """One window, read back off the X server -- `scripts/gpu_parity.sh`'s reader."""
    with tempfile.NamedTemporaryFile(suffix=".xwd") as t:
        r = subprocess.run(["xwd", "-id", xid, "-out", t.name], capture_output=True, text=True)
        if r.returncode != 0:
            fail(f"xwd -id {xid} failed: {r.stderr.strip()}")
        d = open(t.name, "rb").read()
    h = struct.unpack(">25I", d[:100])
    hdr, w, ht, bpp, bpl, ncolors = h[0], h[4], h[5], h[11], h[12], h[19]
    px, step = d[hdr + ncolors * 12:], bpp // 8
    img = Image.new("RGB", (w, ht))
    img.putdata([(px[y * bpl + x * step + 2], px[y * bpl + x * step + 1], px[y * bpl + x * step])
                 for y in range(ht) for x in range(w)])
    return img


def shots(when):
    ids = {}
    deadline = time.time() + 10.0
    while time.time() < deadline:
        ids = x_windows()
        if len(ids) == len(WINDOWS):
            break
        time.sleep(0.25)
    if len(ids) != len(WINDOWS):
        fail(f"setup: {when}, the X server shows windows {sorted(ids)} for pid {pid} (want [1, 2])")
    call("app.diagnostics")  # the process still answers
    out = {k: xwd(x) for k, x in ids.items()}
    for k, img in out.items():
        print(f"  ..  {when}: window {k} ({WINDOWS[k]}) is X {ids[k]}, {img.size}, "
              f"{len(set(img.getdata()))} distinct colours")
    return ids, out


# M89b/T89.10: each program's `ImageId(0)` is its own solid colour
# (examples/two-windows' `Swatch`), the list's red and the detail's blue.
SWATCH = {1: (0xd0, 0x30, 0x30), 2: (0x20, 0x70, 0xd0)}


def swatch_check(when, shots_by_key):
    """The pixel at each window's `#swatch` centre is its own program's colour."""
    bad = []
    for k, img in sorted(shots_by_key.items()):
        b = call("ui.resolve", {"selector": "#swatch"}, window=k)["result"]["bounds"]
        cx, cy = int((b[0] + b[2]) / 2 * SCALE), int((b[1] + b[3]) / 2 * SCALE)
        px = img.getpixel((cx, cy))
        other = SWATCH[3 - k]
        print(f"  ..  {when}: window {k}'s #swatch centre ({cx}, {cy}) is {px}; its own {SWATCH[k]}, "
              f"the other program's {other}")
        if max(abs(a - c) for a, c in zip(px, SWATCH[k])) > 2:
            bad.append(f"{when}: window {k}'s image shows {px}, not its own program's {SWATCH[k]}"
                       + (" -- it is the other window's" if max(abs(a - c) for a, c in zip(px, other)) <= 2 else ""))
    return bad


def shared_lines(lines):
    """M89b/T89.10: every backend line says `shared` and names its surface's
    format, and the second window's counts two surfaces on its context."""
    bad = []
    for i, l in enumerate(lines):
        print(f"  ..  backend line {i + 1}: {l[len('lumen-static: '):]}")
        if ", shared context (" not in l or ", surface " not in l:
            bad.append(f"backend line {i + 1} does not say shared and its surface format: {l!r}")
    for i in (1, 3):
        if i < len(lines) and "(2 surfaces)" not in lines[i]:
            bad.append(f"backend line {i + 1} (window 2's) is not the second surface on one context: {lines[i]!r}")
    return bad


t0 = time.time()
# Setup: the window started on the GPU, and the detail opens.
started = backend_lines()
print(f"  ..  setup: startup line {started[:1]} (want 'lumen-static: gpu (')")
if not started or not started[0].startswith("lumen-static: gpu ("):
    fail(f"setup: the window did not start on the GPU: {started[:1]}")
opened = call("input.click", {"selector": "#row2"}).get("opened")
print(f"  ..  setup: a click on #row2 opened {opened} (want [2])")
if opened != [2]:
    fail(f"setup: the click on #row2 opened {opened!r}, not window 2")
# The detail's first present, polled rather than slept for: the hook runs
# on its own clock from the window's start, and a debug GPU launch takes
# seconds of it, so the first reads are taken as soon as both windows
# have presented.
deadline = time.time() + 5.0
while call("app.presents", window=2)["result"]["presents"] < 1 and time.time() < deadline:
    time.sleep(0.05)
time.sleep(0.2)
if "LUMEN_TEST_RESUME" in log():
    fail("setup: the round trip fired before the first read (the launch took longer than the hook's 8 s)")
ids_a, before = shots("before")
SCALE = call("ui.screenshot", {}, window=1)["result"].get("scale", 1.0)
if "LUMEN_TEST_RESUME" in log():
    fail("setup: the round trip fired during the first reads -- they straddle it")
print(f"  ..  setup: the first reads done {time.time() - t0:.2f} s into the drive, before the round trip")
made_before = len(backend_lines())
if before[1].size == before[2].size:
    fail(f"setup: both reads are {before[1].size} -- one window was read twice")

# The hook fires 8000 ms after the window started; wait for its line, then
# for the frames the resume asks for.
deadline = time.time() + 20.0
while "LUMEN_TEST_RESUME: suspend and resume" not in log() and time.time() < deadline:
    time.sleep(0.25)
time.sleep(3.0)  # the frames the resume asks for, at a present a second if the display sleeps
ids_b, after = shots("after")
for k in WINDOWS:
    before[k].save(f"target/live/{LEG}-window{k}-before.png")
    after[k].save(f"target/live/{LEG}-window{k}-after.png")

hooks = log().count("LUMEN_TEST_RESUME: suspend and resume")
made = len(backend_lines())
per_resume = made - made_before
print(f"  ..  round trips {hooks} (want 1); backends made {made_before} before it (want 2, one per window), "
      f"{per_resume} by it (want 2, one per window)")
print(f"  ..  X ids: before {ids_a}, after {ids_b} (resumed makes new windows)")

bad = swatch_check("before", before) + swatch_check("after", after) + shared_lines(backend_lines())
for k in sorted(WINDOWS):
    a, b = before[k], after[k]
    if a.size != b.size:
        bad.append(f"window {k} changed size {a.size} -> {b.size}")
        continue
    pa, pb = a.tobytes(), b.tobytes()
    total = a.size[0] * a.size[1]
    diff = sum(1 for i in range(0, len(pa), 3) if pa[i:i + 3] != pb[i:i + 3])
    print(f"  ..  window {k} ({WINDOWS[k]}): differing pixels {diff} / {total} ({diff / total:.4%}, "
          f"bound {BOUND:.1%}); distinct colours before {len(set(a.getdata()))}, after {len(set(b.getdata()))}")
    if diff > BOUND * total:
        bad.append(f"window {k} ({WINDOWS[k]}) differs on {diff / total:.1%} of its pixels after the "
                   f"round trip (bound {BOUND:.1%})")
if hooks != 1:
    bad.append(f"the round trip ran {hooks} times (want 1)")
if made_before != 2 or per_resume != 2:
    bad.append(f"backends made {made_before} before the round trip and {per_resume} by it (want 2 and 2, "
               f"one per window per resume)")
if bad:
    fail("; ".join(bad))
print(f"  ok  {LEG}: both windows came back within {BOUND:.1%}, two backends per resume, one context "
      f"shared by both, each window's image its own")
