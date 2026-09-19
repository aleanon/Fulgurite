"""M27/T27.8 — the layout group, live.

`Wrap` and `Grid` are derive axes rather than widgets, so what there is
to drive is **geometry**: a wrap's children on two lines and a grid's on
its column count. Both are static claims and say so, which is what the
`static` flag is for.
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


SPLIT = ("SplitPane", "#d_split_a", [
    Action(
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
    b = c.box()
    y = (b[1] + b[3]) / 2
    x = b[0] + (b[2] - b[0]) * frac
    c.c.rpc("input.pointerDown", x=x, y=y)
    c.c.rpc("input.pointerUp", x=x, y=y)


ACCORDION = ("Accordion", "#d_acc_a", [
    Action(
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
        injection="drop the header press arm in Collapsible::on_event",
    ),
    Action(
        name="a press on the header again closes it",
        needs="an open section",
        do=lambda c: c.click(),
        expect=lambda c: (
            c.value() == "collapsed",
            f"state {c.value()!r}",
        ),
        reflows=True,
        # A collapsing section squeezes its body on the way out; the
        # truncation lint is correct about the frame it sees.
        expects_lint=("W1005",),
        injection="make the press set `open = true` instead of toggling",
    ),
    Action(
        name="the other sections are independent",
        needs="three sections in a column",
        do=lambda c: c.click("#d_acc_b"),
        expect=lambda c: (
            c.value("#d_acc_b") == "expanded" and c.value() == "collapsed",
            f"a={c.value()!r} b={c.value('#d_acc_b')!r} — opening one must not "
            f"touch the others; the one-open rule is the application's",
        ),
        reflows=True,
        injection="share one `open` flag between sections",
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

ALL = [WRAP, GRID, SPLIT, ACCORDION, PANEGRID]
