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
#               the headless Complex render's after the swap;
#   static      the fresh launch of the edited source, with no reload: the
#               same pixels, which `Win::new` taking `Session::shaping`
#               before its first backend is what gives;
#   everything  a fresh copy whose `Strategy` is edited to `Everything`
#               before the build, running step 9 alone -- so both
#               strategies see the swap's repaint-all.
# `HOT_WINDOW_LEGS` picks legs (default "hot static everything");
# `HOT_WINDOW_SHAPING_ONLY=1` runs step 9 alone in the hot leg too.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh  # live windows on the monitor the user is not reading
LAUNCH="python3 scripts/live/launch.py --script hot-window-drive"
$LAUNCH --display-only || exit $?
ROOT="$(pwd)"
WS="$ROOT/target/hot-live/ws"
LEGS="${HOT_WINDOW_LEGS:-hot static everything}"
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
cat > "$WS/Cargo.toml" <<TOML
[workspace]
resolver = "2"
members = ["app"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
TOML
[ -f "$WS/Cargo.lock" ] || cp Cargo.lock "$WS/Cargo.lock"

build_hot() {
    echo "== building the window, and the library it will first swap to =="
    (cd "$WS" && cargo build --release -q -p hot-window --features "$FEATURES")
    # The window's first build runs at start-up; doing it once here first
    # makes that build incremental rather than a cold one of every dependency.
    (cd "$WS" && cargo rustc --release -q -p hot-window --lib --crate-type cdylib \
        --features "$FEATURES" --target-dir "$WS/target/reload")
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
    grep -q 'Shaping::Complex) // EDIT shaping' "$WS/app/src/lib.rs" \
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
