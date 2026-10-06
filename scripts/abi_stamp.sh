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

# ---- M81/T81.1: a shell feature in the stamp -----------------------------
#
# docs/plan-iced-roadmap.md §2.3: every shell feature joins `abi_fields`
# in the task that adds it, starting with `shaping`. A library built with
# it under a loader built without is refused by the shell's feature field;
# both built with it are accepted (and both without, the control).
cargo build --release -q -p stamp-lib --features shell
cp target/release/libstamp_lib.so "$OUT/shell.so"
cargo build --release -q -p stamp-lib --features shaping
cp target/release/libstamp_lib.so "$OUT/shaping.so"
cargo build --release -q -p stamp-host --features shell
cp target/release/stamp-host "$OUT/host-shell"
cargo build --release -q -p stamp-host --features shaping
cp target/release/stamp-host "$OUT/host-shaping"
"$OUT/host-shell" "$OUT/shell.so" accept
"$OUT/host-shell" "$OUT/shaping.so" refuse "lumen-static-shell features"
"$OUT/host-shaping" "$OUT/shaping.so" accept

# ---- M83/T83.3: the two shader entries ----------------------------------
#
# docs/plan-iced-roadmap.md §2.3 (second review A7). `lumen_shader` is the
# core's variant set, read through the shell's `build.rs`; `shader` is the
# shell's own feature (`ShaderTable`'s layout across `Session::shaders`).
# Pair 1: core on, shell off, under a loader with neither -- refused, for
# `lumen_shader`. Pair 2: core and shell on, under a loader with the core
# on and the shell off -- refused, for `shader`. Accept: built alike, in
# all three builds (the plain shell pair above is the third).
cargo build --release -q -p stamp-lib --features shader-core
cp target/release/libstamp_lib.so "$OUT/shader-core.so"
cargo build --release -q -p stamp-lib --features shader
cp target/release/libstamp_lib.so "$OUT/shader.so"
cargo build --release -q -p stamp-host --features shader-core
cp target/release/stamp-host "$OUT/host-shader-core"
cargo build --release -q -p stamp-host --features shader
cp target/release/stamp-host "$OUT/host-shader"
"$OUT/host-shell" "$OUT/shader-core.so" refuse "lumen-static-shell features" lumen_shader
"$OUT/host-shader-core" "$OUT/shader.so" refuse "lumen-static-shell features" shader
"$OUT/host-shader-core" "$OUT/shader-core.so" accept
"$OUT/host-shader" "$OUT/shader.so" accept
# The plain loader again, so the M49 checks below run the host they expect.
cargo build --release -q -p stamp-host

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
./target/release/stamp-host "$OUT/reordered.so" refuse "source lumen-static"

# A widget edited alone: no type of that crate crosses, so it is accepted.
sync_ws
printf '\n// M49/T49.1: an edit to the widgets crate alone.\n' >> "$WS/crates/lumen-static-widgets/src/lib.rs"
build_ws widgets
./target/release/stamp-host "$OUT/widgets.so" accept
sync_ws
echo "ABI STAMP: PASS"
