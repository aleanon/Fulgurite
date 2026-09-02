#!/usr/bin/env bash
# Backticked-identifier liveness (docs/00-handoff-static-tree.md doc-currency rule).
#
# Every `Foo::bar` / `KIND_COUNT` / `offset_of!` written inside a Rust comment
# must still exist as a token in workspace *code*. This is a liveness check,
# not a resolution check: it does not know scope, so it cannot tell you that
# `Widget::hit` is on the wrong type -- only that nothing anywhere is called
# `hit`. That weak bar is deliberate. It is what keeps prose cheap: a
# backticked `layout` or `node` or `Vec` passes for free because those tokens
# exist, so only genuinely dead names reach the allowlist.
#
# Two rules make it work:
#   * the token set is built from code with comments STRIPPED. Without that,
#     a name that exists only in the comment that cites it validates itself --
#     which is exactly how `KIND_COUNT` and `offset_of!` survived until now.
#   * candidates are shape-filtered to bare identifiers. `Vec<u64>`, `&mut T`,
#     `a || b` and prose spans never become candidates, because they do not
#     match ^[A-Za-z_][A-Za-z0-9_]*$ after ::-splitting.
#
# Allowlist: scripts/doc-identifiers.allow, one name per line, `#` comments.
# It is for names that are real but not ours (C functions, rustc features,
# upstream field names) and for names documented as deliberately REMOVED.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

ALLOW=scripts/doc-identifiers.allow
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT

find crates benches examples fixtures -name '*.rs' -not -path '*/target/*' | sort > "$tmp/files"

# The universe: every identifier appearing in code, comments removed.
sed -E 's@//.*$@@' $(cat "$tmp/files") \
  | grep -hoE '\b[A-Za-z_][A-Za-z0-9_]*\b' | sort -u > "$tmp/tokens"

# Candidates: backticked spans in any comment (`///`, `//!`, and plain `//` --
# internal comments drift just as hard), split on `::`, `!` and `()` trimmed.
grep -hE '^[[:space:]]*//' $(cat "$tmp/files") \
  | grep -oE '`[^`]+`' | sed 's/`//g' \
  | tr ':' '\n' | sed -E 's/!$//; s/\(\)$//' \
  | grep -E '^[A-Za-z_][A-Za-z0-9_]*$' | sort -u > "$tmp/cands"

# Rust keywords, attributes and prelude names: never project identifiers.
cat > "$tmp/builtin" <<'EOF'
None Ok Err Some Box String Vec Option Result Cow Rc Arc RefCell Cell
Default Clone Copy Debug Display Drop Iterator Into From Send Sync Sized
as async await break const continue crate dyn else enum extern false fn for
if impl in let loop match mod move mut pub ref return self static struct
super trait true type union unsafe use where while
derive doc cfg no_std no_mangle repr inline test allow deny warn
EOF
tr -s ' \n' '\n' < "$tmp/builtin" | grep -v '^$' | sort -u > "$tmp/builtin.sorted"

if [ -f "$ALLOW" ]; then
    sed -E 's/#.*$//' "$ALLOW" | tr -d '[:blank:]' | grep -v '^$' | sort -u > "$tmp/allow"
else
    : > "$tmp/allow"
fi

sort -u "$tmp/tokens" "$tmp/builtin.sorted" "$tmp/allow" > "$tmp/known"
comm -23 "$tmp/cands" "$tmp/known" > "$tmp/dead"

if [ -s "$tmp/dead" ]; then
    echo "dead backticked identifiers -- documented but absent from workspace code:" >&2
    while IFS= read -r name; do
        echo "  \`$name\`" >&2
        # `|| true` on every stage: grep exits 1 on no match and head closes
        # the pipe early, and under `set -euo pipefail` either would abort the
        # loop -- the same bug class that inverted the old no-unsafe gate.
        { grep -rnE --include='*.rs' "\`$name(!|\(\))?\`" crates benches examples fixtures 2>/dev/null || true; } \
          | { grep -E '^[^:]+:[0-9]+:[[:space:]]*//' || true; } | head -3 | sed 's/^/      /' >&2 || true
    done < "$tmp/dead"
    echo "" >&2
    echo "Fix the comment, or add the name to $ALLOW with a reason." >&2
    exit 1
fi
echo "doc-identifiers: $(wc -l < "$tmp/cands") backticked names, all live"
