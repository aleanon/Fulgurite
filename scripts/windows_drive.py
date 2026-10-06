"""M89 (docs/plan-windows.md): more than one window, through ONE agent port.

Launched by `scripts/windows_drive.sh` through `scripts/live/launch.py`,
one phase per call:

    windows_drive.py PORT PHASE

Phase 1 (T89.2) is one window, `examples/counter-window`, under both
invalidation strategies. It inverts T89.0's phase 0, which pinned the
one-window router (`app.windows` was -32601, and a request carrying
`"window": 7` was answered by the one window anyway):

  (a) `app.windows` lists exactly `[1]`, and its reply names window 1
  (b) a request carrying `"window": 7` answers -32602 naming
      `app.windows`, carries no `"window"` (nobody answered), and the
      click it carried landed nowhere: `#value` does not move
  (c) `input.click` with `"window": 1` on `#inc` is `handled`, the reply
      carries `"window": 1`, and `#value` reads one more -- the accept
      direction; and a request with no `window` member is the primary's
  (d) `app.quit` exits 0 in under 2 s                        -- stays (accept)

(d) is the launcher's own stop (`--stop quit`: `app.quit`, then exit
status 0 timed against a 2000 ms bound and the port closed), so this
script leaves the process running for it. (b) is the router's injection
point (docs/plan-windows.md T89.2: "the router ignores `window` and always
answers from the primary" answers the request, and (b) fails by name).

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


def value():
    return result("ui.resolve", {"selector": "#value"})["value"]


def phase1():
    print("== phase 1 (T89.2): one window, addressed by its key ==")
    # (a) app.windows: exactly [1], and the reply names who answered.
    r = raw("app.windows")
    if "error" in r:
        fail(f"(a) app.windows answered {r['error']} -- want the router to answer it")
    keys = [w.get("key") for w in r["result"].get("windows", [])]
    print(f"  ..  app.windows: {r['result']['windows']} -> keys {keys} (want [1]); reply window {r.get('window')!r} (want 1)")
    if keys != [1]:
        fail(f"(a) app.windows listed keys {keys}, want exactly [1]")
    if r.get("window") != 1:
        fail(f"(a) app.windows' reply names window {r.get('window')!r}, want 1")
    print("  ok  (a) app.windows lists [1]")

    # (b) "window": 7 names no window: -32602 naming app.windows, and the
    # click it carried did not land anywhere.
    before = value()
    click = raw("input.click", {"selector": "#inc"}, window=7)
    after = value()
    err = click.get("error")
    print(f'  ..  "window": 7: {err or click.get("result")} (want -32602 naming app.windows); '
          f"#value {before!r} -> {after!r} (want unchanged)")
    if err is None:
        fail(f'(b) "window": 7: answered {click.get("result")!r} by window {click.get("window")!r} '
             f"-- the router ignored the window member")
    if err.get("code") != -32602 or "app.windows" not in err.get("message", ""):
        fail(f'(b) "window": 7: error {err!r}, want -32602 naming app.windows')
    if "window" in click:
        fail(f'(b) "window": 7: the refusal names window {click["window"]!r} -- nobody answered it')
    if after != before:
        fail(f'(b) "window": 7: #value {before!r} -> {after!r} -- the refused click landed')
    print('  ok  (b) "window": 7 -> -32602 naming app.windows, nothing landed')

    # (c) the accept direction: "window": 1 is the one window.
    before = value()
    click = raw("input.click", {"selector": "#inc"}, window=1)
    if "error" in click:
        fail(f'(c) "window": 1: input.click answered {click["error"]}')
    handled = click["result"].get("handled")
    after = result("ui.resolve", {"selector": "#value"}, window=1)["value"]
    print(f'  ..  "window": 1: input.click #inc handled {handled} (want true), reply window '
          f"{click.get('window')!r} (want 1); #value {before!r} -> {after!r} (want +1)")
    if handled is not True:
        fail(f'(c) "window": 1: the click on #inc was not handled ({click["result"]!r}) -- the setup did not land')
    if click.get("window") != 1:
        fail(f'(c) "window": 1: the reply names window {click.get("window")!r}, want 1')
    try:
        moved = int(after) == int(before) + 1
    except ValueError:
        moved = False
    if not moved:
        fail(f'(c) "window": 1: #value {before!r} -> {after!r}, want one more')
    plain = raw("ui.resolve", {"selector": "#value"})
    print(f"  ..  no window member: reply window {plain.get('window')!r} (want 1, the primary)")
    if plain.get("window") != 1:
        fail(f"(c) a request with no window member was answered by {plain.get('window')!r}, want the primary, 1")
    print('  ok  (c) "window": 1 clicks #inc, handled, and the reply names window 1')
    print("  ..  (d) app.quit exits 0 in under 2 s: the launcher's --stop quit checks it next [stays]")


PHASES = {"1": phase1}
if phase not in PHASES:
    fail(f"no phase {phase!r}; have {sorted(PHASES)}")
PHASES[phase]()
print(f"windows-drive: phase {phase} passed")
