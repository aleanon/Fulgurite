#!/usr/bin/env bash
# M61/T61.0 gate 4 (docs/plan-lss.md §2.2): **an edited `.lss` rebuilds
# the crate that includes it.**
#
# A proc macro that reads a file is invisible to cargo's dependency
# tracking, so a build that keeps the OLD sheet after an edit is a silent
# failure -- nothing errors, the window just shows last week's colours.
# `include_lss!` therefore also expands to an `include_bytes!` of the file,
# which makes rustc record it as an input.
#
# This gate is the only way to see that work: it builds a scratch crate
# (its own workspace, under the target directory) whose `main` prints the
# font size its sheet sets, edits ONLY the `.lss`, rebuilds, and requires
# the new value. Both values are printed beside what was written.
#
# The scratch crate's `Cargo.lock` is the workspace's, and it builds
# `--offline`, as trybuild does, so it resolves what the workspace does.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export LC_ALL=C

DIR="${CARGO_TARGET_DIR:-$ROOT/target}/lss-tracking"
CRATE="$DIR/crate"
mkdir -p "$CRATE/src"
cp Cargo.lock "$CRATE/Cargo.lock"
cat > "$CRATE/Cargo.toml" <<EOF
[package]
name = "lss-tracking"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
fulgurite = { path = "$ROOT/crates/fulgurite" }
fulgurite-widgets = { path = "$ROOT/crates/fulgurite-widgets" }

[workspace]
EOF
cat > "$CRATE/src/main.rs" <<'EOF'
use fulgurite::{include_lss, StaticTree};
use fulgurite_widgets::ops::Label;

#[derive(StaticTree, Default)]
#[widget(column, spacing = 0.0)]
struct Ui {
    #[widget_id("title")]
    title: Label,
}

const SHEET: &[fulgurite::Rule] = include_lss!(Ui, "sheet.lss");

fn main() {
    let px = SHEET.first().and_then(|r| r.patch.font_size);
    println!("{}", px.map_or("none".to_string(), |v| v.to_string()));
}
EOF

# run PX -- write the sheet with font-size PX, build, print what main sees.
run() {
    printf '#title { font-size: %spx; }\n' "$1" > "$CRATE/src/sheet.lss"
    # Nanosecond mtimes on ext4 make this unnecessary there; a coarse
    # filesystem would let an edit land in the build's own second.
    touch -d "@$(( $(date +%s) + ${2:-0} ))" "$CRATE/src/sheet.lss"
    (cd "$CRATE" && CARGO_TARGET_DIR="$DIR/target" cargo run -q --offline 2>"$DIR/build.log") || {
        echo "lss-tracking: the scratch crate failed to build:" >&2
        sed 's/^/    /' "$DIR/build.log" >&2
        return 1
    }
}

rc=0
first=$(run 11) || exit 1
second=$(run 23 2) || exit 1
echo "lss-tracking: wrote 11px, the binary saw ${first}; edited only the .lss to 23px and rebuilt, the binary saw ${second}"
[ "$first" = 11 ] || { echo "lss-tracking: setup: the first build saw '$first', not 11 -- the sheet never reached the binary" >&2; rc=1; }
[ "$second" = 23 ] || { echo "lss-tracking: an edited .lss did not rebuild the crate that includes it -- the build kept the old sheet (docs/plan-lss.md §2.2)" >&2; rc=1; }
exit "$rc"
