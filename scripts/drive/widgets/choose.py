"""M27/T27.9 — the choice group, live.

All three publish their trigger's text as the node's value and their
open state as `expanded`, so a drive reads the selection rather than the
pixels. `OptionList` publishes the *visible* options as a comma list,
which is how a filter is asserted without a screenshot.
"""
from harness import Action


def _shown(c) -> list:
    """The visible options, as `OptionList::observe` publishes them.

    Addressed **through the owner** — `#d_picklist_a > #chooser-list`.
    All three choosers are one type, so all three carry the same part
    ids, and `W1011` reported every one of them as unaddressable the
    first time this ran. A descendant step (M27/T27.0's third landing)
    is the answer that scales: a part id only has to be unique within
    its owner.
    """
    v = c.value(f"{c.sel} > #chooser-list")
    return [x for x in v.split(",") if x]


def _release_row(c, k: int):
    """Press and release over the k-th visible row.

    On the **release**, because that is what chooses — a press-and-drag
    that leaves the list must not commit, and choosing on the press also
    closes the list under a finger that is still down.
    """
    b = c.box(f"{c.sel} > #chooser-list")
    y = b[1] + 6 + 30 * k + 15
    x = (b[0] + b[2]) / 2
    c.c.rpc("input.pointerDown", x=x, y=y)
    c.c.rpc("input.pointerUp", x=x, y=y)


def _is_focused(c) -> bool:
    return c.styles(f"{c.sel} > #chooser-trigger")["state"]["focused"]


def _focus_trigger(c):
    """Press and release the trigger: that focuses the field, and a
    chooser opens *on focus* rather than on the press.

    It opens on focus because the trigger is a `TextInput` and consumes
    the `PointerDown` itself — the bubbling walk stops at whatever took
    the event, so an arm on the wrapper never ran.
    """
    if _is_focused(c):
        # **Already focused: do not press again.** A press on the trigger
        # of an *open* list is a press outside every overlay, so
        # `dispatch_overlaid` dismisses them and consumes it — the field
        # loses focus and the keystrokes that follow go nowhere. That is
        # `Select`'s documented dropdown behaviour, not a defect; see
        # `tests/focus_overlay.rs`.
        return
    t = f"{c.sel} > #chooser-trigger"
    b = c.box(t)
    x, y = (b[0] + b[2]) / 2, (b[1] + b[3]) / 2
    c.c.rpc("input.pointerDown", x=x, y=y)
    c.c.rpc("input.pointerUp", x=x, y=y)


def _type_into_trigger(c, text):
    _focus_trigger(c)
    # Select-all, so the typed text replaces the option already there
    # rather than appending to it — the lesson `NumberInput` recorded in
    # T27.5: typing appends at the caret, like any field.
    for _ in range(24):
        c.key("Backspace")
    c.c.type(text)


def _clear_trigger(c):
    _focus_trigger(c)
    for _ in range(24):
        c.key("Backspace")


PICKLIST = ("PickList", "#d_picklist_a", [
    Action(
        visible=True,
        name="releasing over a row chooses it",
        needs="a closed picklist, opened by pressing its trigger",
        do=lambda c: (c.__dict__.__setitem__("_v0", c.value()),
                      _focus_trigger(c), _release_row(c, 2))[0],
        expect=lambda c: (
            c.value() != c._v0 and c.value() == "Cherry",
            f"trigger {c._v0!r} -> {c.value()!r}, expected 'Cherry' (the third row)",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_v0", c.value()),
                           c.click_outside())[0],
        injection="choose on the press instead of the release",
    ),
    Action(
        name="a picklist does not filter on its trigger",
        needs="a picklist whose trigger reads one of its options",
        do=lambda c: _focus_trigger(c),
        expect=lambda c: (
            len(_shown(c)) == 6,
            f"{len(_shown(c))} of 6 rows visible: {_shown(c)}",
        ),
        static=True,
        injection="filter on every kind, not only on a combobox",
    ),
])


COMBOBOX = ("Combobox", "#d_combobox_a", [
    Action(
        visible=True,
        name="typing filters the list",
        needs="a combobox opened by pressing its trigger",
        # **An explicit press, release, then type** — not `input.fill`.
        # `fill` focuses by clicking and then *verifies* the result, and
        # a chooser's trigger is behind a widget that consumes the
        # press: `Chooser::on_event` takes a `PointerDown` on the
        # trigger to open the list, so the focus a single synthesized
        # click leaves is not the one `fill` assumes. Pressing and
        # releasing explicitly is what a person does, and is what the
        # row-choosing action above already does for the same reason.
        do=lambda c: (c.__dict__.__setitem__("_n0", len(_shown(c))),
                      _type_into_trigger(c, "ap"))[0],
        expect=lambda c: (
            len(_shown(c)) == 2 and set(_shown(c)) == {"Apple", "apricot"},
            f"{c._n0} -> {len(_shown(c))} rows for 'ap': {_shown(c)} — "
            f"the match is case-insensitive and anywhere in the option",
        ),
        injection="make the filter case-sensitive",
    ),
    # The action the inventory never had: filter, *then* press a row.
    # Pressing the list blurred the field, the field's abandoned-query
    # revert cleared the filter, and the release chose by position in the
    # whole list -- "ap" then its second row chose Banana. Reported from
    # the live window; every action here filtered or chose, never both.
    Action(
        visible=True,
        name="a press on a filtered row chooses that row",
        needs="a combobox filtered to Apple, apricot",
        do=lambda c: _menu_row(c, 1),
        expect=lambda c: (
            c.value(f"{c.sel} > #chooser-trigger") == "apricot",
            f"the second of Apple, apricot chose "
            f"{c.value(f'{c.sel} > #chooser-trigger')!r} (wanted 'apricot'; "
            f"'Banana' is the unfiltered list's second row)",
        ),
        injection="leave NodeDesc::KEEPS_FOCUS off the chooser's list",
    ),
    # Reported: after a choice the field's caret followed the pointer, a
    # press on the field did nothing, and only a click outside reset it.
    # The press on the list had given the chooser the pointer capture and
    # the release never ended it; and with the field still focused, the
    # list that opens on focus had no focus transition left to open on.
    Action(
        visible=True,
        region="window",
        name="after a choice a press on the field opens the list again",
        needs="a combobox that has just chosen, its field still focused",
        do=lambda c: c.click(f"{c.sel} > #chooser-trigger"),
        expect=lambda c: (
            _list_h(c) > 0.0 and len(_shown(c)) > 0,
            f"the list is {_list_h(c):.0f} px tall with {len(_shown(c))} rows after a "
            f"press on the field that just chose",
        ),
        isolate=lambda c: c.click_outside(),
        injection="drop the pressed-edge reopen in Chooser::layout",
    ),
    Action(
        visible=True,
        name="clearing the query shows everything again",
        needs="a combobox with a query in it",
        do=lambda c: _clear_trigger(c),
        expect=lambda c: (
            len(_shown(c)) == 6,
            f"{len(_shown(c))} of 6 rows after clearing — an empty query must "
            f"show everything, or the control dies the moment anyone deletes it",
        ),
        injection="treat an empty needle as matching nothing",
    ),
])


MENU = ("Menu", "#d_menu_a", [
    Action(
        name="it shows its commands with no trigger",
        needs="an open menu of four commands",
        do=lambda c: c.hover(f"{c.sel} > #chooser-list"),
        expect=lambda c: (
            len(_shown(c)) == 4
            and (c.box(f"{c.sel} > #chooser-trigger")[3]
                 - c.box(f"{c.sel} > #chooser-trigger")[1]) == 0.0,
            "{} commands, trigger {:.0f} px tall".format(
                len(_shown(c)),
                c.box(f"{c.sel} > #chooser-trigger")[3]
                - c.box(f"{c.sel} > #chooser-trigger")[1],
            ),
        ),
        static=True,
        injection="lay out a trigger for a menu too",
    ),
    # M57/B23: a menu runs a command. It stored the choice as a selection,
    # so "Copy" became the menu's value and no action ran.
    Action(
        name="choosing a command runs it and closes the menu",
        needs="an open menu",
        do=lambda c: _menu_row(c, 1),
        # Closed is a list with no box: a hidden list still publishes its
        # options (with the hidden flag), so they are not what to count.
        expect=lambda c: (
            c.readout() == "ran: Copy" and _list_h(c) == 0.0,
            f"readout {c.readout()!r}, list {_list_h(c):.0f} px tall",
        ),
        static=True,
        # The closed list's body is audited at 0x0, as every closed popup's is.
        expects_lint=("W1001",),
        injection="store a menu's choice as a selection",
    ),
    Action(
        name="its button opens it again",
        needs="a closed menu",
        do=lambda c: c.click("#d_menu_open"),
        expect=lambda c: (
            len(_shown(c)) == 4 and _list_h(c) > 0.0,
            f"{len(_shown(c))} commands, list {_list_h(c):.0f} px tall after pressing Commands",
        ),
        static=True,
        injection="drop the showcase's reopen",
    ),
])


def _list_h(c) -> float:
    b = c.box(f"{c.sel} > #chooser-list")
    return b[3] - b[1]


def _menu_row(c, k):
    b = c.box(f"{c.sel} > #chooser-list")
    c.c.rpc("input.click", x=b[0] + 30.0, y=b[1] + 6.0 + 30.0 * k + 15.0)

ALL = [PICKLIST, COMBOBOX, MENU]
