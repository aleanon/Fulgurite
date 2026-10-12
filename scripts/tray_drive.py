#!/usr/bin/env python3
"""`just tray-drive`'s leg -- M90/T90.7 (`docs/plan-desktop-integration.md`).

Run by `scripts/live/launch.py` against `examples/tray`, a daemon whose only
surface is a tray item, once per invalidation strategy. Every click is sent
the way a panel sends it: the item's `com.canonical.dbusmenu` `Event(id,
"clicked")` over D-Bus, with `gdbus` -- **the OS path, not a debug hook**.
The agent endpoint only observes (`app.windows`, `app.tray`, `#status`),
except for the opened window's own Close button, which is that window's.

In order, each printing its quantity beside what it wants:

(a) `app.windows` is `[0]`, key 0's `windowless` flag true, and `app.tray`
    says the program declares a tray, this build supports one, and the
    watcher holds it (setup landed: `registered`).
(b) the watcher's `RegisteredStatusNotifierItems` holds exactly one item
    whose bus owner's pid is the launched child's, and its dbusmenu layout's
    labels equal `app.tray`'s, Open enabled.
(c) `Event(Open, "clicked")`: `app.windows` goes `[0]` -> `[0, k]`, and Open's
    dbusmenu `enabled` turns false (one window at a time).
(d) the window's Close button: `app.windows` is `[0]` again -- an answer,
    not an error -- and the item is still registered.
(e) Lock: Open's `enabled` false and `#status` "locked"; **accept, the same
    input that must do nothing:** Open clicked while locked leaves
    `app.windows` `[0]`; Lock again: Open's `enabled` true.
(f) Quit: the process exits within 2 s (its status is the launcher's
    `--expect-exit 0`). On a miss, the stderr lines that name
    `Request::Quit` are printed.
"""
from __future__ import annotations

import json
import os
import socket
import subprocess
import sys
import time

from gi.repository import GLib

port = int(sys.argv[1])
PID = int(os.environ["FULGURITE_LIVE_PID"])
s = socket.create_connection(("127.0.0.1", port), timeout=15)
f = s.makefile("rwb")
n = [0]
WATCHER = ("org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher")


class PortLost(Exception):
    """The agent's port went away mid-drive: the process ended."""


def fail(msg):
    sys.exit(f"tray-drive: FAIL -- {msg}")


def raw(method, params=None, **top):
    n[0] += 1
    req = {"jsonrpc": "2.0", "id": n[0], "method": method, "params": params or {}}
    req.update(top)
    f.write((json.dumps(req) + "\n").encode())
    f.flush()
    try:
        line = f.readline()
    except OSError:
        line = b""
    if not line:
        raise PortLost(f"{method}: the agent endpoint closed the connection")
    return json.loads(line)


def result(method, params=None, **top):
    r = raw(method, params, **top)
    if "error" in r:
        fail(f"{method} {top or ''} answered an error where an answer was expected: {r['error']}")
    return r["result"]


def windows():
    return result("app.windows")["windows"]


def keys():
    return [w["key"] for w in windows()]


def gdbus(dest, path, method, *args):
    """One `gdbus call` on the session bus, its GVariant text reply parsed."""
    cmd = ["gdbus", "call", "--session", "--dest", dest, "--object-path", path, "--method", method, *map(str, args)]
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
    if p.returncode != 0:
        fail(f"gdbus {method} on {dest}{path}: {p.stderr.strip()}")
    return GLib.Variant.parse(None, p.stdout.strip(), None, None).unpack()


def prop(dest, path, iface, name):
    return gdbus(dest, path, "org.freedesktop.DBus.Properties.Get", iface, name)[0]


def items():
    return list(prop(*WATCHER, "org.kde.StatusNotifierWatcher", "RegisteredStatusNotifierItems"))


def owner_pid(name):
    return gdbus("org.freedesktop.DBus", "/org/freedesktop/DBus",
                 "org.freedesktop.DBus.GetConnectionUnixProcessID", name)[0]


def split(entry):
    """A registry entry: `name/path`, or a bare name (the default path)."""
    name, sep, rest = entry.partition("/")
    return name, ("/" + rest) if sep else "/StatusNotifierItem"


def ours():
    """The registry entries whose bus owner is the launched child."""
    mine = []
    for e in items():
        name, path = split(e)
        try:
            if owner_pid(name) == PID:
                mine.append((name, path))
        except SystemExit:
            continue
    return mine


def layout(name, menu):
    """The dbusmenu's top-level entries: [(dbusmenu id, label, enabled)]."""
    _rev, (_root, _props, children) = gdbus(name, menu, "com.canonical.dbusmenu.GetLayout", "int32 0",
                                             "int32 -1", "['label', 'enabled', 'visible']")
    out = []
    for c in children:
        cid, props, _ = c
        if props.get("visible", True) is False:
            continue
        # A dbusmenu label doubles `_` (a mnemonic marker); the item does so.
        out.append((cid, props.get("label", "").replace("__", "_"), props.get("enabled", True)))
    return out


def click(name, menu, ids, label):
    # Typed literals: gdbus would read a bare `-1` as an option.
    gdbus(name, menu, "com.canonical.dbusmenu.Event", f"int32 {ids[label]}", "clicked", "<int32 0>", "uint32 0")


def wait(pred, timeout=3.0):
    t0 = time.monotonic()
    while True:
        v = pred()
        if v or time.monotonic() - t0 > timeout:
            return v
        time.sleep(0.03)


def enabled_of(name, menu, label):
    return {lab: en for _, lab, en in layout(name, menu)}[label]


def alive(pid):
    try:
        with open(f"/proc/{pid}/stat") as st:
            return st.read().rsplit(")", 1)[1].split()[0] != "Z"
    except OSError:
        return False


def quit_lines():
    log = open(os.environ["FULGURITE_LIVE_LOG"], errors="replace").read()
    return [ln for ln in log.splitlines() if "Request::Quit" in ln]


def drive():
    strategy = os.environ.get("FULGURITE_LIVE_STRATEGY")
    print(f"== tray-drive: pid {PID}, strategy {strategy} ==")

    # (a) the windowless session, and the tray as the window reports it.
    r = raw("app.windows")
    if "error" in r:
        fail(f"(a) app.windows answered {r['error']} where [0] was expected -- a daemon with no window "
             f"must be observable (G23)")
    ws = r["result"]["windows"]
    print(f"  ..  (a) app.windows {[w['key'] for w in ws]} (want [0]), key 0 windowless "
          f"{[w.get('windowless') for w in ws if w['key'] == 0]} (want [True])")
    if [w["key"] for w in ws] != [0] or ws[0].get("windowless") is not True:
        fail(f"(a) app.windows {ws}, want only the windowless session, key 0")
    tray = wait(lambda: (t := result("app.tray"))["registered"] and t)
    tray = tray or result("app.tray")
    print(f"  ..  (a) app.tray: declared {tray['declared']}, supported {tray['supported']}, registered "
          f"{tray['registered']}, labels {tray['labels']}, enabled {tray['enabled']}, mask {tray['mask']}, "
          f"hash {tray['hash']}, error {tray['error']}")
    if not (tray["declared"] and tray["supported"] and tray["registered"]):
        fail(f"(a) the tray did not land: {tray}")
    print("  ok  (a) a daemon with no window answers app.windows [0] and app.tray, registered")

    # (b) the item in the watcher's registry, by its owner's pid, and its menu.
    mine = wait(ours)
    every = items()
    print(f"  ..  (b) RegisteredStatusNotifierItems: {len(every)} item(s); owned by pid {PID}: {mine} (want 1)")
    if len(mine) != 1:
        fail(f"(b) {len(mine)} registered items are owned by the launched pid {PID} (registry {every}), want 1")
    name, path = mine[0]
    menu = prop(name, path, "org.kde.StatusNotifierItem", "Menu")
    lay = layout(name, menu)
    labels = [lab for _, lab, _ in lay]
    ids = {lab: cid for cid, lab, _ in lay}
    print(f"  ..  (b) {name}{path} menu {menu}: dbusmenu labels {labels}, enabled "
          f"{[en for _, _, en in lay]} / app.tray {tray['labels']}")
    if labels != tray["labels"]:
        fail(f"(b) the dbusmenu shows {labels}, app.tray reports {tray['labels']}")
    if not enabled_of(name, menu, "Open"):
        fail("(b) Open is disabled with no window open and nothing locked")
    print("  ok  (b) the item is the launched process's, and its menu is the model app.tray reports")

    # (c) Open, chosen over dbusmenu.
    before = keys()
    click(name, menu, ids, "Open")
    after = wait(lambda: (k := keys()) != before and k)
    after = after or keys()
    print(f"  ..  (c) Event(Open, clicked): app.windows {before} -> {after} (want [0, k])")
    if len(after) != 2 or after[0] != 0:
        fail(f"(c) app.windows {after} after Open was clicked over dbusmenu, want [0, k]: the click "
             f"never reached the daemon's on_menu")
    k = after[1]
    off = wait(lambda: not enabled_of(name, menu, "Open"))
    print(f"  ..  (c) Open's dbusmenu enabled with window {k} open: {not off} (want False)")
    if not off:
        fail("(c) Open stayed enabled with its window open: the mask never reached the item")
    print(f"  ok  (c) Open over the OS path opened window {k}")

    # (d) the window's own Close button.
    r = raw("input.click", {"selector": "#close"}, window=k)
    if "error" in r or r["result"].get("handled") is not True:
        fail(f"(d) setup: input.click #close on window {k} answered {r.get('error') or r['result']}")
    try:
        back = wait(lambda: keys() == [0])
        now = keys()
    except PortLost:
        fail("(d) the process was gone after its last window closed, before Quit: a daemon runs on "
             "when its windows close (M89)")
    reg = result("app.tray")["registered"]
    still = len(ours())
    print(f"  ..  (d) Close on window {k}: app.windows {now} (want [0]); app.tray registered {reg}, "
          f"registry entries owned by pid {PID}: {still} (want 1)")
    if not back:
        fail(f"(d) app.windows {now} after the window's Close, want [0]")
    if not reg or still != 1:
        fail("(d) the item was unregistered when the window closed")
    print("  ok  (d) closing the window leaves the daemon and its item")

    # (e) Lock flips Open's enabled; Open while locked does nothing.
    click(name, menu, ids, "Lock")
    off = wait(lambda: not enabled_of(name, menu, "Open"))
    status = result("ui.resolve", {"selector": "#status"}, window=0)["value"]
    print(f"  ..  (e) Lock: Open's dbusmenu enabled {not off} (want False), #status {status!r} (want 'locked')")
    if not off or status != "locked":
        fail("(e) Lock did not reach the daemon, or its state did not reach the item's enabled property")
    click(name, menu, ids, "Open")
    time.sleep(0.4)
    nothing = keys()
    print(f"  ..  (e) accept: Open clicked while locked: app.windows {nothing} (want [0])")
    if nothing != [0]:
        fail(f"(e) a disabled Open opened a window: app.windows {nothing}")
    click(name, menu, ids, "Lock")
    on = wait(lambda: enabled_of(name, menu, "Open"))
    print(f"  ..  (e) Lock again: Open's dbusmenu enabled {on} (want True)")
    if not on:
        fail("(e) unlocking did not re-enable Open")
    print("  ok  (e) the application's state flips the entry's dbusmenu enabled property, both ways")

    # (f) Quit, from the tray: Request::Quit ends the process.
    t0 = time.monotonic()
    click(name, menu, ids, "Quit")
    while alive(PID) and time.monotonic() - t0 < 2.0:
        time.sleep(0.02)
    ms = (time.monotonic() - t0) * 1000
    gone = not alive(PID)
    print(f"  ..  (f) Quit: the process {'exited' if gone else 'still running'} after {ms:.0f} ms / bound 2000 ms")
    if not gone:
        lines = quit_lines()
        print(f"  ..  (f) stderr lines naming Request::Quit: {len(lines)}")
        for ln in lines:
            print(f"        {ln}")
        fail("(f) the process was still running 2 s after Quit was chosen from the tray")
    print("  ok  (f) Quit from the tray ended the process within 2 s")


try:
    drive()
except PortLost as e:
    fail(f"the process is gone before Quit: {e}")
print("tray-drive: leg passed")
