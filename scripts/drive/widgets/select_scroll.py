"""M44/T44.10 — `Select` and `ScrollView`, live.

Both were owed by M42 (`scripts/drive/undriven.allow`): `Select` because
choosing a row from the keyboard was R3-F6 and an inventory that avoided
it would have been vacuous for exactly that action, and `ScrollView`
because no milestone owned it until the index assigned it here.

The stage's `Select` is controlled like any application's — the showcase
binds its `open` by hand and reads the choice into the readout — and it is
addressed **through its owner**, `#d_select_a > ...`, because the header's
picker is a `Select` too and carries the same part ids.
"""
from harness import Action

TRIG = "#d_select_a > #select-trigger"
MENU = "#d_select_a > #select-menu"


def _open(c) -> bool:
    try:
        b = c.box(MENU)
        return b[3] - b[1] > 1.0
    except Exception:
        return False


def _row(c, k: int):
    """Press and release on the k-th row of the open menu (30 px rows)."""
    b = c.box(MENU)
    x, y = (b[0] + b[2]) / 2, b[1] + 30.0 * k + 15.0
    c.c.rpc("input.pointerDown", x=x, y=y)
    c.c.rpc("input.pointerUp", x=x, y=y)


def _focused(c, sel: str) -> bool:
    return c.styles(sel)["state"]["focused"]


def _keys(c, *names):
    for n in names:
        c.key(n)


SELECT = ("Select", "#d_select_a", [
    Action(
        name="pressing the trigger opens the menu",
        needs="a closed select",
        prep=lambda c: c.key("Escape"),
        do=lambda c: c.click(TRIG),
        expect=lambda c: (
            _open(c),
            "the menu is {}".format(
                "{:.0f} px tall".format(c.box(MENU)[3] - c.box(MENU)[1]) if _open(c)
                else "closed"),
        ),
        isolate=lambda c: (c.key("Escape"), c.click_outside())[0],
        injection="leave the trigger's on_click unset, so nothing sets `select_open`",
    ),
    Action(
        name="releasing over a row chooses it and closes the menu",
        needs="an open select",
        prep=lambda c: None if _open(c) else c.click(TRIG),
        do=lambda c: (c.__dict__.__setitem__("_r0", c.readout()), _row(c, 2))[0],
        expect=lambda c: (
            c.readout() == "chose: Cherry" and c._r0 != c.readout() and not _open(c),
            f"readout {c._r0!r} -> {c.readout()!r} (wanted 'chose: Cherry', the third row), "
            f"open={_open(c)}",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_r0", c.readout()),
                           c.click(TRIG), c.click_outside())[0],
        injection="make the row's on_click forget to record `cx.row`",
        # Choosing rewrites the trigger's label, which relays out the
        # stage — and the showcase's note is a wrapping `Label` whose
        # unwrapped natural width always exceeds its box, so `W1005`
        # re-reports it; the PickList's choosing action says the same.
        expects_lint=("W1005",),
    ),
    Action(
        name="the keyboard opens it, walks the rows and chooses one",
        needs="a closed select whose trigger has focus",
        # The choice above returned focus to the trigger (T44.9); Escape
        # makes sure nothing is open.
        prep=lambda c: c.key("Escape"),
        do=lambda c: (c.__dict__.__setitem__("_r0", c.readout()),
                      c.__dict__.__setitem__("_f0", _focused(c, TRIG)),
                      _keys(c, "Enter", "Down", "Down", "Down", "Down", "Enter"))[0],
        expect=lambda c: (
            c._f0 and c.readout() == "chose: Damson" and not _open(c)
            and _focused(c, TRIG),
            f"trigger focused before: {c._f0}; readout {c._r0!r} -> {c.readout()!r} after "
            f"Enter, Down x4, Enter (wanted 'chose: Damson', the fourth row); open={_open(c)}; "
            f"focus back on the trigger: {_focused(c, TRIG)}",
        ),
        injection="leave `ctx.row` unset on a key activation, or keep focus off the list",
        # Choosing rewrites the trigger's label, which relays out the
        # stage — and the showcase's note is a wrapping `Label` whose
        # unwrapped natural width always exceeds its box, so `W1005`
        # re-reports it; the PickList's choosing action says the same.
        expects_lint=("W1005",),
    ),
    Action(
        name="Escape closes it and focus returns to the trigger",
        needs="a select opened from the keyboard",
        prep=lambda c: (c.key("Escape"), c.key("Enter"))[0],
        do=lambda c: (c.__dict__.__setitem__("_o0", _open(c)), c.key("Escape"))[0],
        expect=lambda c: (
            c._o0 and not _open(c) and _focused(c, TRIG),
            f"open before {c._o0}, after {_open(c)}; the trigger has focus: {_focused(c, TRIG)}",
        ),
        injection="clear focus on Escape instead of returning it",
        # Closing relays out the stage too, and re-reports the note.
        expects_lint=("W1005",),
    ),
])


def _scroll(c) -> float:
    r = c.readout()
    return float(r.split()[1]) if r.startswith("scroll:") else -1.0


SCROLLVIEW = ("ScrollView", "#d_scrollview_a", [
    Action(
        name="PageDown from a focused row scrolls the view",
        needs="a view at the top, with its first row visible",
        do=lambda c: (c.__dict__.__setitem__("_s0", _scroll(c)),
                      c.click("#d_scroll_first"), c.key("PageDown"))[0],
        expect=lambda c: (
            _scroll(c) > c._s0 + 100.0,
            f"scroll {c._s0:.0f} -> {_scroll(c):.0f} px after PageDown (most of a 180 px view)",
        ),
        injection="drop the PageDown arm in ScrollView::on_event",
    ),
    Action(
        name="the wheel scrolls it back",
        needs="a view scrolled away from the top",
        do=lambda c: (c.__dict__.__setitem__("_s0", _scroll(c)),
                      c.c.rpc("input.scroll", x=(c.box()[0] + c.box()[2]) / 2,
                              y=(c.box()[1] + c.box()[3]) / 2, dy=90.0))[0],
        expect=lambda c: (
            _scroll(c) < c._s0,
            f"scroll {c._s0:.0f} -> {_scroll(c):.0f} px after a wheel up",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_s0", _scroll(c)),
                           c.c.rpc("input.scroll", x=(c.box()[0] + c.box()[2]) / 2,
                                   y=c.box()[1] - 6.0, dy=90.0))[0],
        injection="decline the wheel in ScrollView::on_event",
    ),
    Action(
        name="a drag on the bar scrolls it",
        needs="a view whose bar has room to move",
        prep=lambda c: c.c.rpc("input.scroll", x=(c.box()[0] + c.box()[2]) / 2,
                                y=(c.box()[1] + c.box()[3]) / 2, dy=2000.0),
        do=lambda c: (c.__dict__.__setitem__("_s0", _scroll(c)),
                      _drag(c, c.box()[2] - 4.0))[0],
        expect=lambda c: (
            _scroll(c) > c._s0 + 30.0,
            f"scroll {c._s0:.0f} -> {_scroll(c):.0f} px after dragging the bar 60 px",
        ),
        # The same drag 40 px left of the bar, on a row: a press there is
        # the row's, and dragging it scrolls nothing.
        isolate=lambda c: (c.__dict__.__setitem__("_s0", _scroll(c)),
                           _drag(c, c.box()[2] - 44.0))[0],
        injection="hit-test the content before the bar in ScrollView::hit",
    ),
])


def _drag(c, x: float):
    """Press at `x` near the view's top, move 60 px down, release."""
    b = c.box()
    y = b[1] + 12.0
    c.c.rpc("input.pointerDown", x=x, y=y)
    c.c.rpc("input.hover", x=x, y=y + 60.0)
    c.c.rpc("input.pointerUp", x=x, y=y + 60.0)


ALL = [SELECT, SCROLLVIEW]
