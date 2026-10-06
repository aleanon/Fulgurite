"""M37/T37.4 (docs/plan-window-reload.md): hot reload in a live window.

Over the agent, against fixtures/hot-window's copy:
  1. wait for the first hot build to replace the static application;
     read the bound label.
  2. bump the kept counter.
  3. edit the source: the label's format string, and in the same write
     `#pic`'s colour in the application's store (M84/T84.6).
  4. wait on app.reloads.
  5. the label shows the NEW format and the OLD count; `#pic` shows the
     NEW build's colour -- its store is the application's (`App::images`),
     which the window re-asks the swapped-in session for on every present
     through `Hot`'s forward (step 1 already asserted the first hot
     build's colour, so a forward answering `None` is named there).
  -. a build that fails: the window keeps running the last good build,
     shows a bar, and the fix-back clears it without a swap. While the bar
     is up, `#star` -- a paint box that fills a path -- keeps its
     ui.regionHash (M82/T82.5): the bar is drawn over a copy of the
     session's list, and a copy without the list's path store loses it.
     And `#pic` keeps its ui.regionHash (M84/T84.6): the bar's frame
     forwards the session's images, and one answering `None` would
     repaint the region in the page colour.
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

Usage: hot_window_drive.py PORT SRC PID HASH_EXE [--only-shaping] [--save F] [--gpu]
       hot_window_drive.py PORT --static HASH_EXE SAVED
`--only-shaping` waits for the first hot build and runs step 9 alone (the
`Everything` leg); `--save F` writes step 9's post-swap hash to F.
`--gpu` (M83/T83.8) is the GPU leg, built with `shader` and read back with
`xwd`: it waits for the first hot build and runs step 10 alone --
  10. edit the copy's WGSL colour (`EDIT colour`) from red to blue: after
      the swap `#swatch`'s centre is blue (the window re-sent the new
      build's table to the backend, `Backend::set_shaders` on the swap),
      the swap frame's `damaged_px` is the window's area (the full-repaint
      premise `ShaderId` rests on), and the reload count went up by one;
      then the broken edit: while its bar is up, `#swatch`'s region reads
      the same pixels as before it (the bar draws over a clone of the
      session's list, uniforms included). The same write edits `#pic`'s
      colour in the application's store (M84/T84.6): its frame stays 0, so
      the GPU atlas would hit v1's slot -- after the swap `#pic`'s centre is
      v2's only because the window calls `Backend::invalidate_images` on
      the swap (second review A14).
`--static` is the fresh launch of the edited source, a window with no hot
reload: its `#hebrew` hash must equal the `Complex` headless render's and
the post-swap hash SAVED, and differ from the `Basic` render's -- what
`Win::new` applying `Session::shaping` before its first backend buys.
"""
import base64, json, os, re, socket, struct, subprocess, sys, time, zlib

port = int(sys.argv[1])
static = sys.argv[2] == "--static"
daemon = sys.argv[2] == "--daemon"
windows = False
if daemon:
    src, pid, hash_exe = sys.argv[3], None, None
    only_shaping, save, gpu = False, None, False
elif static:
    src, pid, hash_exe, saved = None, None, sys.argv[3], sys.argv[4]
    only_shaping, save, gpu = False, None, False
else:
    src, pid, hash_exe = sys.argv[2], int(sys.argv[3]), sys.argv[4]
    rest = sys.argv[5:]
    only_shaping = "--only-shaping" in rest
    gpu = "--gpu" in rest
    windows = "--windows" in rest
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


def rpcw(method, params=None, window=None):
    """One request naming its window (M89/T89.2's `window` member)."""
    n[0] += 1
    req = {"jsonrpc": "2.0", "id": n[0], "method": method, "params": params or {}}
    if window is not None:
        req["window"] = window
    f.write((json.dumps(req) + "\n").encode())
    f.flush()
    line = f.readline()
    if not line:
        sys.exit(f"hot-window-drive: {method} on window {window}: the agent endpoint closed the connection")
    return json.loads(line)


def callw(method, params=None, window=None):
    r = rpcw(method, params, window)
    if "error" in r:
        sys.exit(f"hot-window-drive: {method} on window {window} failed: {r['error']}")
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


def edit(tag, new_line, *more):
    """Replace the line carrying `// EDIT <tag>` with `new_line` (which
    keeps the marker), and return that line's number. Further `tag,
    new_line` pairs are edited in the SAME write, so the watcher sees one
    change and the window swaps once."""
    lines = open(src).read().split("\n")
    pairs = [(tag, new_line)] + list(zip(more[::2], more[1::2]))
    first = None
    for t, nl in pairs:
        idx = next(i for i, l in enumerate(lines) if l.rstrip().endswith(f"// EDIT {t}"))
        indent = lines[idx][: len(lines[idx]) - len(lines[idx].lstrip())]
        lines[idx] = f"{indent}{nl} // EDIT {t}"
        first = idx + 1 if first is None else first
    open(src, "w").write("\n".join(lines))
    return first


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


# M84/T84.6: `#pic`'s colour in the checked-in source, and the edit's.
PIC_V1, PIC_V2 = (0x16, 0x7A, 0x3C), (0x1D, 0x4E, 0xD8)
PAGE = (0xFF, 0xFF, 0xFF)


def pic_pixel():
    """`#pic`'s centre, from a screenshot (physical pixels)."""
    b = call("ui.resolve", {"selector": "#pic"})["bounds"]
    shot = call("ui.screenshot")
    k = shot.get("scale", 1.0)
    return png_pixel(shot["image_base64"], int((b[0] + b[2]) / 2 * k), int((b[1] + b[3]) / 2 * k))


def near(a, b, tol=6):
    return max(abs(x - y) for x, y in zip(a, b)) <= tol


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
    edit("shaping", ".shaping(Shaping::Complex),")
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

# ---- 10 (M83/T83.8): a WGSL edit reaches the running window -----------
RED, BLUE, GREY = (219, 31, 41), (31, 77, 230), (0x80, 0x80, 0x80)
COLOUR_TOL = 6


def xwd_shot():
    """The window read back off the X server (`scripts/gpu_parity.sh`'s
    reader): the GPU backend cannot answer `ui.screenshot`."""
    import tempfile
    call("app.diagnostics")  # the process still answers
    with tempfile.NamedTemporaryFile(suffix=".xwd") as t:
        # A few tries: `xwd -name` can miss a window the server is still
        # mapping, and the read is of the window, not of the process.
        for k in range(5):
            if subprocess.run(["xwd", "-name", "hot-window", "-out", t.name], stderr=subprocess.DEVNULL).returncode == 0:
                break
            time.sleep(0.3)
        else:
            sys.exit("hot-window-drive [gpu]: xwd found no window named hot-window in five tries")
        d = open(t.name, "rb").read()
    h = struct.unpack(">25I", d[:100])
    hdr, w, ht, bpp, bpl, ncolors = h[0], h[4], h[5], h[11], h[12], h[19]
    px, step = d[hdr + ncolors * 12:], bpp // 8
    return w, ht, lambda x, y: (px[y * bpl + x * step + 2], px[y * bpl + x * step + 1], px[y * bpl + x * step])


def dist(a, b):
    return max(abs(x - y) for x, y in zip(a, b))


def gpu_step():
    log = open(os.environ["LUMEN_LIVE_LOG"], errors="replace").read()
    assert "lumen-static: gpu" in log, f"setup: the window is not on the GPU backend: {log.splitlines()[:4]}"
    sw = [round(v) for v in call("ui.resolve", {"selector": "#swatch"})["bounds"]]
    cx, cy = (sw[0] + sw[2]) // 2, (sw[1] + sw[3]) // 2
    # The window's first frames land after the agent answers: poll the
    # read until the swatch shows its shader, or give up after 5 s.
    for _ in range(25):
        w, h, px = xwd_shot()
        pre = px(cx, cy)
        if dist(pre, RED) <= COLOUR_TOL:
            break
        time.sleep(0.2)
    area = w * h
    print(f"hot-window-drive [gpu]: #swatch {sw}, centre {pre}: distance from red {dist(pre, RED)} "
          f"(bound {COLOUR_TOL}), from the fallback's grey {dist(pre, GREY)}; window {w}x{h} = {area} px")
    assert dist(pre, RED) <= COLOUR_TOL, "setup: before the edit #swatch is not the shader's red"
    pb = [round(v) for v in call("ui.resolve", {"selector": "#pic"})["bounds"]]
    pcx, pcy = (pb[0] + pb[2]) // 2, (pb[1] + pb[3]) // 2
    pic_pre = px(pcx, pcy)
    print(f"hot-window-drive [gpu]: #pic {pb}, centre {pic_pre}: distance from v1's {PIC_V1} {dist(pic_pre, PIC_V1)} "
          f"(bound {COLOUR_TOL})")
    assert dist(pic_pre, PIC_V1) <= COLOUR_TOL, "setup: before the edit #pic is not the store's v1 colour on the GPU"
    n = reloads()["reloads"]
    edit("colour", "let c = vec3(0.12, 0.30, 0.90);", "pic", "[0x1d, 0x4e, 0xd8, 0xff]")
    r, lat = wait(lambda r: r["reloads"] >= n + 1 and not r["building"], "the WGSL edit's reload")
    assert r["error"] is None, f"step 10: the WGSL edit did not build: {r['error']}"
    # The swap frame's own damage. The new session's agent starts a fresh
    # ring, so its oldest entry is the swap frame; `since` N sums the
    # newest `frame - N` entries, so `since: 0` is every entry and
    # `since: 1` every entry but the oldest (each request is answered in a
    # frame of its own, which both reads count alike).
    every = call("app.perf", {"since": 0})["since"]
    rest = call("app.perf", {"since": 1})["since"]
    frames = every["frames"]
    assert 0 < frames < 128 and not every["truncated"], f"setup: the new session's ring: {every}"
    swap_px = (every["damaged_px"] or 0) - (rest["damaged_px"] or 0)
    for _ in range(30):
        _, _, px = xwd_shot()
        post = px(cx, cy)
        if dist(post, BLUE) <= COLOUR_TOL:
            break
        time.sleep(0.1)
    print(f"hot-window-drive [gpu]: WGSL edit -> swapped in {lat:.2f}s; reloads {n} -> {r['reloads']} "
          f"(setup: +1); #swatch centre {post}: distance from blue {dist(post, BLUE)} (bound {COLOUR_TOL}), "
          f"from red {dist(post, RED)}")
    print(f"hot-window-drive [gpu]: the swap frame (the oldest of the new session's {frames} "
          f"entries): damaged_px {swap_px} (bound: = the window's {area} px)")
    assert r["reloads"] == n + 1, f"setup: the reload count went {n} -> {r['reloads']}, not up by one"
    assert dist(post, BLUE) <= COLOUR_TOL, ("step 10: after the swap #swatch is not the edited blue: the "
                                             "new build's shader table did not reach the backend")
    assert swap_px == area, "step 10: the swap frame was not a full repaint"
    for _ in range(30):
        _, _, px = xwd_shot()
        pic_post = px(pcx, pcy)
        if dist(pic_post, PIC_V2) <= COLOUR_TOL:
            break
        time.sleep(0.1)
    print(f"hot-window-drive [gpu]: #pic centre after the swap {pic_post}: distance from v2's {PIC_V2} "
          f"{dist(pic_post, PIC_V2)} (bound {COLOUR_TOL}), from v1's {dist(pic_post, PIC_V1)}")
    assert dist(pic_post, PIC_V2) <= COLOUR_TOL, ("step 10: after the swap #pic is not the new build's colour on the "
                                                   "GPU: the atlas kept v1's pixels for an unchanged (id, frame) -- "
                                                   "the window did not call Backend::invalidate_images on the swap")

    # The bar keeps the shader (second review A13).
    _, _, px = xwd_shot()
    before = [px(x, y) for y in range(sw[1], sw[3]) for x in range(sw[0], sw[2])]
    foot0 = px(4, h - 4)
    k = reloads()["reloads"]
    edit("handler", 'let step: i64 = "one";')
    r, _ = wait(lambda r: not r["building"] and (r["error"] is not None or r["reloads"] != k), "the broken build's error")
    assert r["reloads"] == k, f"a failed build swapped: reloads {r['reloads']}"
    for _ in range(30):
        _, _, px = xwd_shot()
        foot = px(4, h - 4)
        if is_bar(foot):
            break
        time.sleep(0.1)
    under = [px(x, y) for y in range(sw[1], sw[3]) for x in range(sw[0], sw[2])]
    differ = sum(1 for a, b in zip(before, under) if a != b)
    print(f"hot-window-drive [gpu]: the bar is up: foot pixel {foot0} -> {foot} (setup: differs, red); "
          f"#swatch under the bar: {differ} of {len(under)} px differ (bound 0), centre {px(cx, cy)}")
    assert is_bar(foot) and not is_bar(foot0), f"setup: the bar is not on screen: {foot} (before {foot0})"
    assert differ == 0, "the shader's region changed under the error bar: the bar's copy of the list lost its uniforms"
    edit("handler", "let step = 1;")
    r, _ = wait(lambda r: r["error"] is None and not r["building"], "the fix-back to clear the error")
    assert r["reloads"] == k, f"fixing back to the running source swapped anyway: {r['reloads']}"


# ---- 11 (M89/T89.7): hot reload with several windows ---------------------
def wvalue(sel, window):
    return callw("ui.resolve", {"selector": sel}, window).get("value", "")


def wkeys():
    return [w["key"] for w in call("app.windows")["windows"]]


def wclick(sel, window, what):
    r = rpcw("input.click", {"selector": sel}, window)
    if "error" in r or not r["result"].get("handled") or r.get("window") != window:
        sys.exit(f"hot-window-drive [windows]: {what}: input.click {sel} on window {window} did not land: {r!r}")
    return r


def open_panel(want, what):
    r = wclick("#open-panel", 1, what)
    print(f"hot-window-drive [windows]: {what}: #open-panel's reply opened {r.get('opened')} (want [{want}]); "
          f"app.windows {wkeys()}")
    if r.get("opened") != [want]:
        sys.exit(f"hot-window-drive [windows]: {what}: the click opened {r.get('opened')}, want [{want}]")
    time.sleep(0.6)  # the window maps and draws its first frame


def presents(window):
    p = callw("app.presents", None, window)
    return p["presents"], p["skipped"]


def windows_step():
    leg = os.environ.get("LUMEN_LIVE_STRATEGY", "?").rsplit("::", 1)[-1]
    # (setup) the copy was broken before launch, so the window's first build
    # fails and nothing swaps until the drive's fix lands.
    r, _ = wait(lambda r: not r["building"] and r["error"] is not None, "the first build's failure")
    print(f"hot-window-drive [windows {leg}]: the first build failed on purpose; app.reloads {r['reloads']} (want 0)")
    assert r["reloads"] == 0, f"setup: something swapped before the fix: reloads {r['reloads']}"

    # (1) the panel, opened BEFORE the first swap, bumped to 3; the primary to 2.
    open_panel(2, "(1) before the first swap")
    for _ in range(3):
        wclick("#panel-bump", 2, "(1) bump the panel")
    click(); click()
    p0, v0 = wvalue("#panel-count", 2), value()
    print(f"hot-window-drive [windows {leg}]: (1) at app.reloads {reloads()['reloads']}: panel {p0!r} (want 'panel 3'), "
          f"primary {v0!r} (want 'count 2')")
    assert (p0, v0) == ("panel 3", "count 2"), f"(1) setup: panel {p0!r}, primary {v0!r}"

    # (2) fix the copy and edit the panel's format in one write: the first
    # swap carries the panel's kept count into the new format.
    edit("handler", "let step = 1;", "panel", 'format!("panel v2: {}", p.kept.count)')
    r, lat = wait(lambda r: r["reloads"] >= 1 and not r["building"], "the first swap")
    assert r["error"] is None, f"(2) the first swap did not take: {r['error']}"
    for _ in range(20):
        p1 = wvalue("#panel-count", 2)
        if p1 != "panel 3":
            break
        time.sleep(0.1)
    v1 = value()
    print(f"hot-window-drive [windows {leg}]: (2) swapped in {lat:.2f}s: panel {p1!r} (want 'panel v2: 3'), "
          f"primary {v1!r} (want 'count 2')")
    if p1 == "panel 3":
        sys.exit("hot-window-drive [windows]: (2) the panel kept the OLD format after the swap: it was not swapped")
    if p1 == "panel v2: 0":
        sys.exit("hot-window-drive [windows]: (2) the panel shows the new format with count 0: its kept state was "
                 "not carried -- a window opened before the first swap had no saver")
    assert p1 == "panel v2: 3", f"(2) the panel reads {p1!r}"
    assert v1 == "count 2", f"(2) the primary's own count changed: {v1!r}"

    # (3) each session adopted into its own slot: the panel's layout is its
    # own size, and its timer redraws it and not the primary.
    root = callw("ui.getTree", {"depth": 1}, 2)["nodes"][0]["bounds"]
    shot = callw("ui.screenshot", {}, 2)
    k = shot.get("scale", 1.0)
    inner = [shot["width"] / k, shot["height"] / k]
    box = [root[2] - root[0], root[3] - root[1]]
    print(f"hot-window-drive [windows {leg}]: (3) panel root box {box} against its inner size {inner} "
          f"(the primary is 320x400)")
    assert abs(box[0] - inner[0]) <= 1 and abs(box[1] - inner[1]) <= 1, \
        f"(3) the panel lays out at {box}, not its own size {inner} -- adopted with another window's size"
    time.sleep(0.6)
    a1, a2 = presents(1), presents(2)
    time.sleep(1.2)
    b1, b2 = presents(1), presents(2)
    print(f"hot-window-drive [windows {leg}]: (3) over 1.2 s: panel presents {a2[0]} -> {b2[0]} (want growing), "
          f"primary redraws {sum(a1)} -> {sum(b1)} (want unchanged)")
    assert b2[0] - a2[0] >= 3, f"(3) the panel's timer did not redraw it: presents {a2} -> {b2}"
    assert sum(b1) == sum(a1), f"(3) the panel's timer redrew the primary: {a1} -> {b1} -- adopted with its waker"

    # (4) a panel opened AFTER the swap is the loaded build's.
    r = rpcw("app.close", None, 2)
    assert "error" not in r, f"(4) setup: app.close 2: {r}"
    time.sleep(0.3)
    assert wkeys() == [1], f"(4) setup: app.windows {wkeys()}"
    open_panel(3, "(4) after the swap")
    p3 = wvalue("#panel-count", 3)
    print(f"hot-window-drive [windows {leg}]: (4) the late panel reads {p3!r} (want 'panel v2: 0', the loaded build's)")
    if p3 == "panel 0":
        sys.exit("hot-window-drive [windows]: (4) the late panel shows v1's format: it was opened through the "
                 "build the window started with, not the one loaded (rule 3)")
    assert p3 == "panel v2: 0", f"(4) the late panel reads {p3!r}"

    # (5) v3: the primary's format and a panic in the panel's boot, one
    # write. The swap must refuse whole: both windows stay on v2.
    n0 = reloads()["reloads"]
    edit("format", '#[bind(ui.value, count, "v3 count {}")]', "panel-boot", 'panic!("the panel\'s boot panicked in v3");')
    r, _ = wait(lambda r: not r["building"] and (r["error"] is not None or r["reloads"] != n0), "v3's outcome")
    time.sleep(0.4)
    v5, p5 = value(), wvalue("#panel-count", 3)
    err = r["error"] or ""
    print(f"hot-window-drive [windows {leg}]: (5) v3: app.reloads {n0} -> {r['reloads']} (want unchanged); "
          f"primary {v5!r} (want v2's 'count 2'); panel {p5!r} (want 'panel v2: 0'); error {err[:100]!r}")
    if v5 == "v3 count 2":
        sys.exit(f"hot-window-drive [windows]: (5) the primary runs v3 ({v5!r}) while the panel runs v2 ({p5!r}): "
                 f"sessions were replaced as each one booted")
    assert r["reloads"] == n0, f"(5) a swap whose panel boot panicked went through: reloads {r['reloads']}"
    assert v5 == "count 2" and p5 == "panel v2: 0", f"(5) primary {v5!r}, panel {p5!r}"
    assert "window program 1" in err and "panicked in v3" in err, f"(5) the error does not name the panel's boot: {err[:300]}"
    bar = foot_pixel()
    print(f"hot-window-drive [windows {leg}]: (5) the primary's foot {bar} (want the bar's red)")
    assert is_bar(bar), f"(5) the bar is not up: {bar}"



# ---- 12 (M89/T89.7): a daemon with no window swaps on the build ----------
def daemon_reloads(want, what):
    """Poll `app.reloads` on window 0 -- `Win` answers it and runs no frame,
    so the poll cannot take the step itself -- until `want` swaps, a build
    error, or a landed build left pending for 3 s."""
    t0, last, pend = time.time(), None, None
    while time.time() - t0 < 600:
        r = callw("app.reloads", None, 0)
        last = r
        if r["reloads"] >= want or (not r["building"] and r["error"] is not None):
            break
        if r["pending"] and not r["building"]:
            pend = pend or time.time()
            if time.time() - pend > 3:
                break
        else:
            pend = None
        time.sleep(0.2)
    print(f"hot-window-drive [daemon]: {what}: app.reloads on window 0 {last['reloads']} (want {want}), "
          f"building {last['building']}, pending {last['pending']}, error {last['error']!r}, "
          f"after {time.time() - t0:.1f}s")
    if last["reloads"] < want and last["pending"]:
        sys.exit("hot-window-drive [daemon]: the build landed and is pending, and app.reloads on window 0 stays "
                 f"{last['reloads']}: nothing scheduled the windowless step the swap is taken at")
    assert last["error"] is None and last["reloads"] >= want, f"the daemon's swap: {last}"


if daemon:
    keys = [w["key"] for w in call("app.windows")["windows"]]
    print(f"hot-window-drive [daemon]: app.windows {keys} (want [0]: no window open)")
    assert keys == [0], f"setup: the daemon has windows open: {keys}"
    # The first build may land before or after the launcher's identity
    # check, which is an agent call on window 0 and so a step of its own;
    # wait for it, then make a build land with nothing else happening.
    daemon_reloads(1, "the first build")
    time.sleep(0.5)
    with open(src, "a") as fh:
        fh.write("\n// touched by hot-window-drive's daemon leg\n")
    daemon_reloads(2, "a build landed with no window, no timer and no agent call")
    keys = [w["key"] for w in call("app.windows")["windows"]]
    assert keys == [0], f"the daemon opened a window: {keys}"
    print("hot-window-drive: PASS")
    sys.exit(0)

if gpu:
    r, took = wait(lambda r: r["supported"] and r["reloads"] >= 1 and not r["building"], "the first hot build")
    assert r["error"] is None, f"the first hot build did not take: {r['error']}"
    print(f"hot-window-drive [gpu]: first hot build swapped in after {took:.1f}s")
    gpu_step()
    print("hot-window-drive: PASS")
    sys.exit(0)

if windows:
    windows_step()
    print("hot-window-drive: PASS")
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
# M84/T84.6: the hot build's own store reaches the renderer through the
# frame -- through `Hot`'s forward of `FrameSource::images`.
pic1 = pic_pixel()
print(f"hot-window-drive: #pic after the first hot build: {pic1} (want the store's {PIC_V1}; the page is {PAGE})")
assert not near(pic1, PAGE), ("step 1: #pic shows the page colour: the swapped-in session's images did not reach "
                              "the renderer (Hot's FrameSource::images forward)")
assert near(pic1, PIC_V1), f"step 1: #pic is {pic1}, not the hot build's store colour {PIC_V1}"

# ---- 2. bump the kept counter -----------------------------------------
click(); click()
v = value()
assert v == "count 2", f"step 2: the clicks did not land: `{v}`"

# ---- 3-5. edit the format; the new text shows the old count -----------
edit("format", '#[bind(ui.value, count, "clicks: {}")]', "pic", "[0x1d, 0x4e, 0xd8, 0xff]")
r, lat1 = wait(lambda r: r["reloads"] >= 2 and not r["building"], "the format edit's reload")
v = value()
for _ in range(20):
    pic2 = pic_pixel()
    if near(pic2, PIC_V2):
        break
    time.sleep(0.1)
print(f"hot-window-drive: format edit -> swapped in {lat1:.2f}s; label `{v}`; #pic {pic2} "
      f"(want v2's {PIC_V2}, not v1's {PIC_V1} or the page {PAGE})")
assert r["error"] is None, f"step 4: {r['error']}"
assert v == "clicks: 2", f"step 5: the label must show the new format and the old count, not `{v}`"
assert not near(pic2, PAGE), "step 5: #pic shows the page colour after the swap: the new session's images were not forwarded"
assert near(pic2, PIC_V2), f"step 5: #pic is {pic2} after the swap, not the new build's store colour {PIC_V2}"

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
# M84/T84.6: the application-store image's region, before the bar.
pic_rect = call("ui.resolve", {"selector": "#pic"})["bounds"]
pic_before = live_hash(pic_rect)
assert pic_before["pixels"] > 0, f"setup: #pic's region hashed no pixels: {pic_rect}"
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
pic_under = live_hash(pic_rect)
print(f"hot-window-drive: #pic {pic_rect} under the bar: hash {pic_under['hash']} over {pic_under['pixels']} px "
      f"(before the failed build {pic_before['hash']}); centre {pic_pixel()}")
assert pic_under["hash"] == pic_before["hash"], \
    "#pic's region changed under the error bar: the bar's frame did not forward the session's images (Barred::images)"

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
