#!/usr/bin/env bash
# M37/T37.1: a window library swapped v1 -> v2 headlessly; the swapped
# frame must equal v2 run from the start, byte for byte, and show v2's
# format with v1's count (docs/plan-window-reload.md).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
OUT="target/window-reload"
mkdir -p "$OUT"
cargo build --release -q -p reload-app
cp target/release/libreload_app.so "$OUT/v1.so"
cargo build --release -q -p reload-app --features v2
cp target/release/libreload_app.so "$OUT/v2.so"
cargo build --release -q -p reload-app --features v3
cp target/release/libreload_app.so "$OUT/v3.so"
cargo build --release -q -p reload-host
./target/release/reload-host "$OUT/v1.so" "$OUT/v2.so" "$OUT/v3.so"
