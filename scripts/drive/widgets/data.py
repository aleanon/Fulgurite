"""M27/T27.10 — the data group, live."""
from harness import Action


def _page(c) -> int:
    # `"3 of 30"`.
    return int(c.value().split(" of ")[0])


def _visible(c) -> list:
    return [x for x in c.value().split(",") if x]


def _slot(c, k: int, of: int):
    b = c.box()
    each = (b[2] - b[0]) / of
    c.c.rpc("input.click", x=b[0] + each * (k + 0.5), y=(b[1] + b[3]) / 2)


PAGINATION = ("Pagination", "#d_pagination_a", [
    Action(
        name="the next arrow steps one page",
        needs="a pagination on page 1 of 30",
        do=lambda c: (c.__dict__.__setitem__("_p0", _page(c)), _slot(c, 10, 11))[0],
        expect=lambda c: (
            _page(c) == c._p0 + 1,
            f"page {c._p0} -> {_page(c)}",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_p0", _page(c)),
                           c.click_outside())[0],
        injection="drop the last-slot arm in Pagination::on_event",
    ),
    Action(
        name="pressing a number goes to that page",
        needs="a pagination whose window starts at 0",
        do=lambda c: (c.__dict__.__setitem__("_p0", _page(c)), _slot(c, 4, 11))[0],
        expect=lambda c: (
            _page(c) != c._p0 and _page(c) >= 3,
            f"page {c._p0} -> {_page(c)} after pressing the fourth number",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_p0", _page(c)),
                           c.click_outside())[0],
        injection="make every slot the previous arrow",
    ),
    Action(
        name="the keyboard jumps to the last page",
        needs="a focused pagination away from the end",
        do=lambda c: (c.click(), c.key("End"))[0],
        expect=lambda c: (
            _page(c) == 30,
            f"page {_page(c)} of 30 after End; the node is focusable and "
            f"handles Home/End/Left/Right, and until now nothing drove any "
            f"of them",
        ),
        injection="drop the Home/End arms in Pagination::on_event",
    ),
    Action(
        name="a number is the page it shows, not its position in the strip",
        needs="a pagination on its last page, so the window starts at 22",
        do=lambda c: (c.__dict__.__setitem__("_p0", _page(c)),
                      _slot(c, 1, 11))[0],
        expect=lambda c: (
            _page(c) == 22 and c._p0 != _page(c),
            f"page {c._p0} -> {_page(c)} after pressing the first *number* at the end of "
            f"30 pages: the window there is 22..30, so the slot is page 22 and "
            f"a strip that ignored the window's offset would read 1",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_p0", _page(c)),
                           c.click_outside())[0],
        injection="drop the window offset: `go(k - 1)`",
    ),
    Action(
        name="the previous arrow steps back, it does not jump",
        needs="a pagination away from the first page",
        do=lambda c: (c.__dict__.__setitem__("_p0", _page(c)), _slot(c, 0, 11))[0],
        expect=lambda c: (
            _page(c) == c._p0 - 1,
            f"page {c._p0} -> {_page(c)}; the first slot is `previous`, and a "
            f"slot that were a *page* would jump to the window's first",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_p0", _page(c)),
                           c.click_outside())[0],
        injection="make the first slot go to the window's first page",
    ),
])


TREE = ("Tree", "#d_tree_a", [
    Action(
        name="pressing a parent folds its whole subtree away",
        needs="a tree with everything expanded",
        do=lambda c: (c.__dict__.__setitem__("_n0", len(_visible(c))),
                      _row(c, 1))[0],
        expect=lambda c: (
            len(_visible(c)) == c._n0 - 2,
            f"{c._n0} -> {len(_visible(c))} rows after folding `lumen-static`, "
            f"which has two children: {_visible(c)}",
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_n0", len(_visible(c))),
                           c.click_outside())[0],
        injection="hide only the row pressed, not what is under it",
    ),
    Action(
        name="pressing a leaf reports nothing, because nothing happened",
        needs="a tree with a leaf on screen",
        do=lambda c: (c.__dict__.__setitem__("_n0", len(_visible(c))),
                      c.__dict__.__setitem__("_m", c.mark()),
                      _row(c, 4))[0],
        expect=lambda c: (
            len(_visible(c)) == c._n0 and c._handled is False,
            "{} -> {} rows, and the press was handled={}. **The count is not "
            "the assertion**: a leaf has no children, so folding its bit "
            "changes the visible count by definition either way — an "
            "injection that let a leaf toggle left the count-only version of "
            "this green, and so did a version that asked what changed, "
            "because nothing a leaf's bit does is visible. What a leaf must "
            "not do is *take the press*, and `input.click` now answers that "
            "(M27/T27.11)."
            .format(c._n0, len(_visible(c)), c._handled),
        ),
        static=True,
        injection="drop the `has_children` guard in Tree::toggle",
    ),
    Action(
        name="a hover lights the row under the pointer",
        needs="a tree nobody is pressing",
        do=lambda c: (c.__dict__.__setitem__("_m", c.mark()),
                      _hover_row(c, 3))[0],
        expect=lambda c: (
            len(_changed_since(c)) > 0,
            "the tree reported {} changed node(s) for a hover with no button "
            "held. Until T27.11 a move reached a widget only under pointer "
            "capture, so this could not happen at all and two screenshots of "
            "a hovered tree were byte-identical."
            .format(len(_changed_since(c))),
        ),
        isolate=lambda c: (c.__dict__.__setitem__("_m", c.mark()),
                           c.c.rpc("input.hover", x=4.0, y=4.0))[0],
        injection="stop synthesizing Event::Hover in dispatch_at",
    ),
    Action(
        name="the keyboard folds the lit parent",
        needs="a focused tree with its first root expanded",
        # Focus by a press on the tree's padding, which toggles no row;
        # Home lights the first root, Left folds it (M44/T44.10).
        do=lambda c: (c.__dict__.__setitem__("_n0", len(_visible(c))),
                      c.c.rpc("input.click", x=c.box()[0] + 3, y=c.box()[1] + 3),
                      c.key("Home"), c.key("Left"))[0],
        expect=lambda c: (
            len(_visible(c)) < c._n0 and _visible(c)[0] == "crates",
            f"{c._n0} -> {len(_visible(c))} rows after Home, Left: {_visible(c)}",
        ),
        injection="make Tree::key ignore Left",
    ),
    Action(
        name="the keyboard unfolds it again",
        needs="a focused tree with its first root folded",
        do=lambda c: (c.__dict__.__setitem__("_n0", len(_visible(c))), c.key("Right"))[0],
        expect=lambda c: (
            len(_visible(c)) > c._n0,
            f"{c._n0} -> {len(_visible(c))} rows after Right: {_visible(c)}",
        ),
        injection="make Tree::key ignore Right",
    ),
])


def _changed_since(c) -> list:
    """The nodes the window reported changed since `c._m`.

    Not `Ctx.changed()`, which asks since frame 0 — the whole session's
    changes, which is never empty by the time a drive reaches its second
    action. That difference is the whole assertion for "pressing a leaf
    does nothing", so it is spelled out here rather than borrowed.
    """
    return c.c.rpc("ui.lastChange", since=c._m).get("changed", [])


def _hover_row(c, k: int):
    """Put the pointer on the k-th visible row, pressing nothing."""
    b = c.box()
    c.c.rpc("input.hover", x=b[0] + 40, y=b[1] + 8 + 26 * k + 13)


def _row(c, k: int):
    """Press the k-th *visible* row, and keep whether anything took it."""
    b = c.box()
    r = c.c.rpc("input.click", x=b[0] + 40, y=b[1] + 8 + 26 * k + 13)
    c._handled = r.get("handled")
    return r


def _row_shown(c, text: str) -> bool:
    """Is `text` in the grid's *materialized* window?

    A `Repeat` builds only the rows it can show, so "is it in the tree"
    is the same question as "is it on screen" — and the negative has to
    come from the selector matching *nothing*, not from any error at
    all, or a typo would read as a scrolled-away row and the assertion
    below would pass on a window that never moved.
    """
    try:
        # **Scoped to the grid** (M24/T24.4). This was a bare `~text`,
        # because the descendant step filtered by the table's parent
        # chain and a repeater's rows are not in the table at all — so a
        # row was a descendant of nothing and `#grid > ~file-000.rs`
        # matched nothing however visible the row was. The bare form was
        # safe only while the file names were unique in the window,
        # which is a property of the fixture and not of the widget.
        c.c.rpc("ui.resolve", selector=f"{c.sel} > ~{text}")
        return True
    except RuntimeError as e:
        if "matched nothing" not in str(e):
            raise
        return False


GRID = ("DataGrid", "#d_datagrid_a", [
    Action(
        name="it shows its columns over virtualized rows",
        needs="a grid of 200 rows in a 220 px box",
        do=lambda c: c.hover(),
        expect=lambda c: (
            c.value() == "Name,Size"
            and c.box(f"{c.sel} > #grid-rows")[1] >= c.box()[1] + 27.0,
            "columns {!r}, rows start {:.0f} px below the top (the header is 28)"
            .format(c.value(), c.box(f"{c.sel} > #grid-rows")[1] - c.box()[1]),
        ),
        static=True,
        # **Not "hand the rows the offered limits"**, which was written here
        # first and reddens `a wheel scrolls the rows` instead: this action
        # asserts the columns and the rows' *vertical* offset, and the body's
        # max_h moves neither (M27/T27.11, finding 8).
        injection="place the rows at the grid's own origin, without the header offset",
    ),
    Action(
        name="a wheel scrolls the rows under the header",
        needs="more rows than the body can show",
        do=lambda c: (c.__dict__.__setitem__(
            "_w0", (_row_shown(c, "file-000.rs"), _row_shown(c, "file-012.rs"))),
            c.c.rpc("input.scroll",
                    x=(c.box()[0] + c.box()[2]) / 2,
                    y=(c.box()[1] + c.box()[3]) / 2,
                    dy=-240.0))[0],
        expect=lambda c: (
            c._w0 == (True, False)
            and (_row_shown(c, "file-000.rs"), _row_shown(c, "file-012.rs"))
            == (False, True),
            "(row 0 shown, row 12 shown) {} -> {}; a window that did not "
            "move would read the same pair twice".format(
                c._w0, (_row_shown(c, "file-000.rs"),
                        _row_shown(c, "file-012.rs"))),
        ),
        # **Not "drop the Scroll arm in Repeat"**: the catalogue picker is a
        # `Repeat` too, so that injection stops the drive from ever reaching
        # this widget — it reddens, but never here (finding 8).
        isolate=lambda c: (c.__dict__.__setitem__(
            "_w0", (_row_shown(c, "file-000.rs"), _row_shown(c, "file-012.rs"))),
            c.c.rpc("input.scroll", x=4.0, y=4.0, dy=-240.0))[0],
        injection="make DataGrid::hit answer with its own node instead of the rows",
    ),
    Action(
        name="End from a focused row brings the last row into view",
        needs="a grid of 200 rows scrolled near the top, its rows focused by a press",
        do=lambda c: (c.__dict__.__setitem__("_w0", _row_shown(c, "file-199.rs")),
                      c.click(f"{c.sel} > #grid-rows"), c.key("End"))[0],
        expect=lambda c: (
            c._w0 is False and _row_shown(c, "file-199.rs"),
            "the last row shown: {} -> {} after End; the grid's rows are a Tab stop "
            "with an active row, and End moves it to the last and scrolls it in "
            "(M44/T44.10)".format(c._w0, _row_shown(c, "file-199.rs")),
        ),
        injection="leave the grid's rows unfocusable",
    ),
])

ALL = [PAGINATION, TREE, GRID]
