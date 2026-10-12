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
clock, and differs between two renders by design. A filtered run
(`widget_drive.sh TextInput`) compares the entries it names: each leg
launches its own window, so it is as fresh as a full run's.

**Both renders take one shaping mode** (M81/T81.6): `--demo` prints its
stack's mode, and it must be the leg's -- `Complex` when the process
reports the `shaping` feature (the showcase's sets it), `Basic` when not.
A `--demo` built another way would differ from a `shaping` window's
right-to-left entry by construction, and agree with it about every Latin
one.
"""
from __future__ import annotations

import os
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
BIN = ROOT / "target/release/showcase"


def _entries() -> list[tuple[str, bool]]:
    """(name, animated) for every catalogue entry, in order -- **asked of
    the binary**, `showcase --entries`, never parsed from `catalog.rs`.

    The source text is not the build: an entry behind `#[cfg(feature =
    ..)]` is in the text and not in a build without the feature. Counting
    the text put M83/T83.6's `ShaderView` (behind the showcase's `shader`)
    at index 12 in a default build that has no such entry, so every later
    `--demo N` rendered the entry after the one the live window picked --
    Button's parity compared a CheckBox (`docs/plan-gpu-extension.md`,
    "What building found")."""
    res = subprocess.run([str(BIN), "--entries"], capture_output=True, text=True, check=True)
    out = []
    for line in res.stdout.splitlines():
        name, _, animated = line.partition("\t")
        out.append((name, animated == "true"))
    if not out:
        raise RuntimeError(f"{BIN} --entries listed nothing: {res.stdout!r}")
    return out


def _fnv_rgba(rgb: bytes) -> str:
    """The agent's `ui.regionHash` over RGBA, from the headless RGB crop
    (the canvas is opaque, so alpha is 255 throughout)."""
    h = 0xCBF29CE484222325
    for k in range(0, len(rgb), 3):
        for b in (rgb[k], rgb[k + 1], rgb[k + 2], 255):
            h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"{h:016x}"


def _headless(index: int, name: str) -> tuple[list[float], bytes, str]:
    """Render entry `index` headless; answer its rect, its RGB bytes and
    the stack's shaping mode `--demo` printed.

    **The render must be the entry the live window picks**: `--demo`
    prints the name it rendered, and this refuses a render of any other.
    Without it an index skew compares two different demos and reports a
    pixel difference, which reads as a rendering defect (M83/T83.6)."""
    with tempfile.TemporaryDirectory() as d:
        path = Path(d) / "demo.ppm"
        res = subprocess.run([str(BIN), "--demo", str(index), str(path)],
                             capture_output=True, text=True, check=True)
        got = re.search(r": (.+?) \u2014 \d+x\d+ at", res.stdout)
        if not got or got.group(1) != name:
            raise RuntimeError(
                f"parity setup: --demo {index} rendered "
                f"{got.group(1) if got else res.stdout.strip()!r}, the entry picked live is {name!r} -- "
                f"the entry list and the binary disagree, so the comparison would be of two demos")
        m = re.search(r"(\d+)x(\d+) at \((\d+),(\d+)\)", res.stdout + res.stderr)
        if not m:
            raise RuntimeError(f"--demo {index} printed no rect: {res.stdout!r}")
        w, h, x, y = (int(v) for v in m.groups())
        mode = re.search(r"shaping (\w+)", res.stdout + res.stderr)
        data = path.read_bytes()
        # P6 header: magic, width, height, maxval, then one byte of space.
        parts = data.split(b"\n", 3) if data.startswith(b"P6\n") else None
        if parts and len(parts) == 4:
            body = parts[3]
        else:
            hdr = re.match(rb"P6\s+\d+\s+\d+\s+\d+\s", data)
            body = data[hdr.end():]
        return [float(x), float(y), float(x + w), float(y + h)], body, mode.group(1) if mode else "?"


def _differing(c, rect, rgb: bytes) -> int | None:
    """How many RGB bytes of the live window's crop at `rect` differ from
    the headless render's -- the quantity the hash compares, printed. From
    `ui.screenshot`, at scale 1 (the caller skips any other); `None`
    without PIL, when the hash alone decides."""
    try:
        import base64
        import io
        from PIL import Image
    except ImportError:
        return None
    shot = c.rpc("ui.screenshot")
    img = Image.open(io.BytesIO(base64.b64decode(shot["image_base64"]))).convert("RGB")
    x0, y0, x1, y1 = (int(v) for v in rect)
    live = img.crop((x0, y0, x1, y1)).tobytes()
    if len(live) != len(rgb):
        return max(len(live), len(rgb))
    return sum(1 for a, b in zip(live, rgb) if a != b)


def leg_shaping() -> str:
    """The mode the live window lays text out in: `Complex` when the
    process reports `shaping` (through the launcher, never a label this
    script sets), else `Basic`."""
    feats = os.environ.get("FULGURITE_LIVE_FEATURES", "").split(",")
    return "Complex" if "shaping" in feats else "Basic"


def run_parity(c, pick, strategy: str, only: set[str] | None = None) -> tuple[list[str], int]:
    """Compare every non-animated entry (or those `only` names, lower
    case); answer the lines and the failures."""
    out, failures, checked, skipped, unmatched = [], 0, 0, [], 0
    want_mode, modes = leg_shaping(), set()
    scale = c.rpc("ui.screenshot").get("scale", 1.0)
    if scale != 1.0:
        # The headless render is at scale 1; comparing it with a scale-2
        # window would be comparing different pixels, not finding a defect.
        return [f"parity [{strategy}]: skipped at scale {scale} -- the headless render is at 1"], 0
    for index, (name, animated) in enumerate(_entries()):
        if only is not None and name.lower() not in only:
            continue
        if animated:
            skipped.append(name)
            continue
        try:
            rect, rgb, mode = _headless(index, name)
        except RuntimeError as e:
            failures += 1
            unmatched += 1
            out.append(f"  FAIL {name} [{strategy}] {e}")
            continue
        modes.add(mode)
        if mode != want_mode:
            failures += 1
            out.append(f"  FAIL {name} [{strategy}] parity: --demo laid out {mode}, the live leg "
                       f"{want_mode} -- the two renders were not built the same way")
            continue
        pick(c, name)
        c.rpc("ui.lastChange")
        live = c.rpc("ui.regionHash", rect=rect)
        want = _fnv_rgba(rgb)
        checked += 1
        diff = _differing(c, rect, rgb)
        if live.get("hash") != want or diff not in (0, None):
            failures += 1
            out.append(f"  FAIL {name} [{strategy}] parity: the live demo with the agent "
                       f"differs from the headless render over {live.get('pixels')} px at {rect}: "
                       f"{diff} bytes differ (bound 0)")
        else:
            out.append(f"  {name} [{strategy}] parity: {live.get('pixels')} px at {rect}, "
                       f"{'?' if diff is None else diff} bytes differ (bound 0), shaping {mode}")
    if checked + failures == 0 and not skipped:
        failures += 1
        out.append(f"  FAIL parity [{strategy}]: no entry compared -- {sorted(only or [])} named none")
    out.append(f"parity [{strategy}]: --demo stack {', '.join(sorted(modes)) or 'none run'}, "
               f"leg {want_mode} (process features {os.environ.get('FULGURITE_LIVE_FEATURES', '')!r})")
    out.append(f"parity [{strategy}]: {checked - (failures - unmatched)} of {checked} entries byte-identical "
               f"to a headless render with no agent; skipped (animated): {', '.join(skipped) or 'none'}; "
               f"--demo rendered another entry than the one picked: {unmatched} (bound 0)")
    return out, failures
