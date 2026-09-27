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
    #: runner **counts and prints** those — the claim used to say it did
    #: and nothing counted anything, so four of seven actions in one
    #: inventory had a spatial target and no isolate, invisibly
    #: (M27/T27.11, finding 13).
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
    #: This action changes the widget's **own size**, so everything after
    #: it in the layout moves and the damage is legitimately larger than
    #: the widget.
    #:
    #: **The bound moves, it does not go away.** Damage is measured
    #: against the *stage* instead — opening an accordion section must
    #: still not repaint the window. A flag that switched the check off
    #: would be the bound removed, which is the shape this whole protocol
    #: exists to refuse.
    reflows: bool = False
    #: **Does the consequence show?** — M57/D1. `True`: the pixels in the
    #: widget's box (or `region`'s) must change, asserted from the frame
    #: itself (`ui.regionHash`). `False`: declared invisible there — a
    #: readout-only effect, a focus move — with the reason in a comment.
    #: A non-static action that declares neither **fails the run**, so
    #: "every action says" is an assertion, not a review question.
    #:
    #: The drives read the store and the published values, and CheckBox
    #: passed while its box never repainted (C2): the readout moved and
    #: the picture did not. This is the check that would have caught it.
    visible: bool | None = None
    #: A selector for where a `visible` consequence shows, when it is not
    #: the widget's own box (a popup, a readout of the widget's value), or
    #: `"window"` for the whole window (a toast, a tooltip).
    region: str | None = None
    #: Hover the widget before the "before" frame, so the action's own
    #: pointer move adds no shade. `False` for an action whose consequence
    #: *is* the hover (a highlight, a tooltip): pre-hovering would have
    #: done its work before the frame was taken.
    pre_hover: bool = True
    #: Run before the "before" frame (D1): the state the action starts
    #: from. An action that closed a popup and reopened it inside `do`
    #: compared an open popup with an open popup.
    setup: Callable[["Ctx"], None] | None = None
    #: Lint codes this action may legitimately introduce.
    #:
    #: **Named codes only, never "any".** Squeezing a split pane to 8%
    #: *does* truncate the text in it, and `W1005` saying so is the lint
    #: working — but a run that accepted every new finding would stop
    #: catching the ones nobody intended. Listing the code keeps the
    #: check on for all the others and puts the reason next to the
    #: action that causes it.
    expects_lint: tuple = ()


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
        itself is the only thing that can report.

        **Resolved, not matched.** This compared `wid` literally and so
        understood only a bare `#id`; the moment a selector carried a
        descendant step it reported "no node with wid
        `d_combobox_a > #chooser-list`", which is a true statement about
        a string nobody meant as one. `ui.resolve` is the thing that
        knows the selector grammar, so it does the resolving and this
        looks the answer up by index.
        """
        r = self.c.rpc("ui.resolve", selector=sel or self.sel)
        return r.get("value") or r.get("label") or ""

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


def _region_name(region) -> str:
    if callable(region):
        return getattr(region, "__doc__", None) or "the watched rect"
    return region or "the widget"


def _region_rect(ctx, region):
    """The rect a `visible` action watches, in logical px: a selector,
    `"window"`, or a callable taking the ctx and answering a rect."""
    if callable(region):
        return region(ctx)
    if region == "window":
        shot = ctx.c.rpc("ui.screenshot")
        sc = shot.get("scale", 1.0)
        return [0.0, 0.0, shot["width"] / sc, shot["height"] / sc]
    return ctx.box(region) if region else ctx.box()


def _nav_row(c: AgentClient, widget: str):
    """The bounds of the sidebar row named `widget`, or `None`.

    **Exactly the name, and not a heading.** A row's label is the
    widget's bare name (M56), so equality is exact where `~Name` is
    ambiguous (`RichText` is a prefix of `RichTextEditor`), and a group
    heading announces the `Heading` role, so a group that shared a
    widget's name could not be picked in its place.
    """
    for n in c.rpc("ui.getTree", depth=12)["nodes"]:
        if (n.get("label") or "").strip() != widget or n.get("role") == "Heading":
            continue
        b = n.get("bounds")
        if b and b[2] > b[0] and b[3] > b[1]:
            return b
    return None


def _pick(c: AgentClient, widget: str) -> None:
    """Put `widget` on the stage, by name, through the sidebar (M56).

    By **name**, not by row index — M27/T27.0 exists so this line can be
    written. `agent_drive.sh` used to compute `box[1] + ROW_H * k`, which
    is the first vacuous shape AGENT.md names: reorder the catalogue and
    row k is a different widget, silently.

    **Scrolling, because the catalogue outgrows the sidebar.**
    `Repeat::observe` announces only the rows that intersect the
    viewport — deliberately, so the agent never sees a row no click can
    land on (F4.7) — so a row off the bottom is scrolled into view, a
    real user action needing no new endpoint.
    """
    # **Settle first.** A drive leaves the window however the previous
    # widget's last action left it, and an open overlay is not a neutral
    # state: a press that misses every overlay dismisses them all and is
    # *consumed* — so a pick made while a demo's list is open would only
    # close the list. Escape dismisses the topmost; a press and a hover
    # in the corner (the sidebar's own padding, which answers nothing)
    # dismiss the rest and un-hover whatever a tip was showing for.
    c.key("Escape", True)
    c.key("Escape", False)
    c.rpc("input.click", x=2.0, y=2.0)
    c.rpc("input.hover", x=2.0, y=2.0)
    nav = c.rpc("ui.resolve", selector="#nav").get("bounds")
    if not nav:
        raise Failed("the sidebar has no bounds")
    mid = ((nav[0] + nav[2]) / 2, (nav[1] + nav[3]) / 2)
    # **Back to the top first.** The list keeps its offset, so a drive
    # that only ever scrolls *down* could reach the last widget and then
    # never the first again. A positive `dy` moves toward the top.
    for _ in range(30):
        c.rpc("input.scroll", x=mid[0], y=mid[1], dy=120.0)
    for attempt in range(40):
        box = _nav_row(c, widget)
        # Wholly inside the list's viewport: a row half under its edge
        # is clipped where the click would land.
        if box is not None and box[1] >= nav[1] and box[3] <= nav[3]:
            c.rpc("input.click", x=(box[0] + box[2]) / 2, y=(box[1] + box[3]) / 2)
            break
        if attempt == 39:
            # **Say what *was* visible**, so "never came into view" can
            # tell a list scrolled the wrong way from one covered by an
            # overlay.
            seen = [
                (n.get("label") or "").strip()
                for n in c.tree()["nodes"]
                if (n.get("label") or "").strip()
            ]
            raise Failed(
                f"{widget!r} never came into the sidebar's viewport after "
                f"{attempt + 1} scrolls; visible labels were {seen[-14:]}"
            )
        c.rpc("input.scroll", x=mid[0], y=mid[1], dy=-64.0)
    title = ""
    for n in c.tree()["nodes"]:
        if n.get("wid") == "title":
            title = n.get("value") or n.get("label") or ""
    if title != widget:
        raise Failed(f"picking {widget!r} left the page titled {title!r}")


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

    # **Refuse to drive a tree with a duplicated author id.**
    #
    # `W1011` means two nodes answer to one name, so `ui.resolve` refuses
    # both and *every* selector in the inventory below is suspect —
    # including the ones that happen to work. Carrying on would produce a
    # green run over an ambiguous tree, which is worse than a red one.
    #
    # It has fired twice now on the showcase, both times because a
    # composite was instantiated more than once (`PairCheck` in T27.3,
    # `VeilBody` in T27.7). Checked here rather than left to the
    # per-action lint delta, which only sees findings a *new* action
    # introduced and would miss one that was there from the start.
    dupes = [f for f in c.lint().get("findings", []) if f.get("code") == "W1011"]
    if dupes:
        raise Failed(
            f"{len(dupes)} duplicated author id(s) in the tree — every selector "
            f"here is ambiguous. First: {dupes[0].get('message', '')[:120]}")

    seen: set[str] = set()
    undeclared = [a.name for a in actions if not a.static and a.visible is None]
    if undeclared:
        raise Failed(
            f"{widget}: {len(undeclared)} action(s) do not declare `visible` "
            f"(M57/D1): {', '.join(undeclared)}")
    for a in actions:
        if a.setup is not None:
            a.setup(ctx)
            c.rpc("ui.lastChange")
        before_lint = len(c.lint().get("findings", []))
        # **Hover first, then the "before" frame** (D1): the action's own
        # pointer move would otherwise put a hover shade on the widget,
        # and a checkbox that never repainted its box would pass on the
        # shade alone.
        if a.visible:
            if a.pre_hover:
                try:
                    ctx.hover(ctx.sel)
                except Exception:
                    pass
                c.rpc("ui.lastChange")
            pix_rect = _region_rect(ctx, a.region)
            pix_before = c.rpc("ui.regionHash", rect=pix_rect)
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

        # (2b) and it shows (D1): the same rect's pixels, after.
        if a.visible:
            pix_after = c.rpc("ui.regionHash", rect=pix_rect)
            if pix_before.get("pixels", 0) <= 0:
                raise Failed(f"{widget}/{a.name}: the region to watch is empty {pix_rect}")
            if pix_after.get("hash") == pix_before.get("hash"):
                raise Failed(
                    f"{widget}/{a.name}: the consequence does not show — "
                    f"{pix_before.get('pixels')} px of {_region_name(a.region)} "
                    f"are unchanged (injection: {a.injection})")
            out.append(f"      pixels: {_region_name(a.region)}'s "
                       f"{pix_before.get('pixels')} px changed")

        # **Damage is the widget's own business.** Bounded against the
        # region that covers it, not against the frame's bounding box —
        # the showcase legitimately damages its readout at the same time,
        # at the other end of the window, and a union of the two is the
        # whole frame. Total damage is reported alongside so a run that
        # quietly started repainting everything is visible in the output
        # even though it is not what this bound is about.
        regions = c.rpc("ui.lastDamage", since=since).get("regions", [])
        # An action that resizes its own widget moves everything after it,
        # so the reference box is the stage rather than the widget.
        against = ctx.box("#demo") if a.reflows else box
        mine = [r for r in regions if _overlaps(r, box)]
        if mine:
            worst = max(_area(r) for r in mine)
            ratio = worst / max(_area(against), 1.0)
            total = sum(_area(r) for r in regions)
            out.append(f"      damage: {len(regions)} region(s), "
                       f"{total:.0f} px² total; the one over this widget is "
                       f"{worst:.0f} px² against "
                       f"{'the stage' if a.reflows else 'its own'} "
                       f"{_area(against):.0f} px² "
                       f"= {ratio:.2f}x (bound 4.0)")
            if ratio > 4.0:
                raise Failed(
                    f"{widget}/{a.name}: the damage region covering this widget is "
                    f"{ratio:.2f}x {'the stage' if a.reflows else 'its own box'} "
                    f"({worst:.0f} px² over {_area(against):.0f}), "
                    f"bound 4.0")

        # lint stays clean
        after = c.lint().get("findings", [])
        if len(after) > before_lint:
            new_ones = [f for f in after[before_lint:]
                        if f.get("code") not in a.expects_lint]
            if new_ones:
                raise Failed(
                    f"{widget}/{a.name}: ui.lint gained a finding: {new_ones[0]}")
            out.append(f"      lint: {len(after) - before_lint} expected "
                       f"{'/'.join(a.expects_lint)} finding(s)")

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
    bare = [a.name for a in actions if a.isolate is None and not a.static]
    out.append(
        f"  {widget} [{strategy}]: {len(actions)} actions, all driven"
        + (f"; {len(bare)} with no isolate: {', '.join(bare)}" if bare else "")
    )
    return out


# ---- M57/D3: the keyboard-only pass -----------------------------------

def _focused_index(c: AgentClient):
    for n in c.tree()["nodes"]:
        if n.get("focused"):
            return n["id"] - 1
    return None


def _focusables_in_demo(c: AgentClient) -> list:
    d = c.rpc("ui.resolve", selector="#demo")["bounds"]
    out = []
    for n in c.tree()["nodes"]:
        b = n.get("bounds")
        if not n.get("focusable") or not b or b[2] <= b[0]:
            continue
        if b[0] >= d[0] - 1 and b[2] <= d[2] + 1 and b[1] >= d[1] - 1 and b[3] <= d[3] + 1:
            out.append(n)
    return out


def _target_index(c: AgentClient, target: str):
    # AppBar's slots are buttons with no author id: the first focusable
    # inside the demo stands for them.
    if target == "appbar-first-button":
        f = _focusables_in_demo(c)
        return f[0]["id"] - 1 if f else None
    return c.rpc("ui.resolve", selector=target)["index"]


def run_keys(c: AgentClient, widget: str, target: str, actions: list, strategy: str) -> list[str]:
    """Reach `target` by Tab alone, then drive each action by keys alone."""
    out: list[str] = []
    _pick(c, widget)
    from keys import START
    for k in START.get(widget, []):
        c.rpc("input.key", key=k)
    want = _target_index(c, target)
    if want is None:
        raise Failed(f"{widget}: the keyboard target {target} resolved to nothing")
    tabs = 0
    while _focused_index(c) != want:
        if tabs >= 200:
            raise Failed(f"{widget}: 200 Tabs never reached {target} -- the keyboard cannot get there")
        c.key("Tab", True)
        c.key("Tab", False)
        tabs += 1
    out.append(f"  {widget} [{strategy}] keyboard: {target} reached in {tabs} Tab(s)")
    for a in actions:
        sel = None if target == "appbar-first-button" else target
        r0 = Ctx(c, widget, sel or "#demo").readout()
        v0 = c.rpc("ui.resolve", selector=sel).get("value") if sel else None
        for k in a.keys:
            if isinstance(k, str) and k.startswith("text:"):
                c.rpc("input.type", text=k[5:])
            elif isinstance(k, tuple):
                c.rpc("input.key", key=k[0], mods=k[1])
            else:
                c.rpc("input.key", key=k)
        c.rpc("ui.lastChange")
        kind = a.expect
        if kind == "open":
            owner = target.split(" > ")[0]
            b = c.rpc("ui.resolve", selector=f"{owner} > #picker-pad").get("bounds", [0, 0, 0, 0])
            ok, msg = b[2] > b[0], f"the pad is {b[2]-b[0]:.0f} px wide"
        elif kind == "menu":
            b = c.rpc("ui.resolve", selector="#d_menu_a > #chooser-list").get("bounds", [0, 0, 0, 0])
            ok, msg = b[2] > b[0], f"the menu list is {b[2]-b[0]:.0f} px wide"
        elif kind == "tip":
            b = c.rpc("ui.resolve", selector="#tooltip-tip").get("bounds", [0, 0, 0, 0])
            ok, msg = b[2] > b[0], f"the tip is {b[2]-b[0]:.0f} px wide on focus alone"
        elif kind == "focus-only":
            ok, msg = _focused_index(c) == want, "focus stayed on the chip"
        else:
            r1 = Ctx(c, widget, sel or "#demo").readout()
            v1 = c.rpc("ui.resolve", selector=sel).get("value") if sel else None
            ok = r1 != r0 or v1 != v0
            msg = f"readout {r0!r} -> {r1!r}, value {v0!r} -> {v1!r}"
        out.append(f"    {a.name}: {msg}")
        if not ok:
            raise Failed(f"{widget}/{a.name} (keyboard): {msg}")
    return out
