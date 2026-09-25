#!/usr/bin/env python3
"""Replay a captured browser request against a live agent port, and read
whether it moved the application — M39/T39.0 (R14-F1).

Usage: browser_replay.py ADDR FIXTURE WATCH [--xfail TASK] [--pid PID]

ADDR     the agent endpoint, host:port
FIXTURE  a file under crates/lumen-static-agent/tests/browser/, replayed
         byte for byte on its own connection (the bytes a real browser
         sent; see scripts/capture_browser_requests.py)
WATCH    a selector whose shown text the fixture's JSON lines would move
         (`#value` in counter-window, `#counts` in tasks-demo)

Prints `WATCH A -> B` and passes when the text did not move — the replay
executed nothing. `--xfail TASK` marks the step as expected red until
TASK lands: it then passes when the text *did* move, and fails if it did
not, so the marker is removed in the commit that fixes it.

`--pid PID` first checks that ADDR's listening socket belongs to PID
(`ss -ltnp`), so a stale window from another run cannot answer for the
one the script launched.
"""
import argparse
import os
import socket
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from agent_client import AgentClient, wait_for_port  # noqa: E402


def shown(addr, sel):
    """`sel`'s shown text, on a connection of its own that is closed
    before the replay opens one — the file object too, which holds the
    socket open after `sock.close()`."""
    c = AgentClient(addr)
    try:
        r = c.rpc("ui.resolve", selector=sel)
    finally:
        c.file.close()
        c.sock.close()
    return r.get("value", r.get("label"))


def owner_ok(addr, pid):
    port = addr.rsplit(":", 1)[1]
    out = subprocess.run(["ss", "-ltnpH", f"sport = :{port}"], capture_output=True, text=True).stdout
    return f"pid={pid}," in out, out.strip()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("addr")
    p.add_argument("fixture")
    p.add_argument("watch")
    p.add_argument("--xfail")
    p.add_argument("--pid")
    a = p.parse_args()

    wait_for_port(a.addr, 60)
    if a.pid:
        ok, line = owner_ok(a.addr, a.pid)
        if not ok:
            print(f"browser-replay: {a.addr} is not owned by pid {a.pid}: {line!r}", file=sys.stderr)
            return 1

    data = open(a.fixture, "rb").read()
    before = shown(a.addr, a.watch)
    host, port = a.addr.rsplit(":", 1)
    s = socket.create_connection((host, int(port)))
    s.sendall(data)
    s.settimeout(2.0)
    t0, replies, closed = time.time(), b"", False
    try:
        while True:
            chunk = s.recv(65536)
            if not chunk:
                closed = True
                break
            replies += chunk
    except (socket.timeout, ConnectionResetError):
        pass
    s.close()
    # A click can start deferred work (tasks-demo's `#spawn-one` lands
    # ~40 ms later): read after that could have landed, or a replay that
    # executed reads as one that did not.
    time.sleep(0.5)
    after = shown(a.addr, a.watch)
    moved = before != after
    name = os.path.basename(a.fixture)
    print(f"browser-replay {name}: {a.watch} {before} -> {after}; "
          f"replies {replies.count(b'\n')}; closed {closed} after {int((time.time() - t0) * 1000)} ms")
    if a.xfail:
        if moved:
            print(f"browser-replay: XFAIL (red until {a.xfail}): the replay executed")
            return 0
        print(f"browser-replay: XPASS: {a.xfail} has landed; remove --xfail", file=sys.stderr)
        return 1
    if moved:
        print(f"browser-replay: FAIL: a browser's request moved {a.watch}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
