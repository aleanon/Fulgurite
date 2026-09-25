#!/usr/bin/env bash
# Book reference liveness: every {{#include}} AND every intra-book link
# in book/src must resolve.
#
# WHY THIS EXISTS (measured 2026-09-10, mdbook v0.5.4):
#   * a missing *anchor* expands to the EMPTY STRING. `mdbook build` prints
#     nothing at all and `mdbook test` reports the chapter `ok` -- it
#     compiled an empty snippet. That is the repository's documented
#     vacuous-test shape: the setup silently did not happen, so both sides
#     agree about a frame in which nothing occurred.
#   * a missing *file* logs `ERROR` and then EXITS 0, so a build script that
#     trusts the exit status ships a book with a hole in it.
# Renaming an anchor in examples/ therefore blanks a chapter and turns its
# test green. Nothing else in the toolchain notices. This does.
#
# Line-number includes ({{#include f.rs:10:20}}) are refused rather than
# checked. A line number is a bound computed from a quantity that moves
# every time the file is edited, and it fails silently in the same way --
# it just includes the wrong lines. Use a named anchor.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

[ -d book/src ] || { echo "book-includes: no book/src -- a missing book is a failure, not a skip" >&2; exit 1; }

status=0
n=0
l=0
f_rust=0
f_norun=0
seen=$(mktemp); trap 'rm -f "$seen"' EXIT
while IFS= read -r md; do
    # One include per line is the convention; grep -o handles several.
    while IFS= read -r inc; do
        [ -n "$inc" ] || continue
        n=$((n + 1))
        spec=${inc#\{\{#}
        spec=${spec%\}\}}
        spec=${spec#include }
        spec=${spec#rustdoc_include }
        spec=$(echo "$spec" | sed 's/[[:space:]]*$//')

        file=${spec%%:*}
        rest=""
        [ "$spec" != "$file" ] && rest=${spec#*:}

        target="$(dirname "$md")/$file"
        if [ ! -f "$target" ]; then
            echo "$md: include names a file that does not exist: $file" >&2
            status=1
            continue
        fi

        [ -z "$rest" ] && continue

        if echo "$rest" | grep -qE '^[0-9]*(:[0-9]*)?$'; then
            echo "$md: line-number include, use a named anchor instead: {{#include $spec}}" >&2
            status=1
            continue
        fi

        # Anchor names are [A-Za-z0-9_-]; anything else is a typo, and
        # would also be a regex injection into the grep below.
        if ! echo "$rest" | grep -qE '^[A-Za-z0-9_-]+$'; then
            echo "$md: malformed anchor name in {{#include $spec}}" >&2
            status=1
            continue
        fi

        # END-ANCHORED, and it has to be. The first version of this check
        # used `grep -F "ANCHOR: $rest"`, which is a SUBSTRING match:
        # renaming `// ANCHOR: tree` to `// ANCHOR: treex` in
        # examples/counter left the gate green (6 of 6 "resolve") while
        # two chapters had already stopped compiling. The bound was
        # computed from the wrong quantity -- a prefix, not the name.
        echo "$(cd "$(dirname "$target")" && pwd)/$(basename "$target")	$rest" >> "$seen"

        for marker in "ANCHOR" "ANCHOR_END"; do
            if ! grep -qE "${marker}:[[:space:]]*${rest}[[:space:]]*\$" "$target"; then
                echo "$md: $file has no \`// ${marker}: $rest\` (include would expand to nothing)" >&2
                status=1
            fi
        done
    done < <(grep -oE '\{\{#(include|rustdoc_include) [^}]+\}\}' "$md" || true)
    # `ignore` fences. A ```rust,ignore block compiles nothing, renders
    # identically to a tested one, and is what a writer reaches for when
    # a fragment will not compile on its own. Chapter 2's first draft had
    # three, each duplicating an anchor directly above it. The two honest
    # options are a finer anchor plus hidden setup lines, or inline prose.
    #
    # Any CommonMark fence (M43/T43.2, R17-F16): up to three spaces of
    # indent, and three or more backticks OR tildes. The ban used to match
    # only a backtick fence at column 0, so `~~~rust,ignore` and an
    # indented fence both passed.
    fence='^ {0,3}(`{3,}|~{3,})'
    if grep -qE "${fence}[a-z, _]*ignore" "$md"; then
        echo "$md: \`\`\`rust,ignore compiles nothing -- include it, or make it run" >&2
        grep -nE "${fence}[a-z, _]*ignore" "$md" | head -3 | sed 's/^/    /' >&2
        status=1
    fi
    f_rust=$((f_rust + $(grep -cE "${fence}rust" "$md" || true)))
    f_norun=$((f_norun + $(grep -cE "${fence}[a-z, _]*no_run" "$md" || true)))

    # Intra-book links. A chapter that links to a page nobody has
    # written yet renders as a link that goes nowhere -- silently, like
    # a missing anchor, and for the same reason: nothing in mdbook
    # resolves relative markdown links. Forward references to draft
    # chapters are the obvious way to acquire one.
    while IFS= read -r href; do
        [ -n "$href" ] || continue
        l=$((l + 1))
        [ -f "$(dirname "$md")/$href" ] || {
            echo "$md: links to a page that does not exist: $href" >&2
            status=1
        }
    done < <(grep -oE '\]\(\.?/?[A-Za-z0-9._/-]+\.md\)' "$md" \
             | sed -E 's/^\]\(//; s/\)$//' || true)
done < <(find book/src -name '*.md' | sort)

# --- the reverse direction: an anchor nobody includes -----------------
#
# `check_derive_attrs.sh` requires every declared derive attribute to have
# a user; this is the same relation for anchors. An `// ANCHOR: foo` that
# no chapter includes is a comment whose entire purpose is a reference
# that does not exist -- AGENT.md's fragile kind, pointing at another file
# and unable to notice when that file stops pointing back.
#
# It also settles when anchors may be added: WITH the chapter that
# consumes them, never ahead of it. Harvesting anchors in one commit and
# writing the prose in another is the separate pass the doc-currency rule
# exists to prevent.
orphans=0
while IFS= read -r line; do
    f=${line%%:*}
    name=$(echo "${line#*:}" | sed -E 's/.*ANCHOR:[[:space:]]*//; s/[[:space:]]*$//')
    abs="$(cd "$(dirname "$f")" && pwd)/$(basename "$f")"
    if ! grep -qF "$abs	$name" "$seen" 2>/dev/null; then
        echo "$f: \`// ANCHOR: $name\` is included by no chapter" >&2
        orphans=$((orphans + 1))
        status=1
    fi
done < <(grep -rn --include='*.rs' -E '^[[:space:]]*//[[:space:]]*ANCHOR:' crates examples fixtures benches 2>/dev/null | sed 's/:[0-9]*:/:/' || true)

if [ "$status" -ne 0 ]; then
    echo "" >&2
    if [ "$orphans" -ne 0 ]; then
        echo "Add anchors with the chapter that includes them, or delete them." >&2
    fi
    echo "Fix the reference, or restore what it names." >&2
    exit 1
fi
echo "book-refs: $n includes and $l links all resolve; no orphaned anchors; fences: $f_rust rust, $f_norun no_run, 0 ignore"
