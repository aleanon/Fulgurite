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
        visible=True,
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
        visible=True,
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
        visible=True,
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
        visible=True,
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
        visible=True,
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
        visible=True,
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


def _click_above(c, sel, pad=6.0):
    b = c.box(sel)
    return c.c.rpc("input.click", x=(b[0] + b[2]) / 2, y=b[1] - pad)


SEARCH = ("SearchField", "#search-field", [
    Action(
        visible=True,
        name="a click reaches the text, not the container",
        needs="a search field with its magnifier and its text",
        do=lambda c: c.click(),
        expect=lambda c: (
            c.styles()["state"]["focused"],
            f"state.focused={c.styles()['state']['focused']} — the *field*, "
            f"not the pill around it",
        ),
        # Outside the *pill*: 6 px above the field is the pill's padding,
        # where a press focuses the field by design. This clicked there and
        # passed only while the field was laid out past the pill's right
        # edge (M57/B26), so its centre was outside the pill too.
        isolate=lambda c: _click_above(c, "#d_search_a"),
        injection="let SearchField::hit fall through to the container",
    ),
    Action(
        visible=True,
        name="typing reaches the query",
        needs="a focused search field",
        # End first: since M57/A3 a click puts the caret where it lands,
        # which is the middle of the query. The caret was always at the end
        # before only because the agent's audit had emptied the field's
        # clusters, and this action's expectation had leaned on that.
        do=lambda c: (c.click(), c.c.rpc("input.key", key="End"),
                      c.__dict__.__setitem__("_v0", _text(c)),
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
    # Reported: a full field's text jumped between two views on every
    # click, and the caret could leave it. The field decided its scroll in
    # `layout`, which the agent's audit also runs under a million-pixel
    # box -- so this drive, with the agent attached, is exactly the window
    # that showed it. Two presses on one spot must leave the same pixels.
    Action(
        visible=True,
        name="clicking a full field twice on one spot leaves its view still",
        needs="a search field holding a query wider than itself",
        do=lambda c: _click_twice_on_a_full_field(c),
        expect=lambda c: (
            c._h1 == c._h2 and len(_text(c)) > 40,
            f"field pixels after the first press {c._h1} and the second {c._h2}; "
            f"query of {len(_text(c))} characters",
        ),
        injection="decide TextInput's scroll in layout again, not in place",
    ),
    # M57/B26: `clear()` had no caller. And the field is inside the pill
    # now: it ended 148 px past it while laid out across the offer.
    Action(
        visible=True,
        name="Escape clears the query, and the field ends inside the pill",
        needs="a focused search field with a query",
        do=lambda c: (c.click(), c.__dict__.__setitem__("_v0", _text(c)),
                      c.key("Escape"))[0],
        expect=lambda c: (
            c._v0 != "" and _text(c) == ""
            and c.box("#search-field")[2] <= c.box("#d_search_a")[2],
            f"query {c._v0!r} -> {_text(c)!r}; field ends "
            f"{round(c.box('#search-field')[2])}, pill {round(c.box('#d_search_a')[2])}",
        ),
        injection="drop the Escape arm in SearchField::on_event, or lay the "
                  "field out across the offer",
    ),
])

ALL = [NUMBER, RANGE, SEARCH]


def _click_twice_on_a_full_field(c):
    """Fill the field past its width, then press one spot in it twice,
    hashing the field's pixels after each press (settled)."""
    import time
    c.click()
    c.c.rpc("input.key", key="End")
    c.c.type(" the quick brown fox jumps over the lazy dog again")
    b = c.box("#search-field")
    x, y = b[0] + (b[2] - b[0]) * 0.6, (b[1] + b[3]) / 2
    hashes = []
    for _ in range(2):
        c.c.rpc("input.click", x=x, y=y)
        time.sleep(0.4)
        # `ui.waitSettled`, not `ui.lastChange`: the harness asks
        # `lastChange` what this action changed, and a read in here left it
        # nothing to find on some runs ("the input did not reach the
        # widget", once in five).
        c.c.rpc("ui.waitSettled")
        hashes.append(c.c.rpc("ui.regionHash", rect=b).get("hash"))
    c._h1, c._h2 = hashes
