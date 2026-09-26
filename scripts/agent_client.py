#!/usr/bin/env python3
"""Client for the live-window agent endpoint.

Newline-delimited JSON-RPC 2.0 over plain TCP — what `just tally` serves
on LUMEN_AGENT_ADDR (default 127.0.0.1:9230). Stdlib only. The protocol
and this client's shape are Lumen's, so its `verifying-apps` skill
applies here with the method set below.

Library use:

    from agent_client import AgentClient, wait_for_port
    wait_for_port()
    c = AgentClient()
    c.screenshot("/tmp/before.png")
    c.click("#inc")
    c.screenshot("/tmp/after.png")

CLI use:

    agent_client.py wait-port [--timeout 60]
    agent_client.py call <method> ['{"json":"params"}']
    agent_client.py tree
    agent_client.py layout '<selector>'
    agent_client.py click '<selector>'
    agent_client.py press '<selector>'    # pointer down, held
    agent_client.py release '<selector>'
    agent_client.py type '<text>'          # insert at the caret
    agent_client.py fill '<sel>' '<text>'  # replace a field's whole value
    agent_client.py scroll-into-view '<sel>'
    agent_client.py key '<name>'          # Enter, Escape, Up, ...
    agent_client.py explain '<selector>'  # why a click does/doesn't land
    agent_client.py styles '<selector>'   # the cascade's answer + painted state
    agent_client.py screenshot <out.png>
    agent_client.py lint
    agent_client.py quit

Gotchas, inherited and confirmed:
- Live actions do NOT auto-wait. After acting, re-query.
- `ui.screenshot` returns `image_base64` PNG; this client decodes it.
- Selectors: `#id`, a kind name, or `@file:line`. Node indices from
  ui.getTree are not selectors.
"""
from __future__ import annotations

import argparse
import base64
import json
import os
import socket
import sys
import time

DEFAULT_ADDR = os.environ.get("LUMEN_AGENT_ADDR", "127.0.0.1:9230")


def _split(addr: str) -> tuple[str, int]:
    host, port = addr.rsplit(":", 1)
    return host, int(port)


def wait_for_port(addr: str = DEFAULT_ADDR, timeout: float = 60.0) -> None:
    host, port = _split(addr)
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with socket.create_connection((host, port), timeout=1.0):
                return
        except OSError:
            time.sleep(0.2)
    raise SystemExit(f"agent endpoint {addr} did not come up within {timeout}s")


class AgentClient:
    def __init__(self, addr: str = DEFAULT_ADDR) -> None:
        host, port = _split(addr)
        self.sock = socket.create_connection((host, port))
        self.file = self.sock.makefile("rw", encoding="utf-8", newline="\n")
        self._id = 0

    def rpc(self, method: str, **params):
        self._id += 1
        req = {"jsonrpc": "2.0", "id": self._id, "method": method, "params": params}
        self.file.write(json.dumps(req) + "\n")
        self.file.flush()
        line = self.file.readline()
        if not line:
            raise SystemExit("agent endpoint closed the connection")
        resp = json.loads(line)
        if "error" in resp:
            raise RuntimeError(f"{method}: {resp['error']}")
        return resp["result"]

    # -- conveniences --
    def tree(self):
        return self.rpc("ui.getTree")

    def layout(self, selector: str):
        return self.rpc("ui.getLayout", selector=selector)

    def click(self, selector: str):
        return self.rpc("input.click", selector=selector)

    def press(self, selector: str):
        """Pointer down, held. Pair with `release`."""
        return self.rpc("input.pointerDown", selector=selector)

    def release(self, selector: str):
        return self.rpc("input.pointerUp", selector=selector)

    def type(self, text: str):
        """Insert at the caret. Use `fill` to replace a value."""
        return self.rpc("input.type", text=text)

    def paste(self, text: str):
        """Paste into the focused node as one edit (M45/T45.7); the answer
        carries `kept`, the characters the node took."""
        return self.rpc("input.paste", text=text)

    def fill(self, selector: str, text: str):
        """Replace a field's whole value — focus, clear, type."""
        return self.rpc("input.fill", selector=selector, text=text)

    def scroll_into_view(self, selector: str):
        return self.rpc("input.scrollIntoView", selector=selector)

    def key(self, key: str, down: bool = True):
        return self.rpc("input.key", key=key, down=down)

    def screenshot(self, path: str):
        r = self.rpc("ui.screenshot")
        with open(path, "wb") as f:
            f.write(base64.b64decode(r["image_base64"]))
        return path

    def lint(self):
        return self.rpc("ui.lint")

    def explain(self, selector: str, at=None):
        """Why a click on `selector` would or would not land."""
        return self.rpc("ui.explain", selector=selector, **({"at": at} if at else {}))

    def styles(self, selector: str):
        return self.rpc("ui.getAppliedStyles", selector=selector)

    def quit(self):
        try:
            return self.rpc("app.quit")
        except (RuntimeError, SystemExit):
            return {"ok": True}


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--addr", default=DEFAULT_ADDR)
    sub = p.add_subparsers(dest="cmd", required=True)
    w = sub.add_parser("wait-port")
    w.add_argument("--timeout", type=float, default=60.0)
    c = sub.add_parser("call")
    c.add_argument("method")
    c.add_argument("params", nargs="?", default="{}")
    sub.add_parser("tree")
    sub.add_parser("lint")
    sub.add_parser("quit")
    for name in ("layout", "click", "press", "release", "explain", "styles"):
        s = sub.add_parser(name)
        s.add_argument("selector")
    t = sub.add_parser("type")
    t.add_argument("text")
    fi = sub.add_parser("fill")
    fi.add_argument("selector")
    fi.add_argument("text")
    sv = sub.add_parser("scroll-into-view")
    sv.add_argument("selector")
    k = sub.add_parser("key")
    k.add_argument("key")
    sc = sub.add_parser("screenshot")
    sc.add_argument("out")
    a = p.parse_args(argv)

    if a.cmd == "wait-port":
        wait_for_port(a.addr, a.timeout)
        print("up")
        return 0
    cl = AgentClient(a.addr)
    if a.cmd == "call":
        print(json.dumps(cl.rpc(a.method, **json.loads(a.params)), indent=1))
    elif a.cmd == "tree":
        print(json.dumps(cl.tree(), indent=1))
    elif a.cmd == "layout":
        print(json.dumps(cl.layout(a.selector)))
    elif a.cmd == "click":
        print(json.dumps(cl.click(a.selector)))
    elif a.cmd == "press":
        print(json.dumps(cl.press(a.selector)))
    elif a.cmd == "release":
        print(json.dumps(cl.release(a.selector)))
    elif a.cmd == "explain":
        print(json.dumps(cl.explain(a.selector), indent=1))
    elif a.cmd == "styles":
        print(json.dumps(cl.styles(a.selector), indent=1))
    elif a.cmd == "type":
        print(json.dumps(cl.type(a.text)))
    elif a.cmd == "fill":
        print(json.dumps(cl.fill(a.selector, a.text)))
    elif a.cmd == "scroll-into-view":
        print(json.dumps(cl.scroll_into_view(a.selector)))
    elif a.cmd == "key":
        print(json.dumps(cl.key(a.key)))
    elif a.cmd == "screenshot":
        print(cl.screenshot(a.out))
    elif a.cmd == "lint":
        print(json.dumps(cl.lint(), indent=1))
    elif a.cmd == "quit":
        print(json.dumps(cl.quit()))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
