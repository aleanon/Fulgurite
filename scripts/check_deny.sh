#!/usr/bin/env bash
# Security advisories against the root workspace's lock, offline
# (M43/T43.12). deny.toml lists the accepted advisories, each with a
# reason; any advisory NOT listed fails. Prints the count found against the
# count accepted. Needs `cargo-deny` and a fetched advisory database.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
command -v cargo-deny >/dev/null || { echo "deny: cargo-deny is not installed" >&2; exit 1; }
accepted=$(grep -cE '^\s*\{ id = "RUSTSEC-' deny.toml || true)
# `--offline` reads the advisory database cargo-deny keeps under
# CARGO_HOME; a fresh runner has none, and the check died on a git error
# about its missing directory. Fetched once when absent, then offline.
db="${CARGO_HOME:-$HOME/.cargo}/advisory-dbs"
if ! ls -d "$db"/*/ >/dev/null 2>&1; then
    cargo deny fetch db >/dev/null 2>&1 || { echo "deny: no advisory database, and \`cargo deny fetch db\` failed" >&2; exit 1; }
    echo "deny: fetched the advisory database (none was cached)"
fi
# It resolves the crate graph offline too: the sources, as check_locks.sh
# fetches them, or the check fails on "no matching package" and not on an
# advisory.
cargo fetch --locked --offline -q 2>/dev/null || cargo fetch --locked -q || { echo "deny: cargo fetch --locked failed" >&2; exit 1; }
out=$(cargo deny --offline check advisories 2>&1); rc=$?
# Every advisory, accepted or not, is reported with its ID; the ignored
# ones as `advisory-ignored`/notes, the rest as errors.
failing=$(echo "$out" | grep -cE '^error\[' || true)
ids=$(echo "$out" | grep -oE 'RUSTSEC-[0-9]{4}-[0-9]{4}' | sort -u | tr '\n' ' ')
if [ "$rc" -ne 0 ] && [ "$failing" -eq 0 ]; then
    # Not a verdict: cargo-deny failed before judging anything.
    echo "$out" | tail -5 | sed 's/^/    /' >&2
    echo "deny: cargo deny failed (exit $rc) without an advisory error -- not a verdict" >&2
    exit 1
fi
if [ "$rc" -ne 0 ]; then
    echo "$out" | grep -E '^error\[|├ ID:' | sed 's/^/    /' >&2
    echo "deny: $failing advisory error(s) not accepted in deny.toml ($accepted accepted)" >&2
    exit 1
fi
echo "deny: advisories ok -- 0 unaccepted, $accepted accepted in deny.toml${ids:+ (seen: $ids)}"
