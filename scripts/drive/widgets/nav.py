"""M27/T27.6 — the navigation group, live.

The plan grouped these as one shape; three of the five are, and the
inventories say so by shape rather than by group: the strips share their
actions, `AppBar` has none (nothing in it is selected), and
`PullToRefresh` has a gesture.
"""
from harness import Action


def _sel(c) -> str:
    """The selected item's label, as `Strip::observe` publishes it."""
    return c.value()


def _press_frac(c, frac: float, vertical: bool = False):
    b = c.box()
    if vertical:
        x = (b[0] + b[2]) / 2
        y = b[1] + (b[3] - b[1]) * frac
    else:
        x = b[0] + (b[2] - b[0]) * frac
        y = (b[1] + b[3]) / 2
    c.c.rpc("input.click", x=x, y=y)


def _strip(name, sel, vertical=False):
    return (name, sel, [
        Action(
            name="a press selects the item under it",
            needs="a four-item strip with the first selected",
            do=lambda c: (c.__dict__.__setitem__("_s0", _sel(c)),
                          _press_frac(c, 0.62, vertical))[0],
            expect=lambda c: (
                _sel(c) != c._s0,
                f"selection {c._s0!r} -> {_sel(c)!r} after a press 62% along",
            ),
            isolate=lambda c: (c.__dict__.__setitem__("_s0", _sel(c)),
                               c.click_outside())[0],
            injection=("read the press down the wrong axis — a rail that used "
                       "`local.x` lands on item 0 every time"),
        ),
        Action(
            name="a press on the item already selected changes nothing but is taken",
            needs="a strip with a known selection",
            do=lambda c: (c.__dict__.__setitem__("_s0", _sel(c)),
                          _press_frac(c, 0.62, vertical))[0],
            expect=lambda c: (
                _sel(c) == c._s0,
                f"selection stayed {_sel(c)!r}",
            ),
            static=True,
            injection="make select() move the index on every press",
        ),
    ])


TABS = _strip("Tabs", "#d_tabs_a")
BOTTOM = _strip("BottomNav", "#d_bottomnav_a")
RAIL = _strip("NavigationRail", "#d_rail_a", vertical=True)


APPBAR = ("AppBar", "#d_appbar_a", [
    Action(
        name="it shows its title and does not grow with it",
        needs="an app bar with a title",
        do=lambda c: c.hover(),
        expect=lambda c: (
            c.value() != "" and abs((c.box()[3] - c.box()[1]) - 56.0) < 1.0,
            "title {!r}, height {:.0f} px (its style's 56, never its title's)".format(
                c.value(), c.box()[3] - c.box()[1]
            ),
        ),
        static=True,
        injection="make AppBar::layout measure its title",
    ),
])


PULL = ("PullToRefresh", "#d_pull_a", [
    Action(
        name="a downward drag pulls the content",
        needs="content that has not been pulled",
        do=lambda c: (c.__dict__.__setitem__("_p0", c.value()),
                      c.c.rpc("input.scroll",
                              x=(c.box()[0] + c.box()[2]) / 2,
                              y=(c.box()[1] + c.box()[3]) / 2,
                              dy=80.0))[0],
        expect=lambda c: (
            c.value() == "armed",
            f"state {c._p0!r} -> {c.value()!r} after an 80 px pull "
            f"(the threshold is 64)",
        ),
        injection="drop the Scroll arm in PullToRefresh::on_event",
    ),
    Action(
        name="an upward wheel is not a pull",
        needs="a pulled-down surface",
        do=lambda c: c.c.rpc("input.scroll",
                             x=(c.box()[0] + c.box()[2]) / 2,
                             y=(c.box()[1] + c.box()[3]) / 2,
                             dy=-200.0),
        expect=lambda c: (
            c.value() == "armed",
            f"state {c.value()!r} — an upward wheel belongs to the content, "
            f"or a list inside one could never scroll back",
        ),
        static=True,
        injection="take every Scroll, not just a downward one",
    ),
])

ALL = [TABS, BOTTOM, RAIL, APPBAR, PULL]
