#!/usr/bin/env bash
# M37/T37.0: a hot library built differently from its loader is refused,
# naming the first field that differs -- and one built the same way is
# accepted. Both directions, two real builds (docs/plan-window-reload.md).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
OUT="target/abi-stamp"
mkdir -p "$OUT"
cargo build --release -q -p stamp-host
cargo build --release -q -p stamp-lib
cp target/release/libstamp_lib.so "$OUT/same.so"
cargo build --release -q -p stamp-lib --features observe
cp target/release/libstamp_lib.so "$OUT/observe.so"
./target/release/stamp-host "$OUT/same.so" accept
./target/release/stamp-host "$OUT/observe.so" refuse "lumen-static features"
echo "ABI STAMP: PASS"
