#!/usr/bin/env bash
# check_default_crates.sh [BASE] -- the default window's crate count does
# not move (docs/plan-iced-roadmap.md K2; M84/T84.0(c),
# docs/plan-images-in-motion.md).
#
# K2's exact command,
#   cargo tree -p lumen-static-winit -e normal --prefix none \
#     | sed 's/ (\*)$//' | sort -u | wc -l
# run on the work tree and on BASE, both printed, and the difference
# bounded at 0. Without the `sed` step a `(*)` repeat counts twice, which
# is not K2's count.
#
# Why a comparison and not a number: a default-off feature (M84's `image`
# first, whose decoder links 0 symbols on or off, so K2's `nm` half
# cannot see it -- the index's recorded exception) leaks into the default
# set through a manifest edit, and the only thing that moves is this
# count. A hard-coded expected count would be the bare claim
# scripts/check_ci_claims.sh refuses, and would go stale on every honest
# lock update; the parent commit's count is the bound.
#
# BASE defaults to the commit this work would sit on: HEAD while the work
# tree's manifests or lock differ from it, else HEAD^ -- so a clean tree
# (CI, a pre-push run) checks the last commit against its parent rather
# than comparing HEAD with itself. When no `Cargo.toml` and no
# `Cargo.lock` differs between BASE and the work tree, the two graphs are
# the same by construction (cargo tree reads the manifests and the lock
# and nothing else) and BASE is not extracted; otherwise BASE is
# extracted with `git archive` into target/default-crates/ and counted
# there.
#
# No compile: `cargo tree --locked --offline` only, so this runs in
# `just static`. A `cargo tree` that itself fails is reported as that,
# never as a pass. Only cargo's stdout is counted (see `count`): a
# lock-wait line on stderr is not a crate.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

MANIFESTS=(':(glob)**/Cargo.toml' 'Cargo.toml' 'Cargo.lock')

if [ $# -ge 1 ]; then
    BASE=$1
elif git diff --quiet HEAD -- "${MANIFESTS[@]}"; then
    BASE=HEAD^
else
    BASE=HEAD
fi
SHA=$(git rev-parse --verify -q "$BASE^{commit}") || {
    echo "check_default_crates: '$BASE' is not a commit" >&2; exit 2; }

# count DIR -- K2's count for lumen-static-winit's default build in DIR.
# Only cargo's STDOUT is counted: stderr carries cargo's progress lines,
# and "Blocking waiting for file lock on package cache" appears there
# whenever another cargo process holds the lock (a CI run beside this
# one) -- counted, it made 96 crates read 97 with nothing changed. stderr
# is kept for the error message when cargo fails.
count() {
    local out err
    err=$(mktemp) || return 1
    if ! out=$(cd "$1" && cargo tree -p lumen-static-winit -e normal --prefix none --locked --offline 2>"$err"); then
        echo "check_default_crates: cargo tree failed in $2 -- not a pass:" >&2
        tail -5 "$err" >&2
        rm -f "$err"
        return 1
    fi
    rm -f "$err"
    if [ -z "$out" ]; then
        echo "check_default_crates: cargo tree printed no crates in $2 -- not a pass" >&2
        return 1
    fi
    printf '%s\n' "$out" | sed 's/ (\*)$//' | sort -u | wc -l
}

now=$(count . "the work tree") || exit 1

if git diff --quiet "$SHA" -- "${MANIFESTS[@]}"; then
    base=$now
    how="manifests and lock identical to it"
else
    dir=target/default-crates/$SHA
    rm -rf "$dir" && mkdir -p "$dir"
    git archive "$SHA" | tar -x -C "$dir" || {
        echo "check_default_crates: cannot extract $SHA" >&2; rm -rf "$dir"; exit 1; }
    base=$(count "$dir" "$SHA") || { rm -rf "$dir"; exit 1; }
    rm -rf "$dir"
    how="counted at it"
fi

delta=$((now - base))
short=$(git rev-parse --short "$SHA")
echo "check_default_crates: lumen-static-winit default build, $now crates (K2's command); $base at $short ($how); delta $delta, bound 0"
if [ "$delta" -ne 0 ]; then
    echo "check_default_crates: the default window's crate count moved by $delta against $short -- a default-off feature leaked into the default set, or a default dependency was added (K2: bound 0)" >&2
    exit 1
fi
