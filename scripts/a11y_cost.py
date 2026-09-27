#!/usr/bin/env python3
"""M54/T54.2's kill rule, measured (docs/plan-a11y-platform.md §2.3).

    a11y_cost.py EXE_WITHOUT EXE_WITH [RUNS] [REDRAWS]

Both binaries are the showcase built with `--features agent`, one with the
window crate's `a11y` and one without. Per run and per binary, alternating:
launch on a free port in LUMEN_LIVE_PORT_RANGE, settle, then drive REDRAWS
idle frames -- `ui.waitSettled`, which changes nothing and ends in a redraw
request -- and read the process's CPU time (utime + stime, /proc) before
and after. The figure is CPU microseconds per idle redraw, everything the
process did for it included: the frame, the agent's reply, and in the
`a11y` build the tree build and its hand-off to the platform adapter
while an assistive technology is attached.

Prints each run, then per binary the median and the eight-run spread
(min..max), and whether the `a11y` build's median lies outside the
other's spread -- the rule's first condition. Not a gate: machine
dependent, run under the bench lock (`just a11y-cost`).
"""
import json, os, socket, statistics, subprocess, sys, time

TICK = os.sysconf("SC_CLK_TCK")


def free_port():
    lo, hi = (int(x) for x in os.environ.get("LUMEN_LIVE_PORT_RANGE", "7880-7889").split("-"))
    for p in range(lo, hi + 1):
        with socket.socket() as s:
            try:
                s.bind(("127.0.0.1", p))
                return p
            except OSError:
                continue
    sys.exit("a11y-cost: no free port in LUMEN_LIVE_PORT_RANGE")


def cpu(pid):
    f = open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()
    return (int(f[11]) + int(f[12])) / TICK


def one(exe, redraws):
    port = free_port()
    env = dict(os.environ, LUMEN_AGENT_ADDR=f"127.0.0.1:{port}")
    p = subprocess.Popen([exe, "6"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(600):
            try:
                s = socket.create_connection(("127.0.0.1", port), timeout=0.2)
                break
            except OSError:
                time.sleep(0.1)
        else:
            sys.exit(f"a11y-cost: {exe} did not answer")
        f = s.makefile("rwb")
        n = [0]

        def call(m):
            n[0] += 1
            f.write((json.dumps({"jsonrpc": "2.0", "id": n[0], "method": m}) + "\n").encode())
            f.flush()
            return json.loads(f.readline())

        time.sleep(1.5)
        call("ui.getTree")
        time.sleep(0.5)
        a = cpu(p.pid)
        for _ in range(redraws):
            call("ui.waitSettled")
        time.sleep(0.3)
        b = cpu(p.pid)
        call("app.quit")
        return (b - a) / redraws * 1e6
    finally:
        try:
            p.wait(timeout=3)
        except subprocess.TimeoutExpired:
            p.kill()


def main():
    without, with_, runs, redraws = sys.argv[1], sys.argv[2], int(sys.argv[3] if len(sys.argv) > 3 else 8), int(
        sys.argv[4] if len(sys.argv) > 4 else 400)
    res = {"without": [], "with": []}
    for r in range(runs):
        for name, exe in (("without", without), ("with", with_)):
            us = one(exe, redraws)
            res[name].append(us)
            print(f"run {r + 1} {name:>7}: {us:7.1f} us CPU per idle redraw", flush=True)
    for name, v in res.items():
        print(f"{name:>7}: median {statistics.median(v):.1f} us, spread {min(v):.1f}..{max(v):.1f} us over {len(v)} runs")
    lo, hi = min(res["without"]), max(res["without"])
    m = statistics.median(res["with"])
    print(f"a11y median {m:.1f} us {'OUTSIDE' if not lo <= m <= hi else 'inside'} the no-a11y spread {lo:.1f}..{hi:.1f} us")


if __name__ == "__main__":
    main()
