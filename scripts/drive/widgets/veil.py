"""M27/T27.7 — `Modal`, `Sheet`, `Drawer` live.

The headless test presses the scrim by calling `Node::on_event`
directly. This presses it through the socket and a real hit test, which
is where a modal's whole claim lives: **a press on the scrim must not
reach what is behind it**, and only the dispatch path can say so.
"""
from harness import Action


def _dismissals(c) -> int:
    r = c.readout()
    return int("".join(ch for ch in r if ch.isdigit()) or -1)


def _press_scrim(c, title_sel):
    """A point inside the veil but outside its panel.

    Read from the two resolved boxes rather than computed from a
    fraction: a fraction that happened to land on the panel would report
    that the scrim does not dismiss, which is a true statement about the
    wrong pixel.
    """
    v = c.box()
    p = c.box(title_sel)
    # Below the panel's title and to the right of the panel, whichever
    # the kind leaves free. The veil is always at least as wide as the
    # panel for a drawer, and always taller for a sheet.
    x = min(v[2] - 4, p[2] + 20) if p[2] + 20 < v[2] else (v[0] + v[2]) / 2
    y = v[3] - 6
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
            # A veil never closes itself: the press is a request only the
            # application hears, and nothing on the veil changes until it
            # decides (M48/T48.4 sorted it; the readout is what moves).
            repaints=False,
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
            # Correctly paints nothing: the point is that the veil stays.
            repaints=False,
        ),
        Action(
            name="the control on the panel still reaches the application",
            needs="a veil with a live button on its panel",
            do=lambda c: (c.__dict__.__setitem__("_n", _dismissals(c)),
                          c.click(go_sel))[0],
            # The button bumps the same counter the dismissals use, so a
            # press on it looks like a dismissal from the readout — which
            # is fine here: what this asserts is that the press *reached*
            # something, where a veil that swallowed it would show no
            # change at all.
            expect=lambda c: (
                _dismissals(c) != c._n,
                f"counter {c._n} -> {_dismissals(c)} after pressing the panel's button",
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
