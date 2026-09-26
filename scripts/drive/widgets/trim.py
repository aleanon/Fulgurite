"""M57/B2 — the chip, live.

`Chip` advertised "a selected value, a removable token" and was `nohit`
with no handlers, so the showcase's three chips were pictures of
controls. The readout is the application's own record: which tags are
selected, and how many times a × was pressed.
"""
from harness import Action


def _selected(c) -> str:
    r = c.readout()
    return r.split("·")[0].replace("selected:", "").strip()


def _removed(c) -> int:
    return int(c.readout().rsplit(":", 1)[1].strip() or -1)


def _press_x(c):
    b = c.box("#d_chip_c")
    c.c.rpc("input.click", x=b[2] - 12, y=(b[1] + b[3]) / 2)


def _press_body(c, sel):
    b = c.box(sel)
    c.c.rpc("input.click", x=b[0] + 14, y=(b[1] + b[3]) / 2)


CHIP = ("Chip", "#d_chip_a", [
    Action(
        name="a click selects the chip",
        needs="three filter chips, none selected",
        do=lambda c: (c.__dict__.__setitem__("_s0", _selected(c)), c.click())[0],
        expect=lambda c: (
            "rust" in _selected(c).split() and "rust" not in c._s0.split(),
            f"selected {c._s0!r} -> {_selected(c)!r}",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_s0", _selected(c)), c.click_outside())[0],
        injection="Chip::handler answers None — the chip is a picture of a control",
    ),
    Action(
        name="a second click deselects it",
        needs="the chip selected by the action before",
        do=lambda c: c.click(),
        expect=lambda c: (
            "rust" not in _selected(c).split(),
            f"selected {_selected(c)!r}",
        ),
        injection="set rather than flip the tag in the showcase's handler",
    ),
    Action(
        name="the × removes and does not select",
        needs="a removable chip",
        do=lambda c: (c.__dict__.__setitem__("_r0", _removed(c)),
                      c.__dict__.__setitem__("_s0", _selected(c)), _press_x(c))[0],
        expect=lambda c: (
            _removed(c) == c._r0 + 1 and _selected(c) == c._s0,
            f"removed {c._r0} -> {_removed(c)}, selected {c._s0!r} -> {_selected(c)!r}",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_r0", _removed(c)),
                           c.__dict__.__setitem__("_s0", _selected(c)),
                           _press_body(c, "#d_chip_c"))[0],
        injection="Chip::handler always answers on_click",
    ),
])

ALL = [CHIP]
