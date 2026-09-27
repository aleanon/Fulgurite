#!/usr/bin/env python3
"""M57's closing check: which commits fixed each row of docs/widget-audit.md,
and which live drives check it today -- computed, not restated.

The plan asked for the audit's "fixed in" column to be checked
mechanically against the drives rather than written from memory
(docs/plan-widget-fixes.md, Order). This reads:

- the audit's widget table (row `#` is the plan's B number) and its X table;
- the plan, for which A items a B entry "closes on" and which A item
  carries each X;
- `git log main..HEAD`, for the commits whose subjects name those ids;
- the drive modules, for which widgets have a pointer inventory, a
  keyboard entry, and which the parity pass skips.

`--write` rewrites the two tables with the columns added; without it the
rows are printed, and a row with no commit is listed as a failure.
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "scripts/drive"))

AUDIT = ROOT / "docs/widget-audit.md"
PLAN = ROOT / "docs/plan-widget-fixes.md"


def commits() -> list[tuple[str, str]]:
    out = subprocess.run(["git", "log", "--format=%h %s", "main..HEAD"], cwd=ROOT,
                         capture_output=True, text=True, check=True).stdout
    return [tuple(l.split(" ", 1)) for l in out.splitlines() if l]


def naming(log, ident: str) -> list[str]:
    pat = re.compile(rf"(?<![A-Za-z0-9]){re.escape(ident)}(?![0-9])")
    return [h for h, s in log if "M57" in s and pat.search(s)]


def plan_b_closes(plan: str) -> dict[int, list[str]]:
    """B n -> the A ids its plan entry says it closes on."""
    out = {}
    for m in re.finditer(r"^- \*\*B(\d+)[ ,][^\n]*(?:\n(?!- \*\*B\d).*)*", plan, re.M):
        out[int(m.group(1))] = sorted(set(re.findall(r"\bA(\d+)\b", m.group(0))), key=int)
    return out


def plan_x_to_a(plan: str) -> dict[str, list[str]]:
    """X id -> every A item whose heading names it (X7 is two: A9's
    release rule and A10's key repeat)."""
    out: dict[str, list[str]] = {}
    for m in re.finditer(r"^### (A\d+)\.[^\n]*\((X\d+)", plan, re.M):
        out.setdefault(m.group(2), []).append(m.group(1))
    return out


def drives() -> tuple[set, set, set]:
    import importlib
    import pkgutil
    pointer = set()
    for m in pkgutil.iter_modules([str(ROOT / "scripts/drive/widgets")]):
        for entry in getattr(importlib.import_module(f"widgets.{m.name}"), "ALL", []) or []:
            pointer.add(entry[0])
    from keys import KEYS
    import parity
    animated = {n for n, a in parity._entries() if a}
    return pointer, set(KEYS), animated


def main() -> int:
    write = "--write" in sys.argv
    log = commits()
    plan = PLAN.read_text()
    closes = plan_b_closes(plan)
    x_to_a = plan_x_to_a(plan)
    pointer, keyed, animated = drives()
    text = AUDIT.read_text()
    lines = text.split("\n")
    missing = []
    for k, line in enumerate(lines):
        # The audit's own cells only (verdict, what is wrong): a table
        # already written has the computed two after them, and a rerun
        # replaces those rather than appending to them.
        m = re.match(r"^\| (\d+) \| ([A-Za-z]+) \| ([^|]* \| [^|]*?)(?: \| [^|]*){0,2} \|$", line)
        if m:
            n, name = int(m.group(1)), m.group(2)
            fixed = naming(log, f"B{n}")
            via = ""
            if not fixed:
                ids = closes.get(n, [])
                fixed = sorted({h for a in ids for h in naming(log, f"A{a}")})
                via = " (via " + ", ".join(f"A{a}" for a in ids) + ")" if ids else ""
            if not fixed:
                missing.append(f"B{n} {name}")
            checks = ["a11y"]
            if name in pointer:
                checks.insert(0, "pointer")
            if name in keyed:
                checks.insert(1 if "pointer" in checks else 0, "keyboard")
            checks.append("parity" if name not in animated else "parity skipped (animates)")
            cell = (", ".join(f"`{h}`" for h in fixed) or "**none**") + via
            lines[k] = f"| {n} | {name} | {m.group(3)} | {cell} | {', '.join(checks)} |"
        elif re.match(r"^\| # \| widget \| verdict \| what is wrong \|$", line):
            lines[k] = "| # | widget | verdict | what is wrong | fixed in | checked by |"
        elif line == "|---|---|---|---|" and k > 0 and "| # | widget |" in lines[k - 1]:
            lines[k] = "|---|---|---|---|---|---|"
        x = re.match(r"^\| (X\d+) \| ([^|]*) \| ([^|]*?)(?: \| [^|]*)? \|$", line)
        if x and x.group(1) in x_to_a:
            cells = []
            for a in x_to_a[x.group(1)]:
                fixed = naming(log, a)
                if not fixed:
                    missing.append(f"{x.group(1)} ({a})")
                cells.append(f"{a}: " + (", ".join(f"`{h}`" for h in fixed) or "**none**"))
            lines[k] = f"| {x.group(1)} | {x.group(2)} | {x.group(3)} | " + "; ".join(cells) + " |"
        elif line == "| id | defect | widgets |":
            lines[k] = "| id | defect | widgets | fixed in |"
        elif line == "|---|---|---|" and k > 0 and lines[k - 1].startswith("| id | defect"):
            lines[k] = "|---|---|---|---|"
    if write:
        AUDIT.write_text("\n".join(lines))
    else:
        for l in lines:
            if re.match(r"^\| (\d+|X\d+) \|", l):
                print(l[-160:])
    if missing:
        print("no commit found for: " + ", ".join(missing), file=sys.stderr)
        return 1
    print(f"audit: every row has a commit ({'written' if write else 'dry run'})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
