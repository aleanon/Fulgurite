#!/usr/bin/env python3
"""M27/T27.3 — the per-widget drive harness.

The unit of work in M27 is not "a widget" but a widget and the artifacts
that make a claim about it falsifiable, because the failure mode on this
repository has never been a widget that does not work — it is a widget
that works, is tested, and is tested for the wrong thing. AGENT.md's
table of vacuous gates is thirteen rows long and every row passed.

So a drive script is an **inventory stored as data**. Coverage is an
assertion the runner makes, not something a reviewer has to notice:
`ACTIONS` is a list, and every entry must be exercised or the widget
fails.

Each action asserts three things, and the three map one-to-one onto the
three ways this repository has actually been fooled:

1. **It landed.** The input reached *this widget* — `ui.lastChange` names
   a node, or the demo's readout moved. Never merely that the RPC
   returned. (M16/T16.7: an invalidation oracle compared two identical
   no-ops because every click missed its target and the test only
   checked that the two sides agreed.)
2. **The consequence is the specific one named**, with the measured
   quantity **printed beside its bound**. "The pixels differ" is not an
   assertion. (M16/T16.2: a damage bound computed from the list's total
   height rather than a row's, so the limit was 200x the quantity it
   bounded and could not be exceeded.)
3. **The same input just outside does nothing.** A gate that refuses bad
   input must also accept good input; run backwards, a drive that only
   ever presses the right spot cannot tell a working handler from one
   that fires on everything.

And two more after *every* action, because they are free once a socket
is open and no widget test here has ever made them:

* **`ui.lint` is clean** — the ambient auditor already runs per frame.
* **Damage is bounded by the widget's own box**, ratio printed. A widget
  that repaints the window to show a hover is a defect M16 and M18 exist
  to prevent and nothing checked per widget.

Run by `scripts/widget_drive.sh`, which launches the showcase twice —
once under `Everything` and once under `Boundaries`. That is not
belt-and-braces: M26 found five defects and **four were invisible under
the default**, because a clean subtree is skipped before the wrong answer
can be observed.
"""
from __future__ import annotations

import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from agent_client import AgentClient  # noqa: E402


class Failed(Exception):
    """A drive assertion that did not hold. Carries its own message."""


@dataclass
class Action:
    """One thing a user can do to this widget.

    Written **before** the widget is built. An inventory written
    afterwards describes what was built, which is the whole reason it
    comes first.
    """

    name: str
    #: What the widget must be showing for this action to mean anything.
    #: Asserted *observed*, not assumed — a DataGrid seeded with three
    #: rows cannot be scrolled, and a drive that clicks anyway passes.
    needs: str
    do: Callable[["Ctx"], None]
    #: Returns (ok, "measured X against bound Y"). The message is printed
    #: whether it passes or fails, so a vacuous bound is visible in the
    #: output rather than only in a failure nobody triggers.
    expect: Callable[["Ctx"], tuple[bool, str]]
    #: The same input somewhere it must not work. `None` only for actions
    #: with no spatial target (a keystroke to the focused node), and the
    #: runner counts those.
    isolate: Callable[[Ctx], None] | None = None
    #: Prose: what must be broken for `expect` to go red. Checked by a
    #: human and by the adversary, not by the runner.
    injection: str = ""
    #: This action is expected to change *nothing* — it is a geometric
    #: or stylistic claim about a widget that takes no input, and the
    #: "did it land" check does not apply.
    #:
    #: **A flag, not an inference.** The runner cannot tell "this widget
    #: has no state" from "the input missed", and guessing would turn
    #: the first assertion off for every widget that happened to be
    #: quiet. An author says so, and the runner then *insists* the
    #: expectation is not merely `True`.
    static: bool = False


@dataclass
class Ctx:
    """The live window, plus what this widget's drive needs to address it."""

    c: AgentClient
    #: The showcase catalogue entry name, e.g. "Switch".
    widget: str
    #: A selector for the demo's main node.
    sel: str
    log: list[str] = field(default_factory=list)

    # -- queries -------------------------------------------------------
    def box(self, sel: str | None = None) -> list[float]:
        b = self.c.rpc("ui.resolve", selector=sel or self.sel).get("bounds")
        if not b:
            raise Failed(f"{sel or self.sel} has no bounds")
        return b

    def readout(self) -> str:
        """The showcase's live status line for the demo on the stage."""
        for n in self.c.tree()["nodes"]:
            if n.get("wid") == "readout":
                return n.get("value") or n.get("label") or ""
        return ""

    def value(self, sel: str | None = None) -> str:
        """What the node publishes as its value — a field's text, a
        label's run. Read from the tree, because not every demo has a
        readout: `TextInput`'s status function is `none`, so the widget
        itself is the only thing that can report."""
        want = (sel or self.sel).lstrip("#")
        for n in self.c.tree()["nodes"]:
            if n.get("wid") == want:
                return n.get("value") or n.get("label") or ""
        raise Failed(f"no node with wid {want!r} in the tree")

    def styles(self, sel: str | None = None) -> dict:
        return self.c.rpc("ui.getAppliedStyles", selector=sel or self.sel)

    def changed(self) -> list:
        return self.c.rpc("ui.lastChange", since=0).get("changed", [])

    def damage(self) -> list[list[float]]:
        """The damaged **region set**, not its bounding box.

        `rect` is the union of every changed node's rect, and M18's own
        doc says why that is the wrong quantity: two small changes at
        opposite corners union to the whole window. The first run of this
        harness bounded against `rect` and read 170x the widget's own box
        for a click on a button — the readout at the top and the control
        in the middle, bounding to the frame. That is AGENT.md's second
        vacuous shape, committed by the gate written to catch it.
        """
        return self.c.rpc("ui.lastDamage", since=0).get("regions", [])

    def mark(self) -> int:
        """Start a change/damage window; returns the frame to ask since."""
        return self.c.rpc("ui.lastChange").get("frame", 0)

    # -- inputs --------------------------------------------------------
    def click(self, sel: str | None = None):
        return self.c.click(sel or self.sel)

    def click_outside(self, pad: float = 6.0):
        """The same click, `pad` px past the widget's own edge.

        Above rather than beside: the stage centres its demo, so left and
        right of a FILL widget is still the widget.
        """
        b = self.box()
        return self.c.rpc("input.click", x=(b[0] + b[2]) / 2, y=b[1] - pad)

    def key(self, name: str):
        self.c.key(name, True)
        return self.c.key(name, False)

    def hover(self, sel: str | None = None):
        return self.c.rpc("input.hover", selector=sel or self.sel)


def _pick(c: AgentClient, widget: str) -> None:
    """Put `widget` on the stage, by name.

    By **name**, not by row index — M27/T27.0 exists so this line can be
    written. `agent_drive.sh` used to compute `box[1] + ROW_H * k`, which
    is the first vacuous shape AGENT.md names: reorder the catalogue and
    row k is a different widget, silently.

    **Scrolling, because the catalogue has outgrown its own menu.**
    `Repeat::observe` announces only the rows that intersect the
    viewport — deliberately, so the agent never sees a row no click can
    land on (F4.7). `docs/plan-widget-parity.md` T27.0 predicted this
    would bite when the menu could no longer show every widget and put it
    at fifty-one entries; it arrived at **twenty-one**, the moment
    T27.4's three landed. Scrolling the open menu is a real user action
    and needs no new endpoint, so it is what this does until T27.0's
    third landing gives rows an addressable scroll.
    """
    # **Settle first.** A drive leaves the window however the previous
    # widget's last action left it, and an open overlay is not a neutral
    # state: a `Tooltip`'s tip is a `Popup`, and one left showing sits in
    # the overlay layer above the menu that is about to open. The first
    # run of this walked six widgets and then reported the next fifteen
    # as "never came into the menu's viewport", which is a true statement
    # about a menu nothing could reach.
    #
    # Escape dismisses; the pointer goes to the corner so nothing is
    # hovered, which is what re-opens a tip.
    c.key("Escape", True)
    c.key("Escape", False)
    c.rpc("input.hover", x=2.0, y=2.0)
    c.click("#select-trigger")
    menu = c.rpc("ui.resolve", selector="#select-menu").get("bounds")
    if not menu:
        raise Failed("the picker did not open")
    mid = ((menu[0] + menu[2]) / 2, (menu[1] + menu[3]) / 2)
    # **Back to the top first.** The menu keeps its offset between
    # openings, so a drive that only ever scrolls *down* can reach the
    # last widget and then never the first again — which is what
    # happened the first time this scrolled at all.
    for _ in range(30):
        c.rpc("input.scroll", x=mid[0], y=mid[1], dy=120.0)
    for attempt in range(24):
        try:
            c.rpc("input.click", selector=f"~{widget}")
            break
        except RuntimeError as e:
            if "matched nothing" not in str(e):
                raise
            if attempt == 23:
                raise Failed(
                    f"{widget!r} never came into the menu's viewport after "
                    f"{attempt + 1} scrolls — the catalogue is longer than the "
                    f"menu and the row is not announced while it is off-screen"
                ) from None
            c.rpc("input.scroll", x=mid[0], y=mid[1], dy=-90.0)
    label = ""
    for n in c.tree()["nodes"]:
        if n.get("wid") == "select-trigger":
            label = n.get("label", "")
    if widget not in label:
        raise Failed(f"picking {widget!r} left the trigger reading {label!r}")


def _area(r) -> float:
    return max(0.0, (r[2] - r[0])) * max(0.0, (r[3] - r[1]))


def _overlaps(a, b) -> bool:
    return a[0] < b[2] and a[2] > b[0] and a[1] < b[3] and a[3] > b[1]


def run_widget(c: AgentClient, widget: str, sel: str, actions: list[Action],
               strategy: str) -> list[str]:
    """Drive every action and return the log. Raises `Failed` on the first
    assertion that does not hold."""
    out: list[str] = []
    _pick(c, widget)
    ctx = Ctx(c, widget, sel)
    # The demo is actually on the stage, asserted before anything is
    # clicked. Without this every assertion below could be agreeing about
    # a frame in which nothing happened.
    box = ctx.box()
    if _area(box) <= 0:
        raise Failed(f"{widget}: {sel} resolved to an empty box {box}")
    out.append(f"  {widget} [{strategy}] on stage at "
               f"{int(box[2]-box[0])}x{int(box[3]-box[1])} px")

    seen: set[str] = set()
    for a in actions:
        before_lint = len(c.lint().get("findings", []))
        since = ctx.mark()
        a.do(ctx)

        # (1) it landed -- on this widget, not on the root.
        if not a.static:
            changed = c.rpc("ui.lastChange", since=since).get("changed", [])
            readout = ctx.readout()
            if not changed and not readout:
                raise Failed(
                    f"{widget}/{a.name}: nothing changed and the readout is empty — "
                    f"the input did not reach the widget (injection: {a.injection})")

        # (2) the specific consequence, with the number beside the bound.
        ok, measured = a.expect(ctx)
        out.append(f"    {a.name}: {measured}")
        if not ok:
            raise Failed(f"{widget}/{a.name}: {measured} (injection: {a.injection})")

        # **Damage is the widget's own business.** Bounded against the
        # region that covers it, not against the frame's bounding box —
        # the showcase legitimately damages its readout at the same time,
        # at the other end of the window, and a union of the two is the
        # whole frame. Total damage is reported alongside so a run that
        # quietly started repainting everything is visible in the output
        # even though it is not what this bound is about.
        regions = c.rpc("ui.lastDamage", since=since).get("regions", [])
        mine = [r for r in regions if _overlaps(r, box)]
        if mine:
            worst = max(_area(r) for r in mine)
            ratio = worst / max(_area(box), 1.0)
            total = sum(_area(r) for r in regions)
            out.append(f"      damage: {len(regions)} region(s), "
                       f"{total:.0f} px² total; the one over this widget is "
                       f"{worst:.0f} px² against its own {_area(box):.0f} px² "
                       f"= {ratio:.2f}x (bound 4.0)")
            if ratio > 4.0:
                raise Failed(
                    f"{widget}/{a.name}: the damage region covering this widget is "
                    f"{ratio:.2f}x its own box ({worst:.0f} px² over {_area(box):.0f}), "
                    f"bound 4.0")

        # lint stays clean
        after = c.lint().get("findings", [])
        if len(after) > before_lint:
            raise Failed(f"{widget}/{a.name}: ui.lint gained a finding: {after[-1]}")

        # (3) the same input, somewhere it must not work.
        if a.isolate is not None:
            since = ctx.mark()
            a.isolate(ctx)
            ok_out, m_out = a.expect(ctx)
            if ok_out:
                raise Failed(
                    f"{widget}/{a.name}: the same input OUTSIDE the widget produced "
                    f"the same result ({m_out}) — the handler fires on everything")
            out.append(f"      isolated: outside gives {m_out}")
        seen.add(a.name)

    # Coverage, as an assertion rather than a review question.
    missing = [a.name for a in actions if a.name not in seen]
    if missing:
        raise Failed(f"{widget}: inventory rows never exercised: {missing}")
    out.append(f"  {widget} [{strategy}]: {len(actions)} actions, all driven")
    return out
