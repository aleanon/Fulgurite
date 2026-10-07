#!/usr/bin/env bash
# No integration-test binary may list zero tests without saying why
# (M43/T43.9, docs/plan-gate-hygiene.md §0 rule 4).
#
# A `#![cfg(..)]` at the top of a tests/ file compiles the whole file out
# when the cfg is false -- and cargo still builds and runs the empty
# binary, reports `0 passed`, and the run is green. Cargo's feature
# unification decides which cfgs are true, so a file can be emptied by a
# change in a different crate's manifest. That is how the golden gates ran
# nowhere: `tests/golden.rs` is `#![cfg(not(feature = "truetype"))]`, and
# every `--workspace` build turns `truetype` on (R17-F1).
#
# This builds the same selection `just dev-shape` tests (`--no-run`, so it
# is free after that recipe), runs every `tests/*.rs` binary with `--list`,
# and fails on any that lists zero tests unless scripts/zero-tests.allow
# names it. An allow entry is `package/test: reason`, and a reason of
# exactly `justfile` is CHECKED: the justfile must contain a line running
# `-p package ... --test test`, i.e. the binary is empty here because it
# runs alone somewhere else -- remove that line and the allowance lapses.
#
#   check_test_counts.sh           the dev-shape selection
#   check_test_counts.sh ARGS...   another cargo selection
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

ALLOW=scripts/zero-tests.allow
sel=("$@")
[ "${#sel[@]}" -gt 0 ] || sel=(--workspace --exclude lumen-static-wgpu)

# cargo's stderr is kept, and its tail printed on failure: CI at 89e3f7d
# failed here with only an exit code, and the cause could not be recovered.
errf=$(mktemp)
json=$(cargo test -q "${sel[@]}" --no-run --message-format=json 2>"$errf")
rc=$?
[ "$rc" -eq 0 ] || { echo "test-counts: cargo test --no-run failed (exit $rc)" >&2; tail -20 "$errf" >&2; rm -f "$errf"; exit 1; }
rm -f "$errf"

printf '%s\n' "$json" | python3 scripts/lib/test_counts.py "$ALLOW"
