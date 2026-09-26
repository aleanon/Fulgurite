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
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
LAUNCH="python3 scripts/live/launch.py --script hot-window-drive"
$LAUNCH --display-only || exit $?
ROOT="$(pwd)"
WS="$ROOT/target/hot-live/ws"
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

# One panic, provoked: the drive plants one in the library and checks the
# screen shows it. Exact, so a second is a failure (M42/T42.3).
$LAUNCH --exe "$WS/target/release/hot-window" --env "LUMEN_HOT_FEATURES=hot agent" --expect-panics 1 \
    --expect-strategy Boundaries --expect-features reload \
    -- python3 scripts/hot_window_drive.py {port} "$WS/app/src/lib.rs" {pid}
