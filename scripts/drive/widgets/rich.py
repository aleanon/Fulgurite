"""M27/T27.12 — the text tail, live.

Three of these four take no input at all, which makes them the tranche
where the inventory has to work hardest: an action that presses nothing
still has to *assert* something a defect could move. What each one
claims is therefore a property of the **rendered** text — the value the
widget publishes is what a reader sees, not what the source says — and
for `Markdown` that difference is every asterisk.
"""
from harness import Action, Failed


def _words(c, sel=None):
    return [w for w in c.value(sel).replace("\n", " ").split(" ") if w]


RICHTEXT = ("RichText", "#d_richtext_a", [
    Action(
        name="it publishes what a reader sees, and wraps to fit",
        needs="a paragraph wider than the stage",
        do=lambda c: c.hover(),
        expect=lambda c: (
            len(_words(c)) > 20 and c.box()[3] - c.box()[1] > 40.0,
            "{} words in a box {:.0f} px tall — one line of this font is "
            "about 20, so anything under 40 did not wrap".format(
                len(_words(c)), c.box()[3] - c.box()[1]),
        ),
        static=True,
        injection="stop wrapping: let the line run past the box's width",
    ),
])

MARKDOWN = ("Markdown", "#d_markdown_a", [
    Action(
        name="the markers are markup, not text",
        needs="a document with bold, code and bullets in it",
        do=lambda c: c.hover(),
        expect=lambda c: (
            "**" not in c.value()
            and "`" not in c.value()
            and "- " not in c.value()
            and "# " not in c.value()
            and "Markdown" in c.value(),
            "the rendered text is {!r}".format(c.value()[:70]),
        ),
        static=True,
        injection="keep each word's markers in the range that is drawn",
    ),
    # M87/T87.2: the demo's last line is a blockquote. Its colour is the
    # theme's quote colour, derived from the body's, so it must be the
    # *least* contrasting run in the demo -- and still read at AA -- in
    # both palettes, and differ between them (it follows the palette).
    # Contrast alone cannot see a quote drawn in the body colour, which
    # reads; naming the quote as the minimum is what can. The dark
    # palette is measured in `setup`, before the change window opens, so
    # its full repaint is not this action's damage.
    Action(
        name="a quote is drawn in its own colour",
        needs="a document whose last line is a blockquote, in either palette",
        setup=lambda c: c.__dict__.__setitem__("_dark", _palette_min(c, "dark")),
        do=lambda c: c.hover(),
        expect=lambda c: _quote_is_the_minimum(c),
        static=True,
        injection="paint a quote span in the node's own colour (Markdown::paint "
                  "passes st[i].fg to RichText::draw)",
    ),
])

# The quote line's words that appear nowhere else in the demo (`a` and
# `and` are body words too).
QUOTE_WORDS = {"A", "quote", "is", "set", "apart", "by", "bar", "quieter", "colour."}


def _palette_min(c, name):
    """The demo's least contrasting text run under palette `name`, with the
    window put back on the light palette afterwards."""
    r = c.c.rpc("app.setPalette", name=name)
    if r.get("applied") is not True:
        raise Failed(f"the window did not take the {name} palette: {r!r}")
    m = c.c.rpc("ui.contrast", selector=c.sel).get("min") or {}
    c.c.rpc("app.setPalette", name="light")
    return m


def _quote_is_the_minimum(c):
    light = c.c.rpc("ui.contrast", selector=c.sel).get("min") or {}
    dark = c._dark
    lr, dr = float(light.get("ratio") or 0.0), float(dark.get("ratio") or 0.0)
    ok = (light.get("text") in QUOTE_WORDS and dark.get("text") in QUOTE_WORDS
          and lr >= 4.5 and dr >= 4.5 and light.get("fg") != dark.get("fg"))
    return ok, ("least contrasting run: light {!r} at {:.2}:1 fg {}, dark {!r} at {:.2}:1 fg {} "
                "(bound: a quote word, >= 4.5, and the two fg differ)").format(
        light.get("text"), lr, light.get("fg"), dark.get("text"), dr, dark.get("fg"))

EDITOR = ("RichTextEditor", "#d_editor_a", [
    Action(
        visible=True,
        name="typing markup updates the preview under it",
        needs="an editor whose preview is already parsed",
        do=lambda c: (c.__dict__.__setitem__(
            "_p0", c.value(f"{c.sel} > #editor-preview")),
            c.click(f"{c.sel} > #editor-input"),
            c.c.rpc("input.type", text=" **plus**"))[0],
        expect=lambda c: (
            "plus" in c.value(f"{c.sel} > #editor-preview")
            and "**" not in c.value(f"{c.sel} > #editor-preview")
            and c.value(f"{c.sel} > #editor-preview") != c._p0,
            "the preview reads {!r}".format(
                c.value(f"{c.sel} > #editor-preview")[-40:]),
        ),
        isolate=lambda c: (c.__dict__.__setitem__(
            "_p0", c.value(f"{c.sel} > #editor-preview")),
            c.click_outside(),
            c.c.rpc("input.type", text="x"))[0],
        injection="stop re-parsing in RichTextEditor::layout",
    ),
    # M57/B8: `bold()` existed and nothing called it; and the agent could
    # not hold a modifier, so a shortcut could not be driven at all.
    Action(
        visible=True,
        name="Ctrl+B bolds the selection",
        needs="an editor with focus in its text area",
        do=lambda c: (c.click(f"{c.sel} > #editor-input"),
                      c.c.rpc("input.key", key="a", mods=["ctrl"]),
                      c.c.rpc("input.key", key="b", mods=["ctrl"]))[0],
        expect=lambda c: (
            c.value(f"{c.sel} > #editor-input").startswith("**")
            and c.value(f"{c.sel} > #editor-input").endswith("**")
            and "**" not in c.value(f"{c.sel} > #editor-preview"),
            "the source reads {!r}".format(c.value(f"{c.sel} > #editor-input")[:40]),
        ),
        injection="drop the Ctrl+B arm in RichTextEditor::on_event, or the "
                  "`mods` parameter of input.key",
    ),
])

PAINTBOX = ("PaintBox", "#d_paintbox_a", [
    Action(
        name="the application's drawing is on screen and inside the box",
        needs="a paint box whose callback draws dots across its own edges",
        do=lambda c: c.hover(),
        expect=lambda c: _ink_inside(c),
        static=True,
        injection="stop calling the callback, or stop clipping it",
    ),
    Action(
        name="the star path is filled inside its own outline",
        needs="the callback fills a star with Painter::path (M82/T82.5)",
        do=lambda c: c.hover(),
        expect=lambda c: _star_filled(c),
        static=True,
        injection="the software canvas skips Cmd::Path",
    ),
])


# M82/T82.8: the showcase's VectorText demo is one run at 0°, 30° and 90°
# (`examples/showcase/src/catalog.rs`, anchor `vectortext`); this drives
# the 90° one, whose text is "Vertical".
VECTORTEXT = ("VectorText", "#d_vectortext_c", [
    Action(
        name="drawn rotated",
        needs="a run turned 90° with a measurer that has outlines (the window's vector-text)",
        do=lambda c: c.hover(),
        expect=lambda c: _turned(c),
        static=True,
        injection="VectorText::paint ignores the angle (the run is drawn level)",
    ),
    Action(
        name="it speaks its text, and ~= finds it",
        needs="a VectorText whose run is \"Vertical\"",
        do=lambda c: c.hover(),
        expect=lambda c: _speaks(c),
        static=True,
        injection="VectorText::observe removed, or its value left unset",
    ),
])


def _turned(c):
    """The ink's bounding box, from the screenshot: taller than twice its
    width for a run turned on its side, and none of it outside the box.

    **Pixels, because the angle is a claim about the frame.** The ground
    is read at the box's top-left pixel, and ink is anything 64 levels off
    it in some channel: the run's colour follows the palette, so it is not
    named here. A run drawn level would be about five times wider than
    tall; the fallback (no outlines) is level too.
    """
    shot = c.c.rpc("ui.screenshot")
    png = base64_decode(shot["image_base64"])
    w, h, px = _decode(png)
    sc = shot.get("scale", 1.0)
    b = [v * sc for v in c.box()]
    gx, gy = int(b[0]) + 1, int(b[1]) + 1
    ground = px[(gy * w + gx) * 4:(gy * w + gx) * 4 + 3]
    x0, y0, x1, y1 = w, h, -1, -1
    outside = 0
    for y in range(max(0, int(b[1]) - 6), min(h, int(b[3]) + 6)):
        for x in range(max(0, int(b[0]) - 6), min(w, int(b[2]) + 6)):
            p = px[(y * w + x) * 4:(y * w + x) * 4 + 3]
            if max(abs(p[k] - ground[k]) for k in range(3)) < 64:
                continue
            if x + 1 <= b[0] or x >= b[2] or y + 1 <= b[1] or y >= b[3]:
                outside += 1
                continue
            x0, y0, x1, y1 = min(x0, x), min(y0, y), max(x1, x), max(y1, y)
    iw, ih = x1 - x0 + 1, y1 - y0 + 1
    return (
        x1 >= 0 and ih >= 2 * iw and outside == 0,
        f"ink box {iw} x {ih} px (bound h >= 2 w = {2 * iw}), {outside} px "
        f"of ink outside the box (bound 0), scale {sc}",
    )


def _speaks(c):
    """The node publishes its run as its value, and the exact-text
    selector finds that node and no other."""
    me = c.c.rpc("ui.resolve", selector=c.sel)
    found = c.c.rpc("ui.resolve", selector="~=Vertical")
    return (
        me.get("value") == "Vertical" and found.get("index") == me.get("index"),
        f"value {me.get('value')!r}, role {me.get('role')}; `~=Vertical` "
        f"resolves to node {found.get('index')} (the 90° run is {me.get('index')})",
    )


def _ink_inside(c):
    """Count the callback's own colour inside the box and just outside.

    **Pixels, because there is nothing else to read.** A `PaintBox`
    publishes no value and takes no input; what it promises is that the
    application's `fn` ran and that what it drew stayed inside. Both are
    claims about the frame, so the frame is what this reads.
    """
    import base64
    import io
    import zlib

    shot = c.c.rpc("ui.screenshot", {}) if False else c.c.rpc("ui.screenshot")
    png = base64.b64decode(shot["image_base64"])
    w, h, px = _decode(png)
    # **Physical pixels**: the screenshot is at the window's scale and the
    # box is logical. Unscaled, the scale-2 pass (M57/D6) looked for ink
    # in the top-left quarter of the window and found none.
    sc = shot.get("scale", 1.0)
    b = [v * sc for v in c.box()]
    ink = (0x25, 0x63, 0xeb)
    inside = outside = 0
    for y in range(max(0, int(b[1]) - 6), min(h, int(b[3]) + 6)):
        for x in range(max(0, int(b[0]) - 6), min(w, int(b[2]) + 6)):
            p = px[(y * w + x) * 4:(y * w + x) * 4 + 3]
            if abs(p[0] - ink[0]) < 24 and abs(p[1] - ink[1]) < 24 and abs(p[2] - ink[2]) < 24:
                # **A pixel is outside only if it is wholly outside.** The
                # box's edges need not fall on whole pixels -- a 320 px box
                # centred in a 743 px stage starts at x = 524.5 (M56) --
                # and a pixel straddling an edge is half inside, so the
                # clip may ink it. Comparing the pixel's index with a
                # fractional edge counted it outside: six such pixels
                # failed this action on an exactly clipped sparkline.
                if x + 1 <= b[0] or x >= b[2] or y + 1 <= b[1] or y >= b[3]:
                    outside += 1
                else:
                    inside += 1
    return (
        inside > 200 and outside == 0,
        f"{inside} px of the callback's ink inside the box and {outside} "
        f"outside it — the sparkline's first and last dots straddle the "
        f"box's edges by 3 px, and only the clip keeps that half in "
        f"(18 px outside with the clip removed, M57/B9)",
    )


# The star `examples/showcase/src/catalog.rs` fills (`STAR_*`): its centre
# at this fraction of the box's width (vertically centred), its outer
# radius a fraction of the box's shorter side, its inner one a fraction of
# that, and its colour. The two files move together.
STAR_X, STAR_RADIUS, STAR_INNER = 0.78, 0.38, 0.4
STAR_INK = (0xE1, 0x4D, 0x0A)


def _star_filled(c):
    """The star's colour inside the star's analytic outline, and outside
    the box.

    **Against the outline's area, not a pixel count of the outline**: a
    path that stopped filling would still leave the antialiased edge's
    pixels, and a rasteriser that filled its box would match every pixel
    a count of the outline asked about. The area is the shoelace of the
    ten vertices in physical pixels; an edge pixel is part ink and does not
    match the colour, so the share sits a little under 100%.
    """
    import math

    shot = c.c.rpc("ui.screenshot")
    png = base64_decode(shot["image_base64"])
    w, h, px = _decode(png)
    sc = shot.get("scale", 1.0)
    b = [v * sc for v in c.box()]
    bw, bh = b[2] - b[0], b[3] - b[1]
    cx, cy = b[0] + STAR_X * bw, b[1] + 0.5 * bh
    r = STAR_RADIUS * min(bw, bh)
    poly = []
    for i in range(10):
        rr = r if i % 2 == 0 else r * STAR_INNER
        a = -math.pi / 2 + i * math.pi / 5
        poly.append((cx + rr * math.cos(a), cy + rr * math.sin(a)))
    area = abs(sum(poly[i][0] * poly[(i + 1) % 10][1] - poly[(i + 1) % 10][0] * poly[i][1] for i in range(10))) / 2

    def inside(x, y):
        wn = 0
        for i in range(10):
            (ax, ay), (qx, qy) = poly[i], poly[(i + 1) % 10]
            if (ay <= y) != (qy <= y):
                t = (y - ay) / (qy - ay)
                if x < ax + t * (qx - ax):
                    wn += 1 if qy > ay else -1
        return wn != 0

    def star(p):
        return all(abs(p[k] - STAR_INK[k]) < 24 for k in range(3))

    hit = outside = 0
    for y in range(max(0, int(b[1]) - 6), min(h, int(b[3]) + 6)):
        for x in range(max(0, int(b[0]) - 6), min(w, int(b[2]) + 6)):
            p = px[(y * w + x) * 4:(y * w + x) * 4 + 3]
            if not star(p):
                continue
            if x + 1 <= b[0] or x >= b[2] or y + 1 <= b[1] or y >= b[3]:
                outside += 1
            elif inside(x + 0.5, y + 0.5):
                hit += 1
    share = hit / area if area else 0.0
    return (
        share >= 0.90 and outside == 0,
        f"{hit} px of the star's colour inside its outline of {area:.0f} px² "
        f"({share:.1%}, bound 90%), {outside} outside the box (bound 0), scale {sc}",
    )


def base64_decode(s):
    import base64

    return base64.b64decode(s)


def _decode(png: bytes):
    """Minimal PNG reader: the agent's screenshots are RGBA, no filters
    beyond the per-row ones, which is what the encoder writes."""
    import struct
    import zlib

    pos = 8
    w = h = 0
    idat = b""
    while pos < len(png):
        (ln,) = struct.unpack(">I", png[pos:pos + 4])
        typ = png[pos + 4:pos + 8]
        data = png[pos + 8:pos + 8 + ln]
        if typ == b"IHDR":
            w, h = struct.unpack(">II", data[:8])
        elif typ == b"IDAT":
            idat += data
        pos += 12 + ln
    raw = zlib.decompress(idat)
    out = bytearray(w * h * 4)
    stride = w * 4
    prev = bytearray(stride)
    at = 0
    for y in range(h):
        f = raw[at]
        at += 1
        line = bytearray(raw[at:at + stride])
        at += stride
        for i in range(stride):
            a = line[i - 4] if i >= 4 else 0
            bb = prev[i]
            cc = prev[i - 4] if i >= 4 else 0
            if f == 1:
                line[i] = (line[i] + a) & 0xFF
            elif f == 2:
                line[i] = (line[i] + bb) & 0xFF
            elif f == 3:
                line[i] = (line[i] + (a + bb) // 2) & 0xFF
            elif f == 4:
                p = a + bb - cc
                pa, pb, pc = abs(p - a), abs(p - bb), abs(p - cc)
                pr = a if (pa <= pb and pa <= pc) else (bb if pb <= pc else cc)
                line[i] = (line[i] + pr) & 0xFF
        out[y * stride:(y + 1) * stride] = line
        prev = line
    return w, h, bytes(out)


# M87/T87.3: a LinkedMarkdown's links run the application's handler. A
# link is a `Link` child of the paragraph, named by its text -- one word
# each in the demo, because a `~=` term cannot hold a space -- so the
# click is addressed to the link itself, not to a guessed coordinate.
# The handler writes the store and the readout looks the target up with
# `link_target`; the Markdown itself repaints nothing on a click, so the
# readout is the action's subject.
LINK = "#d_mdlinks_a > ~=changelog"
CHANGES = "link: https://lumen.example/changes"

LINKED_MARKDOWN = ("LinkedMarkdown", "#d_mdlinks_a", [
    Action(
        # The readout is the consequence, and it shows: but an empty
        # readout chip has no area to hash before the click, so the
        # visible check watches the window.
        visible=True,
        region="window",
        subject="#readout",
        name="a click on a link runs the handler with that link",
        needs="a paragraph with two links, each published as a Link child named by its text",
        do=lambda c: (c.__dict__.__setitem__("_r0", c.readout()), c.click(LINK))[1],
        expect=lambda c: (
            c.readout() == CHANGES and c.readout() != c._r0,
            "readout {!r} -> {!r} (bound {!r})".format(c._r0, c.readout(), CHANGES),
        ),
        # The first word, "Read", is plain text: a press there is the
        # paragraph's, and no link's handler may run.
        isolate=lambda c: (c.__dict__.__setitem__("_r0", c.readout()),
                           c.c.rpc("input.click", x=c.box()[0] + 8, y=c.box()[1] + 9))[1],
        injection="push no link children in Markdown::observe (`~=changelog` then "
                  "names no node), or hit answering inner: None",
    ),
    # M87/T87.4: the demo's last line is `![..](spin)`, which names the id
    # the Image demo registers (`catalog::SPIN`). The picture is a block
    # below the paragraph's words, published as an `Image` child whose
    # label is the alt text and whose bounds are where it is drawn.
    Action(
        name="the image is drawn where its markup is",
        needs="a document whose last line is an image naming the showcase's registered id",
        do=lambda c: c.hover(),
        expect=lambda c: _image_drawn(c),
        static=True,
        injection="skip the Painter::image call in RichText::draw (the image's box "
                  "is the page fill)",
    ),
])

ALT = "The showcase's spinning ring"


def _image_drawn(c):
    """The demo's `Image` child: its label is the alt text, its box is
    inside the paragraph below the words, and **ink is in that box** — the
    pixels differ from the page fill, read from the screenshot 16 px right
    of the image at its middle, inside the paragraph, where a block leaves
    nothing drawn. **Read, not assumed**: the fill is the palette's, and
    it is (255, 255, 255) when this entry runs first but (226, 232, 240)
    after `Markdown`'s quote action has put the window on `light` -- a
    fixed ground would count a blank box as all ink in one of the two. The count of off-fill
    pixels is the assertion rather than a `ui.regionHash` inequality,
    because two hashes differing says nothing about *which* one is the
    fill; the box's hash is printed for the record."""
    para = c.box()
    kids = [n for n in c.c.tree()["nodes"] if n.get("role") == "Image"
            and n.get("bounds") and n["bounds"][0] >= para[0] - 1 and n["bounds"][2] <= para[2] + 1
            and n["bounds"][1] >= para[1] - 1 and n["bounds"][3] <= para[3] + 1]
    if len(kids) != 1:
        return False, f"{len(kids)} Image node(s) inside the paragraph {para} (bound 1)"
    b, label = kids[0]["bounds"], kids[0].get("label")
    shot = c.c.rpc("ui.screenshot")
    w, h, px = _decode(base64_decode(shot["image_base64"]))
    sc = shot.get("scale", 1.0)
    gx, gy = int((b[2] + 16) * sc), int((b[1] + b[3]) / 2 * sc)
    if b[2] + 16 >= para[2]:
        return False, f"no room right of the image {b} inside the paragraph {para} to read the fill"
    ground = px[(gy * w + gx) * 4:(gy * w + gx) * 4 + 3]
    ink = total = 0
    for y in range(int(b[1] * sc) + 1, int(b[3] * sc) - 1):
        for x in range(int(b[0] * sc) + 1, int(b[2] * sc) - 1):
            total += 1
            p = px[(y * w + x) * 4:(y * w + x) * 4 + 3]
            if max(abs(p[k] - ground[k]) for k in range(3)) > 24:
                ink += 1
    bw, bh = b[2] - b[0], b[3] - b[1]
    hb = c.c.rpc("ui.regionHash", rect=b).get("hash")
    frac = ink / total if total else 0.0
    ok = label == ALT and bw > 8 and bh > 8 and b[1] > para[1] + 10 and frac >= 0.05
    return ok, ("Image child {!r} (bound {!r}) at {} ({:.0f}x{:.0f} px); {} of {} px off the fill {} "
                "({:.1%}, bound >= 5%); regionHash {}").format(
        label, ALT, [round(v, 1) for v in b], bw, bh, ink, total, tuple(ground), frac, hb)

ALL = [RICHTEXT, MARKDOWN, LINKED_MARKDOWN, EDITOR, PAINTBOX, VECTORTEXT]
