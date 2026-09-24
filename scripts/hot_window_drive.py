"""M37/T37.4 (docs/plan-window-reload.md): hot reload in a live window.

Over the agent, against fixtures/hot-window's copy:
  1. wait for the first hot build to replace the static application;
     read the bound label.
  2. bump the kept counter.
  3. edit the source: the label's format string.
  4. wait on app.reloads.
  5. the label shows the NEW format and the OLD count.
  -. a build that fails: the window keeps running the last good build,
     shows a bar, and the fix-back clears it without a swap.
  6. introduce a panic: M36's screen through app.failure, at the line.
  7. fix it: the next swap clears the screen, and the count survived.
Prints reload latency (edit -> swapped) and resident memory per reload.
Usage: hot_window_drive.py PORT SRC PID
"""
import base64, json, re, socket, struct, sys, time, zlib

port, src, pid = int(sys.argv[1]), sys.argv[2], int(sys.argv[3])


def connect():
    for _ in range(100):
        try:
            return socket.create_connection(("127.0.0.1", port), timeout=30)
        except OSError:
            time.sleep(0.1)
    sys.exit("hot-window-drive: no agent endpoint")


s = connect()
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
        sys.exit(f"hot-window-drive: {method} failed: {r['error']}")
    return r["result"]


def value():
    return call("ui.resolve", {"selector": "#value"}).get("value", "")


def reloads():
    return call("app.reloads")


def wait(pred, what, timeout=600):
    t0 = time.time()
    while time.time() - t0 < timeout:
        r = reloads()
        if pred(r):
            return r, time.time() - t0
        time.sleep(0.2)
    sys.exit(f"hot-window-drive: timed out waiting for {what}: {reloads()!r}")


def click():
    call("input.click", {"selector": "#inc"})


def rss_kb():
    for line in open(f"/proc/{pid}/status"):
        if line.startswith("VmRSS:"):
            return int(line.split()[1])
    return 0


def edit(tag, new_line):
    """Replace the line carrying `// EDIT <tag>` with `new_line` (which
    keeps the marker), and return that line's number."""
    lines = open(src).read().split("\n")
    idx = next(i for i, l in enumerate(lines) if l.rstrip().endswith(f"// EDIT {tag}"))
    indent = lines[idx][: len(lines[idx]) - len(lines[idx].lstrip())]
    lines[idx] = f"{indent}{new_line} // EDIT {tag}"
    open(src, "w").write("\n".join(lines))
    return idx + 1


def png_pixel(b64, x, y):
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


def foot_pixel():
    shot = call("ui.screenshot")
    return png_pixel(shot["image_base64"], 4, shot["height"] - 4)


def is_bar(px):
    return px[0] > px[1] + 40 and px[0] > px[2] + 40


# ---- 1. the first hot build replaces the static application ----------
r, took = wait(lambda r: r["supported"] and r["reloads"] >= 1 and not r["building"], "the first hot build")
assert r["error"] is None, f"the first hot build did not take: {r['error']}"
print(f"hot-window-drive: first hot build swapped in after {took:.1f}s")
rss0 = rss_kb()
v = value()
assert v == "count 0", f"step 1: `{v}`"

# ---- 2. bump the kept counter -----------------------------------------
click(); click()
v = value()
assert v == "count 2", f"step 2: the clicks did not land: `{v}`"

# ---- 3-5. edit the format; the new text shows the old count -----------
edit("format", '#[bind(ui.value, count, "clicks: {}")]')
r, lat1 = wait(lambda r: r["reloads"] >= 2 and not r["building"], "the format edit's reload")
v = value()
print(f"hot-window-drive: format edit -> swapped in {lat1:.2f}s; label `{v}`")
assert r["error"] is None, f"step 4: {r['error']}"
assert v == "clicks: 2", f"step 5: the label must show the new format and the old count, not `{v}`"

# ---- a build that fails ----------------------------------------------
before_bar = foot_pixel()
edit("handler", 'let step: i64 = "one";')
# Stop on the error OR on any swap: a failed build that swaps clears the
# error it should have kept, and waiting only for the error would turn
# that defect into a ten-minute timeout instead of a named failure.
r, _ = wait(lambda r: not r["building"] and (r["error"] is not None or r["reloads"] != 2), "the broken build's error")
assert r["reloads"] == 2, f"a failed build swapped: reloads {r['reloads']}"
assert "mismatched types" in r["error"] or "E0308" in r["error"], f"the error bar's log: {r['error'][:200]}"
v = value()
assert v == "clicks: 2", f"the window must keep running the last good build: `{v}`"
click()
v = value()
assert v == "clicks: 3", f"the last good build still answers input: `{v}`"
bar = foot_pixel()
assert is_bar(bar) and not is_bar(before_bar), f"the bar must be on screen: {bar} (before {before_bar})"
print(f"hot-window-drive: a failed build kept the running library; bar {bar}, label `{v}`")
edit("handler", "let step = 1;")
r, _ = wait(lambda r: r["error"] is None and not r["building"], "the fix-back to clear the error")
assert r["reloads"] == 2, f"fixing back to the running source swapped anyway: {r['reloads']}"
# The window redraws on the outcome; give that frame a moment to land.
for _ in range(20):
    after_fix = foot_pixel()
    if not is_bar(after_fix):
        break
    time.sleep(0.1)
assert not is_bar(after_fix), f"the bar outlived the fix: {after_fix}"

# ---- 6. a panic, through a reload --------------------------------------
line = edit("handler", 'let step = 1; if t.count >= 0 { panic!("introduced by the drive") }')
r, lat2 = wait(lambda r: r["reloads"] >= 3 and not r["building"], "the panic edit's reload")
click()
fail = call("app.failure")["failure"]
assert fail is not None, "step 6: the click did not panic"
loc = fail["location"]
print(f"hot-window-drive: panic at {loc['file']}:{loc['line']} ({fail['message']})")
assert loc["file"].endswith("src/lib.rs") and loc["line"] == line, f"step 6: the report's location {loc}, expected line {line}"
assert "introduced by the drive" in fail["message"]

# ---- 7. fix it: the swap clears the screen; the count survived ---------
edit("handler", "let step = 1;")
r, lat3 = wait(lambda r: r["reloads"] >= 4 and not r["building"], "the fix's reload")
assert call("app.failure")["failure"] is None, "step 7: the swap did not clear the panic screen"
v = value()
assert v == "clicks: 3", f"step 7: the kept count must survive the panic and the swap: `{v}`"
click()
assert value() == "clicks: 4"
rss1 = rss_kb()
print(f"hot-window-drive: latency edit->swap {lat1:.2f}s, {lat2:.2f}s, {lat3:.2f}s; "
      f"RSS {rss0} -> {rss1} kB over 3 reloads ({(rss1 - rss0) / 3:.0f} kB/reload)")
print("hot-window-drive: PASS")
