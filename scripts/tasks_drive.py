"""M28/T28.12's assertions. See scripts/tasks_drive.sh."""
import base64, hashlib, io, json, re, socket, sys, time
import os, subprocess  # noqa: E401


def xclip_preflight(leg):
    """M88/T88.4 (owner decision D7): the primary leg's prerequisite.

    `xclip` is the independent reader of the primary selection, and a
    prerequisite rather than a capability (docs/plan-iced-roadmap.md
    §3.4): `scripts/probe.sh xclip` exits 1 with the install line when it
    is absent, and this propagates that status -- never 77, which is the
    drive's code for a missing display only (M43 §2)."""
    probe = os.path.join(os.path.dirname(os.path.abspath(__file__)), "probe.sh")
    r = subprocess.run([probe, "xclip"], capture_output=True, text=True)
    print(f"  ..  {(r.stdout + r.stderr).strip()} (exit {r.returncode})")
    if r.returncode != 0:
        print(f"tasks-drive[{leg}]: the primary leg needs xclip (D7) -- failing, not skipping")
        sys.exit(r.returncode)


# `tasks_drive.sh` runs the preflight alone with xclip off PATH -- the
# refuse direction, in the same session as every leg's accept direction.
if sys.argv[1:2] == ["--xclip-preflight"]:
    xclip_preflight("preflight")
    sys.exit(0)

port, strategy = int(sys.argv[1]), sys.argv[2]
# M88/T88.2: `gpu` runs only the phases that read pixels, on a window
# drawn by wgpu; every other phase is backend-independent and runs on
# the software legs.
# M88/T88.5: `image` runs only the clipboard-image phase, on a window
# built with tasks-demo's `clipboard-image` feature.
backend = sys.argv[3] if len(sys.argv) > 3 else "software"
leg = f"{strategy}+{backend}" if backend in ("gpu", "image") else strategy

if backend == "gpu":
    # Setup landed: the window really is on the GPU. No adapter is a
    # supported configuration and a skip, as `just gpu-parity` has it.
    gpu_log = open("target/live/tasks-drive-gpu.log", errors="replace").read()
    started = [l for l in gpu_log.splitlines() if l.startswith("lumen-static: ") and "(" in l]
    print(f"  ..  startup line {started[:1]} (must name 'gpu (')")
    if not any(l.startswith("lumen-static: gpu (") for l in started):
        print("tasks-drive[gpu]: no adapter on this machine -- skipped (77)")
        sys.exit(77)

# M39/T39.1 (R14-F1): a browser's no-cors POST, byte for byte, spawns
# nothing -- run before this script's own connection opens, and with the
# window's pid checked against the port. Its second JSON line is a click
# on `#spawn-one`; step 1 below is the ordinary click that must move it.
here = os.path.dirname(os.path.abspath(__file__))
replay = [sys.executable, os.path.join(here, "browser_replay.py"), f"127.0.0.1:{port}",
          os.path.join(here, "..", "crates/lumen-static-agent/tests/browser/firefox156-fetch-no-cors.bin"),
          "#counts"]
if os.environ.get("LUMEN_TASKS_PID"):
    replay += ["--pid", os.environ["LUMEN_TASKS_PID"]]
if backend == "software" and subprocess.call(replay) != 0:
    sys.exit(f"tasks-drive[{strategy}]: a browser's request reached the application")

s = socket.create_connection(("127.0.0.1", port), timeout=15)
f = s.makefile("rwb")
n = [0]


def call(method, params=None):
    n[0] += 1
    f.write((json.dumps({"jsonrpc": "2.0", "id": n[0],
                         "method": method, "params": params or {}}) + "\n").encode())
    f.flush()
    r = json.loads(f.readline())
    if "error" in r:
        sys.exit(f"tasks-drive[{leg}]: {method} failed: {r['error']}")
    return r["result"]


def call_raw(method, params=None):
    """The whole reply, error included -- for a method whose refusal is the claim."""
    n[0] += 1
    f.write((json.dumps({"jsonrpc": "2.0", "id": n[0],
                         "method": method, "params": params or {}}) + "\n").encode())
    f.flush()
    return json.loads(f.readline())


def text_of(wid):
    """A `Label`'s text.

    `value`, not `label`: the agent reports a Button's caption as
    `label` and a Label's content as `value`. The first version of this
    script read `label` and got an empty dict from a tree that was
    perfectly correct -- which is why the clean-start assertion below
    prints what it actually saw.
    """
    r = call("ui.resolve", {"selector": f"#{wid}"})
    return r.get("value", "")


def counts():
    """The counts label, parsed. `one=1 batch=3 chain=3 ...`"""
    out = {}
    for part in text_of("counts").split():
        k, _, v = part.partition("=")
        out[k] = int(v) if v.isdigit() else v
    return out


def status():
    return text_of("status")


def settle(pred, what, budget=4.0):
    """Wait for a consequence, with the elapsed time printed beside its bound.

    Deferred work is asynchronous by definition, so this cannot be a
    single read -- but a bare sleep would hide how long it actually
    took, which is the number worth seeing.
    """
    t0 = time.time()
    while time.time() - t0 < budget:
        if pred():
            el = time.time() - t0
            print(f"  ok  {what} after {el * 1000:.0f} ms (budget {budget * 1000:.0f})")
            return
        time.sleep(0.02)
    sys.exit(f"tasks-drive[{strategy}]: {what} did not happen within {budget}s; "
             f"counts={counts()} status={status()!r}")


def click(wid):
    r = call("input.click", {"selector": f"#{wid}"})
    # "did that input land" -- the only direct answer, and the only way
    # to test an action whose correct outcome is that nothing happens.
    if r.get("handled") is False:
        sys.exit(f"tasks-drive[{strategy}]: the click on #{wid} was not handled")


def screenshot_px(x, y):
    """`ui.screenshot` (canvas), decoded, and the pixel at physical (x, y)."""
    from PIL import Image
    r = call("ui.screenshot")
    im = Image.open(io.BytesIO(base64.b64decode(r["image_base64"]))).convert("RGB")
    return im.size, im.getpixel((x, y))


def swatch_phase():
    """M88/T88.2: `ui.screenshot` reads the renderer back PER CALL.

    A screenshot, a click on `#swatch` (which recolours the 16x16 swatch
    at the origin), a second screenshot; the swatch's `ui.regionHash`
    must differ between them. On the GPU the canvas is a readback of the
    target, so a backend that answered a cached read would hand the
    first frame back twice -- the injection this is for.
    """
    rect = call("ui.resolve", {"selector": "#swatch-box"}).get("bounds")
    print(f"  ..  swatch at {rect} (want [0, 0, 16, 16])")
    if rect != [0, 0, 16, 16]:
        sys.exit(f"tasks-drive[{leg}]: setup: the swatch is not the 16x16 box at the origin: {rect}")
    size1, px1 = screenshot_px(4, 4)
    h1 = call("ui.regionHash", {"rect": rect})
    r = call("input.click", {"selector": "#swatch"})
    print(f"  ..  click on #swatch: handled={r.get('handled')}")
    if r.get("handled") is not True:
        sys.exit(f"tasks-drive[{leg}]: setup: the click on #swatch was not handled: {r}")
    size2, px2 = screenshot_px(4, 4)
    h2 = call("ui.regionHash", {"rect": rect})
    print(f"  ..  swatch hash before {h1['hash']} ({h1['pixels']} px), after {h2['hash']} "
          f"({h2['pixels']} px); screenshot pixel (4, 4) {px1} -> {px2}; size {size1}")
    if h1["pixels"] != 256:
        sys.exit(f"tasks-drive[{leg}]: setup: the swatch's rect hashed {h1['pixels']} px, not 256")
    if h1["hash"] == h2["hash"] or px1 == px2:
        sys.exit(f"tasks-drive[{leg}]: the second screenshot shows the first one's swatch "
                 f"(hash {h1['hash']} == {h2['hash']}, pixel {px1} -> {px2}): "
                 f"the canvas was not read back on this call")
    print(f"  ok  swatch: a click changed the screenshot ({h1['hash']} -> {h2['hash']})")


def hexrgb(px):
    return "#%02x%02x%02x" % tuple(px[:3])


def shot_label():
    """`#shot-value` parsed: (count, colour) -- `shot N WxH #rrggbb`."""
    t = text_of("shot-value")
    m = re.match(r"shot (\d+) (\d+)x(\d+) (#[0-9a-f]{6})$", t)
    return (int(m.group(1)), m.group(4), t) if m else (0, None, t)


def shot_agent_phase():
    """M88/T88.3, the agent path: `#shot` recolours the swatch and pushes
    a `Screenshot` from its handler. The click arrives through the agent,
    so the window answers after that call, from the frame it presented.
    One read after an UNOBSERVED 1 s: the label must name this click's
    colour, not the previous click's."""
    _, prev = screenshot_px(4, 4)
    n0, _, t0 = shot_label()
    r = call("input.click", {"selector": "#shot"})
    if r.get("handled") is not True:
        sys.exit(f"tasks-drive[{leg}]: setup: the click on #shot was not handled: {r}")
    time.sleep(1.0)  # unobserved: every agent reply asks for a redraw
    n1, got, t1 = shot_label()
    _, now = screenshot_px(4, 4)
    print(f"  ..  #shot (agent path): label {t0!r} -> {t1!r}; this click's colour {hexrgb(now)}, "
          f"the previous click's {hexrgb(prev)}")
    if now == prev:
        sys.exit(f"tasks-drive[{leg}]: setup: #shot did not recolour the swatch ({hexrgb(now)})")
    if n1 != n0 + 1 or got != hexrgb(now):
        named = "the previous click's" if got == hexrgb(prev) else "neither click's"
        sys.exit(f"tasks-drive[{leg}]: after #shot the label reads {t1!r}: {named} colour, want "
                 f"shot {n0 + 1} with {hexrgb(now)} -- the answer is not this click's frame")
    print(f"  ok  agent path: shot {n1} shows this click's {got}")


def shot_redraw_phase():
    """M88/T88.3, the redraw path: `#shot-timer`'s ticks push `Screenshot`
    from `apply`, in a pump a timer wake ran, with no agent call in flight
    -- so only the redraw drain can answer them. Tick 1 recolours the
    swatch; tick 2 changes nothing (no damage: the skipped present) and
    asks again. One read after an UNOBSERVED 1.5 s."""
    _, before_px = screenshot_px(4, 4)
    n0, _, t0 = shot_label()
    log0 = text_of("shot-log").split()[1:]
    t_click = time.time()
    r = call("input.click", {"selector": "#shot-timer"})
    click_ms = (time.time() - t_click) * 1000
    if r.get("handled") is not True:
        sys.exit(f"tasks-drive[{leg}]: setup: the click on #shot-timer was not handled: {r}")
    # A GPU window on a display that is asleep (DPMS off) blocks each
    # FIFO present for about a second, so the click -- which presents
    # inside the request -- returns after the first tick is due, and no
    # read can land before it. The answers are still checked, after a
    # longer unobserved wait; the presents bound then cannot be (printed).
    slow = backend == "gpu" and click_ms > 250
    w = call("ui.waitSettled")
    p0 = call("app.presents")
    t_p0 = time.time()
    time.sleep(6.0 if slow else 1.5)  # unobserved: the ticks land at 500 and 1000 ms
    p1 = call("app.presents")
    n1, got, t1 = shot_label()
    log1 = text_of("shot-log").split()[1:]
    _, tick1_px = screenshot_px(4, 4)
    presented, skipped = p1["presents"] - p0["presents"], p1["skipped"] - p0["skipped"]
    new = log1[len(log0):]
    print(f"  ..  #shot-timer (redraw path): waitSettled settled={w.get('settled')}; label {t0!r} -> {t1!r}; "
          f"answers' colours {new}; tick 1's colour {hexrgb(tick1_px)}, the colour before {hexrgb(before_px)}")
    print(f"  ..  presents across {(time.time() - t_p0) * 1000:.0f} ms: presented {presented}, skipped {skipped} "
          f"({'bound: exactly 3 presented, >= 1 skipped' if strategy == 'boundaries' else 'Everything: printed'})")
    if tick1_px == before_px:
        sys.exit(f"tasks-drive[{leg}]: setup: tick 1 did not recolour the swatch")
    if n1 == n0 + 1:
        sys.exit(f"tasks-drive[{leg}]: the label reads {t1!r} (shot {n1}): tick 2's request, pushed on a frame "
                 f"that presented nothing, was not answered -- skipped frame not drained, or the answer "
                 f"never asked for the frame that shows it")
    if n1 != n0 + 2:
        sys.exit(f"tasks-drive[{leg}]: the label reads {t1!r}, want shot {n0 + 2}: an answer applied after "
                 f"the frame did not ask for another (more_deferred), or a request was never drained")
    if hexrgb(before_px) in new:
        sys.exit(f"tasks-drive[{leg}]: an answer shows the colour before tick 1 ({hexrgb(before_px)}; answers "
                 f"{new}): the redraw drain answered before the frame was presented -- the previous frame's pixels")
    if new != [hexrgb(tick1_px)] * 2 or got != hexrgb(tick1_px):
        sys.exit(f"tasks-drive[{leg}]: the answers show {new}, want tick 1's {hexrgb(tick1_px)} twice")
    if slow:
        print(f"  --  presents bound NOT CHECKED on this leg: the click took {click_ms:.0f} ms (bound 250) -- "
              f"the GPU's presents block (is the display asleep? `xset q`), so P0 could not be read before tick 1")
    elif strategy == "boundaries" and (presented != 3 or skipped < 1):
        sys.exit(f"tasks-drive[{leg}]: presented {presented} (bound exactly 3: tick 1, answer 1, answer 2) and "
                 f"skipped {skipped} (bound >= 1: tick 2)")
    print(f"  ok  redraw path: shot {n1}, both answers tick 1's {got}; presented {presented}, skipped {skipped}")


def clip_label(wid):
    """`#clip-write` / `#clip-read` parsed: (count, name, what it said)."""
    t = text_of(wid)
    m = re.match(r"(?:write|read) (\d+) (\w+) (.*)$", t, re.S)
    return (int(m.group(1)), m.group(2), m.group(3)) if m else (0, None, t)


def clip_click(wid, label):
    """Click `#wid` (handled), then wait for `#label`'s count to move; the
    answer, printed. The request is answered after the agent call that
    clicked, and the label is bound on the frame after."""
    n0, _, _ = clip_label(label)
    r = call("input.click", {"selector": f"#{wid}"})
    if r.get("handled") is not True:
        sys.exit(f"tasks-drive[{leg}]: setup: the click on #{wid} was not handled: {r}")
    t0 = time.time()
    while time.time() - t0 < 4.0:
        n, name, said = clip_label(label)
        if n == n0 + 1:
            print(f"  ..  #{wid}: #{label} reads {text_of(label)!r} after {(time.time() - t0) * 1000:.0f} ms")
            return name, said
        time.sleep(0.02)
    sys.exit(f"tasks-drive[{leg}]: #{wid} was never answered: #{label} still reads {text_of(label)!r}")


def xclip_read(selection):
    r = subprocess.run(["xclip", "-o", "-selection", selection], capture_output=True, text=True, timeout=5)
    return r.stdout if r.returncode == 0 else f"<xclip exit {r.returncode}: {r.stderr.strip()}>"


# What tasks-demo's `#copy-html` writes (COPY_HTML in its main.rs).
COPY_HTML = "<p>caf\u00e9 <b>&amp;</b> <i>lumen</i></p>"


def clipboard_phase():
    """M88/T88.4: an application reaches the clipboard -- text, HTML and
    the primary selection -- through `ClipboardWrite`/`ClipboardRead`
    requests, answered by `Win::serve_requests` from the real platform
    clipboard. **This writes the desktop's clipboard and primary
    selection.** Setup landed: each write answered `Written`."""
    xclip_preflight(leg)
    for wid, want in (("copy-text", "text"), ("copy-primary", "primary")):
        name, said = clip_click(wid, "clip-write")
        print(f"  ..  setup: #{wid} answered {said!r} (want 'Written')")
        if name != want or said != "Written":
            sys.exit(f"tasks-drive[{leg}]: setup: #{wid}'s write answered {name} {said!r}, not Written -- "
                     f"the write never reached the clipboard (was clip_for asked?)")
    _, text = clip_click("read-text", "clip-read")
    _, primary = clip_click("read-primary", "clip-read")
    print(f"  ..  #read-text shows {text!r} (want 'A'); #read-primary shows {primary!r} (want 'B')")
    if text != "A" or primary != "B":
        sys.exit(f"tasks-drive[{leg}]: read-text {text!r} and read-primary {primary!r}, want 'A' and 'B' -- "
                 f"the clipboard and the primary selection are not two places (does serve_requests read kind?)")
    ind = xclip_read("primary")
    print(f"  ..  independent `xclip -o -selection primary`: {ind!r} (want 'B')")
    if ind != "B":
        sys.exit(f"tasks-drive[{leg}]: xclip reads the primary selection as {ind!r}, not 'B' -- "
                 f"what the application wrote is not what another program sees")
    name, said = clip_click("copy-html", "clip-write")
    if said != "Written":
        sys.exit(f"tasks-drive[{leg}]: setup: #copy-html answered {said!r}, not Written")
    _, html = clip_click("read-html", "clip-read")
    print(f"  ..  #read-html: {len(html.encode())} B read back, {len(COPY_HTML.encode())} B written; "
          f"equal: {html == COPY_HTML}")
    if html != COPY_HTML:
        sys.exit(f"tasks-drive[{leg}]: the HTML did not round-trip byte for byte: wrote {COPY_HTML!r}, "
                 f"read {html!r}")
    # Accept: a second clipboard write (`C`) leaves the primary selection alone.
    name, said = clip_click("copy-text", "clip-write")
    if said != "Written":
        sys.exit(f"tasks-drive[{leg}]: setup: the second #copy-text answered {said!r}, not Written")
    _, text = clip_click("read-text", "clip-read")
    _, primary = clip_click("read-primary", "clip-read")
    ind = xclip_read("primary")
    print(f"  ..  accept: after writing 'C', #read-text {text!r} (want 'C'); #read-primary {primary!r} and "
          f"xclip's primary {ind!r} (want 'B')")
    if text != "C" or primary != "B" or ind != "B":
        sys.exit(f"tasks-drive[{leg}]: a clipboard write moved the primary selection (or did not land): "
                 f"text {text!r}, primary {primary!r}, xclip {ind!r}")
    print(f"  ok  clipboard: text A, primary B (and xclip B), html {len(COPY_HTML.encode())} B round-trip; "
          f"a write of C left primary B")


def image_phase(built):
    """M88/T88.5: `#copy-image` writes an 8x4 RGBA8 image (`Clip::Image`)
    and `#read-image` reads the clipboard back as `ClipFormat::Image`.
    With `clipboard-image` the read must be 8x4 with the written pixels'
    hash (both printed; setup landed: the write answered `Written`).
    Without it -- the accept direction -- both answer
    `Refused(Unsupported)`, not `Refused(Empty)`: the build cannot, which
    is not the same as a clipboard holding nothing."""
    sent = text_of("clip-image")
    m = re.match(r"sent (\d+x\d+) ([0-9a-f]{16})$", sent)
    if not m:
        sys.exit(f"tasks-drive[{leg}]: setup: #clip-image reads {sent!r}, not 'sent WxH <hash>'")
    _, wrote = clip_click("copy-image", "clip-write")
    _, read = clip_click("read-image", "clip-read")
    if not built:
        print(f"  ..  without clipboard-image: #copy-image answered {wrote!r}, #read-image {read!r} "
              f"(want 'Refused(Unsupported)' both, not 'Refused(Empty)')")
        if wrote != "Refused(Unsupported)" or read != "Refused(Unsupported)":
            sys.exit(f"tasks-drive[{leg}]: a build without clipboard-image answered the image write "
                     f"{wrote!r} and the read {read!r}, want Refused(Unsupported) for both")
        print("  ok  image without the feature: Unsupported, not Empty")
        return
    print(f"  ..  setup: #copy-image answered {wrote!r} (want 'Written'); written {m.group(1)} {m.group(2)}")
    if wrote != "Written":
        sys.exit(f"tasks-drive[{leg}]: setup: #copy-image's write answered {wrote!r}, not Written")
    got = re.match(r"(\d+x\d+) ([0-9a-f]{16})$", read)
    print(f"  ..  #read-image: {read!r}; written {m.group(1)} hash {m.group(2)}")
    if not got:
        sys.exit(f"tasks-drive[{leg}]: #read-image answered {read!r}, not an image")
    if got.group(1) != m.group(1):
        sys.exit(f"tasks-drive[{leg}]: the image read back as {got.group(1)}, written {m.group(1)} -- "
                 f"width and height swapped on the way")
    if got.group(2) != m.group(2):
        sys.exit(f"tasks-drive[{leg}]: the image's pixels changed on the way: hash {got.group(2)}, "
                 f"written {m.group(2)}")
    print(f"  ok  image: {got.group(1)} read back, pixel hash {got.group(2)} equals the one written")


if backend == "image":
    image_phase(True)
    print(f"  ok  {leg}: an 8x4 image round-tripped through the clipboard")
    sys.exit(0)


if backend == "gpu":
    # The surface is still "not checkable" on the GPU (`gpu.rs`,
    # `fetch_rgba`): the target is what was rendered, not what was
    # presented, and only `frame_rgba` may answer from it.
    r = call_raw("ui.screenshot", {"source": "surface"})
    msg = r.get("error", {}).get("message", "") if "error" in r else ""
    print(f"  ..  ui.screenshot source=surface on the GPU: "
          f"{'error ' + repr(msg[:60]) if msg else 'an image'} (want 'cannot read its window back')")
    if "cannot read its window back" not in msg:
        sys.exit(f"tasks-drive[{leg}]: source=surface answered "
                 f"{'an image' if not msg else repr(msg)} on the GPU -- the pre-present target "
                 f"is not the present (gpu.rs, fetch_rgba)")
    print("  ok  surface: not checkable on the GPU, as recorded")
    swatch_phase()
    shot_agent_phase()
    shot_redraw_phase()
    print(f"  ok  {leg}: the GPU answers ui.screenshot per call, and screenshot requests on both paths")
    sys.exit(0)


# M48/T48.9 (R12-F3): the widgets' `observe` is on in this build, or a
# Label reads as GenericContainer -- which the widgets crate now refuses
# to compile. The accept direction of scripts/check_feature_seam.sh.
role = call("ui.resolve", {"selector": "#status"}).get("role")
print(f"  ..  #status role {role!r} (must be 'Label')")
if role != "Label":
    sys.exit(f"tasks-drive[{strategy}]: #status reports role {role!r}: the widgets' observe is off")

base = counts()
if base != {"one": 0, "batch": 0, "chain": 0, "slow": 0, "ticks": 0,
            "failed": 0, "sub": "off"}:
    sys.exit(f"tasks-drive[{strategy}]: setup: expected a clean start, got {base}")
print(f"  ok  clean start {base}")

# ---- 0. the swatch: a screenshot per call (M88/T88.2) -----------------
swatch_phase()
# ---- 0b. screenshot requests, both drain sites (M88/T88.3) -------------
shot_agent_phase()
shot_redraw_phase()
# ---- 0c. the clipboard: text, HTML, primary (M88/T88.4) ----------------
clipboard_phase()
# ---- 0d. an image, in a build without clipboard-image (M88/T88.5) -------
image_phase(False)

# ---- 1. spawn one ----------------------------------------------------
click("spawn-one")
settle(lambda: counts()["one"] == 1, "spawn: exactly 1 result (bound 1)")

# ---- 2. batch: three pushed together, all three arrive ---------------
click("spawn-batch")
settle(lambda: counts()["batch"] == 3, "batch: all 3 results (bound 3)")

# ---- 3. chain: three steps, each from the last -----------------------
click("start-chain")
settle(lambda: counts()["chain"] == 3, "chain: 3 steps (bound 3)")
st = status()
# 1 -> 2 -> 4 -> 8: each step doubles, so the final value proves the
# steps ran IN ORDER and each saw the previous one's payload. A count
# alone would pass for three copies of step 0.
if "value=8" not in st:
    sys.exit(f"tasks-drive[{strategy}]: the chain's last value should be 8 "
             f"(1*2*2*2), got {st!r} -- the steps did not compose")
print(f"  ok  chain: composed to value=8 ({st!r})")

# ---- 4. cancel: a slow task's result never lands ---------------------
before = counts()["slow"]
click("start-slow")
time.sleep(0.05)          # the job is running; it sleeps 400 ms
click("cancel")
time.sleep(1.0)           # well past the job's own duration
after = counts()["slow"]
if after != before:
    sys.exit(f"tasks-drive[{strategy}]: a cancelled task's result landed "
             f"(slow {before} -> {after}); cancel did nothing")
print(f"  ok  cancel: slow stayed at {after} across 1000 ms "
      f"(the job itself takes 400)")

# and work started AFTER the cancel still arrives -- or this would pass
# for a cancel that breaks everything.
click("spawn-one")
settle(lambda: counts()["one"] == 2, "cancel is not permanent: one=2 (bound 2)")

# ---- 4b. ui.waitSettled: unsettled while a job runs, settled after ----
# M48/T48.2 (R13-F1): it answered `settled: true` in every state. The job
# sleeps 400 ms, so a read straight after the click lands inside it.
before = counts()["slow"]
click("start-slow")
w = call("ui.waitSettled")
print(f"  ..  waitSettled right after #start-slow: settled={w.get('settled')} "
      f"pending_tasks={w.get('pending_tasks')}")
if w.get("settled") is not False:
    sys.exit(f"tasks-drive[{strategy}]: waitSettled said {w.get('settled')!r} with a "
             f"400 ms job just started: {w}")
settle(lambda: counts()["slow"] == before + 1, f"slow job landed: slow={before + 1} (bound {before + 1})")
t0 = time.time()
while True:
    w = call("ui.waitSettled")
    if w.get("settled") is True:
        break
    if time.time() - t0 > 2.0:
        sys.exit(f"tasks-drive[{strategy}]: still unsettled 2 s after the job's result "
                 f"was applied: {w}")
    time.sleep(0.02)
print(f"  ok  waitSettled: false during the job (pending {1}), true "
      f"{(time.time() - t0) * 1000:.0f} ms after its result was shown")

# ---- 5. subscribe: on, produces; off, stops --------------------------
click("toggle-sub")
settle(lambda: counts()["ticks"] >= 3, "subscription on: >= 3 ticks (bound 3)")
click("toggle-sub")
time.sleep(0.3)
frozen = counts()["ticks"]
time.sleep(0.6)           # five 120 ms periods
now = counts()["ticks"]
if now != frozen:
    sys.exit(f"tasks-drive[{strategy}]: the subscription kept producing after "
             f"it was switched off ({frozen} -> {now})")
print(f"  ok  subscription off: ticks froze at {frozen} across 600 ms "
      f"(period 120)")

# ---- 6. unobserved: a wake and a deadline on an IDLE window ----------
# M47/T47.1 (R7-F1, R7-F2). Every read above ends in a redraw request,
# so the window was never idle while phases 1-5 watched it: they pass
# with the waker uninstalled and with the deadline ignored. These two
# make ONE read after a sleep with no RPC traffic, and that read is
# answered before the frame it requests -- so what it reports is what a
# wake or a deadline produced on its own (docs/plan-timers-wake.md
# rule 1). Do not "fix" either into a settle(): polling is the defect.
PERIOD = 120
before = counts()["ticks"]
click("toggle-sub")
t0 = time.time()
time.sleep(3.0)
got = counts()["ticks"] - before
x = (time.time() - t0) * 1000
want = int(x // PERIOD)
bound = 1 + want - 2
print(f"  ..  ticks {got} (expected {want}, bound {bound}) after {x:.0f} ms unobserved")
if got < bound:
    sys.exit(f"tasks-drive[{strategy}]: an idle window's {PERIOD} ms timer ticked "
             f"{got} times in {x:.0f} ms with nothing reading it (bound {bound}): "
             f"nothing turns the deadline into a frame")
print(f"  ok  unobserved timer: {got} >= {bound}")
click("toggle-sub")
time.sleep(0.3)

before = counts()["slow"]
click("start-slow")
t0 = time.time()
time.sleep(1.5)
got = counts()["slow"]
x = (time.time() - t0) * 1000
print(f"  ..  slow {got} (want {before + 1}) after {x:.0f} ms unobserved, job takes 400")
if got != before + 1:
    sys.exit(f"tasks-drive[{strategy}]: a finished job's result did not reach an "
             f"idle window in {x:.0f} ms (slow {before} -> {got}): the waker is not wired")
print(f"  ok  unobserved wake: slow {before} -> {got}")

# ---- 7. unobserved: a tick beside a busy worker ---------------------
# M47/T47.3 (R7-F5): jobs run on a small pool now, not a thread each,
# and a job that blocks occupies a worker. The 400 ms job starts FIRST,
# then the subscription (whose start is itself a tick job, then one per
# 120 ms). ONE read at 330 ms after the slow click, while that job still
# runs, must already see a tick. A pool that queued the ticks behind the
# slow job reads 0 here and catches up later -- which the 3 s count
# below cannot see, so this read is the one that can. (Each click is
# answered after a frame, ~40 ms, which is why the clock starts at the
# first click and not the second.)
time.sleep(0.3)
before_t, before_s = counts()["ticks"], counts()["slow"]
t0 = time.time()
click("start-slow")
click("toggle-sub")
t_sub = time.time()
time.sleep(max(0.0, 0.33 - (t_sub - t0)))
mid = counts()
x = (time.time() - t0) * 1000
early, running = mid["ticks"] - before_t, mid["slow"] == before_s
print(f"  ..  ticks {early} (bound 1) {x:.0f} ms after the 400 ms job started "
      f"({(t_sub - t0) * 1000:.0f} ms of clicks), job still running: {running}")
if not running:
    sys.exit(f"tasks-drive[{strategy}]: setup: the 400 ms job finished within {x:.0f} ms")
if early < 1:
    sys.exit(f"tasks-drive[{strategy}]: no tick landed in {x:.0f} ms while a 400 ms job ran: "
             f"a busy worker held the timer back")
print(f"  ok  a tick beside a busy worker: {early} >= 1")
time.sleep(max(0.0, 3.0 - (time.time() - t_sub)))
got = counts()["ticks"] - before_t
x = (time.time() - t_sub) * 1000
want = int(x // PERIOD)
bound = 1 + want - 2
print(f"  ..  ticks {got} (expected {want}, bound {bound}) after {x:.0f} ms, one read in between")
if got < bound:
    sys.exit(f"tasks-drive[{strategy}]: beside a 400 ms job the {PERIOD} ms timer ticked "
             f"{got} times in {x:.0f} ms (bound {bound})")
print(f"  ok  timer beside a busy worker: {got} >= {bound}")
click("toggle-sub")
time.sleep(0.3)

# ---- 8. app.perf measured the frames it reports ----------------------
# M48/T48.3 (R13-F2): every stage read 0 in every turnkey window, and a
# budget passes on 0. Ten clicks, each followed by the redraw it asks for.
for _ in range(10):
    click("spawn-one")
    time.sleep(0.03)
w = call("app.perf")["window"]
fp50, fn = w["frame_ns"]["p50"], w["frame_ns"]["n"]
lp50 = w["layout_ns"]["p50"]
print(f"  ..  frame p50 {fp50} ns (must be > 0), n={fn}; layout p50 {lp50} ns (must be > 0); "
      f"damaged_px p50 {w['damaged_px']['p50']}")
if not fp50 or not lp50:
    sys.exit(f"tasks-drive[{strategy}]: app.perf reports frame p50 {fp50} ns / layout p50 "
             f"{lp50} ns over n={fn} -- a stage nobody measured")
print(f"  ok  app.perf measured: frame p50 {fp50 / 1e3:.0f} us over {fn} frames")

# ---- 9. a sipper: 10,000 progress values, the last one shown ---------
# M88/T88.1: one job reports 0..9999 through `Sender::progress`, a
# one-value slot the window reads at most once a frame. Through `send`
# the same stream lost 9,744 values and its last one (T88.0's pin). Each
# distinct label read must have been a presented frame's, so the reads
# cannot outnumber the presents counted across the run.
if text_of("sip-value") != "sip -":
    sys.exit(f"tasks-drive[{strategy}]: setup: the sipper's label should read 'sip -' "
             f"before the click, got {text_of('sip-value')!r}")
p0 = call("app.presents")
click("sip")
seen, t0, last = set(), time.time(), ""
while time.time() - t0 < 10.0:
    last = text_of("sip-value")
    if last != "sip -":
        seen.add(last)
    if last == "sip 9999":
        break
    time.sleep(0.005)
p1 = call("app.presents")
presents = p1["presents"] - p0["presents"]
print(f"  ..  sipper: label {last!r} after {(time.time() - t0) * 1000:.0f} ms (want 'sip 9999'); "
      f"distinct labels seen {len(seen)}, presents during the run {presents} "
      f"(skipped {p1['skipped'] - p0['skipped']}; seen must be <= presents)")
if last != "sip 9999":
    sys.exit(f"tasks-drive[{strategy}]: the sipper's last value never reached the window: {last!r}")
if len(seen) > presents:
    sys.exit(f"tasks-drive[{strategy}]: {len(seen)} distinct sipper labels were read across "
             f"{presents} presented frames -- a value was shown that no frame presented")
print(f"  ok  sipper: reached 'sip 9999'; {len(seen)} distinct labels <= {presents} presents")

final = counts()
if final["failed"] != 0:
    sys.exit(f"tasks-drive[{strategy}]: {final['failed']} job(s) failed: {status()!r}")
print(f"  ok  no failures; final {final}")
