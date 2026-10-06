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

Phase 2 (T89.3) is `examples/two-windows`, a list (program 0) whose rows
open a detail (program 1), under both strategies. Each gate prints what
it read beside its bound (docs/plan-windows.md T89.3, gates 2-5 and 7):

  (a) a click on `#row2` is `handled`, its reply says its own drain
      opened `[2]` (`"opened"`), `#opened` reads `2`, and `app.windows`
      goes [1] -> [1, 2] -- answered inside the drain
  (b) `#detail-bump` clicked with `"window": 2` is `handled`; the detail
      counts 1, the list's tree has no `#detail-bump` and its values did
      not move (both trees' ids printed)
  (c) OS routing: an XTest click at `#detail-bump`'s centre in window 2's
      screen origin raises the detail's count to 2 (window 2 found by
      `_NET_WM_PID` and title)
  (d) shaping per window: the detail's `Complex` RTL label is painted as
      wide as its own session measured it (ink columns against the box)
  (e) one listener: `ss -ltnpH` counts 1 listening socket for the
      process, though the detail asks for 127.0.0.1:7999
  (f) a panic in the detail shows M36's screen there (`app.failure` on
      window 2, and its `app.presents` grows) while the list still takes
      a `handled` click; R pressed in window 2 (XTest) retries window 2,
      which needs its failure read from its own slot
  (g) closing: `app.close` on window 2, Escape in window 3 (its key
      hook's `KeyOutcome::Quit`) and `#detail-close` in window 4
      (`Request::Close`) each leave window 1 answering `ui.getTree` and a
      `handled` click, with `app.windows` back at [1] and the list told
      `closed N`; `#missing` answers `no program 9`
  (h) `app.close` on window 1 ends the process with status 0 inside 2 s
      (the launcher's `--expect-exit 0` reads the status)

Every check prints the quantity it read beside what it wanted.
"""
import base64
import json
import os
import socket
import struct
import subprocess
import sys
import time
import zlib

port, phase = int(sys.argv[1]), sys.argv[2]
s = socket.create_connection(("127.0.0.1", port), timeout=15)
f = s.makefile("rwb")
n = [0]


class PortLost(Exception):
    """The agent's port went away mid-drive: the process ended."""


def raw(method, params=None, **top):
    """One request; `top` adds top-level members (the `window` field)."""
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
        sys.exit(f"windows-drive: FAIL -- {method} {top or ''} answered an error: {r['error']}")
    return r["result"]


def fail(msg):
    sys.exit(f"windows-drive: FAIL -- {msg}")


def tally():
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
    before = tally()
    click = raw("input.click", {"selector": "#inc"}, window=7)
    after = tally()
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
    before = tally()
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



def png_rows(b64):
    """A minimal PNG decoder (8-bit RGB or RGBA, filters 0-4): width,
    height and the rows as RGB tuples' bytes, bpp per pixel."""
    data = base64.b64decode(b64)
    pos, idat = 8, b""
    while pos < len(data):
        ln, = struct.unpack(">I", data[pos:pos + 4])
        kind, body = data[pos + 4:pos + 8], data[pos + 8:pos + 8 + ln]
        pos += 12 + ln
        if kind == b"IHDR":
            w, h, _depth, ctype = struct.unpack(">IIBB", body[:10])
            bpp = {2: 3, 6: 4}[ctype]
        elif kind == b"IDAT":
            idat += body
    raw, stride = zlib.decompress(idat), w * bpp
    prev, rows = bytearray(stride), []
    for row in range(h):
        ft = raw[row * (stride + 1)]
        line = bytearray(raw[row * (stride + 1) + 1:(row + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b, c = prev[i], (prev[i - bpp] if i >= bpp else 0)
            if ft == 1: line[i] = (line[i] + a) & 255
            elif ft == 2: line[i] = (line[i] + b) & 255
            elif ft == 3: line[i] = (line[i] + (a + b) // 2) & 255
            elif ft == 4:
                p = a + b - c; pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return w, h, bpp, rows


def keys():
    return [w["key"] for w in result("app.windows")["windows"]]


def value(sel, window=1):
    return result("ui.resolve", {"selector": sel}, window=window)["value"]


def wids(window):
    return [n.get("wid") for n in result("ui.getTree", {"depth": 64}, window=window)["nodes"] if n.get("wid")]


def click(sel, window, what):
    r = raw("input.click", {"selector": sel}, window=window)
    if "error" in r:
        fail(f"{what}: input.click {sel} on window {window} answered {r['error']}")
    if r["result"].get("handled") is not True or r.get("window") != window:
        fail(f"{what}: input.click {sel} on window {window}: handled {r['result'].get('handled')}, "
             f"answered by window {r.get('window')!r} -- the setup did not land")
    return r


def wait_keys(want, what, timeout=3.0):
    t0 = time.monotonic()
    while True:
        got = keys()
        if got == want or time.monotonic() - t0 > timeout:
            return got


def open_row(row, want_key, what):
    """Click `#row{row}`: the window must be opened, and the list answered,
    **inside the click's own drain**. The click's reply names the windows
    that drain opened (`"opened"`), which is the only observation that can
    tell: a read after the reply converges either way, because the loop
    goes idle between the reply and the next request -- a sequential read
    of `#opened`, and one sent on a second connection while the click was
    on the loop, both stayed green under T89.3's injection (an open
    deferred to `about_to_wait`)."""
    before = keys()
    r = click(f"#row{row}", 1, what)
    in_drain = r.get("opened")
    opened = value("#opened")
    after = keys()
    print(f"  ..  {what}: click #row{row} handled; the reply's opened {in_drain} (want [{want_key}]); "
          f"#opened {opened!r} (want {str(want_key)!r}); app.windows {before} -> {after} (want {before + [want_key]})")
    if in_drain != [want_key]:
        fail(f"{what}: the click's reply says its drain opened {in_drain}, want [{want_key}] -- answered after the drain")
    if opened != str(want_key):
        fail(f"{what}: #opened reads {opened!r}, want {want_key!r} -- the list was not answered WindowOpened")
    if after != before + [want_key]:
        fail(f"{what}: app.windows {before} -> {after}, want {before + [want_key]}")
    # The window's first frame: give the X server time to map it.
    time.sleep(0.6)


def x_window(pid, title):
    """The display, its root, and the mapped window owned by `pid` titled
    `title` (found by `_NET_WM_PID` and `_NET_WM_NAME`)."""
    try:
        from Xlib import X, display
    except ImportError:
        fail("python3-xlib is missing -- install it: sudo apt install python3-xlib "
             "(a prerequisite on the reference machine, never a skip)")
    d = display.Display()
    root = d.screen().root
    pid_atom, name_atom = d.intern_atom("_NET_WM_PID"), d.intern_atom("_NET_WM_NAME")
    utf8 = d.intern_atom("UTF8_STRING")

    def name_of(w):
        p = w.get_full_property(name_atom, utf8)
        if p:
            return p.value.decode("utf-8", "replace")
        n = w.get_wm_name()
        return n.decode() if isinstance(n, bytes) else (n or "")

    def find(w):
        try:
            p = w.get_full_property(pid_atom, X.AnyPropertyType)
            if p and p.value[0] == pid and w.get_attributes().map_state == X.IsViewable and name_of(w) == title:
                return w
            for c in w.query_tree().children:
                r = find(c)
                if r:
                    return r
        except Exception:
            pass
        return None

    for _ in range(30):
        win = find(root)
        if win:
            # Raise it: on a shared display another window may sit over it.
            top = win
            while top.query_tree().parent.id != root.id:
                top = top.query_tree().parent
            top.configure(stack_mode=X.Above)
            d.sync()
            time.sleep(0.3)
            return d, root, win
        time.sleep(0.1)
    fail(f"no mapped window with _NET_WM_PID {pid} titled {title!r}")


def xtest_click(pid, title, x, y, scale):
    """A real pointer click through the X server at logical (x, y) of the
    window owned by `pid` titled `title` -- the OS routing path."""
    from Xlib import X
    from Xlib.ext import xtest
    d, root, win = x_window(pid, title)
    o = win.translate_coords(root, 0, 0)
    rx, ry = int(-o.x + x * scale), int(-o.y + y * scale)
    xtest.fake_input(d, X.MotionNotify, x=rx, y=ry)
    d.sync()
    time.sleep(0.1)
    xtest.fake_input(d, X.ButtonPress, 1)
    d.sync()
    time.sleep(0.05)
    xtest.fake_input(d, X.ButtonRelease, 1)
    d.sync()
    return (-o.x, -o.y), (rx, ry)


def xtest_key(pid, title, ch):
    """A real key press through the X server, to the window owned by `pid`
    titled `title`: the pointer over it and the input focus set on it, so
    it arrives whether or not a window manager would have focused it."""
    from Xlib import X, XK
    from Xlib.ext import xtest
    d, root, win = x_window(pid, title)
    g = win.get_geometry()
    o = win.translate_coords(root, 0, 0)
    xtest.fake_input(d, X.MotionNotify, x=-o.x + g.width // 2, y=-o.y + g.height // 2)
    win.set_input_focus(X.RevertToParent, X.CurrentTime)
    d.sync()
    time.sleep(0.2)
    code = d.keysym_to_keycode(XK.string_to_keysym(ch))
    xtest.fake_input(d, X.KeyPress, code)
    d.sync()
    time.sleep(0.05)
    xtest.fake_input(d, X.KeyRelease, code)
    d.sync()


def listeners(pid):
    out = subprocess.run(["ss", "-ltnpH"], capture_output=True, text=True, check=True).stdout
    return [ln.split()[3] for ln in out.splitlines() if f"pid={pid}," in ln]


def phase2():
    print("== phase 2 (T89.3): two windows, opened and closed ==")
    pid = int(os.environ["LUMEN_LIVE_PID"])
    if keys() != [1]:
        fail(f"setup: app.windows {keys()} at start, want [1]")

    # (a) open the detail: answered inside the drain.
    open_row(2, 2, "(a)")
    print("  ok  (a) #row2 opened window 2, answered inside the drain")

    # (e) one listener, with the detail's own address set on purpose.
    socks = listeners(pid)
    print(f"  ..  (e) listening sockets of pid {pid}: {socks} -> {len(socks)} (bound 1)")
    if len(socks) != 1:
        fail(f"(e) the process listens on {len(socks)} sockets ({socks}), bound 1 -- a second agent port")
    print("  ok  (e) one listener")

    # (b) the detail's own counter, through the agent.
    list_before = (value("#count"), value("#opened"))
    click("#detail-bump", 2, "(b)")
    count = value("#detail-count", 2)
    list_ids, detail_ids = wids(1), wids(2)
    list_after = (value("#count"), value("#opened"))
    print(f"  ..  (b) window 2 ids {detail_ids}")
    print(f"  ..  (b) window 1 ids {list_ids}")
    print(f"  ..  (b) #detail-count {count!r} (want '1'); list (#count, #opened) {list_before} -> {list_after} (want unchanged)")
    if count != "1":
        fail(f"(b) #detail-count {count!r} after one bump on window 2, want '1'")
    if "detail-bump" in list_ids or "detail-bump" not in detail_ids:
        fail("(b) #detail-bump is in the wrong tree")
    if list_after != list_before:
        fail(f"(b) the list's values moved: {list_before} -> {list_after}")
    print("  ok  (b) window 2's own counter, the list untouched")

    # (c) OS routing: a real click at #detail-bump in window 2.
    b = result("ui.resolve", {"selector": "#detail-bump"}, window=2)["bounds"]
    scale = result("ui.screenshot", {}, window=2).get("scale", 1.0)
    cx, cy = (b[0] + b[2]) / 2, (b[1] + b[3]) / 2
    origin, at = xtest_click(pid, "two-windows — detail", cx, cy, scale)
    for _ in range(30):
        count = value("#detail-count", 2)
        if count == "2":
            break
        time.sleep(0.1)
    print(f"  ..  (c) XTest click at #detail-bump's centre ({cx:.1f}, {cy:.1f}) of window 2 (origin {origin}, "
          f"root {at}): #detail-count {count!r} (want '2'); list #count {value('#count')!r} (want {list_before[0]!r})")
    if count != "2":
        fail(f"(c) the X server's click on window 2 left #detail-count at {count!r}, want '2' -- "
             f"window_event did not route it to window 2's slot")
    print("  ok  (c) the OS click reached window 2")

    # (d) shaping per window: ink against the measured box.
    b = result("ui.resolve", {"selector": "#rtl"}, window=2)["bounds"]
    shot = result("ui.screenshot", {}, window=2)
    w, h, bpp, rows = png_rows(shot["image_base64"])
    k = shot.get("scale", 1.0)
    page = tuple(rows[1][0:3])
    y0, y1 = int(b[1] * k), int(b[3] * k)
    cols = [x for x in range(w) if any(tuple(rows[y][x * bpp:x * bpp + 3]) != page for y in range(y0, min(y1, h)))]
    ink = (cols[0] / k, (cols[-1] + 1) / k) if cols else (0.0, 0.0)
    box_w, ink_w = b[2] - b[0], ink[1] - ink[0]
    print(f"  ..  (d) #rtl box x {b[0]:.1f}..{b[2]:.1f} ({box_w:.1f} px, the detail's Complex measure); "
          f"painted ink x {ink[0]:.1f}..{ink[1]:.1f} ({ink_w:.1f} px); |ink - box| {abs(ink_w - box_w):.1f} (bound 3.0)")
    if not cols:
        fail("(d) setup: #rtl painted nothing")
    if abs(ink_w - box_w) > 3.0:
        fail(f"(d) #rtl painted {ink_w:.1f} px wide in a {box_w:.1f} px box -- window 2 renders with "
             f"another program's stack")
    print("  ok  (d) the detail paints its RTL label with its own stack")

    # (f) a panic in the detail, the list unaffected.
    p0 = result("app.presents", window=2)["presents"]
    r = raw("input.click", {"selector": "#detail-panic"}, window=2)
    print(f"  ..  (f) #detail-panic on window 2 answered {r.get('result') or r.get('error')}")
    fail2 = None
    for _ in range(30):
        fail2 = result("app.failure", window=2).get("failure")
        p1 = result("app.presents", window=2)["presents"]
        if fail2 and p1 > p0:
            break
        time.sleep(0.1)
    fail1 = result("app.failure", window=1).get("failure")
    before = value("#count")
    click("#bump", 1, "(f)")
    after = value("#count")
    msg = (fail2 or {}).get("message", "") if isinstance(fail2, dict) else str(fail2)
    print(f"  ..  (f) window 2 failure {msg[:60]!r}; window 2 presents {p0} -> {p1} (want more: the screen "
          f"is drawn); window 1 failure {fail1!r} (want None); list #bump handled, #count {before!r} -> {after!r}")
    if not fail2:
        fail("(f) window 2 reports no failure after its panic")
    if fail1:
        fail(f"(f) window 1 reports window 2's failure: {fail1!r}")
    if p1 <= p0:
        fail(f"(f) window 2 presented {p1 - p0} frames after its panic -- M36's screen never reached it")
    if int(after) != int(before) + 1:
        fail(f"(f) the list's #count {before!r} -> {after!r} after a handled click")
    # Per window: M36's R retries **the window it is pressed in**. A real
    # key through the X server, to window 2: its failure clears, and window
    # 1 -- which never failed -- is untouched. The failure must be read from
    # the slot the key arrived at; read from another, the R is posted to
    # the failed session as input and nothing retries.
    xtest_key(pid, "two-windows — detail", "r")
    for _ in range(30):
        after2 = result("app.failure", window=2).get("failure")
        if not after2:
            break
        time.sleep(0.1)
    print(f"  ..  (f) R pressed in window 2 (XTest): window 2 failure now {after2!r} (want None: retried); "
          f"window 1 failure {result('app.failure', window=1).get('failure')!r} (want None)")
    if after2:
        fail("(f) R in window 2 did not retry it -- its failure was read from another window's slot")
    print("  ok  (f) the detail's panic is shown there; the list still answers")

    # (g) three ways to close one window, and the list survives each.
    def closed(key, how):
        got = wait_keys([1], how)
        ids = wids(1)
        before = value("#count")
        click("#bump", 1, how)
        after = value("#count")
        said = value("#closed")
        print(f"  ..  (g) {how}: app.windows {got} (want [1]); window 1 getTree {len(ids)} ids, #bump handled, "
              f"#count {before!r} -> {after!r}; #closed {said!r} (want 'closed {key}')")
        if got != [1]:
            fail(f"(g) {how}: app.windows {got}, want [1]")
        if said != f"closed {key}":
            fail(f"(g) {how}: the list was told {said!r}, want 'closed {key}'")

    r = raw("app.close", window=2)
    if r.get("error") or r.get("window") != 2:
        fail(f"(g) app.close on window 2 answered {r}")
    closed(2, "app.close on window 2")
    open_row(1, 3, "(g)")
    r = raw("input.key", {"key": "Escape"}, window=3)
    if r.get("error") or r["result"].get("handled") is not True:
        fail(f"(g) Escape on window 3 answered {r} -- the setup did not land")
    try:
        closed(3, "Escape in window 3 (KeyOutcome::Quit)")
    except PortLost as e:
        fail(f"(g) after Escape in window 3 the agent port is lost ({e}) -- closing one window ended the process")
    open_row(3, 4, "(g)")
    click("#detail-close", 4, "(g)")
    closed(4, "#detail-close in window 4 (Request::Close)")
    click("#missing", 1, "(g)")
    said, now = value("#opened"), keys()
    print(f"  ..  (g) #missing: #opened {said!r} (want 'no program 9'); app.windows {now} (want [1])")
    if said != "no program 9" or now != [1]:
        fail(f"(g) OpenWindow for program 9: #opened {said!r}, app.windows {now}")
    print("  ok  (g) every close leaves window 1 answering")

    # (h) closing the primary ends the process.
    def gone():
        # A zombie has exited: the launcher, its parent, reaps it.
        try:
            return open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()[0] == "Z"
        except OSError:
            return True
    if gone():
        fail(f"(h) setup: pid {pid} reads as exited before app.close -- the exit check cannot fail")
    r = raw("app.close")
    t0 = time.monotonic()
    if r.get("error") or r.get("window") != 1:
        fail(f"(h) app.close on window 1 answered {r}")
    while not gone() and time.monotonic() - t0 < 2.0:
        time.sleep(0.002)
    ms, out = (time.monotonic() - t0) * 1000, gone()
    print(f"  ..  (h) app.close on window 1: the process alive before it (setup), exited {out} after "
          f"{ms:.1f} ms (bound 2000)")
    if not out:
        fail(f"(h) app.close on window 1 left the process running after {ms:.0f} ms")
    print("  ok  (h) closing the primary ends the process (exit status: the launcher's)")


PHASES = {"1": phase1, "2": phase2}
if phase not in PHASES:
    fail(f"no phase {phase!r}; have {sorted(PHASES)}")
try:
    PHASES[phase]()
except PortLost as e:
    fail(str(e))
print(f"windows-drive: phase {phase} passed")
