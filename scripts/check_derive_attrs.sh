#!/usr/bin/env bash
# Derive-attribute liveness: every helper attribute a proc macro declares in
# `#[proc_macro_derive(_, attributes(...))]` must be used by at least one test,
# example or fixture.
#
# An unused helper attribute is a feature with zero coverage that the compiler
# will never complain about: syn parses whatever the derive body asks for, and
# an attribute nobody writes is an attribute nobody has ever run.
#
# `tests/` and `examples/` and `fixtures/` and `benches/` count as uses;
# `src/` does not. The point is demonstrated usage from outside the macro,
# not the macro's own mention of the string.
#
# M43/T43.3 (R17-F5): the declared lists come from scripts/lib/derive_attrs.py,
# which reads each `#[proc_macro_derive(..)]` whole -- a list split across
# lines used to vanish from a line-at-a-time grep, taking its attributes'
# check with it -- and a use counts only in CODE: comments and literals are
# blanked first, so `// #[foo]` is not a user of `foo`.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

decl=$(python3 scripts/lib/derive_attrs.py declared)
nderives=$(echo "$decl" | cut -f1 | sort -u | grep -c . || true)
attrs=$(echo "$decl" | cut -f2 | grep -v '^$' | sort -u)
nattrs=$(echo "$attrs" | grep -c . || true)
if [ "$nattrs" -eq 0 ]; then
    echo "derive-attrs: no proc_macro_derive helper attributes declared" >&2
    exit 1
fi
echo "derive-attrs: derives $nderives, attributes $nattrs"

mapfile -t users < <(find crates/*/tests examples fixtures benches -name '*.rs' -not -path '*/target/*' 2>/dev/null | sort)

status=0
while IFS=$'\t' read -r a n; do
    if [ "$n" -eq 0 ]; then
        echo "helper attribute \`$a\` is declared but used by no test, example or fixture" >&2
        echo "$decl" | awk -F'\t' -v a="$a" '$2 == a {print "    declared by #[derive(" $1 ")]"}' >&2
        status=1
    else
        printf 'derive-attrs: %-14s %3d uses\n' "$a" "$n"
    fi
done < <(python3 scripts/lib/derive_attrs.py uses "${users[@]}")

if [ "$status" -ne 0 ]; then
    echo "" >&2
    echo "Either write a test that uses it, or drop it from the attributes(...) list." >&2
    exit 1
fi
