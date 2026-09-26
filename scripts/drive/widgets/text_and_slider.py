"""M27/T27.3 — `TextInput` and `Slider`: the two stateful widgets.

Both own state the application does not, which is what makes them the
interesting retrofit: a `Slider`'s value lives in the widget, and the
showcase reads it back through `after_update` rather than through a
change callback. `TextInput` has **no readout at all** (its catalogue
status is `none`), so the widget itself is the only thing that can
report — which is exactly the case a drive script has to handle without
quietly asserting nothing.
"""
from harness import Action


def _focused(c, sel=None):
    return c.styles(sel)["state"]["focused"]


# --- TextInput ------------------------------------------------------
def _typed(c, text):
    c.__dict__["_v0"] = c.value()
    c.click()
    c.c.type(text)


TEXTINPUT = ("TextInput", "#d_textinput_a", [
    Action(
        name="a click focuses the field",
        needs="an unfocused text field",
        do=lambda c: c.click(),
        expect=lambda c: (_focused(c), f"state.focused={_focused(c)}"),
        isolate=lambda c: c.click_outside(),
        injection="stop setting focus on PointerDown in dispatch",
    ),
    Action(
        name="typing inserts at the caret",
        needs="a focused field with a known value",
        do=lambda c: _typed(c, "XY"),
        expect=lambda c: (
            c.value() == c._v0 + "XY",
            f"value {c._v0!r} -> {c.value()!r}, expected {(c._v0 + 'XY')!r}",
        ),
        injection="drop the Event::Text arm in TextInput::on_event",
    ),
    Action(
        name="backspace removes the character before the caret",
        needs="a focused field with text in it",
        do=lambda c: (c.__dict__.__setitem__("_v0", c.value()), c.key("Backspace"))[0],
        expect=lambda c: (
            c.value() == c._v0[:-1],
            f"value {c._v0!r} -> {c.value()!r}, expected {c._v0[:-1]!r}",
        ),
        injection="drop the Backspace arm in TextInput::on_event",
    ),
    Action(
        name="fill replaces the whole value",
        needs="a field with any text in it",
        do=lambda c: (c.__dict__.__setitem__("_v0", c.value()),
                      c.c.fill(c.sel, "replaced"))[0],
        expect=lambda c: (
            c.value() == "replaced",
            f"value {c._v0!r} -> {c.value()!r}, expected 'replaced'",
        ),
        injection="make input.fill type without clearing — it then appends",
    ),
])


# --- TextArea -------------------------------------------------------
# M45/T45.7: the inventory M42 listed as owed to M45 for its paste
# (docs/plan-drive-identity.md §4). A paste is one edit that keeps its
# line breaks; Enter is how a keystroke makes one.
def _lines(c) -> int:
    return c.value().count("\n")


def _pasted(c, text):
    c.__dict__["_v0"] = c.value()
    c.__dict__["_kept"] = c.c.paste(text).get("kept")


def _pasted_outside(c, text):
    c.__dict__["_v0"] = c.value()
    c.click_outside()
    c.__dict__["_kept"] = c.c.paste(text).get("kept")


TEXTAREA = ("TextArea", "#d_textarea_a", [
    Action(
        name="a click focuses the area",
        needs="an unfocused text area",
        do=lambda c: c.click(),
        expect=lambda c: (_focused(c), f"state.focused={_focused(c)}"),
        isolate=lambda c: c.click_outside(),
        injection="stop setting focus on PointerDown in dispatch",
    ),
    Action(
        name="a paste keeps its line breaks, as one edit",
        needs="a focused area with a known value",
        prep=lambda c: c.click(),
        do=lambda c: _pasted(c, "one\r\ntwo"),
        # `kept` is what the area took: seven characters, the CRLF as one
        # newline. A paste delivered as typing drops the newline (R8-F4)
        # and one delivered whole but unnormalised keeps a '\r'.
        expect=lambda c: (
            c._kept == 7 and "one\ntwo" in c.value()
            and c.value().count("\n") == c._v0.count("\n") + 1,
            f"kept {c._kept} (want 7), lines {c._v0.count(chr(10))} -> "
            f"{c.value().count(chr(10))} (want +1), value {c.value()!r}",
        ),
        isolate=lambda c: _pasted_outside(c, "one\r\ntwo"),
        injection="make TextArea::paste drop newlines (edit::printable(text, false))",
    ),
    Action(
        name="Enter breaks a line",
        needs="a focused area",
        prep=lambda c: c.click(),
        do=lambda c: (c.__dict__.__setitem__("_v0", c.value()), c.key("Enter"))[0],
        expect=lambda c: (
            c.value().count("\n") == c._v0.count("\n") + 1,
            f"lines {c._v0.count(chr(10))} -> {c.value().count(chr(10))} (want +1)",
        ),
        injection="drop the Key::Enter arm in TextArea::on_event",
    ),
])


# --- Slider ---------------------------------------------------------
# The readout is `value: 0.00`..`1.00`.
def _val(c) -> float:
    return float(c.readout().split(":")[-1])


def _keyed(c, key):
    """Focus, *then* record, then press.

    **The order matters and the first version had it wrong.** A slider
    takes arrows only when focused, and the only way to focus one here is
    to click it — which also *sets its value to where the click landed*.
    Recording before the click measured the arrow's effect from a value
    the click had already replaced: the arrow test read 0.00 -> 0.55 and
    failed, blaming the arrow for the click's 0.50.
    """
    c.click()
    c.__dict__["_x0"] = _val(c)
    c.key(key)


# The three keyed actions below compare against a constant (1.00, 0.00),
# which is safe *because* each one is preceded by an action that moved
# the slider away from it — `needs` on each row says which. A keyed
# action asserting only "the value is 1.00" after another action already
# left it there would be the same hole as the click's was.
SLIDER = ("Slider", "#d_slider_a", [
    Action(
        name="a click sets the value to where it landed",
        needs="a slider whose value is not already at the click point",
        do=lambda c: (c.__dict__.__setitem__("_x0", _val(c)), c.click())[0],
        # **A transition, not a state.** Clicking the centre must land
        # near the middle *and the value must have moved* — the first
        # version asserted only `abs(v - 0.5) < 0.12`, which is a
        # predicate about where the slider *is*. Once a real click had put
        # it at 0.50 the isolate arm clicked outside, nothing moved, and
        # the predicate was still true: the drive reported that the
        # handler "fires on everything". An `expect` that cannot fail
        # after a no-op cannot be isolated, which is the whole point of
        # the third assertion.
        expect=lambda c: (
            _val(c) != c._x0 and abs(_val(c) - 0.5) < 0.12,
            f"value {c._x0:.2f} -> {_val(c):.2f}, expected a move to 0.50 +/- 0.12",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_x0", _val(c)), c.click_outside())[0],
        injection="drop the PointerDown arm in Slider::on_event",
    ),
    Action(
        name="End takes it to the maximum",
        needs="a focused slider below its maximum",
        do=lambda c: _keyed(c, "End"),
        expect=lambda c: (
            _val(c) == 1.0,
            f"value {c._x0:.2f} -> {_val(c):.2f}, expected 1.00",
        ),
        injection="drop the Key::End arm in Slider::on_event",
    ),
    Action(
        name="Home takes it to the minimum",
        needs="a focused slider above its minimum",
        do=lambda c: _keyed(c, "Home"),
        expect=lambda c: (
            _val(c) == 0.0,
            f"value {c._x0:.2f} -> {_val(c):.2f}, expected 0.00",
        ),
        injection="drop the Key::Home arm in Slider::on_event",
    ),
    Action(
        name="an arrow moves it by one step, not to an end",
        needs="a focused slider away from both ends",
        do=lambda c: _keyed(c, "Right"),
        # The delta, printed beside its bound. A step is small; an arrow
        # wired to an end would read a delta near 0.5 from the middle,
        # and one wired to nothing would read 0.
        expect=lambda c: (
            0.0 < _val(c) - c._x0 < 0.25,
            f"value {c._x0:.2f} -> {_val(c):.2f}, delta {_val(c) - c._x0:+.2f}, "
            f"expected a step in (0, 0.25) — an arrow wired to an end reads "
            f"{1.0 - c._x0:+.2f} from here, one wired to nothing reads +0.00",
        ),
        injection="make the arrow arm call set_value(1.0)",
    ),
])

ALL = [TEXTINPUT, TEXTAREA, SLIDER]
