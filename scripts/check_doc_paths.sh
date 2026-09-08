#!/usr/bin/env bash
# Citation liveness: every repo artifact cited from code or docs must exist.
#
# Two kinds, same failure mode -- a pointer to something that was renamed or
# deleted, which nothing in the toolchain notices:
#   (1) `docs/foo.md` paths, cited from Rust comments, other docs, the
#       justfile and Cargo.toml. Two are dangling today.
#   (2) `--bin NAME` / `-p CRATE` targets cited from docs. Two results docs
#       name bench binaries that no longer exist (`memory`, `safety`), which
#       makes their numbers unreproducible -- the doc says how to rerun the
#       measurement and the command does not run.
#
# Cargo target names come from `cargo metadata`, so autodiscovered bins (the
# `fusion`/`repeater` case, which benches/Cargo.toml does not declare with a
# [[bin]] section) count as existing.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
status=0

sources=$(find crates benches examples fixtures docs scripts -type f \
    \( -name '*.rs' -o -name '*.md' -o -name '*.toml' -o -name '*.sh' \) \
    -not -path '*/target/*' -not -name 'check_*.sh' 2>/dev/null; echo justfile; echo AGENT.md; echo Cargo.toml)

# --- (1) docs/*.md paths ---
# Capture any leading path component too, so `Lumen/docs/foo.md` — a
# citation of another repo's tree — extracts whole and is dropped by the
# `^docs/` filter rather than masquerading as one of ours.
{ grep -hoE '[A-Za-z0-9._/-]*docs/[A-Za-z0-9._/-]+\.md' $sources 2>/dev/null || true; } \
    | grep -E '^docs/' | sort -u > "$tmp/paths"

# A plan may name a document it promises to create. Those live in
# scripts/doc-paths.allow with a reason, and come off the list as they
# are written.
allow="$(dirname "$0")/doc-paths.allow"
if [ -f "$allow" ]; then
    sed -e 's/#.*$//' -e 's/[[:space:]]*$//' "$allow" | grep -vE '^$' | sort -u > "$tmp/allow" || true
    comm -23 "$tmp/paths" "$tmp/allow" > "$tmp/paths.f" && mv "$tmp/paths.f" "$tmp/paths"
fi
while IFS= read -r p; do
    [ -n "$p" ] || continue
    if [ ! -f "$p" ]; then
        echo "dangling doc path: $p" >&2
        { grep -rlF "$p" $sources 2>/dev/null || true; } | head -4 | sed 's/^/    cited by /' >&2
        status=1
    fi
done < "$tmp/paths"

# --- (2) cargo targets cited from docs and scripts ---
if command -v cargo >/dev/null; then
    # BOTH workspaces. `benches-competitive` is deliberately its own
    # workspace so foreign toolkits stay out of this one's Cargo.lock, and
    # until 2026-09-08 its targets were invisible here — so a doc citing
    # `--bin bands` failed the gate for existing in the wrong workspace
    # rather than for not existing.
    : > "$tmp/targets"
    for ws in . benches-competitive; do
        [ -f "$ws/Cargo.toml" ] || continue
        cargo metadata --no-deps --format-version 1 --manifest-path "$ws/Cargo.toml" 2>/dev/null \
          | tr ',' '\n' | grep -oE '"name":"[A-Za-z0-9_-]+"' | sed 's/"name":"//; s/"//' \
          >> "$tmp/targets" || true
    done
    sort -u -o "$tmp/targets" "$tmp/targets"

    { grep -hoE '\-\-bin [A-Za-z0-9_-]+' docs/*.md justfile 2>/dev/null || true; } \
      | awk '{print $2}' | sort -u > "$tmp/bins"
    while IFS= read -r b; do
        [ -n "$b" ] || continue
        if ! grep -qxF "$b" "$tmp/targets"; then
            echo "cited --bin target does not exist: $b" >&2
            { grep -rln -- "--bin $b" docs/*.md justfile 2>/dev/null || true; } | sed 's/^/    cited by /' >&2
            status=1
        fi
    done < "$tmp/bins"
fi

if [ "$status" -ne 0 ]; then
    echo "" >&2
    echo "Fix the citation, or restore what it points at." >&2
    exit 1
fi
echo "doc-paths: $(wc -l < "$tmp/paths") doc paths and $(wc -l < "$tmp/bins" 2>/dev/null || echo 0) bin targets, all live"
