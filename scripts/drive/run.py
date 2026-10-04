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
    # M54/T54.3: every entry states its role, and no role names nothing.
    from roles import ROLES
    for w in have:
        if w not in ROLES:
            bad.append(f"{w}: no row in scripts/drive/roles.py -- say what a screen reader is told it is")
    for w in ROLES:
        if w not in have:
            bad.append(f"{w}: a role in scripts/drive/roles.py for no inventory entry")
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
    # A comma-separated list drives several inventories in one window
    # (M80/T80.5: the four text fields' IME check, one process per leg).
    wanted = {n.strip().lower() for n in only.split(",")} if only else set()
    if only:
        entries = [e for e in entries if e[0].lower() in wanted]
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
    # **The window's settled size, not its first one.** A window manager
    # may shrink the window it was asked for a moment after it maps: the
    # scale-2 pass asks 2240x1520 and gets 1920x1008. A drive that began
    # before that read the sidebar's geometry, the window moved under it,
    # and the pick's scroll loop chased a row that had changed place -- and
    # this header printed whichever size it happened to see.
    import time
    last, since = None, time.monotonic()
    deadline = since + 5.0
    while time.monotonic() < deadline:
        b = c.rpc("ui.resolve", selector="#sidebar").get("bounds")
        if b != last:
            last, since = b, time.monotonic()
        elif time.monotonic() - since >= 0.6:
            break
        time.sleep(0.1)
    # Which scale this pass really runs at: a scale-2 pass whose window
    # came up at 1 would compare nothing new and still say "Scale2".
    shot = c.rpc("ui.screenshot")
    print(f"widget-drive [{strategy}]: window {shot['width']}x{shot['height']} px at scale {shot.get('scale')}"
          f"{', full-repaint check on' if os.environ.get('LUMEN_DRIVE_FULLCHECK') == '1' else ''}")

    # M57/D5: first, on the fresh window, the agent must not change what
    # is drawn. A filtered run compares the entries it names (M81/T81.6):
    # each leg launches its own window, so nothing has driven it yet.
    from parity import run_parity  # noqa: E402
    from harness import _pick  # noqa: E402
    lines, pfail = run_parity(c, _pick, strategy, wanted or None)
    for line in lines:
        print(line, file=sys.stderr if line.startswith("  FAIL") else sys.stdout)
    failures += pfail
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

    # M57/D3: the keyboard-only pass. Every demo with a focusable node must
    # have an entry in keys.py -- coverage as data, not as a review.
    from keys import KEYS  # noqa: E402
    from harness import run_keys, _pick, _focusables_in_demo  # noqa: E402
    catalog = Path(HERE.parent.parent / "examples/showcase/src/catalog.rs").read_text()
    names = [n.split('"')[2] for n in catalog.split('entry!("')[1:]]
    if only:
        names = [n for n in names if n.lower() in wanted]
    kfail = kdone = 0
    for w in names:
        try:
            if w in KEYS:
                target, acts = KEYS[w]
                for line in run_keys(c, w, target, acts, strategy):
                    print(line)
                kdone += len(acts)
            else:
                _pick(c, w)
                f = _focusables_in_demo(c)
                if f:
                    raise Failed(f"{len(f)} focusable node(s) and no keyboard entry in keys.py")
        except (Failed, RuntimeError) as e:
            print(f"  FAIL {w} [{strategy}] keyboard: {e}", file=sys.stderr)
            kfail += 1
    print(f"keyboard-drive [{strategy}]: {len(names)} entries, {len(KEYS)} with keys, "
          f"{kdone} actions, {kfail} failed")
    failures += kfail

    # M57/D7: the accessibility sweep, every demo.
    from a11y import run_a11y  # noqa: E402
    lines, afail = run_a11y(c, _pick, names, strategy)
    for line in lines:
        print(line, file=sys.stderr if line.startswith("  FAIL") else sys.stdout)
    failures += afail
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
