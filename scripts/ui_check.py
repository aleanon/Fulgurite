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
    ui_check.py press              # a control changes while held (C6)
    ui_check.py latency [selector] # click -> presented <= 100 ms; frame <= 16 ms
    ui_check.py lint               # the agent's own ambient audit is empty

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

CENTRE_TOL_PX = 2.0
MIN_TARGET_PX = 24
MIN_CONTRAST = 4.5
LATENCY_BUDGET_MS = 100.0
FRAME_BUDGET_MS = 16.0


def screenshot(c: AgentClient) -> Image.Image:
    r = c.rpc("ui.screenshot")
    return Image.open(io.BytesIO(base64.b64decode(r["image_base64"]))).convert("RGB")


def nodes(c: AgentClient, role: str | None = None) -> list[dict]:
    """`ui.getTree` is a flat list; `children` are ids into it."""
    return [n for n in c.tree()["nodes"] if role is None or n.get("role") == role]


def ibox(b: list[float]) -> tuple[int, int, int, int]:
    return int(round(b[0])), int(round(b[1])), int(round(b[2])), int(round(b[3]))


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
    fails = []
    for n in nodes(c, "Button"):
        box = ibox(n["bounds"])
        bg = dominant(im, box)
        ink = ink_bbox(im, box, bg)
        if ink is None:
            fails.append(f"#{n.get('wid', n['id'])}: no ink inside its bounds")
            continue
        dx = (ink[0] + ink[2]) / 2 - (box[0] + box[2]) / 2
        dy = (ink[1] + ink[3]) / 2 - (box[1] + box[3]) / 2
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
    px = im.load()
    fails = []
    for n in nodes(c, "Button"):
        x0, y0, x1, y1 = ibox(n["bounds"])
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
        w, h = b[2] - b[0], b[3] - b[1]
        tag = "ok " if w >= MIN_TARGET_PX and h >= MIN_TARGET_PX else "FAIL"
        print(f"  {tag} target   #{n.get('wid','?'):8} {w:.0f}x{h:.0f} px")
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
    px = im.load()
    fails = []
    for n in nodes(c, "Button") + nodes(c, "Label"):
        box = ibox(n["bounds"])
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
    lat_p95 = max(lat) if lat else float("nan")
    frame_p95 = w["frame_ns"]["p95"] / 1e6
    t1 = "ok " if lat and lat_p95 <= LATENCY_BUDGET_MS else "FAIL"
    t2 = "ok " if frame_p95 <= FRAME_BUDGET_MS else "FAIL"
    print(f"  {t1} latency  click->presented max {lat_p95:.1f} ms over {len(lat)} clicks (budget {LATENCY_BUDGET_MS:.0f})")
    print(f"  {t2} frame    p50 {w['frame_ns']['p50']/1e6:.2f} p95 {frame_p95:.2f} ms  layout {w['layout_ns']['p95']/1e3:.0f} us  paint {w['paint_ns']['p95']/1e3:.0f} us  render {w['render_ns']['p95']/1e6:.2f} ms  present {w['present_ns']['p95']/1e6:.2f} ms (budget {FRAME_BUDGET_MS:.0f})")
    if t1 == "FAIL":
        fails.append(f"input latency {lat_p95:.1f} ms > {LATENCY_BUDGET_MS}")
    if t2 == "FAIL":
        fails.append(f"frame p95 {frame_p95:.2f} ms > {FRAME_BUDGET_MS}")
    return fails


def check_press(c: AgentClient) -> list[str]:
    """A control must change while it is held (C6).

    Held, not clicked: `input.click` queues the press and the release
    into one pump, so the frame it returns has already let go. The two
    halves go in separately and the screenshot is taken between them.
    """
    fails = []
    for n in nodes(c, "Button"):
        wid = n.get("wid", "?")
        box = ibox(n["bounds"])
        rest = screenshot(c)
        c.press(f"#{wid}")
        held = screenshot(c)
        c.release(f"#{wid}")
        back = screenshot(c)

        rest_bg = dominant(rest, box)
        held_bg = dominant(held, box)
        back_bg = dominant(back, box)
        changed = dist(rest_bg, held_bg) > 8
        restored = dist(rest_bg, back_bg) <= 8 or dist(held_bg, back_bg) > 8
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
    "press": check_press,
    "latency": check_latency,
    "lint": check_lint,
}


def main(argv: list[str]) -> int:
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
