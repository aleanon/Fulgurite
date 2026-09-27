#!/usr/bin/env python3
"""M54/T54.0: walk the platform's accessibility bus (AT-SPI) for one
launched window and compare it with what the agent sees.

A unit test of the tree we *would* send proves the builder; only a walk
of the bus proves the window sent it (`docs/plan-a11y-platform.md` §0
rule 1). Run by `scripts/a11y_drive.sh` through `scripts/live/launch.py`,
which puts the window's pid in LUMEN_LIVE_PID.

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


def rpc(port: int, method: str, params: dict | None = None) -> dict:
    with socket.create_connection(("127.0.0.1", port), timeout=15) as s:
        f = s.makefile("rwb")
        f.write((json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}) + "\n").encode())
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
    a = ap.parse_args()
    pid = int(os.environ.get("LUMEN_LIVE_PID", "0"))
    if pid == 0:
        fail("LUMEN_LIVE_PID is not set; run through scripts/a11y_drive.sh")

    listed = apps()
    if not listed:
        skip("the AT-SPI desktop lists no applications (no accessibility bus or registry)")

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
    return 0


if __name__ == "__main__":
    sys.exit(main())
