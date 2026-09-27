"""M57/D5 -- the agent does not change what is drawn.

A3's defect (the agent's audit left estimated layout behind: RichText
split its words, TextInput's caret died, strip labels wrapped mid-word)
was invisible to every headless test, because no headless test had the
agent attached, and the live drives had nothing to compare against. This
compares, for every catalogue entry, the live window's demo -- agent
attached, as the drives always are -- with the same entry rendered
headless by `showcase --demo`, which has no agent at all. Byte for byte,
over the demo's box.

It runs on a **fresh** window, before any drive has changed a demo's
state, and skips the entries that animate: their phase depends on the
clock, and differs between two renders by design.
"""
from __future__ import annotations

import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
BIN = ROOT / "target/release/showcase"


def _entries() -> list[tuple[str, bool]]:
    """(name, animated) for every catalogue entry, in order."""
    src = (ROOT / "examples/showcase/src/catalog.rs").read_text()
    out = []
    for chunk in src.split('entry!("')[1:]:
        name = chunk.split('"')[2]
        animated = re.match(r'[^\n]*\n\s*(true|false),', chunk.split('",', 2)[2])
        out.append((name, bool(animated and animated.group(1) == "true")))
    return out


def _fnv_rgba(rgb: bytes) -> str:
    """The agent's `ui.regionHash` over RGBA, from the headless RGB crop
    (the canvas is opaque, so alpha is 255 throughout)."""
    h = 0xCBF29CE484222325
    for k in range(0, len(rgb), 3):
        for b in (rgb[k], rgb[k + 1], rgb[k + 2], 255):
            h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"{h:016x}"


def _headless(index: int) -> tuple[list[float], bytes]:
    """Render entry `index` headless; answer its rect and its RGB bytes."""
    with tempfile.TemporaryDirectory() as d:
        path = Path(d) / "demo.ppm"
        res = subprocess.run([str(BIN), "--demo", str(index), str(path)],
                             capture_output=True, text=True, check=True)
        m = re.search(r"(\d+)x(\d+) at \((\d+),(\d+)\)", res.stdout + res.stderr)
        if not m:
            raise RuntimeError(f"--demo {index} printed no rect: {res.stdout!r}")
        w, h, x, y = (int(v) for v in m.groups())
        data = path.read_bytes()
        # P6 header: magic, width, height, maxval, then one byte of space.
        parts = data.split(b"\n", 3) if data.startswith(b"P6\n") else None
        if parts and len(parts) == 4:
            body = parts[3]
        else:
            hdr = re.match(rb"P6\s+\d+\s+\d+\s+\d+\s", data)
            body = data[hdr.end():]
        return [float(x), float(y), float(x + w), float(y + h)], body


def run_parity(c, pick, strategy: str) -> tuple[list[str], int]:
    """Compare every non-animated entry; answer the lines and the failures."""
    out, failures, checked, skipped = [], 0, 0, []
    for index, (name, animated) in enumerate(_entries()):
        if animated:
            skipped.append(name)
            continue
        rect, rgb = _headless(index)
        pick(c, name)
        c.rpc("ui.lastChange")
        live = c.rpc("ui.regionHash", rect=rect)
        want = _fnv_rgba(rgb)
        checked += 1
        if live.get("hash") != want:
            failures += 1
            out.append(f"  FAIL {name} [{strategy}] parity: the live demo with the agent "
                       f"differs from the headless render over {live.get('pixels')} px at {rect}")
    out.append(f"parity [{strategy}]: {checked - failures} of {checked} entries byte-identical "
               f"to a headless render with no agent; skipped (animated): {', '.join(skipped) or 'none'}")
    return out, failures
