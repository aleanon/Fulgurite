"""M27/T27.11 — the four pickers, live.

Every one is a trigger and a popup of cells, so three of each
inventory's rows are the same three claims: it opens, a cell changes the
value and closes it, and the same press outside does neither. What
differs is the fourth — a month pages, a clock has two lists, a file
list has one column — and that is where each kind's own arithmetic is.
"""
from harness import Action


def _pad(c):
    return c.box(f"{c.sel} > #picker-pad")


def _open(c):
    """Open the pad if it is not already, and answer its box."""
    if not _is_open(c):
        c.click()
    return _pad(c)


def _is_open(c) -> bool:
    try:
        b = c.box(f"{c.sel} > #picker-pad")
        return b[3] - b[1] > 1.0
    except Exception:
        return False


def _cell(c, k: int, cols: int, cw: float, head: float = 0.0):
    """Press cell `k`, from the pad's own box and the kind's geometry."""
    b = _open(c)
    x = b[0] + 8 + cw * (k % cols + 0.5)
    y = b[1] + 8 + head + 26.0 * (k // cols + 0.5)
    return c.c.rpc("input.click", x=x, y=y)


def _shared(name, sel, cols, cw, cell, want, head=0.0):
    """The three rows every picker has."""
    return [
        Action(
            name="a press opens the popup",
            needs="a closed picker",
            do=lambda c: (c.key("Escape"), c.click())[0],
            expect=lambda c: (
                _is_open(c),
                "the pad is {:.0f}x{:.0f} px".format(
                    _pad(c)[2] - _pad(c)[0], _pad(c)[3] - _pad(c)[1])
                if _is_open(c) else "the pad has no box",
            ),
            isolate=lambda c: (c.key("Escape"), c.click_outside())[0],
            injection="make Picker::on_event ignore the press on its trigger",
        ),
        Action(
            name="a cell sets the value and closes the popup",
            needs="an open picker",
            do=lambda c: (c.__dict__.__setitem__("_v0", c.value()),
                          _cell(c, cell, cols, cw, head))[0],
            expect=lambda c: (
                # **`_v0 != want` is half the assertion.** Without it the
                # isolate passes: after the real press the value already
                # *is* `want`, so "it equals want" stays true however
                # little the press outside did.
                c.value() == want and c._v0 != want and not _is_open(c),
                f"value {c._v0!r} -> {c.value()!r} (wanted {want!r}), "
                f"open={_is_open(c)}",
            ),
            isolate=lambda c: (c.__dict__.__setitem__("_v0", c.value()),
                               c.click_outside())[0],
            injection="make Pad::choose leave `picked` clear, so the picker never takes it",
        ),
        Action(
            name="a press outside closes it and changes nothing",
            needs="an open picker",
            do=lambda c: (_open(c), c.__dict__.__setitem__("_v0", c.value()),
                          c.click_outside())[0],
            expect=lambda c: (
                c.value() == c._v0 and not _is_open(c),
                f"value {c._v0!r} -> {c.value()!r}, open={_is_open(c)}",
            ),
            static=True,
            injection="let dispatch_overlaid route an outside press into the pad",
        ),
    ]


COLOR = ("ColorPicker", "#d_colorpicker_a", _shared(
    "ColorPicker", "#d_colorpicker_a", 4, 32.0, 5, "#dc2626"))

DATE = ("DatePicker", "#d_datepicker_a", _shared(
    "DatePicker", "#d_datepicker_a", 7, 30.0, 8, "2026-09-08", head=48.0) + [
    # M57/B17: paging changed the *chosen* date -- this action asserted
    # exactly that ("the month moved"), which is how the defect passed.
    # Paging moves the month on show; only a day press commits.
    Action(
        name="the arrows page the month and leave the chosen date",
        needs="an open date picker",
        do=lambda c: (c.__dict__.__setitem__("_v0", c.value()),
                      _page(c, forward=True))[0],
        expect=lambda c: (
            c.value() == c._v0 and _is_open(c),
            f"{c._v0} -> {c.value()}: paging must not change the date",
        ),
        injection="make shift_month move the chosen year and month",
    ),
    Action(
        name="a day picked after paging commits the paged month",
        needs="an open date picker",
        do=lambda c: (c.__dict__.__setitem__("_v0", c.value()),
                      _page(c, forward=True),
                      _cell(c, 10, 7, 30.0, 48.0))[0],
        expect=lambda c: (
            c.value()[5:7] != c._v0[5:7],
            f"{c._v0} -> {c.value()}: the picked day is in the month on show",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_v0", c.value()),
                           c.click_outside())[0],
        # The pad closes on the pick, and its body is audited at 0x0.
        expects_lint=("W1001",),
        injection="commit the day in the chosen month rather than the shown one",
    ),
])

# M57/B18: the clock's commit is the *minute*. The shared action pressed
# hour 14 and expected the popup closed -- which asserted the defect, that
# picking the hour closed the clock before a minute could be chosen.
TIME = ("TimePicker", "#d_timepicker_a", _shared(
    "TimePicker", "#d_timepicker_a", 6, 34.0, 27, "09:15") + [
    Action(
        name="an hour sets the hour and keeps the clock open",
        needs="an open time picker on a minute other than the hour's",
        do=lambda c: (c.__dict__.__setitem__("_v0", c.value()),
                      _cell(c, 14, 6, 34.0))[0],
        expect=lambda c: (
            c.value()[:2] == "14" and c._v0[:2] != "14"
            and c.value()[3:] == c._v0[3:] and _is_open(c),
            f"{c._v0} -> {c.value()}, open={_is_open(c)}: cell 14 is hour 14, "
            f"the minute is untouched and the clock stays open for it",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_v0", c.value()),
                           c.click_outside())[0],
        injection="set `picked` on an hour press, so the picker closes on it",
    ),
])

FILE = ("FilePicker", "#d_filepicker_a", _shared(
    "FilePicker", "#d_filepicker_a", 1, 168.0, 3, "lib.rs"))

ALL = [COLOR, DATE, TIME, FILE]


def _page(c, forward: bool):
    """Press the month header's arrow."""
    b = _open(c)
    x = b[2] - 14.0 if forward else b[0] + 14.0
    return c.c.rpc("input.click", x=x, y=b[1] + 8 + 14.0)
