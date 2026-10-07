"""M90/T90.4 — context menus, live; M90/T90.5 — the drawn menu bar.

`ContextMenu<W>` answers `Event::ContextRequest`, which every route to a
context menu becomes (M90/T90.2, T90.3): a right click, Shift+F10 and the
Menu key on the focused control, and a reader's `ShowContextMenu` (sent
here as `input.invokeAction` `contextMenu`, since AccessKit's AT-SPI
adapter offers only `click`). The menu's rows are a `CommandList`, which
publishes its entries as a comma list; a choice reaches the application
through `Program::on_menu`, and the showcase's readout names it.

Every action starts from a known state (`setup`): the menu closed, or
opened by a right click. A right click with the menu open is a press
outside it or on it, so nothing here assumes the state another action
left.
"""
from harness import Action

ROW_H = 30.0
PAD_TOP = 6.0


def _list(c):
    return c.box(f"{c.sel} > #context-menu-list")


def _list_h(c) -> float:
    b = _list(c)
    return b[3] - b[1]


def _open_count(c) -> int:
    return c.c.rpc("app.diagnostics").get("open_overlays") or 0


def _close(c):
    """Escape until no overlay is open: a known closed state."""
    for _ in range(3):
        if _open_count(c) == 0 and _list_h(c) == 0.0:
            return
        c.key("Escape")


def _press_point(c):
    """A point inside the wrapped button, left of its centre, so the
    menu opens over the stage rather than off its edge."""
    b = c.box(f"{c.sel} > #context-menu-content")
    return (round(b[0] + 24.0), round((b[1] + b[3]) / 2))


def _right_click(c, at):
    return c.c.rpc("input.click", x=at[0], y=at[1], button="right")


def _open(c):
    """Closed, then opened by a right click near the button's bottom edge,
    so the rows hang below the wrapper: a click on a row is then outside
    it, and the secondary-button leg's replay of that click asks for
    nothing."""
    _close(c)
    b = c.box(f"{c.sel} > #context-menu-content")
    c._at = (round(b[0] + 24.0), round(b[3] - 4.0))
    _right_click(c, c._at)


def _row(c, k):
    b = _list(c)
    return (b[0] + 40.0, b[1] + PAD_TOP + ROW_H * k + ROW_H / 2.0)


def _focus_button(c):
    c.c.rpc("input.invokeAction", selector=f"{c.sel} > #context-menu-content", action="focus")


def _under_button(c):
    """Where Shift+F10 asks: the focused node's bottom-left."""
    b = c.box(f"{c.sel} > #context-menu-content")
    return (b[0], b[3])


def _at_point(c, want) -> tuple[bool, str]:
    b = _list(c)
    h = b[3] - b[1]
    ok = h > 0.0 and abs(b[0] - want[0]) <= 1.0 and abs(b[1] - want[1]) <= 1.0
    return ok, (f"list {h:.0f} px tall at ({b[0]:.0f}, {b[1]:.0f}), want its top-left at "
                f"({want[0]:.0f}, {want[1]:.0f}) (bound 1 px)")


CONTEXT_MENU = ("ContextMenu", "#d_context_a", [
    # A left click is a press, not a request: the button acts (its count is
    # the application's, not shown) and no menu opens. Static, so its
    # click is replayed by the secondary-button leg, which for this one
    # entry expects the right click to open the menu (expected.json's
    # `secondary.context`).
    Action(
        name="a left click presses the button and opens no menu",
        needs="a closed menu",
        setup=_close,
        do=lambda c: c.click(f"{c.sel} > #context-menu-content"),
        expect=lambda c: (
            _list_h(c) == 0.0 and _open_count(c) == 0,
            f"list {_list_h(c):.0f} px tall, open overlays {_open_count(c)} after a left click "
            f"(want 0, 0)",
        ),
        static=True,
        injection="deliver a left press as a context request too",
    ),
    # First, while nothing has been cut or copied: Paste is disabled.
    Action(
        name="a release on the disabled Paste runs nothing and keeps the menu open",
        needs="an open menu whose Paste is disabled (nothing cut or copied yet)",
        setup=_open,
        do=lambda c: c.c.rpc("input.click", x=_row(c, 2)[0], y=_row(c, 2)[1]),
        expect=lambda c: (
            c.readout() == "chose: nothing yet" and _list_h(c) > 0.0,
            f"readout {c.readout()!r} (want 'chose: nothing yet'), list {_list_h(c):.0f} px tall "
            f"(want open)",
        ),
        static=True,
        injection="ignore the MenuMask in CommandList::choose",
    ),
    Action(
        visible=True,
        region="window",
        name="a right click opens the menu at the press",
        needs="a closed menu",
        setup=_close,
        do=lambda c: (c.__dict__.__setitem__("_at", _press_point(c)), _right_click(c, c._at))[0],
        expect=lambda c: _at_point(c, c._at),
        # Outside the wrapper, a right press opens nothing -- and with the
        # menu open it dismisses it.
        isolate=lambda c: c.c.rpc("input.click", x=c._at[0], y=c.box()[1] - 6.0, button="right"),
        injection="ignore Anchor::At in Popup::place (the menu falls back to its parent's origin)",
    ),
    Action(
        visible=True,
        region="window",
        name="Shift+F10 on the focused button opens the menu under it",
        needs="a closed menu and its button focused",
        setup=_close,
        prep=_focus_button,
        do=lambda c: c.key("F10", ["shift"]),
        expect=lambda c: _at_point(c, _under_button(c)),
        injection="synthesise the keyboard request at (0, 0)",
    ),
    Action(
        visible=True,
        region="window",
        name="the Menu key opens it too",
        needs="a closed menu and its button focused",
        setup=_close,
        prep=_focus_button,
        do=lambda c: c.key("Menu"),
        expect=lambda c: _at_point(c, _under_button(c)),
        injection="translate nothing for the Menu key",
    ),
    Action(
        visible=True,
        region="window",
        name="a reader's contextMenu action opens it",
        needs="a closed menu",
        setup=_close,
        do=lambda c: c.c.rpc("input.invokeAction", selector=c.sel, action="contextMenu"),
        expect=lambda c: (
            _list_h(c) > 0.0,
            f"list {_list_h(c):.0f} px tall after Action::ContextMenu on the wrapper",
        ),
        injection="drop Action::ContextMenu's arm in dispatch",
    ),
    Action(
        visible=True,
        region="window",
        name="a click on Cut runs it and closes the menu",
        needs="a menu opened by a right click",
        setup=_open,
        do=lambda c: (c.__dict__.__setitem__("_r0", c.readout()),
                      c.c.rpc("input.click", x=_row(c, 0)[0], y=_row(c, 0)[1]))[0],
        expect=lambda c: (
            c.readout() == "chose: Cut (id 1)" and c._r0 != c.readout() and _list_h(c) == 0.0,
            f"readout {c._r0!r} -> {c.readout()!r} (want 'chose: Cut (id 1)'), list "
            f"{_list_h(c):.0f} px tall (want closed)",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_r0", c.readout()), c.click_outside())[0],
        expects_lint=("W1001",),
        injection="ask Node::take_menu of the ContextMenu wrapper instead of the consuming node",
    ),
    Action(
        visible=True,
        region="window",
        name="Down, Down and Enter choose Copy",
        needs="a menu opened by a right click, its list focused",
        setup=_open,
        do=lambda c: (c.__dict__.__setitem__("_r0", c.readout()),
                      c.key("Down"), c.key("Down"), c.key("Enter"))[0],
        expect=lambda c: (
            c.readout() == "chose: Copy (id 2)" and _list_h(c) == 0.0,
            f"readout {c._r0!r} -> {c.readout()!r} after Down, Down, Enter (want 'chose: Copy "
            f"(id 2)'), list {_list_h(c):.0f} px tall",
        ),
        expects_lint=("W1001",),
        injection="make CommandList::key a no-op",
    ),
    Action(
        visible=True,
        region="window",
        name="a click outside dismisses it",
        needs="a menu opened by a right click",
        setup=_open,
        do=lambda c: c.click_outside(avoid=[_list(c)]),
        expect=lambda c: (
            _list_h(c) == 0.0,
            f"list {_list_h(c):.0f} px tall after a click outside it (want closed)",
        ),
        # The same click on the menu's own padding, which is inside it,
        # keeps it open.
        isolate=lambda c: (_open(c), c.c.rpc("input.click", x=_list(c)[0] + 40.0, y=_list(c)[1] + 2.0))[0],
        expects_lint=("W1001",),
        injection="answer false from Popup::dismissed_by_outside_press",
    ),
    Action(
        visible=True,
        region="window",
        name="Escape dismisses it",
        needs="a menu opened by a right click",
        setup=_open,
        do=lambda c: c.key("Escape"),
        expect=lambda c: (
            _list_h(c) == 0.0 and _open_count(c) == 0,
            f"list {_list_h(c):.0f} px tall, open overlays {_open_count(c)} after Escape (want 0, 0)",
        ),
        expects_lint=("W1001",),
        injection="make Popup::dismiss close nothing",
    ),
])

# ---- M90/T90.5: the menu bar ------------------------------------------
#
# `MenuBar` draws the same sections the showcase installs with
# `Program::menu` (`catalog::MENU_BAR`). Each title is a `MenuItem` child
# with its cell as bounds, so `> ~=File` addresses it; the open menu is
# `#menu-bar-list`, which publishes its entries as a comma list. A choice
# by row and by accelerator both reach `Program::on_menu`, which the
# readout names. Every action starts closed or opened by a click on a
# title (`setup`); one that starts open is not pre-hovered, since the
# bar's centre is the Edit title and a hover there switches the open menu
# to Edit before the action's frame is taken.

FILE = "New,Open,Save"
EDIT = "Undo,Copy,Paste"


def _bar_list(c):
    return c.box(f"{c.sel} > #menu-bar-list")


def _bar_list_h(c) -> float:
    b = _bar_list(c)
    return b[3] - b[1]


def _bar_shows(c) -> str:
    return c.value(f"{c.sel} > #menu-bar-list") if _bar_list_h(c) > 0.0 else ""


def _title(c, name):
    return f"{c.sel} > ~={name}"


def _bar_close(c):
    """Escape until no overlay is open: a known closed state."""
    for _ in range(3):
        if _open_count(c) == 0 and _bar_list_h(c) == 0.0:
            return
        c.key("Escape")


def _bar_open(c, name="File"):
    _bar_close(c)
    c.click(_title(c, name))


def _bar_row(c, k):
    b = _bar_list(c)
    return (b[0] + 40.0, b[1] + PAD_TOP + ROW_H * k + ROW_H / 2.0)


def _hangs_below(c, name) -> tuple[bool, str]:
    """The open menu's entries and where it hangs, against a title's."""
    t, b = c.box(_title(c, name)), _bar_list(c)
    return abs(b[0] - t[0]) <= 1.0, f"list at x {b[0]:.0f}, title {name} at x {t[0]:.0f} (bound 1 px)"


MENU_BAR = ("MenuBar", "#d_menubar_a", [
    Action(
        visible=True,
        region="window",
        name="a click on a title opens its menu below it",
        needs="a closed bar",
        setup=_bar_close,
        do=lambda c: c.click(_title(c, "File")),
        expect=lambda c: (
            _bar_shows(c) == FILE and _hangs_below(c, "File")[0],
            f"menu shows {_bar_shows(c)!r} (want {FILE!r}); " + _hangs_below(c, "File")[1],
        ),
        # Closed, and the same click just above the bar opens nothing.
        isolate=lambda c: (_bar_close(c), c.click_outside())[0],
        injection="make MenuBar's PointerDown arm open nothing",
    ),
    Action(
        visible=True,
        region="window",
        name="a click on Save runs it and closes the menu",
        needs="the File menu, opened by a click on its title",
        setup=lambda c: _bar_open(c, "File"),
        pre_hover=False,
        do=lambda c: (c.__dict__.__setitem__("_r0", c.readout()),
                      c.c.rpc("input.click", x=_bar_row(c, 2)[0], y=_bar_row(c, 2)[1]))[0],
        expect=lambda c: (
            c.readout() == "chose: Save (id 13)" and c._r0 != c.readout() and _bar_list_h(c) == 0.0,
            f"readout {c._r0!r} -> {c.readout()!r} (want 'chose: Save (id 13)'), list "
            f"{_bar_list_h(c):.0f} px tall (want closed)",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_r0", c.readout()), c.click_outside())[0],
        expects_lint=("W1001",),
        injection="ask Node::take_menu of the MenuBar instead of the list that consumed the release",
    ),
    Action(
        visible=True,
        region="window",
        name="with a menu open, hovering another title switches to its menu",
        needs="the File menu open, the pointer not yet over Edit",
        setup=lambda c: _bar_open(c, "File"),
        # Not pre-hovered: the bar's centre is the Edit title, and a hover
        # there would have switched menus before the frame was taken.
        pre_hover=False,
        do=lambda c: c.hover(_title(c, "Edit")),
        expect=lambda c: (
            _bar_shows(c) == EDIT and _hangs_below(c, "Edit")[0],
            f"menu shows {_bar_shows(c)!r} after hovering Edit (want {EDIT!r}); " + _hangs_below(c, "Edit")[1],
        ),
        # File open again, and the pointer just above the bar switches nothing.
        isolate=lambda c: (_bar_open(c, "File"),
                           c.c.rpc("input.hover", x=c.box(_title(c, "Edit"))[0] + 8.0, y=c.box()[1] - 6.0))[0],
        injection="switch menus on Hover only while none is open",
    ),
    Action(
        visible=True,
        region="window",
        name="Right moves to the next title's menu",
        needs="the File menu open, its list focused",
        setup=lambda c: _bar_open(c, "File"),
        pre_hover=False,
        do=lambda c: c.key("Right"),
        expect=lambda c: (
            _bar_shows(c) == EDIT and _hangs_below(c, "Edit")[0],
            f"menu shows {_bar_shows(c)!r} after Right (want {EDIT!r}); " + _hangs_below(c, "Edit")[1],
        ),
        injection="move the roving title on Right without opening its menu",
    ),
    Action(
        visible=True,
        region="window",
        name="Escape closes it and focus is back on the bar",
        needs="the File menu, opened by a click on its title",
        setup=lambda c: _bar_open(c, "File"),
        pre_hover=False,
        do=lambda c: c.key("Escape"),
        expect=lambda c: (
            _bar_list_h(c) == 0.0 and _open_count(c) == 0 and c.styles()["state"]["focused"],
            f"list {_bar_list_h(c):.0f} px tall, open overlays {_open_count(c)} (want 0, 0); "
            f"bar focused {c.styles()['state']['focused']} (want True)",
        ),
        expects_lint=("W1001",),
        injection="drop FOCUSABLE from the bar's own node (focus has nowhere to come back to)",
    ),
    Action(
        visible=True,
        region="window",
        name="Down on the focused bar opens it, and Enter chooses New",
        needs="a closed bar with focus",
        # The menu opens and closes inside the action, below the bar, and
        # no pointer moves: net, the strip paints what it painted, and what
        # changed is the readout (`visible`'s region).
        repaints=False,
        setup=_bar_close,
        prep=lambda c: c.c.rpc("input.invokeAction", selector=c.sel, action="focus"),
        do=lambda c: (c.__dict__.__setitem__("_r0", c.readout()), c.key("Down"), c.key("Enter"))[0],
        expect=lambda c: (
            c.readout() == "chose: New (id 11)" and c._r0 != c.readout() and _bar_list_h(c) == 0.0,
            f"readout {c._r0!r} -> {c.readout()!r} after Down, Enter (want 'chose: New (id 11)'), "
            f"list {_bar_list_h(c):.0f} px tall",
        ),
        expects_lint=("W1001",),
        injection="open the menu from the keyboard without highlighting its first row",
    ),
    Action(
        visible=True,
        region="window",
        name="Ctrl+O runs Open by its accelerator, with no menu open",
        needs="a closed bar",
        # The host matches the accelerator; the bar is not involved and
        # paints nothing new. What changed is the readout.
        repaints=False,
        setup=_bar_close,
        do=lambda c: (c.__dict__.__setitem__("_r0", c.readout()), c.key("o", ["ctrl"]))[0],
        expect=lambda c: (
            c.readout() == "chose: Open (id 12)" and c._r0 != c.readout() and _bar_list_h(c) == 0.0,
            f"readout {c._r0!r} -> {c.readout()!r} after Ctrl+O (want 'chose: Open (id 12)'), list "
            f"{_bar_list_h(c):.0f} px tall (want closed)",
        ),
        # The same key without Ctrl is no accelerator.
        isolate=lambda c: (c.__dict__.__setitem__("_r0", c.readout()), c.key("o"))[0],
        injection="match no accelerator in Host::accelerator (the precedence over on_key is the headless gate's: the showcase installs no key hook)",
    ),
    # Static: nothing has run Copy, so Paste is disabled and a release on
    # it chooses nothing.
    Action(
        name="a release on the disabled Paste runs nothing and keeps the menu open",
        needs="the Edit menu open, Paste disabled (Copy not run)",
        setup=lambda c: _bar_open(c, "Edit"),
        pre_hover=False,
        do=lambda c: (c.__dict__.__setitem__("_r0", c.readout()),
                      c.c.rpc("input.click", x=_bar_row(c, 2)[0], y=_bar_row(c, 2)[1]))[0],
        expect=lambda c: (
            c.readout() == c._r0 and _bar_shows(c) == EDIT,
            f"readout {c._r0!r} -> {c.readout()!r} (want unchanged), menu shows {_bar_shows(c)!r} "
            f"(want {EDIT!r}, still open)",
        ),
        static=True,
        injection="ignore the section's slice of MenuBar::enabled",
    ),
])

ALL = [CONTEXT_MENU, MENU_BAR]
