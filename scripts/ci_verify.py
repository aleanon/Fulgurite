#!/usr/bin/env python3
"""`just ci-verify [sha] [--allow-skip]`: is there a green `just ci` record
for this commit? (M43/T43.13, docs/plan-gate-hygiene.md §2.1.)

A milestone is not done until this passes for the commit that closed it
(docs/plan-m38-fixes.md §5). It reads `target/ci/<sha>.json`, which
scripts/ci.sh writes, and refuses, by name:

  * no record -- `no CI record for <sha>`;
  * a failed step, or a tree the run left changed (rule 5);
  * a skipped step -- even one FULGURITE_ALLOW_SKIP=1 accepted at run time --
    unless `--allow-skip` is given here too. A green record must never
    hide a skip, so accepting one is said twice: when it ran, and when it
    is cited.

`<sha>.only.json` (an `--only` run) is never read: a partial run is not
a record of the gate set.

On success it prints the one line the closing commit's body carries.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    args = [a for a in sys.argv[1:] if a]
    allow = "--allow-skip" in args
    rest = [a for a in args if a != "--allow-skip"]
    rev = rest[0] if rest else "HEAD"
    try:
        sha = subprocess.run(["git", "rev-parse", "--verify", f"{rev}^{{commit}}"], cwd=ROOT,
                             check=True, capture_output=True, text=True).stdout.strip()
    except subprocess.CalledProcessError:
        print(f"ci-verify: no such commit: {rev}", file=sys.stderr)
        return 2
    path = ROOT / "target" / "ci" / f"{sha}.json"
    if not path.exists():
        print(f"ci-verify: no CI record for {sha} -- run `just ci {sha[:12]}`", file=sys.stderr)
        return 1
    rec = json.loads(path.read_text())
    if rec.get("sha") != sha or rec.get("only"):
        print(f"ci-verify: {path} is not a full record of {sha}", file=sys.stderr)
        return 1
    steps = rec.get("steps", [])
    bad = []
    for s in steps:
        if s["status"] == "fail":
            bad.append(f"step {s['name']} failed (exit {s['exit']})")
        elif s["status"] == "skip" and not allow:
            why = "accepted at run time by FULGURITE_ALLOW_SKIP=1" if rec.get("allow_skip") else "refused"
            bad.append(f"step {s['name']} skipped ({why}); --allow-skip accepts it")
    if not rec.get("tree_clean", False):
        bad.append("tree changed during the run: " + " ".join(rec.get("tree_changes", [])))
    if not steps:
        bad.append("the record has no steps")
    short = sha[:12]
    if bad:
        for b in bad:
            print(f"ci-verify {short}: {b}", file=sys.stderr)
        return 1
    nskip = sum(s["status"] == "skip" for s in steps)
    print(f"ci-verify {short}: green -- {len(steps)} steps, 0 failed, {nskip} skipped, tree clean, "
          f"{rec.get('seconds', '?')} s on {rec.get('host', '?')} at {rec.get('date', '?')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
