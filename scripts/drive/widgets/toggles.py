"""M27/T27.3 — the three toggles and the button: action inventories.

Written as the retrofit the plan asks for: these four widgets shipped in
M23 tranche 1, were tested pass by pass, and have never been checked for
damage bounds, for lint cleanliness after an interaction, or for whether
the same input *outside* them does nothing.

`docs/plan-widgets-gallery.md` §3b is the reason to expect something
here. Five defects survived two rounds of tests on exactly these
widgets, and every one of those tests asked "did it paint".
"""
from harness import Action


def _state(ctx, key, sel=None):
    return ctx.styles(sel)["state"][key]


def _bg(ctx, sel=None):
    return tuple(ctx.styles(sel)["state"]["painted_background"])


# --- Button ---------------------------------------------------------
# Three buttons wired to one counter; the readout is the count.
def _count(ctx) -> int:
    r = ctx.readout()
    return int("".join(ch for ch in r if ch.isdigit()) or -1)


BUTTON = ("Button", "#d_button_a", [
    Action(
        # A button's click leaves its box as it was (the press shade is
        # transient); the consequence is the counter in the readout (D1).
        visible=False,
        name="a click reaches the application",
        needs="a button wired to the press counter",
        do=lambda c: (c.__dict__.setdefault("_n", _count(c)), c.click())[0],
        expect=lambda c: (
            _count(c) == c._n + 1,
            f"count {c._n} -> {_count(c)}, expected {c._n + 1}",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_n", _count(c)), c.click_outside())[0],
        injection="drop the PointerUp arm in Button::on_event",
    ),
    Action(
        visible=True,
        name="a held press shows on the button",
        needs="a button with a background to dim",
        do=lambda c: c.c.rpc("input.pointerDown", selector=c.sel),
        expect=lambda c: (
            _state(c, "pressed"),
            f"state.pressed={_state(c, 'pressed')} while held",
        ),
        injection="stop recording `pressed` in dispatch",
    ),
    Action(
        visible=True,
        name="releasing clears the press",
        needs="a button currently held",
        do=lambda c: c.c.rpc("input.pointerUp", selector=c.sel),
        expect=lambda c: (
            not _state(c, "pressed"),
            f"state.pressed={_state(c, 'pressed')} after release",
        ),
        injection="never clear `pressed` on PointerUp",
    ),
    Action(
        # The consequence is the hover itself (D1): no pre-hover.
        visible=True,
        pre_hover=False,
        # Off the button first: the action before left the pointer on it.
        setup=lambda c: c.c.rpc("input.hover", x=c.box()[0] + (c.box()[2]-c.box()[0])/2, y=c.box()[1] - 8),
        name="hovering is visible and is not a press",
        needs="a button with hover_dim, not already hovered",
        # **From off the button.** The release before this leaves the
        # pointer on the button, so hovering it again changed nothing and
        # this passed on the readout alone -- found when M42/T42.6 made
        # check (1) look at the button's own subtree (R18-F3). The pointer
        # leaves in `prep`, before the change window opens, so the hover
        # is a transition and the leave is not counted as one.
        prep=lambda c: c.c.rpc("input.hover", x=2.0, y=2.0),
        do=lambda c: c.hover(),
        expect=lambda c: (
            _state(c, "hovered") and not _state(c, "pressed"),
            f"hovered={_state(c, 'hovered')} pressed={_state(c, 'pressed')}",
        ),
        injection="stop recording `hovered` in dispatch",
    ),
])


# --- CheckBox -------------------------------------------------------
# The readout is `checked: true|false`. Asserted as a *transition*, not
# as a value: "the readout says true" passes on a checkbox that was
# already true and on one that ignores clicks.
def _flag(ctx) -> str:
    return ctx.readout().split(":")[-1].strip()


def _flip(ctx):
    ctx.__dict__["_f0"] = _flag(ctx)
    ctx.click()


def _flip_outside(ctx):
    ctx.__dict__["_f0"] = _flag(ctx)
    ctx.click_outside()


def _flipped(ctx):
    return _flag(ctx) != ctx._f0, f"readout {ctx._f0!r} -> {_flag(ctx)!r}"


CHECKBOX = ("CheckBox", "#d_checkbox_a > #labelled-control", [
    Action(
        visible=True,
        name="a click flips the bound flag",
        needs="a checkbox bound to the app's flag",
        do=_flip,
        expect=_flipped,
        isolate=_flip_outside,
        injection="drop the toggle in Toggle::on_event",
    ),
    Action(
        visible=True,
        name="a second click flips it back",
        needs="a checkbox that has just been flipped",
        do=_flip,
        expect=_flipped,
        injection="write `true` instead of `!self.on`",
    ),
    Action(
        visible=True,
        name="the second checkbox is independently addressable",
        needs="two checkboxes in the demo, with distinct author ids",
        do=lambda c: (c.__dict__.__setitem__("_f0", _flag(c)),
                      c.click("#d_checkbox_b"))[0],
        expect=_flipped,
        injection="give both PairCheck instances the same #[widget_id] again "
                  "— `ui.resolve` then refuses both (W1011)",
    ),
    # M57/B11: the words are part of the target. They were a separate
    # `Label` with no handler, so "I agree" could not be clicked.
    Action(
        visible=True,
        name="a click on its words flips it",
        needs="a checkbox whose label is part of it (Labelled)",
        do=lambda c: (c.__dict__.__setitem__("_f0", _flag(c)),
                      _click_words(c, 0))[0],
        expect=_flipped,
        isolate=lambda c: (c.__dict__.__setitem__("_f0", _flag(c)),
                           _click_words(c, 8))[0],
        injection="Labelled::hit answers None off the control",
    ),
])


def _click_words(c, past: float):
    """A click on the last letters of `#d_checkbox_a`'s words, or `past`
    px beyond the words' end — in the gap before the next pair."""
    b = c.box("#d_checkbox_a")
    return c.c.rpc("input.click", x=b[2] - 4 + past, y=(b[1] + b[3]) / 2)


# --- Switch ---------------------------------------------------------
# §3b records the knob defect: an *off* switch drew a grey track with an
# accent knob, which reads as on. The painted background alone is NOT a
# safe assertion here and the isolate arm proved it — moving the pointer
# away removes the hover shade, which changes the painted background
# without toggling anything. The readout is the honest signal.
def _track_end(c):
    """the track's last column"""
    b = c.box()
    return [b[2] - 1.5, b[1] + (b[3] - b[1]) / 2 - 2, b[2] - 0.5, b[1] + (b[3] - b[1]) / 2 + 2]


SWITCH = ("Switch", "#d_switch_a > #labelled-control", [
    Action(
        visible=True,
        name="a click flips the bound flag",
        needs="a switch bound to the app's flag",
        do=_flip,
        expect=_flipped,
        isolate=_flip_outside,
        injection="drop the toggle in Toggle::on_event",
    ),
    # **The track's own pixels**, where the knob never is (M57/D1). This
    # compared `painted_background`, which an *on* switch does not paint
    # with -- its body is `color` -- so it passed only because the click
    # left a hover shade the "before" reading lacked. D1's pre-hover took
    # that difference away and the action went red on a correct switch.
    # The knob is 20 px on a 40 px track, so the last column is track in
    # both states; a switch that only moved its knob leaves it unchanged.
    Action(
        visible=True,
        region=_track_end,
        name="the track changes colour with the state",
        needs="a switch that has just been flipped",
        do=lambda c: (c.__dict__.__setitem__("_f0", _flag(c)), c.click())[0],
        expect=lambda c: (
            _flag(c) != c._f0,
            f"flag {c._f0!r} -> {_flag(c)!r}; the track's colour is the pixel check",
        ),
        injection="paint the track from the same colour in both states",
    ),
])


# --- Radio ----------------------------------------------------------
# Three options sharing one value. Clicking the *unselected* one must
# move the selection; the readout is the shared value.
RADIO = ("Radio", "#d_radio_b > #labelled-control", [
    Action(
        visible=True,
        name="clicking the other option moves the selection",
        needs="a radio group whose second option is not selected",
        do=_flip,
        expect=_flipped,
        isolate=_flip_outside,
        injection="drop the selection write in Toggle::on_event",
    ),
    # M57/B13: the arrows walk the group *and* move the selection. The
    # group was the radio's parent -- its Labelled -- so Down did nothing.
    Action(
        visible=True,
        name="Down moves the selection to the next option",
        # Clicked first: the action before ends on a click *outside*,
        # which takes focus away -- and a Down with nothing focused is a
        # Down nobody gets.
        needs="the second option, clicked so it is selected and focused",
        do=lambda c: (c.click(), c.__dict__.__setitem__("_f0", _flag(c)), c.key("Down"))[0],
        expect=lambda c: (
            _flag(c) != c._f0 and "Blue" in _flag(c),
            f"readout {c._f0!r} -> {_flag(c)!r}",
        ),
        injection="group arrows by the parent only, or Radio::selects_on_focus false",
    ),
])

ALL = [BUTTON, CHECKBOX, SWITCH, RADIO]
