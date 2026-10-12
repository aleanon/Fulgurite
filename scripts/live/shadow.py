#!/usr/bin/env python3
"""The shadow oracle's per-leg check -- M42/T42.4, shared by M41/T41.2b.

    python3 scripts/live/shadow.py TAG

Run by a live drive *after* its own assertions, against the window the
launcher (`scripts/live/launch.py`) started: the port comes from
`FULGURITE_LIVE_PORT`, and what the leg declared from
`FULGURITE_LIVE_EXPECT_FEATURES`. One copy, so that every drive's oracle leg
asks the same three questions in the same order.

**"Cannot say" is never "no"** (I-NEW-3). A leg that expects the oracle
asserts, in order:

1. `supported` -- the build has it;
2. `compared > 0` -- it judged at least one frame, because a quiet oracle
   that compared nothing proves nothing;
3. `skipped > 0` -- at least one of those was a frame M53/T53.3's idle
   fast path skipped, so a run in which nothing was skipped cannot pass
   as one in which skipping was checked (`docs/plan-frame-cost.md`
   T53.3); every drive's own reads ask for frames with nothing changed;
4. `first is None` -- no bounded frame differed from the whole-tree one.

A leg that does not expect it asserts `supported` is false. That is the
plain `Boundaries` leg M41 §0 rule 2 asks every drive to keep: the oracle
repairs the frames *after* the one it judges (M41/T41.2,
`Host::shadow_check`'s rustdoc), so a drive with it compiled in reports a
defect once and then hides it, and only a leg without it judges
`Boundaries` as it ships.

The launcher's own feature check (M42/T42.2) refuses a mis-built leg
first; this is the drive's answer from the running process, which is the
one that must agree.

Prints, always: `TAG: shadow: supported=S expected=E, compared N (K
skipped by the idle fast path)`, and on
a divergence the node's index, its author id when it has one, its kind
and the field -- `field: "paint"` means the states agreed and the painted
lists did not (state a widget keeps for itself).
"""
from __future__ import annotations

import json
import os
import socket
import sys


def main() -> int:
    tag = sys.argv[1] if len(sys.argv) > 1 else "drive"
    port = int(os.environ["FULGURITE_LIVE_PORT"])
    expected = "shadow-oracle" in os.environ.get("FULGURITE_LIVE_EXPECT_FEATURES", "").split(",")
    s = socket.create_connection(("127.0.0.1", port), timeout=15)
    f = s.makefile("rwb")
    n = [0]

    def call(method, params=None):
        n[0] += 1
        f.write((json.dumps({"jsonrpc": "2.0", "id": n[0], "method": method,
                             "params": params or {}}) + "\n").encode())
        f.flush()
        r = json.loads(f.readline())
        if "result" not in r:
            sys.exit(f"{tag}: {method} failed: {r}")
        return r["result"]

    sh = call("app.shadow")
    supported = sh.get("supported") is True
    print(f"{tag}: shadow: supported={supported} expected={expected}, "
          f"compared {sh.get('compared', 0)} ({sh.get('skipped', 0)} skipped by the idle fast path)",
          flush=True)
    if not expected:
        assert not supported, f"{tag}: shadow oracle present in a leg that does not expect it: {sh!r}"
        return 0
    assert supported, f"{tag}: expected the shadow oracle in this build: {sh!r}"
    assert sh["compared"] > 0, f"{tag}: the shadow oracle never ran: {sh!r}"
    assert sh.get("skipped", 0) > 0, f"{tag}: no frame was skipped, so the idle fast path went unjudged: {sh!r}"
    first = sh["first"]
    if first is not None:
        # Name the node the way a reader would look for it: a table index
        # is a selector, and `ui.resolve` answers with its kind, author id
        # and source line.
        who = ""
        try:
            r = call("ui.resolve", {"selector": str(first.get("node"))})
            who = (f" ({r.get('kind', '?')} #{r.get('wid') or '-'}, "
                   f"{r.get('src') or 'no src'})")
        except SystemExit:
            pass
        raise AssertionError(
            f"{tag}: a bounded frame diverged from the whole-tree one: frame "
            f"{first.get('frame')}, node {first.get('node')}{who}, field {first.get('field')!r}")
    print(f"{tag}: shadow oracle compared {sh['compared']} frames ({sh['skipped']} skipped), none diverged",
          flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
