#!/usr/bin/env python3
"""M54/T54.0: walk the platform's accessibility bus (AT-SPI) for one
launched window and compare it with what the agent sees.

A unit test of the tree we *would* send proves the builder; only a walk
of the bus proves the window sent it (`docs/plan-a11y-platform.md` §0
rule 1). Run by `scripts/a11y_drive.sh` through `scripts/live/launch.py`,
which puts the window's pid in FULGURITE_LIVE_PID.

    a11y_drive.py PORT --expect present|absent [--button NAME] [--timeout S]

The application is found **by pid**, not by name: AT-SPI names an
application after its executable, and a second copy of the same binary
(another worktree's) would otherwise answer for this one.

`present`: the application is on the bus, its push-button count equals
the agent's visible `Button` count (both printed), the count is not zero,
and a push button named NAME exists when `--button` is given.
`absent`: the expected-to-fail leg (T54.0 until T54.2) -- it passes while
the application is missing, printing XFAIL, and fails as XPASS the moment
it appears, so the expectation cannot be left stale.

`--open SEL` (M89/T89.3 gate 6): click SEL in the primary window first,
which opens a second window, then require **one frame per open window**
on the bus for the pid (`app.windows`), each frame's push-button count
equal to its own window's agent `Button` count (the agent's `"window"`
member), and none of them zero -- each window has its own adapter.

Exit 77, with the reason, when there is no AT-SPI to walk: no `gi`
bindings, or a desktop with no applications on it.
"""
from __future__ import annotations

import argparse
import json
import os
import socket
import sys
import time

T0 = time.monotonic()


def skip(why: str):
    print(f"a11y-drive: skip -- {why}", flush=True)
    raise SystemExit(77)


def fail(why: str):
    print(f"a11y-drive: FAIL -- {why}", file=sys.stderr, flush=True)
    raise SystemExit(1)


try:
    import gi  # noqa: E402

    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi  # noqa: E402
except (ImportError, ValueError) as e:  # pragma: no cover - environment
    skip(f"no AT-SPI bindings (python3-gi with Atspi 2.0): {e}")


def rpc(port: int, method: str, params: dict | None = None, window: int | None = None) -> dict:
    with socket.create_connection(("127.0.0.1", port), timeout=15) as s:
        f = s.makefile("rwb")
        req = {"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}
        if window is not None:
            req["window"] = window
        f.write((json.dumps(req) + "\n").encode())
        f.flush()
        r = json.loads(f.readline())
    if "error" in r:
        fail(f"{method}: {r['error']}")
    return r["result"]


def apps():
    d = Atspi.get_desktop(0)
    out = []
    for i in range(d.get_child_count()):
        a = d.get_child_at_index(i)
        if a is None:
            continue
        try:
            out.append((a.get_process_id(), a.get_name(), a))
        except Exception:  # an application that went away mid-walk
            continue
    return out


def walk(o, out):
    try:
        out.append((o.get_role_name(), o.get_name() or ""))
        for i in range(o.get_child_count()):
            c = o.get_child_at_index(i)
            if c is not None:
                walk(c, out)
    except Exception as e:
        out.append(("error", str(e)))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("port", type=int)
    ap.add_argument("--expect", choices=("present", "absent"), required=True)
    ap.add_argument("--button")
    ap.add_argument("--timeout", type=float, default=10.0)
    ap.add_argument("--open", help="click this selector in window 1, then require a frame per open "
                                    "window, each with its own buttons (M89/T89.3)")
    ap.add_argument("--act", help="perform the named push button's AT-SPI action, and require the "
                                   "application's tree to change (M54/T54.4)")
    a = ap.parse_args()
    pid = int(os.environ.get("FULGURITE_LIVE_PID", "0"))
    if pid == 0:
        fail("FULGURITE_LIVE_PID is not set; run through scripts/a11y_drive.sh")

    listed = apps()
    if not listed:
        skip("the AT-SPI desktop lists no applications (no accessibility bus or registry)")

    if a.open:
        return per_window(a, pid)

    # The agent's view: visible buttons. A hidden node is marked hidden
    # and the platform tree leaves it out, so it is not counted.
    tree = rpc(a.port, "ui.getTree")
    agent_buttons = sum(1 for n in tree["nodes"] if n.get("role") == "Button" and not n.get("hidden"))

    found = None
    deadline = time.monotonic() + a.timeout
    while True:
        listed = apps()
        found = next((x for x in listed if x[0] == pid), None)
        if found or time.monotonic() > deadline:
            break
        time.sleep(0.25)
    waited = time.monotonic() - T0

    if a.expect == "absent":
        if found:
            print(f"a11y-drive: XPASS -- pid {pid} ({found[1]}) is on the AT-SPI bus after {waited:.1f} s; "
                  "the expectation is stale: flip this leg to --expect present", file=sys.stderr)
            return 1
        print(f"a11y-drive: XFAIL (expected until T54.2) -- pid {pid} not in AT-SPI app list "
              f"({len(listed)} applications, waited {waited:.1f} s); agent sees {agent_buttons} buttons")
        return 0

    if not found:
        fail(f"pid {pid} not in AT-SPI app list ({len(listed)} applications, waited {waited:.1f} s)")
    nodes: list[tuple[str, str]] = []
    walk(found[2], nodes)
    # The first walk may land before the first frame: poll until the
    # button count settles or the timeout passes, printing the time.
    buttons = [n for r, n in nodes if r == "push button"]
    while len(buttons) != agent_buttons and time.monotonic() < deadline:
        time.sleep(0.25)
        nodes = []
        walk(found[2], nodes)
        buttons = [n for r, n in nodes if r == "push button"]
    print(f"a11y-drive: pid {pid} is on the bus as {found[1]!r} ({len(nodes)} nodes, "
          f"{time.monotonic() - T0:.1f} s after the walker started)")
    print(f"a11y-drive: push buttons on the bus: {len(buttons)} (agent's visible Button count: {agent_buttons})")
    if agent_buttons == 0:
        fail("setup: the agent sees no buttons, so the comparison proves nothing")
    if len(buttons) != agent_buttons:
        fail(f"push buttons {len(buttons)} != agent buttons {agent_buttons}: {buttons}")
    if a.button:
        if a.button not in buttons:
            fail(f"no push button named {a.button!r} among {buttons}")
        print(f"a11y-drive: push button {a.button!r} found")
    if a.act:
        act(found[2], a.act, a.port)
    return 0


def per_window(a, pid: int) -> int:
    """M89/T89.3 gate 6: a frame per window, each with its own buttons."""
    r = rpc(a.port, "input.click", {"selector": a.open})
    if r.get("handled") is not True:
        fail(f"--open: the click on {a.open} was not handled ({r}) -- the setup did not land")
    keys = [w["key"] for w in rpc(a.port, "app.windows")["windows"]]
    if len(keys) < 2:
        fail(f"--open: app.windows {keys} after clicking {a.open} -- no second window, nothing to compare")
    want = {k: sum(1 for n in rpc(a.port, "ui.getTree", window=k)["nodes"]
                   if n.get("role") == "Button" and not n.get("hidden")) for k in keys}
    deadline = time.monotonic() + a.timeout
    frames: list[tuple[str, int]] = []
    while True:
        found = next((x for x in apps() if x[0] == pid), None)
        frames = []
        if found:
            app = found[2]
            for i in range(app.get_child_count()):
                c = app.get_child_at_index(i)
                if c is None:
                    continue
                nodes: list[tuple[str, str]] = []
                walk(c, nodes)
                frames.append((c.get_name() or "", sum(1 for r_, _ in nodes if r_ == "push button")))
        counts = sorted(n for _, n in frames)
        if counts == sorted(want.values()) or time.monotonic() > deadline:
            break
        time.sleep(0.25)
    print(f"a11y-drive: pid {pid}: windows {keys}, agent buttons per window {want}; "
          f"AT-SPI frames {frames} ({len(frames)} frames, want {len(keys)})")
    if len(frames) != len(keys):
        fail(f"{len(frames)} frames on the bus for {len(keys)} windows -- a window has no adapter")
    if sorted(n for _, n in frames) != sorted(want.values()) or 0 in want.values():
        fail(f"per-frame push buttons {frames} do not match the agent's per-window counts {want}")
    print("a11y-drive: every window is on the bus as its own frame, with its own buttons")
    return 0


def values(port: int) -> list[str]:
    return sorted(n.get("value") or "" for n in rpc(port, "ui.getTree")["nodes"])


def act(app, name: str, port: int) -> None:
    """A screen reader's press, through the bus: the push button's own
    action, which the adapter hands the window as `ActionRequested` and
    the window dispatches as `Event::Action`. The press must land: some
    value in the application's tree changes (printed)."""
    stack, target = [app], None
    while stack and target is None:
        o = stack.pop()
        try:
            if o.get_role_name() == "push button" and o.get_name() == name:
                target = o
                break
            stack.extend(o.get_child_at_index(i) for i in range(o.get_child_count()))
        except Exception:
            continue
    if target is None:
        fail(f"--act: no push button named {name!r}")
    iface = target.get_action_iface()
    n = iface.get_n_actions() if iface else 0
    if n == 0:
        fail(f"--act: {name!r} offers no action on the bus")
    before = values(port)
    ok = iface.do_action(0)
    deadline = time.monotonic() + 3.0
    after = before
    while after == before and time.monotonic() < deadline:
        time.sleep(0.1)
        after = values(port)
    changed = sorted(set(after) - set(before))
    print(f"a11y-drive: AT-SPI action on {name!r} (do_action: {ok}): values changed: {changed}")
    if not changed:
        fail(f"--act: the press on {name!r} changed nothing the agent can see")


if __name__ == "__main__":
    sys.exit(main())
