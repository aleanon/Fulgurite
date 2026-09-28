"""M27/T27.3 — `TextInput` and `Slider`: the two stateful widgets.

Both own state the application does not, which is what makes them the
interesting retrofit: a `Slider`'s value lives in the widget, and the
showcase reads it back through `after_update` rather than through a
change callback. `TextInput` has **no readout at all** (its catalogue
status is `none`), so the widget itself is the only thing that can
report — which is exactly the case a drive script has to handle without
quietly asserting nothing.
"""
import re
import sys

from harness import Action

# M45/T45.6: the word modifier (Left/Right by word) and the jump modifier
# (Home/End to the buffer's ends) are each platform's own.
WORD = ["alt"] if sys.platform == "darwin" else ["ctrl"]
JUMP = ["meta"] if sys.platform == "darwin" else ["ctrl"]


def _focused(c, sel=None):
    return c.styles(sel)["state"]["focused"]


# --- Words, clicks and line ends (M45/T45.6) --------------------------
def _record(c):
    c.__dict__["_v0"] = c.value()


def _word_left_then(c, ch):
    """Caret to the end, one word-Left, then type `ch` where it landed."""
    _record(c)
    c.key("End", JUMP)
    c.key("Left", WORD)
    c.c.type(ch)


def _at_last_word(c, ch):
    v0, v = c._v0, c.value()
    words = list(re.finditer(r"\w+", v0))
    i = words[-1].start() if words else 0
    want = v0[:i] + ch + v0[i:]
    return v == want, f"value {v0!r} -> {v!r}, expected {want!r} (typed at the last word's start, byte {i})"


def _clicked_then(c, n, ch):
    """An `n`-click at the widget's centre, then type `ch` over it."""
    _record(c)
    c.click(clicks=n)
    c.c.type(ch)


def _click_first_line(c, n=1):
    b = c.box()
    return c.c.rpc("input.click", x=(b[0] + b[2]) / 2, y=b[1] + 12.0, clicks=n)


def _clicked_first_line_then(c, n, ch):
    """An `n`-click on the first line, then type `ch` over it: the one
    line the drive knows holds more than one word ("first line...")."""
    _record(c)
    _click_first_line(c, n)
    c.c.type(ch)


def _clicked_outside_then(c, n, ch):
    _record(c)
    b = c.box()
    c.c.rpc("input.click", x=(b[0] + b[2]) / 2, y=b[1] - 6.0, clicks=n)
    c.c.type(ch)


def _replaced_one(c, pattern, ch, what, several=False):
    """The value is the old one with exactly one `pattern` span replaced
    by `ch` — a selection typed over, not an insertion. `several` counts
    only spans of more than one word, so a line selection cannot pass by
    being a word selection on a one-word line."""
    v0, v = c._v0, c.value()
    spans = [(m.start(), m.end()) for m in re.finditer(pattern, v0)
             if not several or re.search(r"\W", m.group())]
    # **Changed, as well as matching.** A span that is already `ch`
    # "replaced by `ch`" is the value unchanged, which is what the isolate
    # arm produces — the first run passed its isolate on exactly that.
    hit = [(a, b) for a, b in spans if v0[:a] + ch + v0[b:] == v] if v != v0 else []
    return bool(hit), (f"value {v0!r} -> {v!r}: the {what} {hit[0]} replaced by {ch!r}" if hit
                       else f"value {v0!r} -> {v!r}: no {what} of {len(spans)} was replaced by {ch!r}")


def _motion_actions(kind, whole):
    """The three T45.6 actions both editors share. `whole` is what a
    triple click selects: the whole value, or the line."""
    return [
        Action(
            name="a word-Left moves the caret to the word's start",
            needs=f"a focused {kind} whose value ends in a word",
            prep=lambda c: c.click(),
            do=lambda c: _word_left_then(c, "X"),
            expect=lambda c: _at_last_word(c, "X"),
            injection=f"drop the word branch in {kind}::on_event (Left moves one grapheme)",
        ),
        Action(
            name="a double click selects the word under it",
            needs=f"a {kind} with a word under its centre",
            do=lambda c: _clicked_then(c, 2, "Z"),
            expect=lambda c: _replaced_one(c, r"\w+", "Z", "word"),
            isolate=lambda c: _clicked_outside_then(c, 2, "Z"),
            injection=f"ignore EventCtx.clicks in {kind}::on_event",
        ),
        Action(
            name=f"a triple click selects the {whole}",
            needs=f"a {kind} with several words where it is clicked",
            do=(lambda c: _clicked_then(c, 3, "T")) if whole == "whole value"
            else (lambda c: _clicked_first_line_then(c, 3, "T")),
            expect=(
                (lambda c: (c.value() == "T" != c._v0, f"value {c._v0!r} -> {c.value()!r}, expected 'T'"))
                if whole == "whole value"
                else (lambda c: _replaced_one(c, r"[^\n]+", "T", "line", several=True))
            ),
            isolate=lambda c: _clicked_outside_then(c, 3, "T"),
            injection=f"treat clicks == 3 as a double click in {kind}::on_event",
        ),
    ]


# --- TextInput ------------------------------------------------------
def _typed(c, text):
    c.__dict__["_v0"] = c.value()
    c.click()
    c.c.type(text)


TEXTINPUT = ("TextInput", "#d_textinput_a", [
    Action(
        visible=True,
        name="a click focuses the field",
        needs="an unfocused text field",
        do=lambda c: c.click(),
        expect=lambda c: (_focused(c), f"state.focused={_focused(c)}"),
        isolate=lambda c: c.click_outside(),
        injection="stop setting focus on PointerDown in dispatch",
    ),
    Action(
        visible=True,
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
        visible=True,
        name="backspace removes the character before the caret",
        needs="a focused field with text in it",
        do=lambda c: (c.__dict__.__setitem__("_v0", c.value()), c.key("Backspace"))[0],
        expect=lambda c: (
            c.value() == c._v0[:-1],
            f"value {c._v0!r} -> {c.value()!r}, expected {c._v0[:-1]!r}",
        ),
        injection="drop the Backspace arm in TextInput::on_event",
    ),
    *_motion_actions("TextInput", "whole value"),
    Action(
        visible=True,
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
    Action(
        name="Ctrl+End goes to the end of the text",
        needs="a focused area whose caret is on its first line, of several",
        # On the *first* line: the first version clicked the centre, which
        # is the last line here, where the line's end is the text's end and
        # a plain End passed too.
        prep=lambda c: _click_first_line(c),
        # Three characters, so the Home after it is not one Left away.
        do=lambda c: (_record(c), c.key("End", JUMP), c.c.type("End"))[0],
        expect=lambda c: (
            c.value() == c._v0 + "End",
            f"value {c._v0!r} -> {c.value()!r}, expected {(c._v0 + 'End')!r}",
        ),
        injection="drop the jump branch of Key::End in TextArea::on_event",
    ),
    Action(
        name="Home goes to the line's start",
        needs="a focused area whose caret ends a three-character line",
        do=lambda c: (_record(c), c.key("Home"), c.c.type("H"))[0],
        expect=lambda c: (
            c.value() == c._v0[:c._v0.rfind(chr(10)) + 1] + "H" + c._v0[c._v0.rfind(chr(10)) + 1:],
            f"value {c._v0!r} -> {c.value()!r}, expected H at byte {c._v0.rfind(chr(10)) + 1}",
        ),
        injection="drop the Key::Home arm in TextArea::on_event (Home then does nothing)",
    ),
    *_motion_actions("TextArea", "line"),
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
        visible=True,
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
        visible=True,
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
        visible=True,
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
        visible=True,
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
