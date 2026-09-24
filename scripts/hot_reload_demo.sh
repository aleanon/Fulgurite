#!/usr/bin/env bash
# Step 4 integration test: build hot-app twice (v1, then --features v2 as
# "the edited code"), run the host through the §7 swap sequence, and let
# its assertions decide. Exit code is the verdict.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

OUT="target/hot-demo"
mkdir -p "$OUT"

echo "== building hot-app v1 =="
cargo build --release -p hot-app --quiet
cp target/release/libhot_app.so "$OUT/libhot_app_v1.so"

echo "== building hot-app v2 (the edited code) =="
cargo build --release -p hot-app --features v2 --quiet
cp target/release/libhot_app.so "$OUT/libhot_app_v2.so"

echo "== building hot-app v3 (an edit that panics in its frame) =="
cargo build --release -p hot-app --features v3 --quiet
cp target/release/libhot_app.so "$OUT/libhot_app_v3.so"

echo "== building hot-app v4 (the fix) =="
cargo build --release -p hot-app --features v4 --quiet
cp target/release/libhot_app.so "$OUT/libhot_app_v4.so"

echo "== building host =="
cargo build --release -p hot-host --quiet

echo "== running the swaps =="
# M36/T36.4: a panic that escapes the library aborts the loader. Say so,
# rather than leave a bare "Aborted" and an exit code to decode.
rc=0
./target/release/hot-host "$OUT/libhot_app_v1.so" "$OUT/libhot_app_v2.so" \
    "$OUT/libhot_app_v3.so" "$OUT/libhot_app_v4.so" || rc=$?
if [ "$rc" -ge 128 ]; then
    echo "HOT RELOAD DEMO: FAIL — the loader died on signal $((rc - 128))" \
         "(a panic escaped a library shim uncaught?)" >&2
fi
exit "$rc"
