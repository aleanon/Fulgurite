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
        injection="make every slot the previous arrow",
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
        name="pressing a leaf changes nothing",
        needs="a tree with a leaf on screen",
        do=lambda c: (c.__dict__.__setitem__("_n0", len(_visible(c))),
                      _row(c, 4))[0],
        expect=lambda c: (
            len(_visible(c)) == c._n0,
            f"{c._n0} -> {len(_visible(c))} rows after pressing a leaf",
        ),
        static=True,
        injection="let a leaf toggle",
    ),
])


def _row(c, k: int):
    """Press the k-th *visible* row."""
    b = c.box()
    c.c.rpc("input.click", x=b[0] + 40, y=b[1] + 8 + 26 * k + 13)


def _row_shown(c, text: str) -> bool:
    """Is `text` in the grid's *materialized* window?

    A `Repeat` builds only the rows it can show, so "is it in the tree"
    is the same question as "is it on screen" — and the negative has to
    come from the selector matching *nothing*, not from any error at
    all, or a typo would read as a scrolled-away row and the assertion
    below would pass on a window that never moved.
    """
    try:
        # **Bare `~text`, not `#grid > ~text`.** The descendant step
        # filters by the table's parent chain, and a repeater's rows are
        # not in the table at all — so a row is a descendant of nothing
        # and `#d_datagrid_a > ~file-000.rs` matches nothing however
        # visible the row is. The file names are unique in this window,
        # which is what makes the bare form safe here (M27/T27.10).
        c.c.rpc("ui.resolve", selector=f"~{text}")
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
        injection="hand the rows the limits the grid was offered, not its own box",
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
        injection="drop the Scroll arm in Repeat::on_event",
    ),
])

ALL = [PAGINATION, TREE, GRID]
