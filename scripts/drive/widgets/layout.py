"""M27/T27.8 — the layout group, live.

`Wrap` and `Grid` are derive axes rather than widgets, so what there is
to drive is **geometry**: a wrap's children on two lines and a grid's on
its column count. Both are static claims and say so, which is what the
`static` flag is for. `Layout` (M85/T85.1) is the same kind of claim
about a length, `Flexible`, rather than a widget.
"""
from harness import Action


def _rows(c, ids):
    """Distinct y positions, rounded — how many lines the children are on."""
    ys = sorted({round(c.box(i)[1]) for i in ids})
    return ys


WRAP_IDS = [f"#d_wrap_{k}" for k in "abcdef"]
GRID_IDS = [f"#d_grid_{k}" for k in "abcdef"]


WRAP = ("Wrap", "#d_wrap_a", [
    Action(
        name="six chips land on more than one line",
        needs="a wrap narrower than its six children laid end to end",
        do=lambda c: c.hover(),
        expect=lambda c: (
            len(_rows(c, WRAP_IDS)) >= 2,
            f"children are on {len(_rows(c, WRAP_IDS))} line(s) at y={_rows(c, WRAP_IDS)}",
        ),
        static=True,
        injection="never break the line in the flow layout body",
    ),
    Action(
        name="every child is inside the container it reported",
        needs="a wrapped container",
        do=lambda c: c.hover(),
        expect=lambda c: (
            all(
                c.box(i)[0] >= c.box("#demo")[0] - 1
                and c.box(i)[2] <= c.box("#demo")[2] + 1
                for i in WRAP_IDS
            ),
            "all six within the stage's {}".format([round(v) for v in c.box("#demo")]),
        ),
        static=True,
        injection="make `place` forget the line height — children then "
                  "overlap and run past the box `layout` reported",
    ),
])


GRID = ("Grid", "#d_grid_a", [
    Action(
        name="six badges land on exactly two rows of three",
        needs="a grid wide enough that a wrap would not break",
        do=lambda c: c.hover(),
        expect=lambda c: (
            len(_rows(c, GRID_IDS)) == 2,
            f"children are on {len(_rows(c, GRID_IDS))} row(s) at y={_rows(c, GRID_IDS)} — "
            f"a width-based break in this box would read 1",
        ),
        static=True,
        injection="break the grid on width instead of on its column count",
    ),
])


LAYOUT_IDS = ["#d_layout_flex", "#d_layout_fill"]
#: The flexible bar's content width: the showcase's `Fixed { w: 80.0 }`.
FLEX_CONTENT = 80.0


def _flexible(c):
    """M85/T85.1 — `Length::Flexible` beside a fill, read off the window.

    The flexible bar's share is half the row, so it is
    `min(80, row / 2)` wide and the fill is the rest of the row. The row's
    width is read from the two boxes, so the rule holds at whatever width
    the stage gave it."""
    f, g = c.box("#d_layout_flex"), c.box("#d_layout_fill")
    row = g[2] - f[0]
    fw, gw = f[2] - f[0], g[2] - g[0]
    want = min(FLEX_CONTENT, row / 2)
    lines = len(_rows(c, LAYOUT_IDS))
    ok = (abs(fw - want) <= 0.5 and abs(gw - (row - fw)) <= 0.5
          and lines == 1 and row > 2 * FLEX_CONTENT)
    return ok, (f"row {row:.1f} px, {lines} line(s) (bound 1): flexible {fw:.1f} px "
                f"(bound min({FLEX_CONTENT:.0f}, {row / 2:.1f}) = {want:.1f}), "
                f"fill {gw:.1f} px (bound {row - fw:.1f})")


LAYOUT = ("Layout", "#d_layout_flex", [
    Action(
        name="a flexible bar keeps its content up to half; the fill takes the rest",
        needs="a row wider than twice the flexible bar's 80 px of content",
        do=lambda c: c.hover(),
        expect=_flexible,
        static=True,
        injection="treat Flexible as FillPortion in the derive's scan 2 — the "
                  "bar then reads half the row, 150 px of 300",
    ),
])


SPLIT = ("SplitPane", "#d_split_a", [
    Action(
        visible=True,
        name="dragging the divider moves it",
        needs="a split pane at its default halfway point",
        # **A transition, not a state.** `split < 0.45` is a fact about
        # where the divider *is*; once a real drag has put it at 0.30 the
        # isolate arm's click-outside changes nothing and the predicate
        # is still true. The slider had the same hole in T27.5.
        do=lambda c: (c.__dict__.__setitem__("_s0", c.value()),
                      _drag_divider(c, 0.3))[0],
        expect=lambda c: (
            c.value() != c._s0 and float(c.value()) < 0.45,
            f"split {c._s0!r} -> {c.value()!r} after a drag to 30%",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_s0", c.value()),
                           c.click_outside())[0],
        injection="drop the PointerDown arm in SplitPane::on_event",
    ),
    Action(
        visible=True,
        name="it stops short of the edge",
        needs="a split pane that can be dragged",
        # **Inside the widget, at its very edge.** A press at a negative
        # x never reaches the widget at all, and the harness correctly
        # reported that nothing had — which is a true statement about a
        # click that missed, not about the clamp.
        do=lambda c: _drag_divider(c, 0.002),
        expect=lambda c: (
            0.0 < float(c.value()) < 0.2,
            f"split is {c.value()} after dragging to the far left — it must stop "
            f"short, because a pane with no area cannot be clicked back",
        ),
        # Squeezing a pane to its stop truncates the text in it, and
        # `W1005` saying so is the lint working — not a defect this
        # action introduced.
        expects_lint=("W1005",),
        injection="clamp the split to 0..1 instead of to the stops",
    ),
])


def _drag_divider(c, frac):
    """Press **on the divider**, move to `frac` of the width, let go.

    This pressed *at* `frac` and let go, inside a pane — which moved the
    divider only because a press anywhere in a pane jumped it there, the
    defect M57/A4 fixed (`docs/widget-audit.md` X2). The drive asserted
    the bug as the behaviour. The divider is drawn at `(w - 6) * split`,
    6 px wide, and its grab zone is three times that.
    """
    b = c.box()
    y = (b[1] + b[3]) / 2
    w = b[2] - b[0]
    split = float(c.value() or 0.5)
    on_bar = b[0] + (w - 6.0) * split + 3.0
    to = b[0] + w * frac
    c.c.rpc("input.pointerDown", x=on_bar, y=y)
    c.c.rpc("input.hover", x=to, y=y)
    c.c.rpc("input.pointerUp", x=to, y=y)


# M57/C7: the demo opens its first section, as its comment always said,
# and the application keeps one open at a time -- which is what an
# accordion is. So the inventory drives the *second* section, which starts
# closed, and its last action asserts the one-open rule; "the other
# sections are independent" asserted the rule's absence.
def _click_header(c):
    b = c.box()
    return c.c.rpc("input.click", x=(b[0] + b[2]) / 2, y=b[1] + 20)


ACCORDION = ("Accordion", "#d_acc_b", [
    Action(
        visible=True,
        name="a press on the header opens the section",
        needs="a collapsed section",
        do=lambda c: (c.__dict__.__setitem__("_h0", c.box()[3] - c.box()[1]),
                      c.click())[0],
        expect=lambda c: (
            c.value() == "expanded" and (c.box()[3] - c.box()[1]) > c._h0,
            f"state {c.value()!r}, height {c._h0:.0f} -> {c.box()[3] - c.box()[1]:.0f} px",
        ),
        # The capture has to be re-taken here, or the isolate arm
        # compares the *outside* press against the height from before the
        # real one and reports a section that never moved as having moved.
        isolate=lambda c: (c.__dict__.__setitem__("_h0", c.box()[3] - c.box()[1]),
                           c.click_outside())[0],
        reflows=True,
        # The first section closing is the one-open rule at work.
        expects_lint=("W1005", "W1001"),
        injection="drop the header press arm in Collapsible::on_event",
    ),
    Action(
        visible=True,
        name="a press on the header again closes it",
        needs="an open section",
        # **On the header**, 20 px into its 40: this clicked the open
        # section's centre, which landed in the header only while the
        # body was short -- a 12 px gap under the body (M57/B47) moved the
        # centre onto the body and the press did nothing.
        do=lambda c: _click_header(c),
        expect=lambda c: (
            c.value() == "collapsed",
            f"state {c.value()!r}",
        ),
        reflows=True,
        # A collapsing section's body leaves the screen, and the lint is
        # correct about the frame it sees: squeezed on the way out
        # (`W1005`) while the demo sat in the stage's old 248 px share,
        # laid out at 0x0 (`W1001`) since M56 gives it its own 320 px. A
        # collapsed body is audited because its addresses still answer
        # with the body, where an absent `Option` branch answers with its
        # container and is not — recorded in docs/plan-showcase-design.md
        # T56.4, not changed here.
        expects_lint=("W1005", "W1001"),
        injection="make the press set `open = true` instead of toggling",
    ),
    Action(
        # The section that closes is another one: watch the stage (D1).
        visible=True,
        region="#demo",
        name="opening one closes the one that was open",
        needs="three sections, the second open",
        do=lambda c: (c.click(), c.click("#d_acc_c")),
        expect=lambda c: (
            c.value("#d_acc_c") == "expanded" and c.value() == "collapsed"
            and c.value("#d_acc_a") == "collapsed",
            f"a={c.value('#d_acc_a')!r} b={c.value()!r} c={c.value('#d_acc_c')!r} "
            f"— one section open at a time is the application's rule",
        ),
        reflows=True,
        expects_lint=("W1005", "W1001"),
        injection="drop the one-open rule from the showcase's refresh",
    ),
])

def _fracs(c):
    """The grid's three dividers, as it publishes them."""
    return [float(x) for x in c.value().split(",")]


def _pane_h(c, text: str) -> float:
    """How tall the pane holding `text` is.

    By its label's text, because the demo's four panes are `Label`s and
    a bare `Label` carries no id. Geometry rather than the widget's own
    fractions: the fractions are *state*, and an injection that ignores
    one while still reporting it moves the picture and not the number.
    """
    b = c.box(f"~{text}")
    return b[3] - b[1]


def _drag(c, x_frac: float, y_frac: float, grab: str):
    """Press on one divider and drag to a point.

    `grab` names which: the press decides, and the drag keeps it — which
    is the property the second action is about.
    """
    b = c.box()
    (w, h) = (b[2] - b[0], b[3] - b[1])
    v, left, _ = _fracs(c)
    if grab == "v":
        start = (b[0] + w * v, b[1] + h * 0.25)
    else:
        start = (b[0] + w * 0.25, b[1] + h * left)
    c.c.rpc("input.pointerDown", x=start[0], y=start[1])
    c.c.rpc("input.hover", x=b[0] + w * x_frac, y=b[1] + h * y_frac)
    c.c.rpc("input.pointerUp", x=b[0] + w * x_frac, y=b[1] + h * y_frac)


PANEGRID = ("PaneGrid", "#d_panegrid_a", [
    Action(
        visible=True,
        name="a column's divider moves only that column",
        needs="a grid whose three dividers all sit at 0.5",
        do=lambda c: (c.__dict__.__setitem__("_f0", _fracs(c)),
                      c.__dict__.__setitem__(
                          "_panes", [_pane_h(c, "northwest"),
                                     _pane_h(c, "northeast")]),
                      _drag(c, 0.25, 0.25, "h"))[0],
        expect=lambda c: (
            _fracs(c)[1] < c._f0[1] - 0.1
            and _fracs(c)[2] == c._f0[2]
            and _fracs(c)[0] == c._f0[0]
            and _pane_h(c, "northwest") < c._panes[0] - 10.0
            and abs(_pane_h(c, "northeast") - c._panes[1]) < 0.5,
            "v,left,right {} -> {}; the top panes are {:.0f} and {:.0f} px "
            "tall, from {:.0f} and {:.0f}. **The fractions alone are not the "
            "assertion**: an injection that gave both columns one divider "
            "left them unchanged and moved only the geometry, which is the "
            "half a reader sees."
            .format(c._f0, _fracs(c), _pane_h(c, "northwest"),
                    _pane_h(c, "northeast"), c._panes[0], c._panes[1]),
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_f0", _fracs(c)),
                           c.__dict__.__setitem__(
                               "_panes", [_pane_h(c, "northwest"),
                                          _pane_h(c, "northeast")]),
                           c.click_outside())[0],
        injection="give both columns one horizontal divider",
    ),
    Action(
        visible=True,
        name="a drag keeps the divider it started on",
        needs="a grid with a divider to grab",
        do=lambda c: (c.__dict__.__setitem__("_f0", _fracs(c)),
                      _drag(c, 0.8, 0.8, "v"))[0],
        expect=lambda c: (
            _fracs(c)[0] > c._f0[0] + 0.1
            and _fracs(c)[1] == c._f0[1]
            and _fracs(c)[2] == c._f0[2],
            "v,left,right {} -> {}: the drag started on the vertical divider "
            "and crossed both horizontals' grab margins"
            .format(c._f0, _fracs(c)),
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_f0", _fracs(c)),
                           c.click_outside())[0],
        # Squeezing the right column to a fifth of the grid does truncate
        # the labels in it, and `W1005` saying so is the lint working —
        # the code is named so the check stays on for every other one.
        expects_lint=("W1005",),
        injection="re-decide the grabbed divider on every move",
    ),
])

ALL = [WRAP, GRID, LAYOUT, SPLIT, ACCORDION, PANEGRID]
