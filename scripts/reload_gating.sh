#!/usr/bin/env bash
# M49/T49.0 (docs/plan-reload-hardening.md, R5-F8): hot reload is a dev
# surface, so an application that derives `Reloadable` must build without
# it and carry none of it. `fixtures/hot-window` is resolved and built
# twice: without `hot`, serde_json must not be in its dependency graph at
# all and the build must succeed; with `hot`, it must be. Both counts are
# printed beside their bounds.
#
# Counted in the dependency graph, not with `nm`: the first version of
# this gate counted serde_json symbols in the release binary, and it read
# 0 without `hot` while `reload` was switched on unconditionally -- the
# linker drops the derive's unused `save`/`restore`, so the symbol count
# cannot see a dependency that is compiled and never called.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
deps() { cargo tree -q -p hot-window -e normal --prefix none "$@" 2>/dev/null | grep -c '^serde_json ' || true; }
off=$(deps)
on=$(deps --features hot)
echo "serde_json in the graph: without hot $off (bound: 0), with hot $on (bound: > 0)"
cargo build --release -q -p hot-window
echo "  built without hot"
[ "$on" -gt 0 ] || { echo "RELOAD GATING: FAIL -- the hot graph has no serde_json, so this count proves nothing"; exit 1; }
[ "$off" -eq 0 ] || { echo "RELOAD GATING: FAIL -- a build without hot carries the reload surface"; exit 1; }
echo "RELOAD GATING: PASS"
