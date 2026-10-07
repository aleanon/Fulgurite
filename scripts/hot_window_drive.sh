#!/usr/bin/env bash
# M37/T37.4 (docs/plan-window-reload.md): a live window that reloads
# itself. fixtures/hot-window is COPIED into a workspace of its own under
# target/hot-live and its copy's source is edited -- never the checked-in
# file. Built --release --features hot,agent; driven over the agent by
# scripts/hot_window_drive.py. Needs a display; skips with exit 77
# without one (M43 §2).
#
# **The window is launched by `scripts/live/launch.py`** (M42/T42.2): a
# port that already answers is refused, and the socket's owner and the
# process's own `app.diagnostics` (answered by the session inside the
# library) must name the launched pid, the default strategy and `reload`.
#
# **Three legs since M81/T81.4** (docs/plan-shaped-paint.md), all built
# with `shaping`, which `--expect-features` names:
#   hot         the drive above, then step 9: the copy's program is edited
#               to `.shaping(Shaping::Complex)` and #hebrew's pixels must be
#               the headless Complex render's after the swap. Since
#               M84/T84.6 it also edits `#pic`'s colour in the application's
#               store (`App::images`): the swapped-in build's pixels must be
#               on screen, and the failed build's bar must leave them be.
#               Since M90/T90.5 (step 7b) it edits the program's one menu
#               accelerator from Ctrl+K to Ctrl+J: after the swap Ctrl+J is
#               handled and Ctrl+K is not;
#   static      the fresh launch of the edited source, with no reload: the
#               same pixels, which `Win::new` taking `Session::shaping`
#               before its first backend is what gives;
#   everything  a fresh copy whose `Strategy` is edited to `Everything`
#               before the build, running step 9 alone -- so both
#               strategies see the swap's repaint-all.
#   gpu         M83/T83.8: a fresh copy built `hot agent gpu shader` and
#               run on the GPU backend, read back with `xwd`: the copy's
#               WGSL colour is edited red -> blue and must reach the
#               running window through the swap's `Backend::set_shaders`;
#               the swap frame repaints the whole window; and the broken
#               edit's bar leaves `#swatch`'s pixels as they were.
#   gpu-everything  the same under `Everything`.
#   windows     M89/T89.7: the copy is broken before launch, so the window's
#               first build fails; the drive opens the panel (program 1)
#               BEFORE any swap and bumps it, then fixes the copy and edits
#               the panel's format in one write: the first swap boots both
#               windows from the new build, the panel keeping its count. The
#               panel lays out at its own size and its timer redraws it alone;
#               a panel opened after the swap is the new build's; and a v3
#               whose panel boot panics is refused whole, both windows on v2.
#   windows-everything  the same under `Everything` (the fixture's feature).
#   daemon      M89/T89.7: examples/daemon built `hot`, `--still`, no window:
#               the build's arrival swaps it (`app.reloads` on window 0).
# The GPU legs skip with exit 77 without `xwd` or an adapter -- and a skip
# is not a pass (M43): `just live` refuses it unless LUMEN_ALLOW_SKIP=1.
# `HOT_WINDOW_LEGS` picks legs (default "hot static everything gpu
# gpu-everything"); `--gpu` runs the two GPU legs alone;
# `HOT_WINDOW_SHAPING_ONLY=1` runs step 9 alone in the hot leg too.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh  # live windows on the monitor the user is not reading
LAUNCH="python3 scripts/live/launch.py --script hot-window-drive"
$LAUNCH --display-only || exit $?
ROOT="$(pwd)"
WS="$ROOT/target/hot-live/ws"
LEGS="${HOT_WINDOW_LEGS:-hot static everything gpu gpu-everything windows windows-everything daemon}"
[ "${1:-}" = --gpu ] && LEGS="gpu gpu-everything"
FEATURES="hot agent shaping"
# The live hashes are compared with headless renders at scale 1.
SCALE=(--env WINIT_X11_SCALE_FACTOR=1)
mkdir -p "$WS"
fresh_copy() {
    rm -rf "$WS/app"
    cp -r fixtures/hot-window "$WS/app"
    sed -i "s|\.\./\.\./crates|$ROOT/crates|g" "$WS/app/Cargo.toml"
}
# The static leg alone runs on the copy a hot leg left edited.
[ -d "$WS/app" ] || fresh_copy
# examples/daemon, copied as the fixture is (M89/T89.7's daemon leg edits
# the copy to make a build land with nothing else happening).
rm -rf "$WS/daemon"
cp -r examples/daemon "$WS/daemon"
sed -i "s|\.\./\.\./crates|$ROOT/crates|g" "$WS/daemon/Cargo.toml"
cat > "$WS/Cargo.toml" <<TOML
[workspace]
resolver = "2"
members = ["app", "daemon"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
TOML
[ -f "$WS/Cargo.lock" ] || cp Cargo.lock "$WS/Cargo.lock"

build_hot() { # [FEATURES]
    local features="${1:-$FEATURES}"
    echo "== building the window, and the library it will first swap to ($features) =="
    (cd "$WS" && cargo build --release -q -p hot-window --features "$features")
    # The window's first build runs at start-up; doing it once here first
    # makes that build incremental rather than a cold one of every dependency.
    (cd "$WS" && cargo rustc --release -q -p hot-window --lib --crate-type cdylib \
        --features "$features" --target-dir "$WS/target/reload")
    # The headless half of step 9's comparison, kept apart from the window
    # binary the static leg rebuilds.
    cp "$WS/target/release/hot-window" "$WS/hash-hot"
}
SAVED="$WS/post-swap.hash"

if [[ " $LEGS " == *" hot "* ]]; then
    fresh_copy
    build_hot
    ONLY=()
    [ "${HOT_WINDOW_SHAPING_ONLY:-}" = 1 ] && ONLY=(--only-shaping)
    PANICS=1
    [ ${#ONLY[@]} -gt 0 ] && PANICS=0
    # One panic, provoked: the drive plants one in the library and checks the
    # screen shows it. Exact, so a second is a failure (M42/T42.3).
    $LAUNCH --leg hot --exe "$WS/target/release/hot-window" --env "LUMEN_HOT_FEATURES=$FEATURES" "${SCALE[@]}" \
        --expect-panics $PANICS --expect-strategy Boundaries --expect-features reload,shaping \
        -- python3 scripts/hot_window_drive.py {port} "$WS/app/src/lib.rs" {pid} "$WS/hash-hot" "${ONLY[@]}" --save "$SAVED"
fi

if [[ " $LEGS " == *" static "* ]]; then
    # The copy as the hot leg left it -- its program says Complex -- built
    # as a plain window: no `hot`, so no reload.
    echo "== building the edited source as a static window =="
    grep -q 'Shaping::Complex), // EDIT shaping' "$WS/app/src/lib.rs" \
        || { echo "hot-window-drive: the static leg needs the hot leg's edited copy" >&2; exit 1; }
    (cd "$WS" && cargo build --release -q -p hot-window --features "agent shaping")
    $LAUNCH --leg static --exe "$WS/target/release/hot-window" "${SCALE[@]}" \
        --expect-strategy Boundaries --expect-features shaping \
        -- python3 scripts/hot_window_drive.py {port} --static "$WS/target/release/hot-window" "$SAVED"
fi

if [[ " $LEGS " == *" everything "* ]]; then
    fresh_copy
    sed -i 's|^pub type Strategy = .*// EDIT strategy$|pub type Strategy = lumen_static::Everything; // EDIT strategy|' "$WS/app/src/lib.rs"
    grep -q 'lumen_static::Everything; // EDIT strategy' "$WS/app/src/lib.rs" \
        || { echo "hot-window-drive: the strategy edit did not apply" >&2; exit 1; }
    build_hot
    $LAUNCH --leg everything --exe "$WS/target/release/hot-window" --env "LUMEN_HOT_FEATURES=$FEATURES" "${SCALE[@]}" \
        --expect-strategy Everything --expect-features reload,shaping \
        -- python3 scripts/hot_window_drive.py {port} "$WS/app/src/lib.rs" {pid} "$WS/hash-hot" --only-shaping
fi

# M83/T83.8: a WGSL edit reaches a running window, on the GPU.
GPU_FEATURES="hot agent gpu shader"
gpu_leg() { # LEG STRATEGY
    if ! command -v xwd >/dev/null; then
        echo "hot-window-drive: no xwd -- the GPU legs skipped (77)"
        exit 77
    fi
    fresh_copy
    if [ "$2" = Everything ]; then
        sed -i 's|^pub type Strategy = .*// EDIT strategy$|pub type Strategy = lumen_static::Everything; // EDIT strategy|' "$WS/app/src/lib.rs"
        grep -q 'lumen_static::Everything; // EDIT strategy' "$WS/app/src/lib.rs" \
            || { echo "hot-window-drive: the strategy edit did not apply" >&2; exit 1; }
    fi
    build_hot "$GPU_FEATURES"
    $LAUNCH --leg "$1" --exe "$WS/target/release/hot-window" --env "LUMEN_HOT_FEATURES=$GPU_FEATURES" \
        --env LUMEN_BACKEND=gpu "${SCALE[@]}" \
        --expect-strategy "$2" --expect-features reload \
        -- python3 scripts/hot_window_drive.py {port} "$WS/app/src/lib.rs" {pid} "$WS/hash-hot" --gpu || {
        rc=$?
        if ! grep -q '^lumen-static: gpu' "target/live/hot-window-drive-$1.log"; then
            echo "hot-window-drive: no adapter on this machine -- skipped (77): $(head -3 "target/live/hot-window-drive-$1.log")"
            exit 77
        fi
        exit $rc
    }
}
if [[ " $LEGS " == *" gpu "* ]]; then gpu_leg gpu Boundaries; fi
if [[ " $LEGS " == *" gpu-everything "* ]]; then gpu_leg gpu-everything Everything; fi

# M89/T89.7: hot reload with several windows.
windows_leg() { # LEG STRATEGY FEATURES
    fresh_copy
    build_hot "$3"
    # Broken AFTER the build: the binary and the warm library are the good
    # source, and the window's own first build is the broken one.
    sed -i 's|^\(\s*\)let step = 1; // EDIT handler$|\1let step: i64 = "one"; // EDIT handler|' "$WS/app/src/lib.rs"
    grep -q 'let step: i64 = "one"; // EDIT handler' "$WS/app/src/lib.rs" \
        || { echo "hot-window-drive: the break did not apply" >&2; exit 1; }
    # One panic, provoked: v3's panel boot, caught in the library -- or two,
    # when the watcher builds v3 twice and the refused build is booted again.
    $LAUNCH --leg "$1" --exe "$WS/target/release/hot-window" --env "LUMEN_HOT_FEATURES=$3" "${SCALE[@]}" \
        --expect-panics 1-2 --expect-strategy "$2" --expect-features reload,shaping \
        -- python3 scripts/hot_window_drive.py {port} "$WS/app/src/lib.rs" {pid} "$WS/hash-hot" --windows
}
if [[ " $LEGS " == *" windows "* ]]; then windows_leg windows Boundaries "$FEATURES"; fi
if [[ " $LEGS " == *" windows-everything "* ]]; then windows_leg windows-everything Everything "$FEATURES everything"; fi

if [[ " $LEGS " == *" daemon "* ]]; then
    echo "== building examples/daemon's copy with hot, and the library it will swap to =="
    (cd "$WS" && cargo build --release -q -p daemon --features hot)
    (cd "$WS" && cargo rustc --release -q -p daemon --lib --crate-type cdylib --features hot \
        --target-dir "$WS/target/reload")
    # The drive waits for the first swap, then edits the copy: that build
    # lands with no agent call and no timer -- `--still` has none -- so only
    # the window scheduling a windowless step on `Built` can take the swap.
    $LAUNCH --leg daemon --exe "$WS/target/release/daemon" --arg=--still --env LUMEN_HOT_FEATURES=hot \
        --expect-strategy Boundaries --expect-features reload \
        -- python3 scripts/hot_window_drive.py {port} --daemon "$WS/daemon/src/lib.rs"
fi
