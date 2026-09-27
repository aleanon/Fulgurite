#!/usr/bin/env python3
"""The one launcher every live script uses — M42/T42.2
(`docs/plan-drive-identity.md` §2.3).

A live gate is only evidence about the process it drove, so this names
that process and refuses any other. Until M42 each script launched its
own window: a leftover window holding the port answered for a build
that had failed to bind, and both "strategy" legs of `just widget-drive`
drove that one stranger and printed `0 failed` (R18-F1). The strategy a
leg reported was a label the script set for itself.

In order, for one leg:

1. **A display**, or exit 77 — M43 §2's skip code. What 77 means to an
   aggregate is M43's policy; this is only where the code lives.
2. **A port**: `--port`, else `LUMEN_LIVE_PORT`, else a free one —
   from `LUMEN_LIVE_PORT_RANGE` (`7880-7899`) when set, from the kernel
   otherwise — and **a port that already answers is refused** before
   anything is launched: `port already answering on 127.0.0.1:P (owner
   pid Q)`.
3. **The binary itself**, not `cargo run` (whose pid is cargo's), with
   `LUMEN_AGENT_ADDR` set (M48/T48.11 settled on the one name, R13-F10),
   and its log at `target/live/<script>-<leg>.log` — per
   worktree, per leg, so two worktrees or two legs never share one.
4. **Waits for the port while its own child is alive**, and prints the
   log's tail if the child dies first: `child exited 3: agent: cannot
   bind ...` (M42/T42.1 made a failed bind exit 3).
5. **Identity, twice, independently.** The kernel's answer — the
   listening socket's owner, from `ss -ltnpH`, or `/proc/net/tcp` inode
   matching when `ss` shows no owner — must be the launched pid. Then the
   process's own answer — `app.diagnostics`' `process` block
   (`AgentHost::process_info`) — must name the same pid, the leg's
   declared strategy and exactly the leg's declared features among the
   ones that change what a drive can observe (`shadow-oracle`,
   `reload`; the strategy is its own check since M55/T55.4). `--identity socket` skips the second, for
   binaries that predate it (T42.8's re-run of old commits).
   It prints: `drove pid P (socket owner P), strategy S, features F, exe E`.
6. **The drive**, with `{port}`, `{addr}`, `{pid}`, `{log}` substituted in
   its argv and `LUMEN_AGENT_ADDR`, `LUMEN_LIVE_PORT`, `LUMEN_LIVE_PID`,
   `LUMEN_LIVE_STRATEGY` (the process's, not the script's),
   `LUMEN_LIVE_FEATURES` and `LUMEN_LIVE_EXPECT_FEATURES` in its
   environment.
7. **Stop** (M42/T42.3, R18-F9): the child must still be alive — one
   that died mid-drive fails the leg with its status — and its log must
   hold exactly `--expect-panics` lines containing `panicked at` (0 unless
   the leg provokes panics on purpose); it prints `app log: N panics`.
   **Only then** is it stopped: killed, or with `--stop quit` asked to
   quit over `app.quit` and required to **exit 0** — a panic on the way
   out is visible only in that status, which is why the log is read
   first. **Every window with an endpoint quits** (M48/T48.13, R13-F9:
   until then only counter-window had `app.quit`): the default `--stop`
   is `quit` with an agent and `kill` without; the exit is timed against
   a 2 s bound and the port must stop listening, and the log scan above
   stays as the second check.

Exit status: the drive's when it failed, 1 for a refusal of the
launcher's own, 77 for no display, 0 otherwise.

Without an agent (`--no-agent`: `gpu_parity`, `wake_gate`) steps 2, 4
and 5 are skipped; `--settle S` waits S seconds, checking the child is
alive, instead of waiting for a port.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
#: The features a leg declares, because they change what a drive can
#: observe: `shadow-oracle` decides whether `app.shadow` can say
#: anything, `reload` whether `app.failure` can. **Not the strategy**
#: since M55/T55.4: it was the `everything` feature, and is now the
#: session's type parameter, checked by `--expect-strategy` alone.
TRACKED = ("shadow-oracle", "reload")
NAMES = {"shadow-oracle": "the shadow oracle", "reload": "panic recovery (reload)"}


def say(tag: str, msg: str) -> None:
    print(f"{tag}: {msg}", flush=True)


def fail(tag: str, msg: str, code: int = 1):
    print(f"{tag}: FAIL -- {msg}", file=sys.stderr, flush=True)
    raise SystemExit(code)


def has_display() -> bool:
    return bool(os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"))


# -- who owns a port ---------------------------------------------------------

def answering(port: int) -> bool:
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.5):
            return True
    except OSError:
        return False


def _owners_ss(port: int) -> list[int]:
    try:
        out = subprocess.run(["ss", "-ltnpH", f"sport = :{port}"],
                             capture_output=True, text=True, timeout=5).stdout
    except (OSError, subprocess.TimeoutExpired):
        return []
    return sorted({int(p) for p in re.findall(r"pid=(\d+),", out)})


def _owners_proc(port: int) -> list[int]:
    """The fallback §5 names: listening inodes from /proc/net/tcp{,6},
    matched against every /proc/<pid>/fd we may read."""
    inodes = set()
    for table in ("/proc/net/tcp", "/proc/net/tcp6"):
        try:
            lines = open(table).read().splitlines()[1:]
        except OSError:
            continue
        for ln in lines:
            f = ln.split()
            if len(f) > 9 and f[3] == "0A" and int(f[1].rsplit(":", 1)[1], 16) == port:
                inodes.add(f[9])
    pids = set()
    for d in Path("/proc").iterdir():
        if not d.name.isdigit():
            continue
        try:
            for fd in (d / "fd").iterdir():
                link = os.readlink(fd)
                if link.startswith("socket:[") and link[8:-1] in inodes:
                    pids.add(int(d.name))
        except OSError:
            continue
    return sorted(pids)


def owners(port: int) -> tuple[list[int], str]:
    o = _owners_ss(port)
    if o:
        return o, "ss"
    return _owners_proc(port), "/proc"


def free_port() -> int:
    rng = os.environ.get("LUMEN_LIVE_PORT_RANGE")
    if rng:
        lo, hi = (int(x) for x in rng.split("-"))
        for p in range(lo, hi + 1):
            if answering(p) or owners(p)[0]:
                continue
            try:
                with socket.socket() as s:
                    s.bind(("127.0.0.1", p))
                return p
            except OSError:
                continue
        raise SystemExit(f"launch: no free port in LUMEN_LIVE_PORT_RANGE={rng}")
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


# -- the agent ----------------------------------------------------------------

def rpc(port: int, method: str, params: dict | None = None, timeout: float = 15.0) -> dict:
    with socket.create_connection(("127.0.0.1", port), timeout=timeout) as s:
        f = s.makefile("rwb")
        f.write((json.dumps({"jsonrpc": "2.0", "id": 1, "method": method,
                             "params": params or {}}) + "\n").encode())
        f.flush()
        line = f.readline()
    if not line:
        raise RuntimeError(f"{method}: the connection closed without a reply")
    r = json.loads(line)
    if "error" in r:
        raise RuntimeError(f"{method}: {r['error']}")
    return r["result"]


def log_tail(log: Path, n: int = 15) -> str:
    try:
        lines = log.read_text(errors="replace").splitlines()
    except OSError:
        return "(no log)"
    return "\n".join("    | " + ln for ln in lines[-n:])


def last_line(log: Path) -> str:
    try:
        lines = [ln for ln in log.read_text(errors="replace").splitlines() if ln.strip()]
    except OSError:
        return ""
    return lines[-1] if lines else ""


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--script", required=True, help="the live script's name, for messages and the log path")
    ap.add_argument("--leg", default="only")
    ap.add_argument("--display-only", action="store_true",
                    help="only step 1: exit 77 without a display, 0 with one")
    ap.add_argument("--exe")
    ap.add_argument("--arg", action="append", default=[], help="an argument for the binary; `--arg=--flag` for one that starts with a dash")
    ap.add_argument("--env", action="append", default=[], help="K=V for the binary's environment")
    ap.add_argument("--port", type=int)
    ap.add_argument("--no-agent", action="store_true")
    ap.add_argument("--settle", type=float, default=0.0)
    ap.add_argument("--timeout", type=float, default=60.0, help="seconds to wait for the port")
    ap.add_argument("--identity", choices=("full", "socket"), default="full")
    ap.add_argument("--expect-strategy",
                    help="what the process must report, by its last path segment with or without "
                         "its parameters: `Everything`, `Boundaries` or `Boundaries<256>`")
    ap.add_argument("--expect-features", default="",
                    help=f"comma-separated, among {', '.join(TRACKED)}: exactly these, no others")
    ap.add_argument("--stop", choices=("kill", "quit", "wait"), default=None,
                    help="kill: check the app is alive and its log clean, then kill it; "
                         "quit: the same checks, then `app.quit` and its exit status must be 0; "
                         "wait: the app is the test, and runs to its own exit within --timeout. "
                         "Default: quit with an agent, kill without (M48/T48.13)")
    ap.add_argument("--expect-exit",
                    help="the app must exit by itself after the drive: an integer, or `nonzero`")
    ap.add_argument("--expect-panics", type=int, default=0,
                    help="how many `panicked at` lines the log must hold -- the panics this leg "
                         "provokes on purpose. Exact, so one more is still a failure")
    ap.add_argument("drive", nargs=argparse.REMAINDER)
    a = ap.parse_args()
    if a.stop is None:
        # Every window with an endpoint quits over `app.quit` and must exit
        # 0 within 2 s (M48/T48.13, §2.5 of plan-drive-identity.md).
        a.stop = "kill" if a.no_agent else "quit"
    tag = f"{a.script} [{a.leg}]"

    # 1. a display
    if not has_display():
        say(a.script, "no display -- skipped (77)")
        return 77
    if a.display_only:
        return 0
    if not a.exe:
        fail(tag, "--exe is required")
    drive = a.drive[1:] if a.drive[:1] == ["--"] else a.drive
    expect = {f for f in a.expect_features.split(",") if f}
    unknown = expect - set(TRACKED)
    if unknown:
        fail(tag, f"--expect-features names {sorted(unknown)}, not among {TRACKED}")

    env = dict(os.environ)
    env.setdefault("RUST_BACKTRACE", "1")
    for kv in a.env:
        k, _, v = kv.partition("=")
        env[k] = v

    # 2. a port nobody else answers on
    port = None
    if not a.no_agent:
        port = a.port or int(os.environ.get("LUMEN_LIVE_PORT") or 0) or free_port()
        if answering(port):
            o, how = owners(port)
            fail(tag, f"port already answering on 127.0.0.1:{port} "
                      f"(owner pid {', '.join(map(str, o)) or '?'}, by {how}) -- refusing to "
                      f"launch into it: whatever answers is not the build this leg names")
        addr = f"127.0.0.1:{port}"
        env["LUMEN_AGENT_ADDR"] = addr

    # 3. the binary itself, its log per script and leg
    logdir = ROOT / "target" / "live"
    logdir.mkdir(parents=True, exist_ok=True)
    safe_leg = re.sub(r"[^A-Za-z0-9_.-]", "_", a.leg)
    log = logdir / f"{a.script}-{safe_leg}.log"
    logf = open(log, "wb")
    child = subprocess.Popen([a.exe, *a.arg], env=env, stdout=logf, stderr=subprocess.STDOUT,
                             stdin=subprocess.DEVNULL, start_new_session=True)
    say(tag, f"launched {a.exe} as pid {child.pid}"
             + (f" on 127.0.0.1:{port}" if port else "") + f", log {log.relative_to(ROOT)}")

    def stop() -> int | None:
        """Kill what is still running. The verdict on how it ended is
        the caller's."""
        if child.poll() is None:
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
        logf.close()
        return child.returncode

    try:
        # 4. wait for the port, while the child lives
        if port:
            deadline = time.time() + a.timeout
            while not answering(port):
                if child.poll() is not None:
                    print(log_tail(log), file=sys.stderr)
                    fail(tag, f"child exited {child.returncode}: {last_line(log)}")
                if time.time() > deadline:
                    print(log_tail(log), file=sys.stderr)
                    fail(tag, f"127.0.0.1:{port} did not come up within {a.timeout:.0f} s")
                time.sleep(0.1)
        elif a.settle:
            deadline = time.time() + a.settle
            while time.time() < deadline:
                if child.poll() is not None:
                    print(log_tail(log), file=sys.stderr)
                    fail(tag, f"child exited {child.returncode} while settling: {last_line(log)}")
                time.sleep(0.1)

        # 5. identity: the kernel's answer, then the process's
        strategy, features = "?", []
        if port:
            o, how = owners(port)
            if o != [child.pid]:
                fail(tag, f"socket owner {', '.join(map(str, o)) or '(none visible)'} is not the "
                          f"launched pid {child.pid} (by {how})")
            if a.identity == "full":
                p = rpc(port, "app.diagnostics").get("process") or {}
                if p.get("supported") is not True:
                    fail(tag, f"the process cannot say which it is: app.diagnostics.process = {p!r}")
                if p.get("pid") != child.pid:
                    fail(tag, f"diagnostics pid {p.get('pid')} is not the launched pid {child.pid}")
                strategy = p.get("strategy", "?")
                features = list(p.get("features") or [])
                short = strategy.rsplit("::", 1)[-1]
                if a.expect_strategy and a.expect_strategy not in (short, short.split("<")[0]):
                    fail(tag, f"expected {a.expect_strategy}, got {strategy}")
                have = {f for f in features if f in TRACKED}
                for f in sorted(expect - have):
                    fail(tag, f"expected {NAMES[f]} in this build (feature `{f}`); "
                              f"the process reports features {features}")
                for f in sorted(have - expect):
                    fail(tag, f"{NAMES[f]} present in a leg that does not expect it (feature "
                              f"`{f}`); the process reports features {features}")
                say(tag, f"drove pid {child.pid} (socket owner {o[0]}), strategy {strategy}, "
                         f"features {','.join(features) or '-'}, exe {p.get('exe')}")
            else:
                say(tag, f"drove pid {child.pid} (socket owner {o[0]}, by {how}), "
                         f"identity: socket only")
        env["LUMEN_LIVE_PID"] = str(child.pid)
        env["LUMEN_LIVE_STRATEGY"] = strategy
        env["LUMEN_LIVE_FEATURES"] = ",".join(features)
        env["LUMEN_LIVE_EXPECT_FEATURES"] = ",".join(sorted(expect))
        env["LUMEN_LIVE_LOG"] = str(log)
        if port:
            env["LUMEN_LIVE_PORT"] = str(port)

        # 6. the drive
        rc = 0
        if drive:
            subst = {"{port}": str(port or ""), "{addr}": f"127.0.0.1:{port}" if port else "",
                     "{pid}": str(child.pid), "{log}": str(log)}
            argv = []
            for x in drive:
                for k, v in subst.items():
                    x = x.replace(k, v)
                argv.append(x)
            rc = subprocess.call(argv, env=env, cwd=ROOT)
            if rc:
                say(tag, f"the drive exited {rc}; the app's log ends:")
                print(log_tail(log, 30), flush=True)

        # 7. stop -- liveness and the log first, then the app's own exit
        # (M42/T42.3, R18-F9). A panic after the drive's last assertion is
        # still a panic, and a kill hides the app's exit status.
        verdict = []
        if a.stop in ("kill", "quit") and not a.expect_exit and child.poll() is not None:
            verdict.append(f"the app died during the drive: child exited {child.returncode}: "
                           f"{last_line(log)}")
        if a.expect_exit and child.poll() is None:
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
        logf.flush()
        text = log.read_text(errors="replace")
        panics = [ln for ln in text.splitlines() if "panicked at" in ln]
        allowed = a.expect_panics
        if a.expect_exit == "nonzero" or (a.expect_exit and a.expect_exit not in ("0",)):
            allowed = max(allowed, len(panics))  # this leg exists to die
        say(tag, f"app log: {len(panics)} panic{'s' if len(panics) != 1 else ''} "
                 f"(expected {allowed})")
        if len(panics) != allowed:
            verdict.append(f"app log: {len(panics)} panic{'s' if len(panics) != 1 else ''}, "
                           f"expected {allowed}; first: {panics[0] if panics else '-'}")
        quit_at = None
        if a.stop == "quit" and child.poll() is None:
            try:
                rpc(port, "app.quit")
                quit_at = time.time()
            except (OSError, RuntimeError, ValueError) as e:
                verdict.append(f"app.quit was not answered: {e}")
        if a.stop in ("quit", "wait") or a.expect_exit:
            try:
                child.wait(timeout=a.timeout if a.stop == "wait" else 10)
            except subprocess.TimeoutExpired:
                stop()
                verdict.append("the app was expected to exit and was still running after "
                               f"{a.timeout if a.stop == 'wait' else 10:.0f} s")
            code = child.returncode
            if quit_at is not None and code is not None:
                ms = (time.time() - quit_at) * 1000
                still = answering(port)
                say(tag, f"app.quit: exited in {ms:.0f} ms / bound 2000; port still listening: {still}")
                if ms > 2000:
                    verdict.append(f"app.quit: exited in {ms:.0f} ms, bound 2000")
                if still:
                    verdict.append(f"app.quit: port {port} still answers after the app exited")
            want = a.expect_exit or "0"
            ok = code != 0 if want == "nonzero" else code == int(want)
            if a.stop == "wait":
                # The app is the test: its own output is the report.
                print(log_tail(log, 40), flush=True)
            say(tag, f"app exit status: {code} (expected {want})")
            if not ok:
                verdict.append(f"app exited {code}, expected {want}")
        stop()
        if verdict:
            print(log_tail(log), file=sys.stderr)
            for v in verdict[:-1]:
                print(f"{tag}: FAIL -- {v}", file=sys.stderr)
            fail(tag, verdict[-1])
        return rc
    except SystemExit:
        stop()
        raise
    except BaseException:
        stop()
        raise


if __name__ == "__main__":
    raise SystemExit(main())
