#!/usr/bin/env bash
# M83/T83.7 (docs/plan-gpu-extension.md): examples/shader -- an animated
# WGSL plasma in a `ShaderView` behind a `Glass` card -- driven live.
#
# Legs, each launched by `scripts/live/launch.py` (M42/T42.2), which names
# the process it drove and its strategy:
#   software-boundaries  the default software canvas: the fallback at the
#   software-everything  view's centre, 0 px changed outside view ∪ card
#                        across two animated frames (ui.screenshot), and
#                        ui.lastDamage per animated frame against view ∪
#                        card as a ratio (bound 1.00). The Boundaries leg
#                        also records app.perf's layout_ns, animated against
#                        still (A12; docs/results-shader.md T83.7).
#   gpu-boundaries       LUMEN_BACKEND=gpu, read back with `xwd`: plasma at
#   gpu-everything       the centre, 0 px changed outside, and a
#                        LUMEN_TEST_RESUME round trip that keeps the plasma
#                        -- `resume_backend`'s `set_shaders`, driven live.
#   gpu-bad              LUMEN_GPU_BAD_SHADER=1: the window stays up, answers
#                        after 3 s, shows the fallback and names the shader.
# Debug builds, because LUMEN_TEST_RESUME and LUMEN_GPU_BAD_SHADER are
# debug-only hooks. `SHADER_DRIVE_LEGS` picks legs (default: all five).
#
# Needs a display; skips with exit 77 without one (M43 §2). The GPU legs
# also need `xwd` and an adapter, and skip with 77 without either -- a
# skip is not a pass (`just live` refuses it unless LUMEN_ALLOW_SKIP=1).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh  # live windows on the monitor the user is not reading
LAUNCH="python3 scripts/live/launch.py --script shader-drive"
$LAUNCH --display-only || exit $?
LEGS="${SHADER_DRIVE_LEGS:-software-boundaries software-everything gpu-boundaries gpu-everything gpu-bad}"
OUT=target/shader-drive
mkdir -p "$OUT"
# The drive's coordinates are the agent's logical ones, compared with
# window pixels.
SCALE=(--env WINIT_X11_SCALE_FACTOR=1)
want() { [[ " $LEGS " == *" $1 "* ]]; }

# build FEATURES NAME: one variant, kept under its own name (every
# variant is `target/debug/shader`).
build() {
    cargo build -q -p shader --features "$1"
    cp target/debug/shader "$OUT/$2"
}

if want software-boundaries; then
    build shader,agent sw
    $LAUNCH --leg software-boundaries --exe "$OUT/sw" "${SCALE[@]}" --env LUMEN_BACKEND=software \
        --expect-strategy Boundaries --expect-features "" \
        -- python3 scripts/shader_drive.py {port} software-boundaries-perf software
fi
if want software-everything; then
    build shader,agent,everything sw-everything
    $LAUNCH --leg software-everything --exe "$OUT/sw-everything" "${SCALE[@]}" --env LUMEN_BACKEND=software \
        --expect-strategy Everything --expect-features "" \
        -- python3 scripts/shader_drive.py {port} software-everything software
fi

if want gpu-boundaries || want gpu-everything || want gpu-bad; then
    if ! command -v xwd >/dev/null; then
        echo "shader-drive: no xwd -- the GPU legs skipped (77)"
        exit 77
    fi
    build gpu,agent gpu
    build gpu,agent,everything gpu-everything
fi
# The round trip fires this long after the window starts; the drive reads
# the plasma before it and again 3 s after it.
RESUME_MS=10000
gpu_leg() { # LEG EXE STRATEGY
    $LAUNCH --leg "$1" --exe "$OUT/$2" "${SCALE[@]}" --env LUMEN_BACKEND=gpu \
        --env LUMEN_TEST_RESUME=$RESUME_MS --env SHADER_DRIVE_RESUME_S=$((RESUME_MS / 1000)) \
        --expect-strategy "$3" --expect-features "" \
        -- python3 scripts/shader_drive.py {port} "$1" gpu || {
        rc=$?
        if ! grep -q '^lumen-static: gpu' "target/live/shader-drive-$1.log"; then
            echo "shader-drive: no adapter on this machine -- skipped (77): $(head -3 "target/live/shader-drive-$1.log")"
            exit 77
        fi
        exit $rc
    }
}
want gpu-boundaries && gpu_leg gpu-boundaries gpu Boundaries
want gpu-everything && gpu_leg gpu-everything gpu-everything Everything
if want gpu-bad; then
    $LAUNCH --leg gpu-bad --exe "$OUT/gpu" "${SCALE[@]}" --env LUMEN_BACKEND=gpu --env LUMEN_GPU_BAD_SHADER=1 \
        --expect-strategy Boundaries --expect-features "" \
        -- python3 scripts/shader_drive.py {port} gpu-bad bad
fi
echo "shader-drive: every leg passed ($LEGS)"
