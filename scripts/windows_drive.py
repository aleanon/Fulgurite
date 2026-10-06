"""M89 (docs/plan-windows.md): more than one window, through ONE agent port.

Launched by `scripts/windows_drive.sh` through `scripts/live/launch.py`,
one phase per call:

    windows_drive.py PORT PHASE

Phase 0 (T89.0) pins TODAY's behaviour on `examples/counter-window`, a
one-window program, so the task that changes each line flips it rather
than writing it fresh (docs/plan-iced-roadmap.md §3.1, S3). Each
assertion names the task that inverts it:

  (a) `app.windows` answers -32601, method not found       -- T89.2 inverts
  (b) a request carrying `"window": 7` is answered by the
      one window anyway: a click on `#inc` is `handled` and
      `#value` reads 1 after it                              -- T89.2 inverts
  (c) `app.quit` exits 0 in under 2 s                        -- stays (accept)

(c) is the launcher's own stop (`--stop quit`: `app.quit`, then exit
status 0 timed against a 2000 ms bound and the port closed), so this
script leaves the process running for it.

Every check prints the quantity it read beside what it wanted.
"""
import json
import socket
import sys

port, phase = int(sys.argv[1]), sys.argv[2]
s = socket.create_connection(("127.0.0.1", port), timeout=15)
f = s.makefile("rwb")
n = [0]


def raw(method, params=None, **top):
    """One request; `top` adds top-level members (the `window` field)."""
    n[0] += 1
    req = {"jsonrpc": "2.0", "id": n[0], "method": method, "params": params or {}}
    req.update(top)
    f.write((json.dumps(req) + "\n").encode())
    f.flush()
    line = f.readline()
    if not line:
        sys.exit(f"windows-drive: FAIL -- {method}: the agent endpoint closed the connection")
    return json.loads(line)


def result(method, params=None, **top):
    r = raw(method, params, **top)
    if "error" in r:
        sys.exit(f"windows-drive: FAIL -- {method} {top or ''} answered an error: {r['error']}")
    return r["result"]


def fail(msg):
    sys.exit(f"windows-drive: FAIL -- {msg}")


def phase0():
    print("== phase 0 (T89.0): today's one-window behaviour, pinned ==")
    # (a) -- T89.2 inverts: the router answers `app.windows` itself.
    r = raw("app.windows")
    err = r.get("error")
    if err is None:
        fail(f"app.windows: answered {r.get('result')!r} -- want -32601 until T89.2 builds it")
    print(f"  ..  app.windows: {err.get('code')} {err.get('message')} (want -32601)")
    if err.get("code") != -32601:
        fail(f"app.windows: error {err.get('code')}, want -32601 method not found")
    print("  ok  (a) app.windows: -32601 method not found [T89.2 inverts]")

    # (b) -- T89.2 inverts: `"window": 7` names no window and is -32602.
    # Setup landed: the click is `handled`, and the label it drives moved.
    before = result("ui.resolve", {"selector": "#value"})["value"]
    click = raw("input.click", {"selector": "#inc"}, window=7)
    if "error" in click:
        fail(f'"window": 7: input.click answered {click["error"]} -- want the one window to answer it until T89.2')
    handled = click["result"].get("handled")
    after = result("ui.resolve", {"selector": "#value"}, window=7)["value"]
    print(f'  ..  "window": 7: input.click #inc handled {handled} (want true); '
          f"#value {before!r} -> {after!r} (want +1)")
    if handled is not True:
        fail(f'"window": 7: the click on #inc was not handled ({click["result"]!r}) -- the setup did not land')
    try:
        moved = int(after) == int(before) + 1
    except ValueError:
        moved = False
    if not moved:
        fail(f'"window": 7: #value {before!r} -> {after!r}, want one more -- the click landed nowhere visible')
    if "window" in click:
        fail(f'"window": 7: the reply names a window ({click["window"]!r}) -- pinned: no reply carries one until T89.2')
    print('  ok  (b) "window": 7 answered by the one window [T89.2 inverts]')
    print("  ..  (c) app.quit exits 0 in under 2 s: the launcher's --stop quit checks it next [stays]")


PHASES = {"0": phase0}
if phase not in PHASES:
    fail(f"no phase {phase!r}; have {sorted(PHASES)}")
PHASES[phase]()
print(f"windows-drive: phase {phase} passed")
