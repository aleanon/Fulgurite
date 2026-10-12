#!/usr/bin/env bash
# `just release-shape`: core's own tests, lint and smoke run in the shipped
# shape -- default features, no dev surface (M43/T43.7).
#
# It is the ONLY build that compiles what docs/verification-notes.md scopes
# its argument to; `--workspace` cannot produce it, because the agent, the
# examples and the hot-reload fixtures turn on `observe`/`reload` and cargo
# unifies features across the selection. And `-p fulgurite` alone is
# not enough by itself either: a dev-dependency's features unify into the
# crate under test, so until M43 core's dev edge on the widgets (with
# `observe`) made this gate a dev build (R12-F1).
#
# So three things, each of which a reader can check in the output:
#   * `--cfg fulgurite_release_shape`, in a target directory of its own (a
#     RUSTFLAGS change would otherwise rebuild the shared one): under it
#     core's lib.rs refuses to compile with `observe` or `reload` on -- the
#     tripwire, which names the fault rather than letting the tests pass;
#   * the resolved feature set of `fulgurite`, printed from `cargo tree`;
#   * the number of core tests that ran, which must be non-zero, so an
#     emptied test set is visible.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

export CARGO_TARGET_DIR=target/release-shape
export RUSTFLAGS="${RUSTFLAGS:-} --cfg fulgurite_release_shape"

feats=$(cargo tree -q -p fulgurite -e features -i fulgurite --prefix none 2>/dev/null \
    | grep -oE '^fulgurite feature "[a-z_]+"' | sed -E 's/.*"(.*)"/\1/' | sort -u | tr '\n' ' ')
echo "release-shape: fulgurite features: ${feats:-(none)}"

log=$(mktemp); trap 'rm -f "$log"' EXIT
set +e
cargo test -p fulgurite 2>&1 | tee "$log"
rc=${PIPESTATUS[0]}
set -e
if grep -q 'release-shape tripwire' "$log"; then
    echo "release-shape: the tripwire fired -- fulgurite was built with observe/reload" >&2
    exit 1
fi
[ "$rc" -eq 0 ] || exit "$rc"
n=$(grep -oE 'test result: ok\. [0-9]+ passed' "$log" | awk '{s += $4} END {print s + 0}')
echo "release-shape: $n core tests passed, features: ${feats:-(none)}"
if [ "$n" -eq 0 ]; then
    echo "release-shape: zero core tests ran -- the test set is empty, not green" >&2
    exit 1
fi

cargo clippy -p fulgurite --all-targets -- -D warnings
cargo run -p miri-smoke
