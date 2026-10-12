"""M89b/T89.9: one launch of `just windows-rss`. See scripts/windows_rss.sh.

`VmRSS` of `examples/two-windows` (release, the GPU or the software
backend as the leg's FULGURITE_BACKEND says) 4 s after the process started
with one window, then 4 s after a click on `#row2` opened the second --
`docs/results-gpu.md` §5's method (`VmRSS` from `/proc/<pid>/status`,
4 s in), applied once per window. Prints one machine-readable line,
`rss: <leg> <kB with one window> <kB with two>`, which the shell script
takes the median of over three launches.
"""
import json, os, socket, sys, time

port, pid, leg = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
s = socket.create_connection(("127.0.0.1", port), timeout=60)
f = s.makefile("rwb")
n = [0]


def call(method, params=None, **top):
    n[0] += 1
    req = {"jsonrpc": "2.0", "id": n[0], "method": method, "params": params or {}}
    req.update(top)
    f.write((json.dumps(req) + "\n").encode())
    f.flush()
    r = json.loads(f.readline())
    if "error" in r:
        sys.exit(f"windows-rss[{leg}]: {method} failed: {r['error']}")
    return r


def rss_kb():
    for line in open(f"/proc/{pid}/status"):
        if line.startswith("VmRSS:"):
            return int(line.split()[1])
    sys.exit(f"windows-rss[{leg}]: no VmRSS for pid {pid}")


def age_s():
    """Seconds since the process started, from /proc (clock ticks)."""
    start = int(open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()[19])
    up = float(open("/proc/uptime").read().split()[0])
    return up - start / os.sysconf("SC_CLK_TCK")


time.sleep(max(0.0, 4.0 - age_s()))
keys = [w["key"] for w in call("app.windows")["result"]["windows"]]
if keys != [1]:
    sys.exit(f"windows-rss[{leg}]: setup: app.windows {keys} before the click (want [1])")
one, at = rss_kb(), age_s()
# `opened` is a top-level member of the click's reply (M89/T89.3).
opened = call("input.click", {"selector": "#row2"}).get("opened")
t = time.time()
if opened != [2]:
    sys.exit(f"windows-rss[{leg}]: setup: the click on #row2 opened {opened!r} (want [2])")
# The second window's first present, then the rest of its 4 s.
while call("app.presents", window=2)["result"]["presents"] < 1 and time.time() - t < 4.0:
    time.sleep(0.05)
time.sleep(max(0.0, 4.0 - (time.time() - t)))
keys = [w["key"] for w in call("app.windows")["result"]["windows"]]
if keys != [1, 2]:
    sys.exit(f"windows-rss[{leg}]: setup: app.windows {keys} after the click (want [1, 2])")
two = rss_kb()
print(f"  ..  {leg}: VmRSS {one} kB with one window ({at:.1f} s in), "
      f"{two} kB with two (4 s after the open); delta {two - one} kB")
print(f"rss: {leg} {one} {two}")
