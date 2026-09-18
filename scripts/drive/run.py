#!/usr/bin/env python3
"""Run every widget's drive against the window on LUMEN_AGENT_ADDR."""
from __future__ import annotations

import importlib
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


def main() -> int:
    only = sys.argv[1] if len(sys.argv) > 1 and sys.argv[1] else None
    strategy = os.environ.get("LUMEN_DRIVE_STRATEGY", "?")
    c = AgentClient(os.environ.get("LUMEN_AGENT_ADDR", "127.0.0.1:7656"))
    entries = inventories()
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
    for widget, sel, acts in entries:
        actions += len(acts)
        try:
            for line in run_widget(c, widget, sel, acts, strategy):
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
    print(f"widget-drive [{strategy}]: {len(entries)} widgets, {actions} actions, "
          f"{failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
