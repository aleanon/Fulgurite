#!/usr/bin/env bash
# M37/T37.4 (docs/plan-window-reload.md): a live window that reloads
# itself. fixtures/hot-window is COPIED into a workspace of its own under
# target/hot-live and its copy's source is edited -- never the checked-in
# file. Built --release --features hot,agent; driven over the agent by
# scripts/hot_window_drive.py. Needs a display; skips without one.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh  # live windows on the monitor the user is not reading
if [ -z "${DISPLAY:-}" ]; then
    echo "hot-window-drive: no X display — skipped"
    exit 0
fi
ROOT="$(pwd)"
WS="$ROOT/target/hot-live/ws"
PORT="${LUMEN_HOT_PORT:-7671}"
mkdir -p "$WS"
rm -rf "$WS/app"
cp -r fixtures/hot-window "$WS/app"
sed -i "s|\.\./\.\./crates|$ROOT/crates|g" "$WS/app/Cargo.toml"
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

echo "== building the window, and the library it will first swap to =="
(cd "$WS" && cargo build --release -q -p hot-window --features hot,agent)
# The window's first build runs at start-up; doing it once here first
# makes that build incremental rather than a cold one of every dependency.
(cd "$WS" && cargo rustc --release -q -p hot-window --lib --crate-type cdylib \
    --features "hot agent" --target-dir "$WS/target/reload")

LUMEN_AGENT="127.0.0.1:$PORT" LUMEN_HOT_FEATURES="hot agent" \
    "$WS/target/release/hot-window" >/tmp/hot_window_drive.log 2>&1 &
pid=$!
trap 'kill $pid 2>/dev/null || true' EXIT
sleep 2
python3 scripts/hot_window_drive.py "$PORT" "$WS/app/src/lib.rs" "$pid" || {
    echo "--- window log ---"; tail -30 /tmp/hot_window_drive.log; exit 1; }
