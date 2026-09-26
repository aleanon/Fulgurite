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


def _wheel(c, dy):
    b = c.box()
    return c.c.rpc("input.scroll", x=(b[0] + b[2]) / 2, y=(b[1] + b[3]) / 2, dy=dy)


def _settle(c, ms):
    """Wait on the window's clock, not ours: the spring-back is the window
    waking on its own deadline (M57/B40), which only a real window does."""
    import time
    time.sleep(ms / 1000.0)
    c.c.rpc("ui.waitSettled", timeout_ms=1000)


# M57/B40: a wheel pull resists (half the wheel's distance), an upward
# wheel takes the pull back, and a wheel pull springs back and fires when
# the wheel goes quiet. The last is the end-to-end check of the window
# asking for a frame when a deadline comes due -- nothing did, so an idle
# window never ran the tick that ends the gesture.
def _refreshes(c) -> int:
    r = c.readout()
    return int("".join(ch for ch in r if ch.isdigit()) or -1)


PULL = ("PullToRefresh", "#d_pull_a", [
    # The live window answers slower than the gesture ends: a wheel pull
    # springs back 150 ms after the last wheel (M57/B40), so "armed" is
    # gone before a drive can read it. What a user sees is the refresh, so
    # that is what is asserted; `tests/pull_refresh.rs` holds the states.
    Action(
        name="a wheel pull past the threshold refreshes when the wheel stops",
        needs="content that has not been pulled",
        do=lambda c: (c.__dict__.__setitem__("_n", _refreshes(c)),
                      _wheel(c, 160.0), _settle(c, 400))[0],
        expect=lambda c: (
            _refreshes(c) == c._n + 1 and c.value() == "idle",
            f"refreshes {c._n} -> {_refreshes(c)}, state {c.value()!r} after a "
            f"160 px wheel and 400 ms -- it must fire once and spring back",
        ),
        injection="drop the ScrollEnd arm, or the window's wake on a deadline",
    ),
    Action(
        name="a short pull springs back without refreshing",
        needs="content that has not been pulled",
        do=lambda c: (c.__dict__.__setitem__("_n", _refreshes(c)),
                      _wheel(c, 40.0), _settle(c, 400))[0],
        expect=lambda c: (
            _refreshes(c) == c._n and c.value() == "idle",
            f"refreshes {c._n} -> {_refreshes(c)}, state {c.value()!r} after a "
            f"20 px pull",
        ),
        static=True,
        injection="release every pull as armed",
    ),
    Action(
        name="an upward wheel is not a pull",
        needs="content at rest",
        do=lambda c: (c.__dict__.__setitem__("_n", _refreshes(c)), _wheel(c, -200.0))[0],
        expect=lambda c: (
            c.value() == "idle" and _refreshes(c) == c._n,
            f"state {c.value()!r} -- an upward wheel belongs to the content",
        ),
        static=True,
        injection="take every Scroll, not just a downward one",
    ),
])

ALL = [TABS, BOTTOM, RAIL, APPBAR, PULL]
