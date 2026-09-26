#!/usr/bin/env python3
"""Run every widget's drive against the window on LUMEN_AGENT_ADDR."""
from __future__ import annotations

import importlib
import json
import os
import pkgutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))

from agent_client import AgentClient  # noqa: E402
from harness import Failed, run_widget  # noqa: E402


def inventories():
    """Every (widget, selector, actions) any module under `widgets/`
    exports as `ALL`.

    Discovered rather than listed, so adding a widget is one file — and
    so a module that exists but exports nothing is visible as a widget
    with no inventory rather than as a silent omission.
    """
    found = []
    for m in pkgutil.iter_modules([str(HERE / "widgets")]):
        mod = importlib.import_module(f"widgets.{m.name}")
        entries = getattr(mod, "ALL", None)
        if not entries:
            print(f"widget-drive: {m.name} exports no ALL — nothing to drive", file=sys.stderr)
            continue
        found.extend(entries)
    return found


def counts(entries) -> dict:
    """The quantities the manifest pins, measured from the inventories."""
    acts = [a for e in entries for a in e[2]]
    return {
        "entries": len(entries),
        "actions": len(acts),
        "landed_checked": sum(1 for a in acts if not a.static),
        "isolated": sum(1 for a in acts if a.isolate is not None),
    }


def check_manifest(entries) -> list[str]:
    """Compare what was discovered with `expected.json` — M42/T42.5.

    **Before any window opens**, and against a file the drive did not
    write. The coverage check this replaces built its `missing` list from
    the same actions the loop had just walked, so it could not fire, and
    the review's I5b/I5c (Radio's inventory deleted, one Switch row
    deleted) ran green at 38/92 against 39/94 (R18-F2). A removal fails
    here; so does an addition, until the manifest is edited to name it.
    """
    m = json.loads((HERE / "expected.json").read_text())
    want = m["entries"]
    have = {e[0]: [a.name for a in e[2]] for e in entries}
    bad = []
    for w in want:
        if w not in have:
            bad.append(f"manifest expects {w}, which no module under widgets/ exports")
            continue
        for name in want[w]:
            if name not in have[w]:
                bad.append(f"{w}: missing action {name!r}")
        for name in have[w]:
            if name not in want[w]:
                bad.append(f"{w}: action {name!r} is not in the manifest -- add it to "
                           f"scripts/drive/expected.json")
    for w in have:
        if w not in want:
            bad.append(f"{w}: an inventory the manifest does not name -- add it to "
                       f"scripts/drive/expected.json")
    static = sorted(f"{e[0]}/{a.name}" for e in entries for a in e[2] if a.static)
    for s_ in sorted(set(m["static"]) - set(static)):
        bad.append(f"{s_}: the manifest lists it static, and it is checked for landing now "
                   f"-- remove it from `static`")
    for s_ in sorted(set(static) - set(m["static"])):
        bad.append(f"{s_}: static (not checked for landing), and the manifest does not say so")
    got = counts(entries)
    print(f"inventories: {got['entries']} entries / {got['actions']} actions "
          f"(manifest {m['counts']['entries']}/{m['counts']['actions']}); "
          f"actions {got['actions']}, landed-checked {got['landed_checked']}, "
          f"isolated {got['isolated']} (manifest {m['counts']['actions']}, "
          f"{m['counts']['landed_checked']}, {m['counts']['isolated']})")
    for k, v in m["counts"].items():
        if got.get(k) != v:
            bad.append(f"count {k}: discovered {got.get(k)}, manifest {v}")
    return bad


def main() -> int:
    if sys.argv[1:2] == ["--manifest-only"]:
        bad = check_manifest(inventories())
        for b in bad:
            print(f"widget-drive: {b}", file=sys.stderr)
        return 1 if bad else 0
    only = sys.argv[1] if len(sys.argv) > 1 and sys.argv[1] else None
    # The strategy the *process* reported, through the launcher (M42/
    # T42.2) -- never a label a script set for itself. Printed short.
    strategy = os.environ.get("LUMEN_LIVE_STRATEGY", "?").rsplit("::", 1)[-1]
    addr = os.environ.get("LUMEN_AGENT_ADDR")
    if not addr:
        print("widget-drive: no LUMEN_AGENT_ADDR -- run through scripts/widget_drive.sh, "
              "whose launcher names the process it drives", file=sys.stderr)
        return 2
    c = AgentClient(addr)
    entries = inventories()
    bad = check_manifest(entries)
    if bad:
        for b in bad:
            print(f"widget-drive: {b}", file=sys.stderr)
        return 1
    if only:
        entries = [e for e in entries if e[0].lower() == only.lower()]
        if not entries:
            print(f"widget-drive: no inventory named {only!r}", file=sys.stderr)
            return 2
    if not entries:
        # A runner that drives nothing must not exit 0: that is the gate
        # reporting success for having done no work, which is the shape
        # AGENT.md's table is full of.
        print("widget-drive: no inventories found at all", file=sys.stderr)
        return 2

    failures = 0
    actions = 0
    bare = 0
    for widget, sel, acts, *root in entries:
        actions += len(acts)
        try:
            bare += sum(
                1 for a in acts if a.isolate is None and not a.static
            )
            for line in run_widget(c, widget, sel, acts, strategy, *root):
                print(line)
        except Failed as e:
            print(f"  FAIL {widget} [{strategy}]: {e}", file=sys.stderr)
            failures += 1
        except RuntimeError as e:
            # An RPC that errored rather than an assertion that failed —
            # a refused selector, a method that does not exist. Counted
            # and carried on: one widget's broken drive must not hide the
            # other twenty's results, which is what an uncaught exception
            # did the first time this happened.
            print(f"  ERROR {widget} [{strategy}]: {e}", file=sys.stderr)
            failures += 1
    n = counts(entries)
    print(f"widget-drive [{strategy}]: {len(entries)} widgets, {actions} actions, "
          f"{bare} without an isolate, {failures} failed; "
          f"actions {n['actions']}, landed-checked {n['landed_checked']}, "
          f"isolated {n['isolated']}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
