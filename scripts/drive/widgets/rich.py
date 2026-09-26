"""M27/T27.12 — the text tail, live.

Three of these four take no input at all, which makes them the tranche
where the inventory has to work hardest: an action that presses nothing
still has to *assert* something a defect could move. What each one
claims is therefore a property of the **rendered** text — the value the
widget publishes is what a reader sees, not what the source says — and
for `Markdown` that difference is every asterisk.
"""
from harness import Action


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
])

EDITOR = ("RichTextEditor", "#d_editor_a", [
    Action(
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
])


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
    b = c.box()
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


ALL = [RICHTEXT, MARKDOWN, EDITOR, PAINTBOX]
