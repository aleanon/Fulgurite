"""M27/T27.4 — `Card`, `Toast`, `Tooltip` against the live window.

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
])


# --- Toast ----------------------------------------------------------
TOAST = ("Toast", "#d_toast_a", [
    Action(
        name="it shrinks to its text rather than filling the stage",
        needs="a short toast and a long one, side by side",
        do=lambda c: c.hover(),
        expect=lambda c: (
            _box(c, "#d_toast_a")[2] - _box(c, "#d_toast_a")[0]
            < _box(c, "#d_toast_b")[2] - _box(c, "#d_toast_b")[0],
            "short {:.0f} px wide, long {:.0f} px — a FILL toast would read equal".format(
                _box(c, "#d_toast_a")[2] - _box(c, "#d_toast_a")[0],
                _box(c, "#d_toast_b")[2] - _box(c, "#d_toast_b")[0],
            ),
        ),
        static=True,
        injection="make Toast::type_defaults width FILL",
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
        name="hovering the content opens the tip",
        needs="a tooltip whose tip is closed",
        do=lambda c: (c.__dict__.__setitem__("_a0", _tip_area(c)), c.hover())[0],
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
])

ALL = [CARD, TOAST, TOOLTIP]
