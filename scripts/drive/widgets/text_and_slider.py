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
            visible=True,
            name="a word-Left moves the caret to the word's start",
            needs=f"a focused {kind} whose value ends in a word",
            prep=lambda c: c.click(),
            do=lambda c: _word_left_then(c, "X"),
            expect=lambda c: _at_last_word(c, "X"),
            injection=f"drop the word branch in {kind}::on_event (Left moves one grapheme)",
        ),
        Action(
            visible=True,
            name="a double click selects the word under it",
            needs=f"a {kind} with a word under its centre",
            do=lambda c: _clicked_then(c, 2, "Z"),
            expect=lambda c: _replaced_one(c, r"\w+", "Z", "word"),
            isolate=lambda c: _clicked_outside_then(c, 2, "Z"),
            injection=f"ignore EventCtx.clicks in {kind}::on_event",
        ),
        Action(
            visible=True,
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


# --- TextInput, right to left (M81/T81.6) ----------------------------
# `שלום world 123`: a right-to-left paragraph whose last run (Latin and
# digits) is left to right, so its logical end is the visual LEFT end --
# where `w`'s leading edge also is. A click there is past the line's end
# and must land at the end (`TextInput::offset_at`: the end wins a tie).
# That is the `+shaping` legs of `scripts/widget_drive.sh`, whose process
# lays text out `Complex`. Every other leg's is `Basic`, which lays the
# same string out in logical order left to right (M81/T81.0's pin), so its
# left end is the logical *start*, and the character goes there: the
# expectation follows the mode the process reports (`parity.leg_shaping`,
# from the launcher's `LUMEN_LIVE_FEATURES`), never a label set here.
RTL = "#d_textinput_rtl"


def _left_end_click(c, ch, sel=RTL, dy=0.0):
    """Press 4 px inside the field's left edge (its padding, left of the
    run), then type `ch`."""
    c.__dict__["_v0"] = c.value(sel)
    b = c.box(sel)
    r = c.c.rpc("input.click", x=b[0] + 4.0, y=(b[1] + b[3]) / 2 + dy)
    c.__dict__["_handled"] = r.get("handled")
    c.c.type(ch)


def _appended(c, ch):
    from parity import leg_shaping
    mode = leg_shaping()
    v0, v = c._v0, c.value(RTL)
    at = v.find(ch)
    want, wb, where = ((v0 + ch, len(v0.encode()), "the logical end") if mode == "Complex"
                       else (ch + v0, 0, "the start: Basic lays the run out left to right"))
    return (v == want and c.__dict__.get("_handled") is True,
            f"[{mode}] value {v0!r} -> {v!r}, expected {want!r} (the character at byte "
            f"{len(v[:at].encode()) if at >= 0 else None}, want {wb} ({where})); "
            f"press handled {c.__dict__.get('_handled')}")


def _left_end_click_outside(c, ch="!"):
    """The same press, 6 px above the field: it focuses nothing, so the
    character goes nowhere."""
    c.__dict__["_v0"] = c.value(RTL)
    b = c.box(RTL)
    r = c.c.rpc("input.click", x=b[0] + 4.0, y=b[1] - 6.0)
    c.__dict__["_handled"] = r.get("handled")
    c.c.type(ch)


TEXTINPUT_RTL = ("TextInput RTL", RTL, [
    Action(
        visible=True,
        name="a click at the visual left end then a character appends at the logical end",
        needs="the field seeded `שלום world 123`, on the stage (laid out right to left in a "
              "shaping build, in logical order left to right in any other)",
        do=lambda c: _left_end_click(c, "!"),
        expect=lambda c: _appended(c, "!"),
        isolate=_left_end_click_outside,
        # The plan's injection (offset_at measuring trailing edges) leaves
        # this green: the direction change before the end pushes a second
        # byte-18 point whose successor is at x 0 (docs/plan-shaped-paint.md,
        # T81.6). What it rests on is the tie rule.
        injection="drop the end's tie rule in TextInput::offset_at (`w`'s leading edge, "
                  "byte 9, wins at x 0: '!' lands before 'world')",
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
        visible=True,
        name="a click focuses the area",
        needs="an unfocused text area",
        do=lambda c: c.click(),
        expect=lambda c: (_focused(c), f"state.focused={_focused(c)}"),
        isolate=lambda c: c.click_outside(),
        injection="stop setting focus on PointerDown in dispatch",
    ),
    Action(
        visible=True,
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
        visible=True,
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
        visible=True,
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
        visible=True,
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

# --- Label (M80/T80.2) ------------------------------------------------
# A label takes no input; its demo's switch restyles it. The program's
# `style_by` turns the store's flag into `line_height(1.5)` on
# `#d_label_a`, so the press is a style change reaching a measured box --
# the label must come out exactly `lines x pitch` tall, `pitch` = 1.5 x its
# font size, from a height that was `lines x` the face's own spacing.
LABEL_SWITCH = "#d_label_spacing > #labelled-control"


def _label_h(c):
    b = c.box()
    return b[3] - b[1]


def _press_spacing(c):
    c.__dict__["_h0"] = _label_h(c)
    c.click(LABEL_SWITCH)


def _press_outside(c):
    c.__dict__["_h0"] = _label_h(c)
    c.click_outside()


def _spaced(c):
    """`lines x pitch` exactly, and grown by `lines x (pitch - before)`."""
    st = c.styles()
    byte, px = st.get("line_height"), st["font_size"]
    pitch = byte / 16.0 * px if byte else 0.0
    h, h0 = _label_h(c), c._h0
    lines = round(h / pitch) if pitch else 0
    before = h0 / lines if lines else 0.0
    want = lines * (pitch - before)
    ok = (byte == 24 and lines >= 2 and abs(h - lines * pitch) <= 0.5
          and h - h0 > 0.5 and abs((h - h0) - want) <= 0.5)
    return ok, (f"line_height byte {byte} (want 24); height {h0:.1f} -> {h:.1f}, "
                f"{lines} lines x pitch {pitch:.1f} = {lines * pitch:.1f} (+- 0.5); "
                f"grew {h - h0:+.1f} against lines x (pitch - before {before:.2f}) = {want:+.1f}")


def _unspaced(c):
    """Back at the face's spacing: the byte 0 and the height it had."""
    byte = c.styles().get("line_height")
    h, h0 = _label_h(c), c._h0
    ok = byte == 0 and h0 - h > 0.5 and abs(h - c._h_face) <= 0.5
    return ok, (f"line_height byte {byte} (want 0); height {h0:.1f} -> {h:.1f} "
                f"(want the face's {c._h_face:.1f} +- 0.5)")


def _press_back(c):
    c.__dict__["_h0"] = _label_h(c)
    c.click(LABEL_SWITCH)


def _remember_face(c):
    c.__dict__["_h_face"] = _label_h(c)


# --- M80/T80.3: the second switch cuts the same paragraph to one line ---
LABEL_ELLIPSIS = "#d_label_ellipsis > #labelled-control"


def _press_ellipsis(c):
    c.__dict__["_h0"] = _label_h(c)
    c.__dict__["_prose"] = c.value()
    c.click(LABEL_ELLIPSIS)


def _one_line(c):
    """One line of the height it wrapped at, and still the whole text.

    The wrapped height is a whole number of lines at the face's spacing,
    so one line is that height over the line count; and the agent still
    publishes, and `~` still matches, the **whole** paragraph: the cut is
    paint's, and a reader is not short of room (M57's D7). The selector
    names the paragraph's **last** word, which the cut drops, so only the
    whole text can match it (a selector term cannot hold a space, so `~=`
    cannot name the paragraph).
    """
    st = c.styles()
    h, h0 = _label_h(c), c._h0
    lines = round(h0 / h) if h else 0
    whole = c.value()
    tail = c._prose.split()[-1]
    hit = c.c.rpc("ui.resolve", selector="~" + tail).get("index")
    me = c.c.rpc("ui.resolve", selector=c.sel).get("index")
    ok = (st.get("overflow") == "Ellipsis" and lines >= 2 and abs(h0 - lines * h) <= 0.5
          and whole == c._prose and hit is not None and hit == me)
    return ok, (f"overflow {st.get('overflow')} (want Ellipsis); height {h0:.1f} -> {h:.1f}, "
                f"{h0:.1f} = {lines} x {h:.1f} (+- 0.5; lines >= 2); published value whole: "
                f"{whole == c._prose} ({len(whole)} of {len(c._prose)} chars); ~{tail} "
                f"(past the cut) resolves to node {hit} (the label is {me})")


def _press_wrap(c):
    c.__dict__["_h0"] = _label_h(c)
    c.click(LABEL_ELLIPSIS)


def _wrapped(c):
    """Wrapped again: the byte back to Wrap and the height it had."""
    st = c.styles()
    h, h0 = _label_h(c), c._h0
    ok = st.get("overflow") == "Wrap" and h - h0 > 0.5 and abs(h - c._h_wrap) <= 0.5
    return ok, (f"overflow {st.get('overflow')} (want Wrap); height {h0:.1f} -> {h:.1f} "
                f"(want the wrapped {c._h_wrap:.1f} +- 0.5)")


def _remember_wrap(c):
    c.__dict__["_h_wrap"] = _label_h(c)


LABEL = ("Label", "#d_label_a", [
    Action(
        visible=True,
        name="the switch sets a 1.5x line height and the paragraph is lines x pitch",
        needs="a label that wraps to several lines, at the face's spacing",
        setup=_remember_face,
        do=_press_spacing,
        expect=_spaced,
        isolate=_press_outside,
        # The label grows and the stage re-centres the demo, so the switch
        # moves too: the switch is the action's `subject`, and each damage
        # region is bounded by the box it covers -- the label's or the
        # switch's, old or new -- not by the stage.
        subject="#d_label_spacing",
        injection="Label measures with measure_weighted (the height stays lines x the "
                  "face's spacing); or record_painted does not stamp the byte",
    ),
    Action(
        visible=True,
        name="a second press restores the face's spacing",
        needs="the label at 1.5x",
        do=_press_back,
        expect=_unspaced,
        subject="#d_label_spacing",
        injection="drop `flow` from MeasureKey::of (the memo serves the spaced box)",
    ),
    Action(
        visible=True,
        name="the ellipsis switch cuts the paragraph to one line, and it still publishes the whole text",
        needs="the paragraph wrapped at the face's spacing",
        setup=_remember_wrap,
        do=_press_ellipsis,
        expect=_one_line,
        isolate=_press_outside,
        subject="#d_label_ellipsis",
        injection="Label reads every overflow as Wrap (the leaf tier's rule, on the host): "
                  "the height stays wrapped",
    ),
    Action(
        visible=True,
        name="a second press wraps the paragraph again",
        needs="the label cut to one line",
        do=_press_wrap,
        expect=_wrapped,
        subject="#d_label_ellipsis",
        injection="drop the overflow byte from MeasureKey's flow (the memo serves the one-line box)",
    ),
])

# --- Selectable (M80/T80.4) -------------------------------------------
# Read-only text a reader selects and copies. A drive's Ctrl+C reaches the
# session as an `input.key`, never the window's clipboard shortcut, so the
# copy is `app.copy`, which the window answers: the session's selection
# onto the system clipboard, then read back. `copied` and `clipboard` must
# both be the selection -- the X11 clipboard needs no external tool, so a
# clipboard that does not read back fails here, with its error printed,
# and is never a skip (D7).
SEL = "#d_selectable_a"


def _copy(c):
    r = c.c.rpc("app.copy")
    c.__dict__["_copy"] = r
    return r


def _drag_first_line(c):
    """Press at the first line's left edge, drag under capture to 70% of
    the width, release: several words of the first line."""
    b = c.box(SEL)
    y = b[1] + 10.0
    x0, x1 = b[0] + 1.0, b[0] + (b[2] - b[0]) * 0.7
    c.__dict__["_handled"] = [
        c.c.rpc("input.pointerDown", x=x0, y=y).get("handled"),
        c.c.rpc("input.hover", x=x1, y=y).get("handled"),
    ]
    c.c.rpc("input.pointerUp", x=x1, y=y)
    _copy(c)


def _drag_outside(c):
    """The same drag, above the widget: it must select nothing here, and a
    press outside also clears what was selected."""
    b = c.box(SEL)
    y = b[1] - 6.0
    x0, x1 = b[0] + 1.0, b[0] + (b[2] - b[0]) * 0.7
    c.c.rpc("input.pointerDown", x=x0, y=y)
    c.c.rpc("input.hover", x=x1, y=y)
    c.c.rpc("input.pointerUp", x=x1, y=y)
    _copy(c)


def _copied_words(c, at_least):
    r = c._copy
    copied, board, err = r.get("copied"), r.get("clipboard"), r.get("error")
    value = c.value(SEL)
    words = len(copied.split()) if copied else 0
    ok = (copied is not None and board == copied and err is None
          and copied in value and words >= at_least)
    return ok, (f"copied {copied!r} ({len(copied) if copied else 0} chars, {words} words; "
                f"want >= {at_least}, a substring of the text); clipboard {board!r} "
                f"(must equal copied); error {err!r}; handled {c.__dict__.get('_handled')}")


def _double_click_word(c):
    b = c.box(SEL)
    c.c.rpc("input.click", x=b[0] + (b[2] - b[0]) * 0.3, y=b[1] + 10.0, clicks=2)
    _copy(c)


def _one_word(c):
    r = c._copy
    copied, board, err = r.get("copied"), r.get("clipboard"), r.get("error")
    ok = (copied is not None and re.fullmatch(r"\w+", copied) is not None
          and board == copied and err is None and copied in c.value(SEL))
    return ok, (f"copied {copied!r} (want one word of the text); clipboard {board!r} "
                f"(must equal copied); error {err!r}")


def _click_outside_copy(c):
    c.click_outside()
    _copy(c)


SELECTABLE = ("Selectable", SEL, [
    Action(
        visible=True,
        name="a drag across the first line selects its words, and app.copy puts them on the clipboard",
        needs="the paragraph on the stage, wrapped to several lines",
        do=_drag_first_line,
        expect=lambda c: _copied_words(c, 3),
        # A press above the widget selects nothing, and with focus gone the
        # copy is null -- the "press just outside clears it" leg.
        isolate=_drag_outside,
        injection="remove Selectable's Node::selection override (the drag is handled, "
                  "and app.copy answers copied: null)",
    ),
    Action(
        visible=True,
        name="a double click copies one word",
        needs="the paragraph, nothing selected",
        do=_double_click_word,
        expect=_one_word,
        isolate=_click_outside_copy,
        injection="drop the double-press arm (clicks == 2) in Selectable::on_event: the "
                  "second press only moves the caret and nothing is copied",
    ),
])

ALL = [TEXTINPUT, TEXTINPUT_RTL, TEXTAREA, SLIDER, LABEL, SELECTABLE]
