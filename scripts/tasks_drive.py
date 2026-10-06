"""M28/T28.12's assertions. See scripts/tasks_drive.sh."""
import base64, hashlib, io, json, socket, sys, time

port, strategy = int(sys.argv[1]), sys.argv[2]
# M88/T88.2: `gpu` runs only the phases that read pixels, on a window
# drawn by wgpu; every other phase is backend-independent and runs on
# the software legs.
backend = sys.argv[3] if len(sys.argv) > 3 else "software"
leg = f"{strategy}+gpu" if backend == "gpu" else strategy

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
import os, subprocess  # noqa: E401,E402
here = os.path.dirname(os.path.abspath(__file__))
replay = [sys.executable, os.path.join(here, "browser_replay.py"), f"127.0.0.1:{port}",
          os.path.join(here, "..", "crates/lumen-static-agent/tests/browser/firefox156-fetch-no-cors.bin"),
          "#counts"]
if os.environ.get("LUMEN_TASKS_PID"):
    replay += ["--pid", os.environ["LUMEN_TASKS_PID"]]
if backend != "gpu" and subprocess.call(replay) != 0:
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
    print(f"  ok  {leg}: the GPU answers ui.screenshot per call")
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
