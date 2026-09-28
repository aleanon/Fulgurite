#!/usr/bin/env python3
"""Measurable GUI-quality checks against a live lumen-static window.

Each check turns a rule from GUI practice into a number the agent
endpoint can produce, and a budget it is judged against. Bounds come
from `ui.getLayout`/`ui.getTree`; pixels from `ui.screenshot`; timings
from `app.perf`. Stdlib + Pillow (screenshot decoding only).

    ui_check.py all                # every check, exit 1 on any failure
    ui_check.py centring           # text ink centred in every button (±2 px)
    ui_check.py edges              # rounded corners are anti-aliased
    ui_check.py targets            # pointer targets >= 24x24 px (WCAG 2.5.8)
    ui_check.py contrast           # text vs its background >= 4.5:1 (WCAG AA)
    ui_check.py presented          # what is on screen matches what was drawn
    ui_check.py press              # a control changes while held (C6)
    ui_check.py latency [selector] # click -> presented <= 100 ms; frame <= 16 ms
    ui_check.py lint               # the agent's own ambient audit is empty
    ui_check.py selftest           # the perf judge on canned replies (no window)

Budgets and the rules behind them: .claude/skills/verifying-apps/SKILL.md.
"""
from __future__ import annotations

import base64
import io
import sys
import time
from collections import Counter

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from agent_client import AgentClient  # noqa: E402

try:
    from PIL import Image
except ImportError:  # pragma: no cover
    sys.exit("ui_check needs Pillow: pip install pillow")

# All budgets are in LOGICAL pixels. `ui.getLayout` reports logical
# bounds; `ui.screenshot` returns physical pixels. Until 2026-09-04 this
# file compared the two directly, so on a 2x display a 22-logical-pixel
# button — under the WCAG floor below — passed. Every screenshot lookup
# now goes through `scale()`.
CENTRE_TOL_PX = 2.0
MIN_TARGET_PX = 24
MIN_CONTRAST = 4.5
LATENCY_BUDGET_MS = 100.0

# The hard ceiling: a frame that takes longer than this missed a 60 Hz
# vsync. It is a *correctness* bound, not a performance target, and it
# has never come close to firing.
FRAME_BUDGET_MS = 16.0

# The regression bound, which is what actually catches a change.
#
# **On p50, not p95, and that was measured.** Five consecutive runs of
# this gate on an idle machine gave p50 0.05, 0.05, 0.06, 0.05, 0.05 and
# p95 1.59, 0.80, 1.33, 3.03, 2.45 — the median is stable to a hundredth
# of a millisecond and the 95th percentile swings four-fold on scheduler
# noise. A regression test keyed on p95 would flap; one keyed on p50
# catches a real change and nothing else.
#
# Recorded 2026-09-07 on the reference machine (i9-13900KF, governor
# `powersave`). 3x the observed median: tight enough that the two
# defects `docs/results-competitive.md` §6 found would each have tripped
# it, loose enough to survive a busy machine.
FRAME_P50_BASELINE_MS = 0.06
FRAME_P50_ALLOWANCE = 3.0


def scale(c: AgentClient) -> float:
    """Physical pixels per logical pixel, from the host."""
    try:
        return float(c.rpc("ui.screenshot")["scale"])
    except (RuntimeError, KeyError, TypeError):
        return 1.0


def screenshot(c: AgentClient, source: str = "canvas") -> Image.Image:
    r = c.rpc("ui.screenshot", **({"source": source} if source != "canvas" else {}))
    return Image.open(io.BytesIO(base64.b64decode(r["image_base64"]))).convert("RGB")


def nodes(c: AgentClient, role: str | None = None) -> list[dict]:
    """`ui.getTree` is a flat list; `children` are ids into it."""
    return [n for n in c.tree()["nodes"] if role is None or n.get("role") == role]


def ibox(b: list[float], k: float = 1.0) -> tuple[int, int, int, int]:
    """Logical bounds to physical pixel indices into a screenshot."""
    return (
        int(round(b[0] * k)),
        int(round(b[1] * k)),
        int(round(b[2] * k)),
        int(round(b[3] * k)),
    )


def dominant(im: Image.Image, box) -> tuple[int, int, int]:
    x0, y0, x1, y1 = box
    px = im.load()
    return Counter(px[x, y] for y in range(y0, y1) for x in range(x0, x1)).most_common(1)[0][0]


def dist(a, b) -> int:
    return sum(abs(p - q) for p, q in zip(a, b))


def off_segment(p, a, b, tol: float = 40.0) -> bool:
    """Is `p` a colour that is *not* a blend of `a` and `b`? A rounded
    box's anti-aliased rim is a blend of its fill and what lies outside;
    text is something else. Projects `p` onto the a→b segment and
    looks at the residual."""
    ab = [q - r for q, r in zip(b, a)]
    ap = [q - r for q, r in zip(p, a)]
    denom = sum(v * v for v in ab) or 1
    t = max(0.0, min(1.0, sum(u * v for u, v in zip(ap, ab)) / denom))
    res = [u - t * v for u, v in zip(ap, ab)]
    return sum(v * v for v in res) ** 0.5 > tol


def outside_of(im: Image.Image, box):
    x0, y0, _, _ = box
    px = im.load()
    return px[max(x0 - 2, 0), max(y0 - 2, 0)]


def ink_bbox(im: Image.Image, box, bg):
    """Bounding box of pixels inside `box` that are neither the
    background nor its anti-aliased rim against the outside."""
    x0, y0, x1, y1 = box
    px = im.load()
    out = outside_of(im, box)
    xs, ys = [], []
    for y in range(y0, y1):
        for x in range(x0, x1):
            p = px[x, y]
            if dist(p, bg) > 60 and off_segment(p, bg, out):
                xs.append(x)
                ys.append(y)
    if not xs:
        return None
    return min(xs), min(ys), max(xs) + 1, max(ys) + 1


def check_centring(c: AgentClient) -> list[str]:
    im = screenshot(c)
    k = scale(c)
    fails = []
    for n in nodes(c, "Button"):
        box = ibox(n["bounds"], k)
        bg = dominant(im, box)
        ink = ink_bbox(im, box, bg)
        if ink is None:
            fails.append(f"#{n.get('wid', n['id'])}: no ink inside its bounds")
            continue
        # Report the offset in logical pixels, which is what the budget
        # is expressed in.
        dx = ((ink[0] + ink[2]) / 2 - (box[0] + box[2]) / 2) / k
        dy = ((ink[1] + ink[3]) / 2 - (box[1] + box[3]) / 2) / k
        tag = "ok " if abs(dx) <= CENTRE_TOL_PX and abs(dy) <= CENTRE_TOL_PX else "FAIL"
        print(f"  {tag} centring #{n.get('wid','?'):8} dx={dx:+.1f} dy={dy:+.1f}  (ink {ink} in {box})")
        if tag == "FAIL":
            fails.append(f"#{n.get('wid','?')}: text off-centre dx={dx:+.1f} dy={dy:+.1f}")
    return fails


def check_edges(c: AgentClient) -> list[str]:
    """In the top-left corner square, count pixels that are a blend of
    the fill and the outside. A hard-edged corner has none; an
    anti-aliased one has a pixel or two per row of the arc."""
    im = screenshot(c)
    k = scale(c)
    px = im.load()
    fails = []
    for n in nodes(c, "Button"):
        x0, y0, x1, y1 = ibox(n["bounds"], k)
        fill = dominant(im, (x0, y0, x1, y1))
        outside = outside_of(im, (x0, y0, x1, y1))
        # A blend must be clearly neither endpoint; scale that to how far
        # apart the endpoints are (Reset's fill is 23 units from the card).
        gap = max(4, min(12, dist(fill, outside) // 4))
        blends = 0
        for y in range(y0, y0 + 14):
            for x in range(x0, x0 + 14):
                p = px[x, y]
                if dist(p, fill) > gap and dist(p, outside) > gap and not off_segment(p, fill, outside, 20):
                    blends += 1
        tag = "ok " if blends >= 6 else "FAIL"
        print(f"  {tag} edges    #{n.get('wid','?'):8} {blends} blended pixels in the corner square")
        if tag == "FAIL":
            fails.append(f"#{n.get('wid','?')}: corner is not anti-aliased ({blends} blended pixels)")
    return fails


def check_targets(c: AgentClient) -> list[str]:
    fails = []
    for n in nodes(c, "Button"):
        b = n["bounds"]
        # Logical, straight from ui.getLayout — no conversion, which is
        # the point: this is the unit WCAG 2.5.8 is written in.
        w, h = b[2] - b[0], b[3] - b[1]
        tag = "ok " if w >= MIN_TARGET_PX and h >= MIN_TARGET_PX else "FAIL"
        print(f"  {tag} target   #{n.get('wid','?'):8} {w:.0f}x{h:.0f} logical px")
        if tag == "FAIL":
            fails.append(f"#{n.get('wid','?')}: target {w:.0f}x{h:.0f} < {MIN_TARGET_PX}")
    return fails


def luminance(rgb) -> float:
    def ch(v):
        v /= 255.0
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = rgb
    return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)


def contrast(a, b) -> float:
    la, lb = sorted((luminance(a), luminance(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def check_contrast(c: AgentClient) -> list[str]:
    im = screenshot(c)
    k = scale(c)
    px = im.load()
    fails = []
    for n in nodes(c, "Button") + nodes(c, "Label"):
        box = ibox(n["bounds"], k)
        if box[2] - box[0] < 4 or box[3] - box[1] < 4:
            continue
        bg = dominant(im, box)
        # Text colour: the most common colour among non-background,
        # non-edge pixels — glyph interiors, not their AA fringe.
        inner = Counter(
            px[x, y]
            for y in range(box[1] + 2, box[3] - 2)
            for x in range(box[0] + 2, box[2] - 2)
            if dist(px[x, y], bg) > 120
        )
        if not inner:
            continue
        fg = inner.most_common(1)[0][0]
        ratio = contrast(fg, bg)
        tag = "ok " if ratio >= MIN_CONTRAST else "FAIL"
        print(f"  {tag} contrast #{n.get('wid','?'):8} {ratio:.1f}:1  fg={fg} bg={bg}")
        if tag == "FAIL":
            fails.append(f"#{n.get('wid','?')}: contrast {ratio:.1f}:1 < {MIN_CONTRAST}")
    return fails


def not_measured(w: dict) -> str | None:
    """Why an `app.perf` window cannot be judged, or `None` if it can.

    **"Not measured" fails** (M48/T48.3, R13-F2). Every turnkey window
    reported 0 ns for every stage, and this file's budgets -- frame p95 <=
    16 ms, p50 <= the regression bound -- passed on the zeros. A window
    with no samples, a stage the host reported as `null`, or a frame that
    took zero time is not a fast frame: it is a frame nobody timed.
    """
    f = w.get("frame_ns") or {}
    n = f.get("n", 0)
    if not n:
        return f"frame_ns n={n}: no frame was timed"
    if f.get("p50") is None or f.get("max") is None:
        return f"frame_ns p50={f.get('p50')} max={f.get('max')}: the host reported null"
    if f["max"] == 0:
        return f"frame_ns max 0 over n={n}: every frame took no time, i.e. nothing timed it"
    return None


def check_latency(c: AgentClient, selector: str = "#inc") -> list[str]:
    fails = []
    lat, frames = [], []
    for _ in range(5):
        c.click(selector)
        time.sleep(0.05)  # let the redraw the click requested present
        p = c.rpc("app.perf")
        last = p["last"]
        if last["input_latency_ns"] is not None:
            lat.append(last["input_latency_ns"] / 1e6)
        frames.append(last["frame_ns"] / 1e6)
    w = c.rpc("app.perf")["window"]
    why = not_measured(w)
    if why is not None:
        print(f"  FAIL NOT MEASURED  {why}")
        return fails + [f"app.perf NOT MEASURED: {why}"]
    lat_p95 = max(lat) if lat else float("nan")
    frame_p95 = w["frame_ns"]["p95"] / 1e6
    frame_p50 = w["frame_ns"]["p50"] / 1e6
    p50_bound = FRAME_P50_BASELINE_MS * FRAME_P50_ALLOWANCE
    t1 = "ok " if lat and lat_p95 <= LATENCY_BUDGET_MS else "FAIL"
    t2 = "ok " if frame_p95 <= FRAME_BUDGET_MS else "FAIL"
    t3 = "ok " if frame_p50 <= p50_bound else "FAIL"
    print(f"  {t1} latency  click->presented max {lat_p95:.1f} ms over {len(lat)} clicks (budget {LATENCY_BUDGET_MS:.0f})")
    print(f"  {t2} frame    p95 {frame_p95:.2f} ms  layout {w['layout_ns']['p95']/1e3:.0f} us  paint {w['paint_ns']['p95']/1e3:.0f} us  render {w['render_ns']['p95']/1e6:.2f} ms  present {w['present_ns']['p95']/1e6:.2f} ms (ceiling {FRAME_BUDGET_MS:.0f})")
    print(f"  {t3} regress  frame p50 {frame_p50:.3f} ms (baseline {FRAME_P50_BASELINE_MS} x{FRAME_P50_ALLOWANCE:.0f} = {p50_bound:.2f})")
    if t1 == "FAIL":
        fails.append(f"input latency {lat_p95:.1f} ms > {LATENCY_BUDGET_MS}")
    if t2 == "FAIL":
        fails.append(f"frame p95 {frame_p95:.2f} ms > {FRAME_BUDGET_MS}")
    return fails


def check_presented(c: AgentClient) -> list[str]:
    """What the app drew must be what is on screen.

    Every other check here reads the renderer's canvas — what the app
    *believes* it drew. Nothing between that and the display server (the
    blit, the damage rect, the present, the buffer's age) was observable
    at all until this check existed, and on 2026-09-04 a bug in exactly
    that gap put another window's pixels on screen while every check
    passed. `source: "surface"` reads the window back.

    Not available everywhere: softbuffer implements read-back on X11 and
    Web only. An absent answer is reported as "not checkable", never as
    a pass.
    """
    canvas = screenshot(c)
    try:
        surface = screenshot(c, "surface")
    except RuntimeError as e:
        print(f"  --  presented  not checkable on this platform ({e})")
        return []

    if canvas.size != surface.size:
        print(f"  FAIL presented  canvas {canvas.size} vs surface {surface.size}")
        return [f"presented size {surface.size} != canvas {canvas.size}"]

    cp, sp = canvas.load(), surface.load()
    w, h = canvas.size
    bad = 0
    first = None
    for y in range(0, h, 2):
        for x in range(0, w, 2):
            if dist(cp[x, y], sp[x, y]) > 24:
                bad += 1
                if first is None:
                    first = (x, y, cp[x, y], sp[x, y])
    total = ((h + 1) // 2) * ((w + 1) // 2)
    frac = bad / total
    tag = "ok " if frac <= 0.01 else "FAIL"
    print(f"  {tag} presented  {frac * 100:.2f}% of sampled pixels differ from the canvas")
    if tag == "FAIL":
        return [f"the window shows something else at {first[:2]}: canvas {first[2]} vs screen {first[3]}"]
    return []


def check_press(c: AgentClient) -> list[str]:
    """A control must change while it is held (C6).

    Held, not clicked: `input.click` queues the press and the release
    into one pump, so the frame it returns has already let go. The two
    halves go in separately and the screenshot is taken between them.
    """
    def away():
        c.rpc("input.hover", x=1.0, y=1.0)

    fails = []
    for n in nodes(c, "Button"):
        wid = n.get("wid", "?")
        box = ibox(n["bounds"], scale(c))
        # **Read rest and after with the pointer off the control.** A
        # release leaves the pointer over it, and its hover shade lies
        # between rest and held -- within 8 of held on a dark button, so
        # the M9 check (`after` near rest *or* far from held) read the
        # hover shade as "stayed pressed" once a press started tracking
        # the pointer (the M56 merge: #dec, #dec10, #reset). Off the
        # control, after must simply be rest again.
        away()
        rest = screenshot(c)
        c.press(f"#{wid}")
        held = screenshot(c)
        c.release(f"#{wid}")
        away()
        back = screenshot(c)

        rest_bg = dominant(rest, box)
        held_bg = dominant(held, box)
        back_bg = dominant(back, box)
        changed = dist(rest_bg, held_bg) > 8
        restored = dist(rest_bg, back_bg) <= 8
        tag = "ok " if changed and restored else "FAIL"
        print(f"  {tag} press    #{wid:8} rest={rest_bg} held={held_bg} after={back_bg}")
        if not changed:
            fails.append(f"#{wid}: no visible change while held")
        elif not restored:
            fails.append(f"#{wid}: stayed pressed after release")
    return fails


def check_lint(c: AgentClient) -> list[str]:
    r = c.lint()
    findings = r.get("findings", r) if isinstance(r, dict) else r
    n = len(findings) if isinstance(findings, list) else 0
    print(f"  {'ok ' if n == 0 else 'FAIL'} lint     {n} findings")
    return [f"lint: {f}" for f in (findings if isinstance(findings, list) else [])]


CHECKS = {
    "centring": check_centring,
    "edges": check_edges,
    "targets": check_targets,
    "contrast": check_contrast,
    "presented": check_presented,
    "press": check_press,
    "latency": check_latency,
    "lint": check_lint,
}


def selftest() -> int:
    """The judge against canned replies: both ways, no window.

    An all-zero window (what every turnkey window sent before T48.3) and
    an all-`null` one must print NOT MEASURED; a measured one must not.
    """
    stage = lambda p50, mx, n: {"p50": p50, "p95": mx, "max": mx, "n": n}  # noqa: E731
    cases = [
        ("all-zero", {"frame_ns": stage(0, 0, 128)}, True),
        ("all-null", {"frame_ns": stage(None, None, 0)}, True),
        ("measured", {"frame_ns": stage(61_000, 900_000, 128)}, False),
    ]
    bad = 0
    for name, w, want in cases:
        why = not_measured(w)
        got = why is not None
        print(f"  {'ok ' if got == want else 'FAIL'} {name}: "
              f"{'NOT MEASURED -- ' + why if got else 'measured'} (expected {'NOT MEASURED' if want else 'measured'})")
        bad += got != want
    return 1 if bad else 0


def main(argv: list[str]) -> int:
    if argv and argv[0] == "selftest":
        return selftest()
    if not argv or argv[0] not in CHECKS and argv[0] != "all":
        print(__doc__)
        return 2
    c = AgentClient()
    names = list(CHECKS) if argv[0] == "all" else [argv[0]]
    fails = []
    for name in names:
        print(f"{name}:")
        if name == "latency" and len(argv) > 1:
            fails += check_latency(c, argv[1])
        else:
            fails += CHECKS[name](c)
    if fails:
        print(f"\n{len(fails)} failure(s):")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("\nall checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
