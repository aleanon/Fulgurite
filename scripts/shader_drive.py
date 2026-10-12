"""M83/T83.7's live legs (docs/plan-gpu-extension.md). See scripts/shader_drive.sh.

Usage: shader_drive.py PORT LEG MODE
  MODE software  ui.screenshot twice, an animated frame apart: the view's
                 centre is the fallback's field, pixels inside the view
                 changed (setup) and none outside `view ∪ card` did;
                 ui.lastDamage per animated frame against `view ∪ card`
                 as a ratio (bound 1.00). With LEG ending `-perf` it also
                 records app.perf's layout_ns, animated against still.
  MODE gpu       the same two reads by `xwd` (the GPU backend cannot answer
                 ui.screenshot): the centre is plasma (its distance from
                 the field printed against a floor), nothing outside
                 changed; then, after the window's own FULGURITE_TEST_RESUME
                 round trip, the centre is plasma again -- the second
                 backend was handed the table by `resume_backend`.
  MODE bad       FULGURITE_GPU_BAD_SHADER=1: after 3 s the window still
                 answers, the centre is the field (the fallback), and the
                 log names the refused shader.
Every bound is printed beside its quantity.
"""
import base64, io, json, os, socket, struct, subprocess, sys, tempfile, time

from PIL import Image

port, leg, mode = int(sys.argv[1]), sys.argv[2], sys.argv[3]
TAG = f"shader-drive[{leg}]"
LOG = os.environ.get("FULGURITE_LIVE_LOG", "")
# examples/shader's `FIELD`: the fallback's colour at the view's centre.
FIELD = (0x1A, 0x1D, 0x2B)
# The plasma's brightest channel is at least 191 (`PLASMA`'s rustdoc), so
# its distance from the field is at least 191 - 43 = 148 on that channel.
PLASMA_FLOOR = 48
# How close a read must be to the field to be the fallback: the field is
# a flat fill, so a backend's rounding is all that may move it.
FALLBACK_TOL = 4

s = socket.create_connection(("127.0.0.1", port), timeout=60)
f = s.makefile("rwb")
n = [0]


def call(method, params=None):
    n[0] += 1
    f.write((json.dumps({"jsonrpc": "2.0", "id": n[0], "method": method,
                         "params": params or {}}) + "\n").encode())
    f.flush()
    line = f.readline()
    if not line:
        sys.exit(f"{TAG}: {method}: the agent endpoint closed the connection")
    r = json.loads(line)
    if "error" in r:
        sys.exit(f"{TAG}: {method} failed: {r['error']}")
    return r["result"]


def bounds(sel):
    b = call("ui.resolve", {"selector": sel})["bounds"]
    return [round(v) for v in b]


def canvas_shot():
    r = call("ui.screenshot")
    img = Image.open(io.BytesIO(base64.b64decode(r["image_base64"]))).convert("RGB")
    return img, r.get("scale", 1.0)


def xwd_shot():
    """The window read back off the X server -- `scripts/gpu_parity.sh`'s reader."""
    call("app.diagnostics")  # the process still answers
    with tempfile.NamedTemporaryFile(suffix=".xwd") as t:
        subprocess.run(["xwd", "-name", "shader", "-out", t.name], check=True, stderr=subprocess.DEVNULL)
        d = open(t.name, "rb").read()
    h = struct.unpack(">25I", d[:100])
    hdr, w, ht, bpp, bpl, ncolors = h[0], h[4], h[5], h[11], h[12], h[19]
    px, step = d[hdr + ncolors * 12:], bpp // 8
    img = Image.new("RGB", (w, ht))
    img.putdata([(px[y * bpl + x * step + 2], px[y * bpl + x * step + 1], px[y * bpl + x * step])
                 for y in range(ht) for x in range(w)])
    return img


def dist(a, b):
    return max(abs(x - y) for x, y in zip(a, b))


def inside(r, x, y):
    return r[0] <= x < r[2] and r[1] <= y < r[3]


def compare(a, b, view, card):
    """(changed inside the view, changed outside view ∪ card, first outside)"""
    if a.size != b.size:
        sys.exit(f"{TAG}: the window changed size {a.size} -> {b.size}")
    w, h = a.size
    pa, pb = a.load(), b.load()
    ins = out = 0
    first = None
    for y in range(h):
        for x in range(w):
            if pa[x, y] != pb[x, y]:
                if inside(view, x, y):
                    ins += 1
                elif not inside(card, x, y):
                    out += 1
                    first = first or (x, y, pa[x, y], pb[x, y])
    return ins, out, first


def area(rects):
    """Area of a union of axis-aligned rects, by coordinate compression."""
    xs = sorted({v for r in rects for v in (r[0], r[2])})
    ys = sorted({v for r in rects for v in (r[1], r[3])})
    tot = 0.0
    for i in range(len(xs) - 1):
        for j in range(len(ys) - 1):
            cx, cy = (xs[i] + xs[i + 1]) / 2, (ys[j] + ys[j + 1]) / 2
            if any(r[0] <= cx < r[2] and r[1] <= cy < r[3] for r in rects):
                tot += (xs[i + 1] - xs[i]) * (ys[j + 1] - ys[j])
    return tot


def clip(r, c):
    x0, y0, x1, y1 = max(r[0], c[0]), max(r[1], c[1]), min(r[2], c[2]), min(r[3], c[3])
    return [x0, y0, x1, y1] if x0 < x1 and y0 < y1 else None


def log_text():
    return open(LOG, errors="replace").read() if LOG else ""


view, card = bounds("#plasma"), bounds("#card")
cx, cy = (view[0] + view[2]) // 2, (view[1] + view[3]) // 2
print(f"  ..  view {view}, card {card}, centre ({cx}, {cy})")
if view[2] - view[0] < 100 or card[2] - card[0] < 100:
    sys.exit(f"{TAG}: setup: the view or the card did not lay out")

if mode == "software":
    time.sleep(1.0)
    a, scale = canvas_shot()
    if scale != 1.0:
        sys.exit(f"{TAG}: setup: the window's scale is {scale}; the script sets WINIT_X11_SCALE_FACTOR=1")
    time.sleep(0.4)
    b, _ = canvas_shot()
    centre = a.getpixel((cx, cy))
    ins, out, first = compare(a, b, view, card)
    print(f"  ..  centre {centre}, field {FIELD}, distance {dist(centre, FIELD)} (bound {FALLBACK_TOL}: the fallback)")
    print(f"  ..  changed between two animated frames: inside the view {ins} px (setup > 0), "
          f"outside view ∪ card {out} px (bound 0){'; first ' + str(first) if first else ''}")
    if dist(centre, FIELD) > FALLBACK_TOL:
        sys.exit(f"{TAG}: the view's centre is {centre}, not the fallback's field {FIELD}")
    if ins == 0:
        sys.exit(f"{TAG}: setup: nothing inside the view changed -- the clock is not animating")
    if out != 0:
        sys.exit(f"{TAG}: {out} px outside the view and the card changed between two frames: "
                 f"the fallback was not clipped to its bracket")
    # ui.lastDamage per animated frame, against view ∪ card.
    allowed = [view, card]
    worst, worst_out, samples = 0.0, 0.0, 0
    for _ in range(6):
        d = call("ui.lastDamage")
        regs = [[round(v) for v in r] for r in d["regions"]]
        if not regs:
            time.sleep(0.05)
            continue
        samples += 1
        dmg = area(regs)
        outside = dmg - area([c for r in regs for a_ in allowed for c in [clip(r, a_)] if c])
        ratio = dmg / area(allowed)
        worst, worst_out = max(worst, ratio), max(worst_out, outside)
        print(f"  ..  frame {d['frame']}: lastDamage {regs} -> {dmg:.0f} px of view ∪ card "
              f"{area(allowed):.0f} px, ratio {ratio:.2f} (bound 1.00), outside {outside:.0f} px (bound 0)")
        time.sleep(0.05)
    if samples == 0:
        sys.exit(f"{TAG}: setup: no animated frame reported damage")
    if worst > 1.0 or worst_out > 0:
        sys.exit(f"{TAG}: an animated frame damaged {worst:.2f}x view ∪ card, {worst_out:.0f} px outside it")
    if leg.endswith("-perf"):
        # A12: the whole-tree mark's price, recorded, not gated.
        nodes = call("app.diagnostics")["nodes"]
        animated = call("app.perf")["window"]["layout_ns"]
        call("input.click", {"selector": "#pause"})
        time.sleep(0.3)
        frames, still = set(), []
        for k in range(24):
            call("input.hover", {"selector": "#pause" if k % 2 == 0 else "#title"})
            time.sleep(0.06)
            r = call("app.perf")
            last = r["last"]
            if last.get("frame_ns") is not None and r["frame"] not in frames:
                frames.add(r["frame"])
                still.append(last.get("layout_ns"))
        ran = sorted(v for v in still if v is not None)
        med = ran[len(ran) // 2] if ran else None
        print(f"  ..  perf: layout_ns per animated frame p50 {animated['p50']} ns (n {animated['n']}); "
              f"still (paused, hover frames): {len(still)} frames, layout ran in {len(ran)}, median "
              f"{med} ns; tree {nodes} nodes")
        if not still:
            sys.exit(f"{TAG}: setup: no still frame was observed after the pause")
    print(f"  ok  {leg}: fallback at the centre, 0 px outside, damage ratio {worst:.2f}")

elif mode == "gpu":
    resume_at = float(os.environ.get("SHADER_DRIVE_RESUME_S", "0"))
    t0 = time.time()
    time.sleep(1.0)
    a = xwd_shot()
    time.sleep(0.4)
    b = xwd_shot()
    log = log_text()
    if resume_at and "FULGURITE_TEST_RESUME" in log:
        sys.exit(f"{TAG}: setup: the round trip fired before the first two reads -- raise RESUME_MS")
    if "fulgurite: gpu" not in log:
        sys.exit(f"{TAG}: setup: the window is not on the GPU backend: {log.strip().splitlines()[:3]}")
    if "refused" in log:
        sys.exit(f"{TAG}: setup: a shader was refused: {[l for l in log.splitlines() if 'refused' in l]}")
    ca, cb = a.getpixel((cx, cy)), b.getpixel((cx, cy))
    ins, out, first = compare(a, b, view, card)
    print(f"  ..  centre {ca} then {cb}, field {FIELD}: distance {dist(ca, FIELD)}, {dist(cb, FIELD)} "
          f"(floor {PLASMA_FLOOR}: the plasma)")
    print(f"  ..  changed between two captures: inside the view {ins} px (setup > 0), "
          f"outside view ∪ card {out} px (bound 0){'; first ' + str(first) if first else ''}")
    if min(dist(ca, FIELD), dist(cb, FIELD)) <= PLASMA_FLOOR:
        sys.exit(f"{TAG}: the view's centre is the fallback's field, not the plasma")
    if ins == 0:
        sys.exit(f"{TAG}: setup: nothing inside the view changed -- the plasma is not animating")
    if out != 0:
        sys.exit(f"{TAG}: {out} px outside the view and the card changed: the shader's pass was not "
                 f"bounded by its scissor")
    if resume_at:
        time.sleep(max(0.0, resume_at + 3.0 - (time.time() - t0)))
        c = xwd_shot()
        log = log_text()
        hooks = log.count("FULGURITE_TEST_RESUME: suspend and resume")
        backends = sum(1 for l in log.splitlines() if l.startswith("fulgurite: gpu"))
        cc = c.getpixel((cx, cy))
        print(f"  ..  resume: round trips {hooks} (want 1), GPU backends made {backends} (want 2); "
              f"centre {cc}, distance {dist(cc, FIELD)} (floor {PLASMA_FLOOR})")
        if (hooks, backends) != (1, 2):
            sys.exit(f"{TAG}: setup: the suspend/resume round trip did not happen as asked")
        if dist(cc, FIELD) <= PLASMA_FLOOR:
            sys.exit(f"{TAG}: after the resume the view shows the fallback: the second backend "
                     f"was not handed the shader table")
    print(f"  ok  {leg}: plasma at the centre, 0 px outside" + (", kept across the resume" if resume_at else ""))

elif mode == "bad":
    time.sleep(3.0)
    d = call("app.diagnostics")
    img = xwd_shot()
    centre = img.getpixel((cx, cy))
    log = log_text()
    named = [l for l in log.splitlines() if 'shader "plasma" refused' in l]
    print(f"  ..  after 3 s: app.diagnostics answered (frame {d['frame']}); centre {centre}, field {FIELD}, "
          f"distance {dist(centre, FIELD)} (bound {FALLBACK_TOL}: the fallback); refusals naming the shader "
          f"{len(named)} (want 1): {named[0][:120] if named else '-'}")
    if "fulgurite: gpu" not in log:
        sys.exit(f"{TAG}: setup: the window is not on the GPU backend")
    if len(named) != 1:
        sys.exit(f"{TAG}: the log does not name the refused shader once")
    if dist(centre, FIELD) > FALLBACK_TOL:
        sys.exit(f"{TAG}: the refused shader's view does not show its fallback")
    print(f"  ok  {leg}: the window stayed up and drew the fallback")
else:
    sys.exit(f"{TAG}: unknown mode {mode}")
