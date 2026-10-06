"""M47/T47.8: what an idle window costs, in counts -- docs/results-frame-cost.md, "Idle".

Launched by `scripts/idle_count.sh` against `examples/tasks-demo`. Two
phases, each ONE read before and ONE read after a sleep with no RPC
traffic (docs/plan-timers-wake.md rule 1: a read asks for a frame, so a
polled window is never idle):

  still -- nothing scheduled;
  timer -- the 120 ms subscription on (T47.1 makes its deadline a frame).

Per phase: frames drawn (the agent's frame counter, less the first read's
own frame), wakes of the event-loop thread (its voluntary and involuntary
context switches from /proc), context switches of the whole process, and
threads alive / threads created (distinct task ids seen). Counts, not
clocks: none of these moves with machine load the way a microsecond does.
Usage: idle_count.py PORT SECONDS [daemon-still|daemon-timer]

**The daemon leg** (M89/T89.6, K89d in docs/plan-windows.md) runs one
phase against `examples/daemon`, which has no window: `daemon-still`
against `daemon --still` (no deadline at all) and `daemon-timer` against
`daemon --steady` (its 120 ms deadline, nothing else). Its frames are the
windowless session's. K89d is judged here and exits 1 when it fires: an
idle daemon draws 0 frames and its loop thread wakes for nothing but the
two reads' own (2, as a still window's do, docs/results-windows.md
T89.0), and a ticking one runs at most one windowless frame a period --
periods + 1 over the interval, since one of N.x periods holds N + 1
deadlines.
"""
import json, os, socket, sys, time

port, secs = int(sys.argv[1]), float(sys.argv[2])
pid = os.environ["LUMEN_LIVE_PID"]
s = socket.create_connection(("127.0.0.1", port), timeout=15)
f = s.makefile("rwb")
n = [0]


def call(method, params=None):
    n[0] += 1
    f.write((json.dumps({"jsonrpc": "2.0", "id": n[0], "method": method,
                         "params": params or {}}) + "\n").encode())
    f.flush()
    r = json.loads(f.readline())
    if "error" in r:
        sys.exit(f"idle-count: {method} failed: {r['error']}")
    return r["result"]


def switches(tid):
    st = open(f"/proc/{pid}/task/{tid}/status").read()
    return sum(int(l.split()[1]) for l in st.splitlines()
               if l.startswith(("voluntary_ctxt_switches", "nonvoluntary_ctxt_switches")))


def sample():
    tids = os.listdir(f"/proc/{pid}/task")
    total = 0
    for t in tids:
        try:
            total += switches(t)
        except OSError:
            pass
    return {"frame": call("app.diagnostics")["frame"], "loop": switches(pid),
            "proc": total, "tids": set(tids)}


def phase(name, periods_of=None, settle=1.0):
    time.sleep(settle)
    a = sample()
    seen = set(a["tids"])
    t0 = time.time()
    # Sample the task list, not the window, while it sleeps: /proc reads
    # do not reach the application.
    while time.time() - t0 < secs:
        time.sleep(0.05)
        seen |= set(os.listdir(f"/proc/{pid}/task"))
    el = time.time() - t0
    b = sample()
    frames = b["frame"] - a["frame"] - 1  # the first read's own frame
    loop = b["loop"] - a["loop"]
    proc = b["proc"] - a["proc"]
    created = len(seen - a["tids"])
    due = f", {int(el * 1000 // periods_of)} periods of {periods_of} ms" if periods_of else ""
    print(f"idle-count [{name}]: {el:.1f} s{due}: frames {frames}, loop-thread wakes {loop}, "
          f"process context switches {proc}, threads alive {len(b['tids'])}, created {created}")
    return {"frames": frames, "loop": loop, "periods": int(el * 1000 // periods_of) if periods_of else 0}


# The daemon leg (M89/T89.6): one phase, no toggles, K89d judged.
leg = sys.argv[3] if len(sys.argv) > 3 else None
if leg == "daemon-still":
    r = phase("daemon still", settle=2.0)
    print(f"idle-count [daemon still]: K89d -- frames {r['frames']} (bound 0), loop-thread wakes "
          f"{r['loop']} (bound 2: the two reads' own)")
    if r["frames"] > 0 or r["loop"] > 2:
        sys.exit("idle-count: K89d FIRED -- an idle daemon with no deadline ran frames or woke: the "
                 "windowless pump is driven by something other than a deadline or a wake")
    sys.exit(0)
if leg == "daemon-timer":
    r = phase("daemon timer", 120, settle=2.0)
    # An interval of N.x periods holds up to N + 1 deadlines (the
    # fencepost): one frame a period is that bound. Two a period -- a
    # `Timers` firing's deadline frame plus its wake -- would read 2N.
    bound = r["periods"] + 1
    print(f"idle-count [daemon timer]: K89d -- frames {r['frames']} for {r['periods']} periods "
          f"(bound {bound}: one windowless frame a period, and the fencepost)")
    if r["frames"] > bound:
        sys.exit("idle-count: K89d FIRED -- the daemon ran more than one windowless frame a period")
    sys.exit(0)
if leg is not None:
    sys.exit(f"idle-count: no leg {leg!r}; have daemon-still, daemon-timer")


# A fresh window is not idle: the window manager maps, places and exposes
# it, and on a shared display other windows opening do the same -- the
# first seconds drew hundreds of frames in a probe. So the still phase
# waits 20 s before its first read.
phase("still", settle=20.0)
call("input.click", {"selector": "#toggle-sub"})
phase("timer", 120)
call("input.click", {"selector": "#toggle-sub"})
