#!/usr/bin/env bash
# Book include liveness: every {{#include}} in book/src must resolve.
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

[ -d book/src ] || { echo "book-includes: no book/src, skipped"; exit 0; }

status=0
n=0
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
        for marker in "ANCHOR" "ANCHOR_END"; do
            if ! grep -qE "${marker}:[[:space:]]*${rest}[[:space:]]*\$" "$target"; then
                echo "$md: $file has no \`// ${marker}: $rest\` (include would expand to nothing)" >&2
                status=1
            fi
        done
    done < <(grep -oE '\{\{#(include|rustdoc_include) [^}]+\}\}' "$md" || true)
done < <(find book/src -name '*.md' | sort)

if [ "$status" -ne 0 ]; then
    echo "" >&2
    echo "Fix the include, or restore the anchor it names." >&2
    exit 1
fi
echo "book-includes: $n includes, all resolve to a live file and anchor"
