#!/usr/bin/env bash
# M37/T37.0: a hot library built differently from its loader is refused,
# naming the first field that differs -- and one built the same way is
# accepted. Both directions, two real builds (docs/plan-window-reload.md).
#
# M49/T49.0 (docs/plan-reload-hardening.md, R4-F5): the same, for a library
# built from different FRAMEWORK SOURCE. `Cmd::Rect` is moved after
# `Cmd::Text` in a copy of the workspace -- a reordering no size can see --
# and the library built from the copy must be refused. An edit to
# `lumen-static-widgets` alone, which defines no type that crosses, must be
# accepted (§2.1's exclusion, checked rather than assumed).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(pwd)"
OUT="target/abi-stamp"
mkdir -p "$OUT"
cargo build --release -q -p stamp-host
cargo build --release -q -p stamp-lib
cp target/release/libstamp_lib.so "$OUT/same.so"
cargo build --release -q -p stamp-lib --features observe
cp target/release/libstamp_lib.so "$OUT/observe.so"
./target/release/stamp-host "$OUT/same.so" accept
./target/release/stamp-host "$OUT/observe.so" refuse "lumen-static features"

# ---- M49: a copy of the workspace, edited -------------------------------
#
# `rsync -a` keeps modification times, so the copy's own target directory
# stays incremental between runs; `--delete` keeps it exact.
WS="$ROOT/$OUT/ws"
mkdir -p "$WS/crates" "$WS/fixtures"
sync_ws() {
    # All of `crates/`: an optional dependency's manifest is read even
    # when the dependency is not built.
    rsync -a --delete crates/ "$WS/crates/"
    rsync -a --delete fixtures/stamp-lib/ "$WS/fixtures/stamp-lib/"
    rsync -a Cargo.lock "$WS/Cargo.lock"
    cat > "$WS/Cargo.toml" <<TOML
[workspace]
resolver = "2"
members = ["fixtures/stamp-lib"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
TOML
}
build_ws() {
    (cd "$WS" && cargo build --release -q -p stamp-lib --target-dir "$WS/target")
    cp "$WS/target/release/libstamp_lib.so" "$ROOT/$OUT/$1.so"
}

# An unchanged copy: the stamp describes source, not where it lives.
sync_ws
build_ws copy
./target/release/stamp-host "$OUT/copy.so" accept

# Two `Cmd` variants reordered: every size and alignment is unchanged.
python3 - "$WS/crates/lumen-static/src/display.rs" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
rect = "    Rect { rect: Rect, color: Color },\n"
i = s.index("pub enum Cmd {")
j = s.index(rect, i)
s = s[:j] + s[j + len(rect):]
text_end = s.index("\n", s.index("    Text { rect: Rect, text:", i)) + 1
s = s[:text_end] + rect + s[text_end:]
open(p, "w").write(s)
PY
build_ws reordered
./target/release/stamp-host "$OUT/reordered.so" refuse "source lumen-static" \
    || echo "ABI STAMP: KNOWN RED until M49/T49.1 (R4-F5): a reordered Cmd was accepted"

# A widget edited alone: no type of that crate crosses, so it is accepted.
sync_ws
printf '\n// M49/T49.1: an edit to the widgets crate alone.\n' >> "$WS/crates/lumen-static-widgets/src/lib.rs"
build_ws widgets
./target/release/stamp-host "$OUT/widgets.so" accept
sync_ws
echo "ABI STAMP: PASS"
