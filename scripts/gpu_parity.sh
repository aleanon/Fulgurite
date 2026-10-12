#!/usr/bin/env bash
# T15.12: the *live* half of the backend comparison.
#
# `cargo test -p fulgurite-wgpu` compares the two renderers headlessly,
# which is the gate. This compares them through the whole stack -- window,
# swapchain, present, compositor -- by running ONE binary twice, once on each
# backend, and reading the window back off the X server both times.
#
# It exists because "rendered" and "shown" are different claims, and this
# project has been caught by the gap between them before: the first partial
# redraw skipped the present entirely and left another window's pixels on
# screen (crates/fulgurite-winit/src/surface.rs). A headless test cannot
# see that. Measured 2026-09-07: worst channel delta 1, docs/results-gpu.md.
#
# Needs a display, an adapter, and `xwd`, and skips with exit 77 when any
# of the three is missing (M43 §2.2). A machine with no adapter is the
# configuration the CPU fallback exists for, but this gate claims a
# comparison happened, and until M43/T43.13 it exited 0 without making
# one; `just live` refuses a 77 unless FULGURITE_ALLOW_SKIP=1.
#
# Each window is launched by `scripts/live/launch.py` (M42/T42.2) with
# `--no-agent`: the grab runs while the launcher checks the window is
# still alive, and the log is per worktree (`target/live/gpu-parity-*.log`).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh  # live windows on the monitor the user is not reading
LAUNCH="python3 scripts/live/launch.py --script gpu-parity"
$LAUNCH --display-only || exit $?
if ! command -v xwd >/dev/null; then
    echo "gpu-parity: no xwd -- skipped (77)"
    exit 77
fi

cargo build --release -p minimal --features gpu
bin=target/release/minimal
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT

# grab LEG OUT [K=V]: the window on one backend, read back off the X server.
grab() {
    $LAUNCH --no-agent --settle 3 --leg "$1" --exe "$bin" ${3:+--env "$3"} \
        -- bash -c "xwd -name minimal -out '$2' 2>/dev/null || true"
}

grab gpu "$tmp/gpu.xwd"
if ! grep -q '^fulgurite: gpu' target/live/gpu-parity-gpu.log; then
    echo "gpu-parity: no adapter on this machine -- skipped (77) ($(cat target/live/gpu-parity-gpu.log))"
    exit 77
fi
grab software "$tmp/cpu.xwd" "FULGURITE_BACKEND=software"

python3 - "$tmp/gpu.xwd" "$tmp/cpu.xwd" <<'PY'
import struct, sys

def load(path):
    d = open(path, 'rb').read()
    h = struct.unpack('>25I', d[:100])
    hdr, w, ht, bpp, bpl, ncolors = h[0], h[4], h[5], h[11], h[12], h[19]
    px = d[hdr + ncolors * 12:]
    return w, ht, [
        (px[y * bpl + x * (bpp // 8) + 2], px[y * bpl + x * (bpp // 8) + 1],
         px[y * bpl + x * (bpp // 8)])
        for y in range(ht) for x in range(w)
    ]

w, h, a = load(sys.argv[1])
_, _, b = load(sys.argv[2])
if len(a) != len(b):
    sys.exit("gpu-parity: the two windows are different sizes")

worst = 0
over = 0
for pa, pb in zip(a, b):
    d = max(abs(x - y) for x, y in zip(pa, pb))
    worst = max(worst, d)
    over += d > 8

frac = over / len(a)
print(f"gpu-parity: {w}x{h}, worst channel delta {worst}, "
      f"{over} px ({frac * 100:.3f}%) over 8")

# The live tolerance is much tighter than the headless one, and on purpose:
# `examples/minimal` is rects, rounded rects and text at whole-pixel
# positions, so there is no fractional edge for the two rasterisers to
# disagree about. Anything over 1% here is a real difference, not
# antialiasing.
if frac > 0.01:
    sys.exit(f"gpu-parity: the two backends disagree on {frac * 100:.2f}% of the window")
PY

# ---- M82/T82.6: paths, through the whole stack -------------------------
# The showcase opened on its PaintBox entry (a filled star and a stroked
# arc, `examples/showcase/src/catalog.rs`) on each backend, read back off the
# X server and compared as above. The GPU draws a path as R8 masks in the
# glyph atlas, rasterised by the software canvas's own `rasterize_path`, so
# the headless gate (`tests/parity.rs`, `paths_agree_across_backends`) holds
# them to one blend's rounding; this leg holds the presented window to the
# minimal leg's 1% over 8 and prints the worst channel delta. The GPU
# window logs K82d's number once -- path-mask texels as a share of the glyph
# atlas's starting size (bound 25%) -- and the leg requires the line: a
# window that drew no path mask has no number to give.
cargo build --release -p showcase --features gpu
# M82/T82.8: and the VectorText entry (one run at 0, 30 and 90 degrees, its
# outlines one path each), whose first frame is K82d's second reading.
for name in PaintBox VectorText; do
# The index in the binary's own catalogue (`showcase --entries`), not in
# `catalog.rs`'s text, which counts entries behind a feature this build may
# lack (M83/T83.6's `ShaderView`).
entry=$(target/release/showcase --entries | awk -F'\t' -v n="$name" '$1 == n && !f {print NR - 1; f = 1}')
[ -n "$entry" ] || { echo "gpu-parity: no $name entry in the showcase's --entries" >&2; exit 1; }
leg=$(echo "$name" | tr '[:upper:]' '[:lower:]')
sgrab() {
    $LAUNCH --no-agent --settle 3 --leg "$1" --exe target/release/showcase --arg "$entry" ${3:+--env "$3"} \
        -- bash -c "xwd -name showcase -out '$2' 2>/dev/null || true"
}
sgrab "$leg-gpu" "$tmp/pb-gpu.xwd"
sgrab "$leg-software" "$tmp/pb-cpu.xwd" "FULGURITE_BACKEND=software"
k82d=$(grep -m1 '^fulgurite: path masks' "target/live/gpu-parity-$leg-gpu.log" || true)
if [ -z "$k82d" ]; then
    echo "gpu-parity: the $name window (entry $entry) logged no path-mask line -- the GPU drew no path" >&2
    exit 1
fi
echo "gpu-parity: $name (entry $entry): ${k82d#fulgurite: }"
python3 - "$tmp/pb-gpu.xwd" "$tmp/pb-cpu.xwd" "$k82d" "$name" <<'PY'
import re, struct, sys

name = sys.argv[4]

def load(path):
    d = open(path, 'rb').read()
    h = struct.unpack('>25I', d[:100])
    hdr, w, ht, bpp, bpl, ncolors = h[0], h[4], h[5], h[11], h[12], h[19]
    px, step = d[hdr + ncolors * 12:], bpp // 8
    return w, ht, [tuple(px[y * bpl + x * step:y * bpl + x * step + 3]) for y in range(ht) for x in range(w)]

w, h, a = load(sys.argv[1])
_, _, b = load(sys.argv[2])
if len(a) != len(b):
    sys.exit(f"gpu-parity: the two {name} windows are different sizes")
worst, over = 0, 0
for pa, pb in zip(a, b):
    d = max(abs(x - y) for x, y in zip(pa, pb))
    worst = max(worst, d)
    over += d > 8
print(f"gpu-parity: {name} {w}x{h}, worst channel delta {worst}, {over} px ({over / len(a) * 100:.3f}%) over 8 (bound 1%)")
if over / len(a) > 0.01:
    sys.exit(f"gpu-parity: the backends disagree on {over / len(a) * 100:.2f}% of the {name} window")
share = float(re.search(r'([0-9.]+)% of the glyph atlas', sys.argv[3]).group(1))
if share > 25.0:
    sys.exit(f"gpu-parity: K82d fires: path masks take {share}% of the glyph atlas's start (bound 25%)")
PY
done

# ---- M47/T47.0 (R7-F7): a dropped present on an IDLE window ------------
# `Gpu::present` swallowed Outdated/Lost and returned nothing, and the
# window asks for another frame only while something animates, so an idle
# window whose present was dropped stayed unpainted until input arrived.
# A DEBUG build with FULGURITE_GPU_DROP_FIRST=n drops the first n presents;
# the window is left alone for 3 s and read back once, then compared with
# the GPU capture above. Not "more than one distinct value", which the
# plan named: a window that never presented reads back whatever the X
# server has under it -- 2578 distinct values with every present dropped
# -- so only a comparison with the real frame can tell. n=0 is the
# accepting leg: the same debug build with nothing dropped must match.
# FULGURITE_GPU_DROP_MORE=k adds a leg (it fails the comparison when k passes
# the retry bound, which is what the last step below is for).
cargo build -q -p minimal --features gpu
for n in 0 2 ${FULGURITE_GPU_DROP_MORE:-}; do
    echo "== dropped the first $n presents, idle =="
    $LAUNCH --no-agent --settle 3 --leg "drop$n" --exe target/debug/minimal \
        --env FULGURITE_GPU_DROP_FIRST=$n \
        -- bash -c "xwd -name minimal -out '$tmp/drop.xwd' 2>/dev/null || true"
    python3 - "$tmp/drop.xwd" "$tmp/gpu.xwd" "$n" <<'PY'
import struct, sys

def load(path):
    d = open(path, 'rb').read()
    h = struct.unpack('>25I', d[:100])
    hdr, w, ht, bpp, bpl, ncolors = h[0], h[4], h[5], h[11], h[12], h[19]
    px, step = d[hdr + ncolors * 12:], bpp // 8
    return w, ht, [bytes(px[y * bpl + x * step:y * bpl + x * step + 3]) for y in range(ht) for x in range(w)]

w, h, a = load(sys.argv[1])
_, _, b = load(sys.argv[2])
if len(a) != len(b):
    sys.exit("gpu-parity: the dropped-present window is a different size")
over = sum(1 for pa, pb in zip(a, b) if max(abs(x - y) for x, y in zip(pa, pb)) > 8)
print(f"gpu-parity: dropped the first {sys.argv[3]} presents, idle 3 s: {over} px "
      f"({over / len(a) * 100:.3f}%) differ from the presented frame (bound 1%)")
if over / len(a) > 0.01:
    sys.exit(f"gpu-parity: after {sys.argv[3]} dropped presents the idle window is not the "
             f"frame -- nothing asked for another one (R7-F7)")
PY
done

# ---- M47/T47.4: a surface that never comes back must not spin ---------
# Past MAX_PRESENT_RETRIES (3) dropped presents in a row the window stops
# asking. With twenty drops armed, count the presents it attempted in 3 s
# of idle (the switch logs one line per drop): the first frame, three
# retries, and whatever the window manager's own exposes ask for -- well
# under twenty. A window that retried without a bound would burn through
# all twenty and then present, which is why the count and not the
# picture is the check here.
echo "== dropped the first 20 presents: the window stops asking =="
$LAUNCH --no-agent --settle 3 --leg drop20 --exe target/debug/minimal \
    --env FULGURITE_GPU_DROP_FIRST=20 -- true
tried=$(grep -c "FULGURITE_GPU_DROP_FIRST: present dropped" target/live/gpu-parity-drop20.log || true)
echo "gpu-parity: 20 drops armed: the window attempted $tried presents in 3 s idle (bound 8: 1 + 3 retries + exposes)"
if [ "$tried" -gt 8 ]; then
    echo "gpu-parity: a surface that never came back kept the window retrying ($tried presents)" >&2
    exit 1
fi

# ---- M51/T51.1 (R9-F1): an atlas give-up reaches app.perf --------------
# The glyph atlas grows to the device's limit, so no real screen gives up
# any more; a DEBUG build with FULGURITE_GPU_ATLAS_MAX=64 caps it at 64², which
# the showcase's first frame overflows. The renderer logs its first
# give-up once; app.perf must report a non-zero running count beside it.
# The accepting leg, uncapped, must report 0 -- a number, not null, which
# is what says the frame came from a renderer with an atlas at all.
cargo build -q -p showcase --features gpu,agent
for cap in 64 none; do
    echo "== glyph atlas cap: $cap =="
    envs=(); [ "$cap" = none ] || envs=(--env "FULGURITE_GPU_ATLAS_MAX=$cap")
    $LAUNCH --leg "atlas-$cap" --exe target/debug/showcase "${envs[@]}" \
        -- bash -c 'sleep 1; python3 scripts/agent_client.py call app.perf > "$1"' _ "$tmp/perf-$cap.json"
    python3 - "$tmp/perf-$cap.json" "target/live/gpu-parity-atlas-$cap.log" "$cap" <<'PY'
import json, sys
perf = json.load(open(sys.argv[1]))
perf = perf.get("result", perf)
log = open(sys.argv[2], errors="replace").read()
logged = log.count("did not fit the glyph atlas")
n = perf.get("atlas_give_ups")
print(f"gpu-parity: atlas cap {sys.argv[3]}: app.perf atlas_give_ups = {n}; the renderer logged "
      f"{logged} give-up line(s) (it logs the first only)")
if n is None:
    sys.exit("gpu-parity: app.perf carried no atlas count -- the frames did not come from the GPU backend")
if sys.argv[3] == "none":
    if n != 0 or logged:
        sys.exit(f"gpu-parity: an uncapped atlas gave up ({n}, {logged} logged)")
elif n < 1 or logged != 1:
    sys.exit(f"gpu-parity: a 64² atlas gave up but app.perf reports {n} and the log {logged}")
PY
done
