"""M90/T90.4 — context menus, live.

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

ALL = [CONTEXT_MENU]
