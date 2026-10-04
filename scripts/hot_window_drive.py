"""M37/T37.4 (docs/plan-window-reload.md): hot reload in a live window.

Over the agent, against fixtures/hot-window's copy:
  1. wait for the first hot build to replace the static application;
     read the bound label.
  2. bump the kept counter.
  3. edit the source: the label's format string.
  4. wait on app.reloads.
  5. the label shows the NEW format and the OLD count.
  -. a build that fails: the window keeps running the last good build,
     shows a bar, and the fix-back clears it without a swap. While the bar
     is up, `#star` -- a paint box that fills a path -- keeps its
     ui.regionHash (M82/T82.5): the bar is drawn over a copy of the
     session's list, and a copy without the list's path store loses it.
  6. introduce a panic: M36's screen through app.failure, at the line.
  7. fix it: the next swap clears the screen, and the count survived.
  8. focus the field: app.imeArea is non-null and inside its physical box,
     answered by the swapped-in build through `Hot`'s forward (M80/T80.5).
  9. M81/T81.4: edit the program to `.shaping(Shaping::Complex)`; after the
     swap `#hebrew`'s ui.regionHash equals the headless `Complex` render's
     (`hot-window --hash`) and differs from the pre-swap hash, which is the
     `Basic` render's. The new session answers `Session::shaping` Complex
     through `Hot`'s forward and the window calls `Backend::set_font`.
Prints reload latency (edit -> swapped) and resident memory per reload.

Usage: hot_window_drive.py PORT SRC PID HASH_EXE [--only-shaping] [--save F]
       hot_window_drive.py PORT --static HASH_EXE SAVED
`--only-shaping` waits for the first hot build and runs step 9 alone (the
`Everything` leg); `--save F` writes step 9's post-swap hash to F.
`--static` is the fresh launch of the edited source, a window with no hot
reload: its `#hebrew` hash must equal the `Complex` headless render's and
the post-swap hash SAVED, and differ from the `Basic` render's -- what
`Win::new` applying `Session::shaping` before its first backend buys.
"""
import base64, json, os, re, socket, struct, subprocess, sys, time, zlib

port = int(sys.argv[1])
static = sys.argv[2] == "--static"
if static:
    src, pid, hash_exe, saved = None, None, sys.argv[3], sys.argv[4]
    only_shaping, save = False, None
else:
    src, pid, hash_exe = sys.argv[2], int(sys.argv[3]), sys.argv[4]
    rest = sys.argv[5:]
    only_shaping = "--only-shaping" in rest
    save = rest[rest.index("--save") + 1] if "--save" in rest else None


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


# ---- 9 (M81/T81.4): the shaping a swap brings, and the window following it
W = 320  # fixtures/hot-window's `program().size(320, 160)`


def band():
    """`#hebrew`'s rows across the window: one rect for both modes, whose
    label boxes differ in width."""
    b = call("ui.resolve", {"selector": "#hebrew"})["bounds"]
    return [0, b[1], W, b[3]]


def live_hash(rect):
    return call("ui.regionHash", {"rect": rect})


def headless(mode, rect):
    out = subprocess.run([hash_exe, "--hash", mode, *map(str, rect)], check=True, capture_output=True, text=True).stdout
    resolve, region, info = (json.loads(l) for l in out.strip().split("\n")[-3:])
    return region["result"], resolve["result"]["bounds"], info["shaping"]


def scale_one():
    k = call("ui.screenshot").get("scale", 1.0)
    assert k == 1.0, (f"setup: the window's scale is {k}; the headless renders this compares with are at 1 "
                      f"(WINIT_X11_SCALE_FACTOR=1 is set by the script; a Wayland session ignores it)")


def shaping_step(leg):
    scale_one()
    rect = band()
    pre = live_hash(rect)
    hb, bb, mb = headless("basic", rect)
    hc, bc, mc = headless("complex", rect)
    print(f"hot-window-drive [{leg}]: #hebrew band {rect}: pre-swap {pre['hash']} ({pre['pixels']} px); "
          f"headless {mb} {hb['hash']}, {mc} {hc['hash']}")
    assert pre["pixels"] > 0, "setup: the band hashed no pixels"
    assert (mb, mc) == ("Basic", "Complex"), f"setup: the headless renders' modes are {mb}, {mc}"
    assert hb["hash"] != hc["hash"], "setup: Basic and Complex draw #hebrew alike, so nothing here could fail"
    assert pre["hash"] == hb["hash"], "setup: before the edit the window does not draw the Basic program's frame"
    n = reloads()["reloads"]
    edit("shaping", ".shaping(Shaping::Complex)")
    r, lat = wait(lambda r: r["reloads"] >= n + 1 and not r["building"], "the shaping edit's reload")
    assert r["error"] is None, f"step 9: the shaping edit did not build: {r['error']}"
    for _ in range(20):
        post = live_hash(rect)
        if post["hash"] == hc["hash"]:
            break
        time.sleep(0.1)
    after = band()
    print(f"hot-window-drive [{leg}]: shaping edit -> swapped in {lat:.2f}s; post-swap {post['hash']} "
          f"(bound: = Complex {hc['hash']}, != pre-swap {pre['hash']}); band after {after}")
    assert after[1] == rect[1] and after[3] == rect[3], f"step 9: #hebrew moved rows: {rect} -> {after}"
    assert post["hash"] != pre["hash"], "step 9: the swap left #hebrew's pixels as they were"
    assert post["hash"] == hc["hash"], ("step 9: after the swap the window does not draw the Complex frame "
                                        "(a mix of a Complex measure and a Basic paint matches neither render)")
    if save:
        open(save, "w").write(post["hash"])
    return post


if static:
    # ---- the fresh launch of the edited source: a window with no reload --
    scale_one()
    rect = band()
    live = live_hash(rect)
    hb, _, mb = headless("basic", rect)
    hc, _, mc = headless("complex", rect)
    want = open(saved).read().strip()
    print(f"hot-window-drive [static]: #hebrew band {rect}: live {live['hash']} ({live['pixels']} px); "
          f"headless {mb} {hb['hash']}, {mc} {hc['hash']}; hot leg's post-swap {want}")
    assert live["pixels"] > 0 and hb["hash"] != hc["hash"], "setup: nothing here could fail"
    assert live["hash"] != hb["hash"], "a static Complex window draws the Basic frame: Win::new ignored Session::shaping"
    assert live["hash"] == hc["hash"], "a static Complex window does not draw the Complex headless frame"
    assert live["hash"] == want, "the fresh launch of the edited source differs from the swapped-in frame"
    print("hot-window-drive [static]: PASS")
    sys.exit(0)

# ---- 1. the first hot build replaces the static application ----------
r, took = wait(lambda r: r["supported"] and r["reloads"] >= 1 and not r["building"], "the first hot build")
assert r["error"] is None, f"the first hot build did not take: {r['error']}"
print(f"hot-window-drive: first hot build swapped in after {took:.1f}s")
if only_shaping:
    shaping_step(os.environ.get("LUMEN_LIVE_STRATEGY", "?"))
    print("hot-window-drive: PASS")
    sys.exit(0)
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
# M82/T82.5: the star's region, painted by a path, before the bar.
star_rect = call("ui.resolve", {"selector": "#star"})["bounds"]
shot = call("ui.screenshot")
k = shot.get("scale", 1.0)
mid = png_pixel(shot["image_base64"], int((star_rect[0] + star_rect[2]) / 2 * k), int((star_rect[1] + star_rect[3]) / 2 * k))
assert all(abs(mid[i] - (0xC2, 0x41, 0x0C)[i]) < 24 for i in range(3)), \
    f"setup: #star's centre must be the path's colour, not {mid}"
star_before = live_hash(star_rect)
assert star_before["pixels"] > 0, f"setup: #star's region hashed no pixels: {star_rect}" 
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
star_under = live_hash(star_rect)
print(f"hot-window-drive: #star {star_rect} under the bar: hash {star_under['hash']} over {star_under['pixels']} px "
      f"(before the failed build {star_before['hash']})")
assert star_under["hash"] == star_before["hash"], \
    "the path's region changed under the error bar: the bar's copy of the list lost its paths"

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

# ---- 8. the IME area, from the swapped-in build (M80/T80.5) -------------
# `Session::caret_rect` reaches the library's session through the reload
# crate's `Hot` forward: the compiler refuses a `Hot` with no forward, so
# the runtime question is whether the forward answers. Focus the field;
# the window must have passed an area inside the field's physical box.
r = rpc("input.click", {"selector": "#field"})
assert "error" not in r and r["result"]["handled"], f"step 8: the press on #field did not land: {r!r}"
focused = call("ui.getAppliedStyles", {"selector": "#field"})["state"]["focused"]
assert focused, "step 8: #field is not focused after the press"
fb = call("ui.resolve", {"selector": "#field"})
k = call("ui.screenshot").get("scale", 1.0)
box = [v * k for v in fb["bounds"]]
area = call("app.imeArea")["area"]
print(f"hot-window-drive: after {reloads()['reloads']} swaps, #field focused: app.imeArea {area}, "
      f"field's physical box {[round(v, 1) for v in box]} (scale {k})")
assert area is not None, "step 8: app.imeArea is null after focusing a field in the swapped-in build"
inside = box[0] - 1 <= area[0] and area[2] <= box[2] + 1 and box[1] - 1 <= area[1] and area[3] <= box[3] + 1
assert inside, f"step 8: the IME area {area} is outside the field's physical box {box}"
rss1 = rss_kb()
print(f"hot-window-drive: latency edit->swap {lat1:.2f}s, {lat2:.2f}s, {lat3:.2f}s; "
      f"RSS {rss0} -> {rss1} kB over 3 reloads ({(rss1 - rss0) / 3:.0f} kB/reload)")

# ---- 9. the shaping a swap brings (M81/T81.4) --------------------------
shaping_step(os.environ.get("LUMEN_LIVE_STRATEGY", "?"))
print("hot-window-drive: PASS")
