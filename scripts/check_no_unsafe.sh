#!/usr/bin/env bash
# M0 gate: the release core contains no `unsafe` at all
# (docs/plan-architecture-v2.md, docs/results-safe-prototype.md).
#
# Two bugs in the previous justfile version this replaces:
#   1. the pattern `unsafe (\{|fn |impl |trait )` missed `unsafe extern "C" fn`
#      and `unsafe{`. `\bunsafe\b` has no such holes. It also matches the word
#      in prose, so comments are stripped first rather than narrowing the
#      pattern -- a narrow pattern is a silent bypass, a stripped comment is not.
#   2. it scanned only crates/lumen-static/src/*.rs, so lumen-static-macros,
#      -agent and -shell could contain anything.
#
# Two exclusions, both because the code *is* the FFI boundary:
#   - fixtures/hot-host: dlopen requires unsafe, and it is a fixture.
#   - crates/lumen-static-reload: the `extern "C"` shims a reloadable app
#     exports. They exist in their own crate precisely so this check can stay
#     absolute over the core rather than growing a cfg-gated exception --
#     which is how a gate stops meaning anything.
# Nothing else is excluded, and adding one means editing this list in a commit
# that has to say why.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

status=0
while IFS= read -r f; do
    # Strip line comments and doc comments before matching, so "no unsafe"
    # can still be *written about*. Naive `//`-to-EOL; it only ever removes
    # text, so it cannot hide a real `unsafe` token outside a string literal.
    hits=$(sed -E 's@//.*$@@' "$f" | grep -nE '\bunsafe\b' || true)
    if [ -n "$hits" ]; then
        echo "$f:" >&2
        echo "$hits" | sed 's/^/  /' >&2
        status=1
    fi
done < <(find crates -path '*/src/*' -name '*.rs' \
    -not -path 'crates/lumen-static-reload/*' | sort)

if [ "$status" -ne 0 ]; then
    echo "release core must contain no unsafe (see above)" >&2
    exit 1
fi
echo "no-unsafe: clean across crates/*/src (except lumen-static-reload, the FFI crate)"
