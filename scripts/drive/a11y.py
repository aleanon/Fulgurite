"""M57/D7 -- every leaf on every demo tells a reader what it is.

A17 found ten widgets publishing `GenericContainer` with no value: the
agent could not read a badge's count and a screen reader met a progress
bar as an empty box. `observe_every_leaf.rs` holds a fixed tree to it;
this holds **every showcase demo**, live, so a widget added to the
catalogue is swept the day it appears.

A leaf passes if it has a role other than `GenericContainer`, or a value,
or a label, or is hidden from readers on purpose.
"""
from __future__ import annotations


def run_a11y(c, pick, names, strategy: str) -> tuple[list[str], int]:
    out, failures, leaves = [], 0, 0
    for name in names:
        pick(c, name)
        d = c.rpc("ui.resolve", selector="#demo")["bounds"]
        silent = []
        for n in c.tree()["nodes"]:
            b = n.get("bounds")
            if n.get("children") or not b or b[2] <= b[0] or b[3] <= b[1]:
                continue
            if not (b[0] >= d[0] - 1 and b[2] <= d[2] + 1 and b[1] >= d[1] - 1 and b[3] <= d[3] + 1):
                continue
            leaves += 1
            if (n.get("role") == "GenericContainer" and not n.get("value")
                    and not n.get("label") and not n.get("hidden")):
                silent.append(n.get("wid") or n.get("src") or str(n["id"]))
        if silent:
            failures += 1
            out.append(f"  FAIL {name} [{strategy}] a11y: {len(silent)} leaf/leaves say nothing: "
                       f"{', '.join(silent[:6])}")
    out.append(f"a11y [{strategy}]: {len(names)} demos, {leaves} leaves, {failures} demo(s) with a silent leaf")
    return out, failures
