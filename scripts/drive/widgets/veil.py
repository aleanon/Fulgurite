"""M27/T27.7 — `Modal`, `Sheet`, `Drawer` live.

The headless test presses the scrim by calling `Node::on_event`
directly. This presses it through the socket and a real hit test, which
is where a modal's whole claim lives: **a press on the scrim must not
reach what is behind it**, and only the dispatch path can say so.
"""
from harness import Action


def _count(c, name) -> int:
    """The number after `name:` in the readout — `dismissals: 2 ·
    confirmed: 1` since M57/C6, which split the two. This used to join
    every digit in the readout, which was right only while it held one
    number."""
    r = c.readout()
    tail = r.split(name + ":", 1)[1] if (name + ":") in r else ""
    digits = "".join(ch for ch in tail.strip().split(" ")[0] if ch.isdigit())
    return int(digits or -1)


def _dismissals(c) -> int:
    return _count(c, "dismissals")


def _confirmed(c) -> int:
    return _count(c, "confirmed")


def _press_scrim(c, title_sel):
    """A point inside the veil but outside its panel: the veil's top-right
    corner, which is scrim for all three kinds -- a modal's panel is
    centred, a sheet's sits along the bottom, a drawer's down the left.

    Asserted rather than assumed. The first version aimed right of the
    panel at the veil's *bottom* edge, which on a sheet is the panel; it
    counted anyway only because a press on the panel used to ask to
    dismiss too (the defect M57/A4 fixed), so the check proved nothing.
    """
    v = c.box()
    p = c.box(title_sel)
    x, y = v[2] - 6.0, v[1] + 6.0
    assert not (p[0] <= x <= p[2] and p[1] <= y <= p[3]), \
        f"setup: the scrim point ({x:.0f}, {y:.0f}) is on the panel's title"
    c.c.rpc("input.click", x=x, y=y)


def _veil(name, sel, title_sel, go_sel):
    return (name, sel, [
        Action(
            name="a press on the scrim asks to dismiss",
            needs="an open veil with its panel showing",
            do=lambda c: (c.__dict__.__setitem__("_n", _dismissals(c)),
                          _press_scrim(c, title_sel))[0],
            expect=lambda c: (
                _dismissals(c) == c._n + 1,
                f"dismissals {c._n} -> {_dismissals(c)}",
            ),
            # Outside the veil entirely: the stage around it. Nothing
            # must be requested from there, or the veil is asking to
            # close on presses it never received.
            isolate=lambda c: (c.__dict__.__setitem__("_n", _dismissals(c)),
                               c.click_outside(20.0))[0],
            injection="stop setting `dismissed` in Veil::on_event",
        ),
        Action(
            name="it does not close itself",
            needs="a veil whose scrim has just been pressed",
            do=lambda c: _press_scrim(c, title_sel),
            expect=lambda c: (
                c.styles()["state"] is not None and c.box()[2] > c.box()[0],
                "the veil is still laid out at {:.0f}x{:.0f} px — closing itself "
                "would zero it, and an app that must confirm could not undo that"
                .format(c.box()[2] - c.box()[0], c.box()[3] - c.box()[1]),
            ),
            injection="set `self.open = false` on a scrim press",
        ),
        Action(
            name="the control on the panel still reaches the application",
            needs="a veil with a live button on its panel",
            do=lambda c: (c.__dict__.__setitem__("_n", (_confirmed(c), _dismissals(c))),
                          c.click(go_sel))[0],
            # Its own counter since M57/C6: a confirm is counted, and it is
            # *not* a dismissal — a press on the panel used to read as one.
            expect=lambda c: (
                _confirmed(c) == c._n[0] + 1 and _dismissals(c) == c._n[1],
                f"confirmed {c._n[0]} -> {_confirmed(c)}, dismissals {c._n[1]} -> {_dismissals(c)}",
            ),
            injection="make Veil::handler return None — the shape `Dyn` shipped as",
        ),
    ])


# **One id per instance.** `VeilBody` was one type used three times and
# every id inside it was therefore on three nodes — `W1011`, and
# `ui.resolve` refused all three. The showcase now stamps a body type per
# veil, which is the same split `PairCheckA`/`PairCheckB` made in T27.3
# and the third time this shape has come up in M27.
ALL = [
    _veil("Modal", "#d_modal_a", "#d_modal_title", "#d_modal_go"),
    _veil("Sheet", "#d_sheet_a", "#d_sheet_title", "#d_sheet_go"),
    _veil("Drawer", "#d_drawer_a", "#d_drawer_title", "#d_drawer_go"),
]
