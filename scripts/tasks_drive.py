"""M28/T28.12's assertions. See scripts/tasks_drive.sh."""
import base64, hashlib, json, socket, sys, time

port, strategy = int(sys.argv[1]), sys.argv[2]

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
if subprocess.call(replay) != 0:
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
        sys.exit(f"tasks-drive[{strategy}]: {method} failed: {r['error']}")
    return r["result"]


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


base = counts()
if base != {"one": 0, "batch": 0, "chain": 0, "slow": 0, "ticks": 0,
            "failed": 0, "sub": "off"}:
    sys.exit(f"tasks-drive[{strategy}]: setup: expected a clean start, got {base}")
print(f"  ok  clean start {base}")

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

final = counts()
if final["failed"] != 0:
    sys.exit(f"tasks-drive[{strategy}]: {final['failed']} job(s) failed: {status()!r}")
print(f"  ok  no failures; final {final}")
