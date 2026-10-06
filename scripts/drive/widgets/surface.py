"""M27/T27.4 — `Card`, `Toast`, `Tooltip` against the live window, and
M85/T85.2's `Pin` and M85/T85.4's `Pop`.

The Card's one action is the claim `Dyn` failed: a control *inside* a
wrapper must reach the application. The headless test makes it too, but
only the live drive makes it through the socket, the event-loop proxy and
a real hit test.

A `Toast` has no actions — it is a message, not a control. That is not a
reason to drive nothing: a widget with an empty inventory would make
`run_widget` pass having asserted only that the demo is on stage, which
is the vacuous shape this protocol exists to refuse. Its inventory is
geometric, and the assertions are the ones a screenshot cannot make.
"""
from harness import Action


def _presses(c) -> int:
    r = c.readout()
    return int("".join(ch for ch in r if ch.isdigit()) or -1)


def _box(c, sel):
    return c.box(sel)


def _area(b) -> float:
    return max(0.0, b[2] - b[0]) * max(0.0, b[3] - b[1])


# --- Card -----------------------------------------------------------
CARD = ("Card", "#d_card_a", [
    Action(
        visible=True,
        name="a button on the card reaches the application",
        needs="a card whose content is wired to the press counter",
        do=lambda c: (c.__dict__.__setitem__("_n", _presses(c)),
                      c.click("#d_card_go"))[0],
        expect=lambda c: (
            _presses(c) == c._n + 1,
            f"presses {c._n} -> {_presses(c)}, expected {c._n + 1}",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_n", _presses(c)),
                           c.click_outside())[0],
        injection="make Card::handler return None — the shape `Dyn` shipped as",
    ),
    Action(
        name="the content is inset by the card's padding",
        needs="a card with its default 16 px padding",
        do=lambda c: c.hover(),
        expect=lambda c: (
            _box(c, "#d_card_title")[0] > c.box()[0] + 8.0,
            f"title starts at x={_box(c, '#d_card_title')[0]:.0f}, card at "
            f"x={c.box()[0]:.0f} — expected at least 8 px of inset",
        ),
        static=True,
        injection="drop the padding from Card::type_defaults",
    ),
    Action(
        # M82/T82.3: the demo's ground is the program's fill 1, a radial
        # from `raised` to `track` (by role) under a transparent
        # background, rounded by the card's radius. Read off the window's
        # own pixels, at whatever scale the leg runs.
        name="the card's fill is a rounded radial",
        needs="a card whose background_fill names the program's radial",
        do=lambda c: c.hover(),
        expect=lambda c: _fill_check(c),
        static=True,
        injection="remove ops::fill from Card::paint",
    ),
])


def _fill_check(c):
    """The card's centre against its two side edges (a radial differs by
    at least 16 levels there), and its corner against the stage just
    outside it (a rounded fill leaves the corner unpainted)."""
    import base64, io
    from PIL import Image
    shot = c.c.rpc("ui.screenshot")
    k = shot.get("scale", 1.0)
    img = Image.open(io.BytesIO(base64.b64decode(shot["image_base64"]))).convert("RGB")
    px = img.load()
    b = c.box()
    at = lambda x, y: px[int(x * k), int(y * k)]
    cy = (b[1] + b[3]) / 2
    centre = at((b[0] + b[2]) / 2, cy)
    sides = [at(b[0] + 2, cy), at(b[2] - 3, cy)]
    corner, ground = at(b[0] + 1, b[1] + 1), at(b[0] - 4, b[1] + 1)
    diff = lambda p, q: max(abs(x - y) for x, y in zip(p, q))
    edge = min(diff(centre, s) for s in sides)
    off = diff(corner, ground)
    return (
        edge >= 16 and off <= 2,
        f"centre {centre} vs sides {sides}: {edge} levels (bound >= 16); "
        f"corner {corner} vs the stage {ground}: {off} levels (bound <= 2), scale {k}",
    )


# --- Toast ----------------------------------------------------------
# M57/B34: `#d_toast_a` floats at the window's bottom centre, hidden until
# "Save" shows it, closable, and gone after TOAST_MS (3000). `#d_toast_b`
# is the long message, in the flow.
import time


def _shown_w(c) -> float:
    """The floating toast's width, 0 when hidden (a hidden subtree has no
    box, and `ui.resolve` may then answer with none at all)."""
    try:
        b = c.box("#d_toast_a")
        return b[2] - b[0]
    except Exception:
        return 0.0


def _centre_x(c) -> float:
    b = _box(c, "#d_toast_a")
    return (b[0] + b[2]) / 2


def _window_w(c) -> float:
    """The window's width in logical px, from the screenshot."""
    shot = c.c.rpc("ui.screenshot")
    return shot["width"] / shot.get("scale", 1.0)


def _show(c):
    return c.click("#d_toast_show")


TOAST = ("Toast", "#d_toast_b", [
    Action(
        # The toast floats at the window's bottom, outside the demo (D1).
        visible=True,
        region="window",
        # The toast shown is `#d_toast_a`, a sibling of the inventory's
        # widget: it lands and damages where it appears, not on `#d_toast_b`.
        subject="#d_toast_a",
        name="Save shows a toast below the stage, centred on the window",
        needs="a hidden floating toast and its button",
        do=lambda c: (c.__dict__.__setitem__("_w0", _shown_w(c)), _show(c))[0],
        expect=lambda c: (
            c._w0 == 0.0 and _shown_w(c) > 0.0
            and _box(c, "#d_toast_a")[1] > _box(c, "#d_toast_b")[3]
            and abs(_centre_x(c) - _window_w(c) / 2) <= 1.0,
            "hidden width {:.0f} -> {:.0f}; toast top {} below the flow's toast bottom {}; "
            "centre x {} in a {:.0f} px window".format(
                c._w0, _shown_w(c),
                round(_box(c, "#d_toast_a")[1]) if _shown_w(c) else None,
                round(_box(c, "#d_toast_b")[3]),
                round(_centre_x(c)) if _shown_w(c) else None, _window_w(c)),
        ),
        injection="ignore Anchor::WindowBottom, or never call Toast::show",
    ),
    Action(
        name="it shrinks to its text rather than filling the window",
        needs="the short floating toast shown, and the long one in the flow",
        do=lambda c: c.hover(),
        expect=lambda c: (
            0.0 < _shown_w(c) < _box(c, "#d_toast_b")[2] - _box(c, "#d_toast_b")[0],
            "short {:.0f} px wide, long {:.0f} px — a FILL toast would read equal".format(
                _shown_w(c), _box(c, "#d_toast_b")[2] - _box(c, "#d_toast_b")[0],
            ),
        ),
        static=True,
        injection="make Toast::type_defaults width FILL",
    ),
    Action(
        # The toast floats at the window's bottom, outside the demo (D1).
        visible=True,
        region="window",
        subject="#d_toast_a",
        name="its × closes it",
        needs="the floating toast shown, with a ×",
        do=lambda c: (c.c.rpc("input.click", x=_box(c, "#d_toast_a")[2] - 6,
                              y=(_box(c, "#d_toast_a")[1] + _box(c, "#d_toast_a")[3]) / 2)),
        expect=lambda c: (_shown_w(c) == 0.0, f"width after the × {_shown_w(c):.0f}"),
        injection="drop the PointerUp arm in Toast::on_event",
    ),
    Action(
        visible=True,
        region="window",
        subject="#d_toast_a",
        name="shown again, it goes away by itself",
        needs="a toast shown for 3000 ms",
        # Shown in setup, so the "before" frame has it (D1).
        setup=_show,
        do=lambda c: (c.__dict__.__setitem__("_w0", _shown_w(c)), time.sleep(3.4), c.hover())[0],
        expect=lambda c: (
            c._w0 > 0.0 and _shown_w(c) == 0.0,
            f"width {c._w0:.0f} just after showing, {_shown_w(c):.0f} after 3.4 s",
        ),
        injection="stop ticking the toast in the showcase's advance, or in Toast::tick",
    ),
    Action(
        name="it paints a ground, so its light text is readable",
        needs="a toast with its default dark fill",
        do=lambda c: c.hover(),
        expect=lambda c: (
            c.styles()["state"]["painted_background"][3] > 200,
            f"painted background {c.styles()['state']['painted_background']} — "
            f"alpha must be opaque or the light text is on the page",
        ),
        static=True,
        injection="drop the background from Toast::type_defaults",
    ),
])


# --- Tooltip --------------------------------------------------------
def _tip_area(c) -> float:
    """The tip's area, read from the **content**, not the popup.

    A `Popup` node is a marker: it resolves to a zero-sized box at its
    anchor whether it is open or shut, and its content carries the
    geometry. Reading the popup reports `0 -> 0` for a tip that opened
    perfectly well — which this drive did, and the headless test could
    not contradict because it asks `Tooltip::is_open()` directly.
    `scripts/agent_drive.sh` has always read `#select-menu` rather than
    `#select-popup` for the same reason and never said why.
    """
    r = c.c.rpc("ui.resolve", selector="#tooltip-tip").get("bounds")
    return _area(r) if r else 0.0


TOOLTIP = ("Tooltip", "#tooltip-content", [
    Action(
        # The tip is the hover's consequence, drawn outside the content (D1).
        visible=True,
        region="window",
        pre_hover=False,
        name="resting on the content opens the tip",
        needs="a tooltip whose tip is closed",
        # M57/B35: the tip waits for the pointer to rest (500 ms), as
        # toolkits do -- it opened on the first hover frame, strobing every
        # tip a pointer crossed. The request after the wait runs a frame
        # whose tick covers the rest.
        do=lambda c: (c.__dict__.__setitem__("_a0", _tip_area(c)), c.hover(),
                      __import__("time").sleep(0.7),
                      c.c.rpc("ui.waitSettled", timeout_ms=1000))[0],
        expect=lambda c: (
            _tip_area(c) > 0.0 and c._a0 == 0.0,
            f"tip area {c._a0:.0f} -> {_tip_area(c):.0f} px²",
        ),
        # Moving the pointer off it must close the tip again. The
        # isolate arm is doing double duty here and that is the point:
        # a tip that opens and never closes passes any "did it open"
        # check ever written.
        isolate=lambda c: (c.__dict__.__setitem__("_a0", _tip_area(c)),
                           c.click_outside())[0],
        injection="key the tip on the wrapper's own `hovered`, which dispatch never sets",
    ),
    Action(
        # A claim about hit routing: the content's click lands, in the
        # readout; nothing in the box changes (D1).
        visible=False,
        name="the tip does not take the pointer from its content",
        needs="a tooltip with its tip open over the content",
        do=lambda c: (c.__dict__.__setitem__("_n", _presses(c)), c.click())[0],
        expect=lambda c: (
            _presses(c) == c._n + 1,
            f"presses {c._n} -> {_presses(c)} — a tip that took the hit would "
            f"leave this unchanged and close itself the instant it opened",
        ),
        injection="hit-test the tip before the content in Tooltip::hit",
    ),
], "#d_tooltip_a")

# --- Pin -----------------------------------------------------------
# M85/T85.2: `#d_pin_a` is a 240 x 120 pin (its demo box, raised so the
# edges show) holding `#d_pin_go` at (112, 64).
PIN_AT = (112.0, 64.0)


def _pin_offset(c):
    p, b = _box(c, "#d_pin_a"), _box(c, "#d_pin_go")
    dx, dy = b[0] - p[0], b[1] - p[1]
    return (
        abs(dx - PIN_AT[0]) <= 0.5 and abs(dy - PIN_AT[1]) <= 0.5
        and b[2] <= p[2] + 0.5 and b[3] <= p[3] + 0.5,
        f"button at ({dx:.1f}, {dy:.1f}) from the pin's origin (bound {PIN_AT}); "
        f"button {[round(v) for v in b]} inside the pin {[round(v) for v in p]}",
    )


def _pin_press(c):
    """The press counter moved by one, and the damage over the pin is the
    button's own box: each region that meets the pin, against the
    button's area, printed."""
    b = _box(c, "#d_pin_go")
    p = _box(c, "#d_pin_a")
    regions = c.c.rpc("ui.lastDamage", since=c._since).get("regions", [])
    over = [r for r in regions if not (r[2] <= p[0] or r[0] >= p[2] or r[3] <= p[1] or r[1] >= p[3])]
    worst = max((_area(r) for r in over), default=0.0)
    ratio = worst / max(_area(b), 1.0)
    return (
        _presses(c) == c._n + 1 and over and ratio <= 1.0 + 1e-3,
        f"presses {c._n} -> {_presses(c)}; {len(over)} damage region(s) over the pin, "
        f"the largest {worst:.0f} px² against the button's {_area(b):.0f} px² = "
        f"{ratio:.2f}x (bound 1.00)",
    )


PIN = ("Pin", "#d_pin_a", [
    Action(
        name="the button sits at the pin's origin plus its offset",
        needs="a 240 x 120 pin holding a button at (112, 64)",
        do=lambda c: c.hover(),
        expect=_pin_offset,
        static=True,
        injection="make Pin::place ignore the offset -- the button then sits "
                  "at the pin's origin, (0, 0)",
    ),
    Action(
        visible=True,
        subject="#d_pin_go",
        name="a click on the pinned button reaches the application",
        needs="a pinned button wired to the press counter",
        do=lambda c: (c.__dict__.__setitem__("_n", _presses(c)),
                      c.__dict__.__setitem__("_since", c.mark()),
                      c.click("#d_pin_go"))[0],
        expect=_pin_press,
        isolate=lambda c: (c.__dict__.__setitem__("_n", _presses(c)),
                           c.click_outside())[0],
        injection="make Pin::handler return None, or Pin::place ignore the offset",
    ),
])

# --- Pop -----------------------------------------------------------
# M85/T85.4: `#d_pop_a` is a 260 x 180 `ScrollView` (the ScrollView demo's
# shape) holding four rows and `#d_pop_item`, a `Pop` around an image-sized
# placeholder `#d_pop_img`, below its fold. The live gate for T85.3's
# visibility pass: nothing but the pass tells the application it is in
# view. The inventory's node is the `Pop` itself (its role is
# GenericContainer), so the harness bounds the scroll's damage against the
# placeholder's box and where it was and is.


def _window_h(c) -> float:
    """The window's height in logical px, from the screenshot."""
    shot = c.c.rpc("ui.screenshot")
    return shot["height"] / shot.get("scale", 1.0)


def _meets(a, b) -> bool:
    return max(a[0], b[0]) < min(a[2], b[2]) and max(a[1], b[1]) < min(a[3], b[3])


def _pop_fold(c):
    v, item, win = _box(c, "#d_pop_a"), _box(c, "#d_pop_item"), _window_h(c)
    vh = v[3] - v[1]
    return (
        vh < win and not _meets(item, v) and c.readout() == "waiting",
        f"viewport {vh:.0f} px tall in a {win:.0f} px window (bound viewport < window); "
        f"placeholder y {item[1]:.0f}..{item[3]:.0f} against the viewport's "
        f"{v[1]:.0f}..{v[3]:.0f} (outside); readout {c.readout()!r}",
    )


def _pop_reveal(c):
    """PageDown until the readout says loaded, at most five times."""
    c._s0 = c.mark()
    c._pages = 0
    while c.readout() != "loaded" and c._pages < 5:
        c.key("PageDown")
        c._pages += 1


def _last_changed(c, since: int):
    """The regions of the latest frame after `since` that changed anything,
    and that frame's number: walk back from the newest frame until a
    window holds a change."""
    now = c.mark()
    for k in range(now - 1, since - 1, -1):
        regions = c.c.rpc("ui.lastDamage", since=k).get("regions", [])
        if regions:
            return regions, k + 1, now
    return [], None, now


def _pop_loaded(c):
    """The application says loaded (its state, not a delivery count), the
    placeholder is in the view and shows it, and the frame that loaded it
    is printed with its damage over the view.

    **Not bounded at 1.00x here**: the window's input RPC pumps a frame the
    journal does not record (the one whose `place` queues the change), so
    the scroll and the load land in one journal frame, and the region
    over the view is the scroll's. The harness bounds that frame against
    the placeholder's box and where it was (4.0); the load's own damage,
    1.00x the placeholder, is gated per frame headlessly
    (`crates/lumen-static-widgets/tests/pop.rs`)."""
    item, v = _box(c, "#d_pop_item"), _box(c, "#d_pop_a")
    regions, frame, now = _last_changed(c, c._s0)
    over = [r for r in regions if _meets(r, v)]
    worst = max((_area(r) for r in over), default=0.0)
    return (
        c.readout() == "loaded" and _meets(item, v) and c.value("#d_pop_img") == "loaded",
        f"readout {c.readout()!r} after {c._pages} PageDown(s) (bound 'loaded'); placeholder y "
        f"{item[1]:.0f}..{item[3]:.0f} in the viewport {v[1]:.0f}..{v[3]:.0f}; its text "
        f"{c.value('#d_pop_img')!r} (bound 'loaded'); the frame that loaded it ({frame} of "
        f"{c._s0}..{now}, scroll and load together) has {len(over)} region(s) over the view, "
        f"the largest {worst:.0f} px² against the placeholder's {_area(item):.0f} px²",
    )


POP = ("Pop", "#d_pop_item", [
    Action(
        name="the placeholder starts below the fold of a view shorter than the window",
        needs="a freshly seeded Pop demo",
        do=lambda c: c.hover(),
        expect=_pop_fold,
        static=True,
        injection="make the view as tall as the window -- the bound viewport < window "
                  "fires, and no step could tell the viewport's clip from the window's",
    ),
    Action(
        # Shows in the view, not in the pop's starting box: that box is
        # below the fold, where nothing is drawn before or after.
        visible=True,
        region="#d_pop_a",
        name="PageDown until the placeholder is in view loads it",
        needs="a view at the top with focus on its first row, the placeholder waiting",
        prep=lambda c: c.click("#d_pop_first"),
        do=_pop_reveal,
        expect=_pop_loaded,
        injection="have Pop leave NodeDesc::WATCHES_VIEW off its entry -- the host never "
                  "watches it, the readout stays 'waiting', and the action fails its "
                  "consequence",
    ),
])

ALL = [CARD, PIN, POP, TOAST, TOOLTIP]
