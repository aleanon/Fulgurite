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
Usage: idle_count.py PORT SECONDS
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


# A fresh window is not idle: the window manager maps, places and exposes
# it, and on a shared display other windows opening do the same -- the
# first seconds drew hundreds of frames in a probe. So the still phase
# waits 20 s before its first read.
phase("still", settle=20.0)
call("input.click", {"selector": "#toggle-sub"})
phase("timer", 120)
call("input.click", {"selector": "#toggle-sub"})
