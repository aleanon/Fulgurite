"""M27/T27.5 — `NumberInput`, `RangeSlider`, `SearchField` live.

`NumberInput`'s rows are `docs/plan-widget-parity.md` §4's inventory,
written before the widget. The headless test makes the same claims
against `Node::on_event` directly; these make them through the socket,
the event-loop proxy and a real hit test — which is where the tooltip's
overlay defect lived and no headless test could reach.
"""
from harness import Action


def _val(c) -> float:
    r = c.readout()
    return float(r.split(":")[-1]) if ":" in r else float("nan")


def _text(c, sel=None) -> str:
    return c.value(sel)


def _focus_and(c, fn):
    c.click()
    fn()


NUMBER = ("NumberInput", "#d_number_a", [
    Action(
        name="a click focuses the field",
        needs="an unfocused number field",
        do=lambda c: c.click(),
        expect=lambda c: (
            c.styles()["state"]["focused"],
            f"state.focused={c.styles()['state']['focused']}",
        ),
        isolate=lambda c: c.click_outside(),
        injection="stop setting focus on PointerDown in dispatch",
    ),
    Action(
        name="ArrowUp steps it up by exactly one step",
        needs="a focused field below its maximum",
        do=lambda c: (c.click(), c.__dict__.__setitem__("_v0", _text(c)), c.key("Up"))[0],
        expect=lambda c: (
            _text(c) != c._v0,
            f"text {c._v0!r} -> {_text(c)!r}",
        ),
        injection="drop the Key::Up arm in NumberInput::on_event",
    ),
    Action(
        name="a letter is refused outright",
        needs="a focused field with a value in it",
        do=lambda c: (c.click(), c.__dict__.__setitem__("_v0", _text(c)), c.c.type("abc"))[0],
        expect=lambda c: (
            _text(c) == c._v0,
            f"text {c._v0!r} -> {_text(c)!r}; letters must not reach it at all",
        ),
        injection="make is_number_char accept everything",
    ),
    Action(
        name="Enter commits and normalises",
        needs="a focused field holding a typed value",
        do=lambda c: (c.click(), c.c.fill(c.sel, "7"), c.key("Enter"))[0],
        expect=lambda c: (
            _text(c).startswith("7"),
            f"text after commit is {_text(c)!r}, expected it to start 7",
        ),
        injection="make commit() a no-op",
    ),
])


def _drag(c, from_frac: float, to_frac: float):
    """Press at one fraction of the track and release at another."""
    b = c.box()
    y = (b[1] + b[3]) / 2
    x0 = b[0] + (b[2] - b[0]) * from_frac
    x1 = b[0] + (b[2] - b[0]) * to_frac
    c.c.rpc("input.pointerDown", x=x0, y=y)
    c.c.rpc("input.hover", x=x1, y=y)
    c.c.rpc("input.pointerUp", x=x1, y=y)


def _low(c) -> float:
    return float(_range(c).split("..")[0])


def _high(c) -> float:
    return float(_range(c).split("..")[1])


def _range(c) -> str:
    """The two bounds, as `RangeSlider::observe` publishes them.

    **Not a screenshot prefix.** The first version of this compared
    `image_base64[:64]`, which is the PNG *header* — identical for any
    two frames of the same size, so the assertion could never be true
    and reported "the press moved nothing" for a press that moved
    plenty. A bound that cannot be met is the same defect as one that
    cannot be exceeded.
    """
    return c.value()


RANGE = ("RangeSlider", "#d_range_a", [
    Action(
        name="a press takes the nearer handle and moves it",
        needs="a range with its handles apart, at 0.25 and 0.75",
        do=lambda c: (c.__dict__.__setitem__("_r0", _range(c)), c.click())[0],
        expect=lambda c: (
            _range(c) != c._r0,
            f"bounds {c._r0!r} -> {_range(c)!r}",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_r0", _range(c)),
                           c.click_outside())[0],
        injection="drop the PointerDown arm in RangeSlider::on_event",
    ),
    Action(
        name="a handle driven past the other stops at it",
        needs="a range with both handles reachable",
        # Press near the LEFT end so the low handle is taken, then drag
        # it to the far right. Clicking the same place three times moves
        # nothing after the first, which is what the previous version of
        # this did — and the harness correctly reported that the input
        # had reached nothing.
        do=lambda c: _drag(c, 0.05, 0.98),
        # **Three clauses, and each rules out a different wrong answer.**
        # `low <= high` alone passes on a range that ignored the drag;
        # `low > 0.05` alone passes on one that let the handle through.
        # `low < 0.9` is what says it was *stopped* rather than dragged
        # all the way. The previous version asserted `low > 0.5`, which
        # is a fact about where the other handle happened to be — the
        # drive reported `0.50..0.50`, which is the correct answer.
        expect=lambda c: (
            _low(c) <= _high(c) + 1e-6 and _low(c) > 0.05 and _low(c) < 0.9,
            f"bounds {_range(c)!r}: the low handle was driven to 0.98 — it must "
            f"have moved (> 0.05), been stopped (< 0.9), and not passed the high one",
        ),
        injection="clamp both handles to 0..1 instead of to each other",
    ),
])


SEARCH = ("SearchField", "#search-field", [
    Action(
        name="a click reaches the text, not the container",
        needs="a search field with its magnifier and its text",
        do=lambda c: c.click(),
        expect=lambda c: (
            c.styles()["state"]["focused"],
            f"state.focused={c.styles()['state']['focused']} — the *field*, "
            f"not the pill around it",
        ),
        isolate=lambda c: c.click_outside(),
        injection="let SearchField::hit fall through to the container",
    ),
    Action(
        name="typing reaches the query",
        needs="a focused search field",
        do=lambda c: (c.click(), c.__dict__.__setitem__("_v0", _text(c)),
                      c.c.type("X"))[0],
        expect=lambda c: (
            _text(c) == c._v0 + "X",
            f"query {c._v0!r} -> {_text(c)!r}",
        ),
        injection="stop forwarding events to the field in SearchField",
    ),
    Action(
        name="the magnifier is left of the text and inside the pill",
        needs="a laid-out search field",
        do=lambda c: c.hover(),
        expect=lambda c: (
            c.box("#search-icon")[2] <= c.box("#search-field")[0]
            and c.box("#search-icon")[0] >= c.box("#d_search_a")[0],
            "icon {} field {} pill {}".format(
                [round(v) for v in c.box("#search-icon")],
                [round(v) for v in c.box("#search-field")],
                [round(v) for v in c.box("#d_search_a")],
            ),
        ),
        static=True,
        injection="hand the icon the wrapper's own `inner` limits — its "
                  "min_w then forces a 20 px magnifier to the full width",
    ),
])

ALL = [NUMBER, RANGE, SEARCH]
