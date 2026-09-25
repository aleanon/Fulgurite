#!/usr/bin/env bash
# Both workspaces' Cargo.lock files match their manifests (M43/T43.12).
#
# `benches-competitive/Cargo.lock` did not match its manifests from M30 to
# M43 (R17-F12): `cargo metadata --locked` there exited 101, and `--locked`
# appeared nowhere in the justfile, ci.yml or the scripts -- so every
# recipe that built there silently rewrote the committed lock, and since
# M35/T35.0 `just test` did too (through check_observe_inlining.sh). A
# gate that edits the tree it is judging proves the working tree, not the
# commit (docs/plan-gate-hygiene.md §0 rule 5).
#
# `cargo metadata --locked --offline` resolves without touching the network
# and refuses to write the lock; a stale lock is an error naming the
# workspace. No compile, so it lives in `just static`.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

status=0
out=""
for ws in . benches-competitive; do
    if err=$(cargo metadata --locked --offline --format-version 1 --manifest-path "$ws/Cargo.toml" 2>&1 >/dev/null); then
        out="$out${out:+ ; }lock ok: $ws"
    else
        echo "lock STALE: $ws -- $ws/Cargo.lock does not match its manifests:" >&2
        echo "$err" | head -4 | sed 's/^/    /' >&2
        echo "    fix: (cd $ws && cargo metadata --offline >/dev/null), review the diff, commit it" >&2
        status=1
    fi
done
[ "$status" -eq 0 ] && echo "$out"
exit "$status"
