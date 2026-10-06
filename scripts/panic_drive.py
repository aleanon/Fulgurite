"""M36/T36.3 (docs/plan-panic-recovery.md): the panic screen, live.

For each of panic-app's ten sites, over the agent endpoint:
  1. arm the site (and press #go where the site needs a click, or focus
     #fuse for site 8, whose panic is the window asking the focused
     field's caret for its IME area -- M80/T80.5; site 9's is the window
     asking App::wake_in_ms for its next deadline after the arming
     click's own frame -- M84/T84.3; site 10's is App::images, which the
     arming click's own frame asks inside its guarded entry before the
     present reads the store -- M84/T84.6);
  2. wait for app.failure, and assert its file and line are the site's;
  3. check a screenshot pixel is the panic screen's page, not the app's;
  4. assert input.* is refused, and app.imeArea is null, while the screen
     is up;
  5. app.retry, and assert the application answers again: no failure,
     the tree readable, the app's own page back on screen.
Usage: panic_drive.py PORT STRATEGY
"""
import base64, json, re, socket, struct, sys, time, zlib

port, strategy = int(sys.argv[1]), sys.argv[2]
s = socket.create_connection(("127.0.0.1", port), timeout=15)
f = s.makefile("rwb")
n = [0]


def rpc(method, params=None):
    n[0] += 1
    f.write((json.dumps({"jsonrpc": "2.0", "id": n[0], "method": method, "params": params or {}}) + "\n").encode())
    f.flush()
    return json.loads(f.readline())


def call(method, params=None):
    r = rpc(method, params)
    if "error" in r:
        sys.exit(f"panic-drive [{strategy}]: {method} failed: {r['error']}")
    return r["result"]


def png_pixel(b64, x, y):
    """A minimal PNG decoder: 8-bit RGB or RGBA, filters 0-4."""
    data = base64.b64decode(b64)
    pos, idat, w = 8, b"", 0
    while pos < len(data):
        ln, = struct.unpack(">I", data[pos:pos + 4]); kind = data[pos + 4:pos + 8]
        body = data[pos + 8:pos + 8 + ln]; pos += 12 + ln
        if kind == b"IHDR":
            w, h, depth, ctype = struct.unpack(">IIBB", body[:10])
            bpp = {2: 3, 6: 4}[ctype]
        elif kind == b"IDAT":
            idat += body
    raw, stride = zlib.decompress(idat), w * bpp
    prev, out = bytearray(stride), None
    for row in range(y + 1):
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
        prev, out = line, line
    return tuple(out[x * bpp:x * bpp + 3])


def corner():
    return png_pixel(call("ui.screenshot")["image_base64"], 2, 2)


# The site lines, from the fixture's own `// SITE n` markers.
src = open("fixtures/panic-app/src/lib.rs").read().splitlines()
line_of = {int(m.group(1)): i + 1 for i, l in enumerate(src) for m in [re.search(r"// SITE (\d+)$", l.rstrip())] if m}
SITES = sorted(line_of)
assert SITES == list(range(1, 11)), f"panic-drive: the fixture's SITE markers are {SITES}, not 1..10"
print(f"  ..  {len(SITES)} sites in fixtures/panic-app/src/lib.rs (ten since M84/T84.6's App::images)")

PANIC_PAGE = (0x1b, 0x1f, 0x2a)
first = call("app.failure")
assert first.get("supported") is True, f"panic-drive: recovery is not compiled in: {first!r}"
assert first["failure"] is None, f"panic-drive: a failure before anything was armed: {first!r}"
app_page = corner()

# Site 8's setup, before it is armed: the window places an IME area for
# the focused field at all, so "null while the screen is up" below is the
# screen's doing and not a window that never places one (M80/T80.5).
call("input.click", {"selector": "#fuse"})
ime = call("app.imeArea")["area"]
assert ime is not None, f"panic-drive [{strategy}]: setup: focusing #fuse placed no IME area"
print(f"  ..  site 8 setup: #fuse focused, app.imeArea {ime}")

for site in SITES:
    call("input.click", {"selector": f"#arm{site}"})
    if site in (1, 2, 7):
        r = rpc("input.click", {"selector": "#go"})
    if site == 8:
        # A press on the field: the frame focuses it and the window asks
        # its caret after the frame. The press itself must land.
        r = rpc("input.click", {"selector": "#fuse"})
    deadline, fail = time.time() + 5, None
    while time.time() < deadline:
        fail = call("app.failure")["failure"]
        if fail:
            break
        time.sleep(0.05)
    assert fail, f"panic-drive [{strategy}]: site {site} never reported a failure"
    loc = fail["location"]
    if site == 7:
        assert loc["file"].startswith("crates/lumen-static/src/"), f"site 7 is in the framework: {loc}"
        # A --release build has no debug info, so the backtrace has no
        # lines: the screen must then *say* so rather than show nothing
        # (plan §2.1). With debug info, the frame is the fixture's line.
        if fail["app_frame"] is None:
            assert fail["note"] and "debug info" in fail["note"], f"site 7: no app frame and no note: {fail!r}"
        else:
            want = ("fixtures/panic-app/src/lib.rs", line_of[7])
            got = (fail["app_frame"]["file"], fail["app_frame"]["line"])
            assert got == want, f"site 7: app frame {got} != {want}"
    else:
        got, want = (loc["file"], loc["line"]), ("fixtures/panic-app/src/lib.rs", line_of[site])
        assert got == want, f"site {site}: location {got} != {want}"
    time.sleep(0.2)
    px = corner()
    assert px == PANIC_PAGE, f"site {site}: the corner is {px}, not the panic screen's {PANIC_PAGE}"
    refused = rpc("input.click", {"selector": "#go"})
    assert "error" in refused, f"site {site}: input was accepted while the screen was up: {refused!r}"
    ime = call("app.imeArea")["area"]
    assert ime is None, f"site {site}: app.imeArea is {ime} while the panic screen is up"
    assert call("app.retry")["retried"] is True
    time.sleep(0.2)
    after = call("app.failure")["failure"]
    assert after is None, f"site {site}: still failed after retry: {after!r}"
    assert "nodes" in call("ui.getTree"), f"site {site}: the tree did not answer after retry"
    px = corner()
    assert px == app_page, f"site {site}: after retry the corner is {px}, not the app's {app_page}"
    print(f"  ok  site {site}: {loc['file']}:{loc['line']}, screen drawn, input refused, retried")

# ---- M47/T47.1: a retry does not credit the timers with the screen's time
# While the screen is up the timers' clock is frozen (M36 §2.2). Until
# T47.1 the window kept the last frame's instant, so the retry's first
# frame ticked by the whole time the screen was up and a 120 ms timer
# fired ~16 times at once. The count is read unobserved-style: one read
# before the wait, one after, and the time between them printed.
def ticks():
    return int(call("ui.resolve", {"selector": "#ticks"}).get("value", "0") or 0)


def loop_wakes():
    """The window's event-loop thread's context switches, from /proc.

    M47/T47.8: nothing the agent reports counts frames while a panic is
    shown (the journal is not fed on those frames), so "no deadline while
    the screen is up" is counted where the loop sleeps: every wake of the
    main thread is a voluntary switch. A deadline armed at the timer's
    rate would add about 8 a second.
    """
    import os
    pid = os.environ["LUMEN_LIVE_PID"]
    st = open(f"/proc/{pid}/task/{pid}/status").read()
    return sum(int(l.split()[1]) for l in st.splitlines()
               if l.startswith(("voluntary_ctxt_switches", "nonvoluntary_ctxt_switches")))


PERIOD, UP_S = 120, 2.0
call("input.click", {"selector": "#timer"})
time.sleep(0.5)
assert ticks() >= 2, f"panic-drive [{strategy}]: setup: the timer is not ticking ({ticks()})"
call("input.click", {"selector": "#arm1"})
rpc("input.click", {"selector": "#go"})
deadline = time.time() + 5
while time.time() < deadline and not call("app.failure")["failure"]:
    time.sleep(0.05)
assert call("app.failure")["failure"], f"panic-drive [{strategy}]: timer step: site 1 never failed"
t_fail = ticks()
time.sleep(0.2)  # the read's own frame, and anything it set off
w0 = loop_wakes()
time.sleep(UP_S)
woke = loop_wakes() - w0
WAKE_BOUND = 4  # a 120 ms deadline would give ~16 in 2 s
print(f"  ..  panic screen up {UP_S * 1000:.0f} ms with a {PERIOD} ms timer: the loop woke {woke} "
      f"times (bound {WAKE_BOUND}; ~{int(UP_S * 1000 // PERIOD)} if a deadline were armed)")
assert woke <= WAKE_BOUND, (f"panic-drive [{strategy}]: the loop woke {woke} times in {UP_S:.0f} s "
                            f"with the panic screen up (bound {WAKE_BOUND}): a deadline is armed "
                            f"while the timers are frozen")
frozen = ticks()
assert frozen == t_fail, f"panic-drive [{strategy}]: the timer ran while the screen was up ({t_fail} -> {frozen})"
assert call("app.retry")["retried"] is True
t0 = time.time()
time.sleep(0.25)
after = ticks()
x = (time.time() - t0) * 1000
added = after - frozen - int(x // PERIOD)
print(f"  ..  retry after {UP_S * 1000:.0f} ms up: ticks {frozen} -> {after} in {x:.0f} ms, "
      f"{added} beyond the {int(x // PERIOD)} due (bound 2)")
assert added <= 2, (f"panic-drive [{strategy}]: the retry's first frame fired {added} extra ticks "
                    f"(bound 2): it was handed the time the panic screen was up")
call("input.click", {"selector": "#timer"})
print(f"  ok  retry: the timer resumed from dt 0 ({added} extra, bound 2)")
print(f"panic-drive [{strategy}]: all {len(SITES)} sites caught, shown and recovered")
