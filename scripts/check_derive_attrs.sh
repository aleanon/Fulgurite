#!/usr/bin/env bash
# Derive-attribute liveness: every helper attribute a proc macro declares in
# `#[proc_macro_derive(_, attributes(...))]` must be used by at least one test,
# example or fixture.
#
# An unused helper attribute is a feature with zero coverage that the compiler
# will never complain about: syn parses whatever the derive body asks for, and
# an attribute nobody writes is an attribute nobody has ever run. Today
# `widget_class` is in the StaticTree list with zero uses anywhere.
#
# `tests/` and `examples/` and `fixtures/` count as uses; `src/` does not.
# The point is demonstrated usage from outside the macro, not the macro's own
# mention of the string.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

decl=$(grep -rhoE 'attributes\([^)]*\)' --include='*.rs' crates/*/src/ || true)
attrs=$(echo "$decl" | sed -E 's/attributes\(//; s/\)//' | tr ',' '\n' | tr -d '[:blank:]' | grep -v '^$' | sort -u)

if [ -z "$attrs" ]; then
    echo "derive-attrs: no proc_macro_derive helper attributes declared" >&2
    exit 1
fi

# Users: anything outside src/. Matches both `#[attr...]` outer form and the
# `attr = ..` / `attr(..)` inner-key form used inside `#[widget(...)]`.
mapfile -t users < <(find crates/*/tests examples fixtures benches -name '*.rs' -not -path '*/target/*' 2>/dev/null | sort)

status=0
for a in $attrs; do
    n=$({ grep -hoE "#\[$a\b|(^|[[:space:],(])$a[[:space:]]*[=(]" "${users[@]}" 2>/dev/null || true; } | wc -l)
    if [ "$n" -eq 0 ]; then
        echo "helper attribute \`$a\` is declared but used by no test, example or fixture" >&2
        grep -rn "attributes(.*\b$a\b" --include='*.rs' crates/*/src/ | sed 's/^/    declared at /' >&2 || true
        status=1
    else
        printf 'derive-attrs: %-14s %3d uses\n' "$a" "$n"
    fi
done

if [ "$status" -ne 0 ]; then
    echo "" >&2
    echo "Either write a test that uses it, or drop it from the attributes(...) list." >&2
    exit 1
fi
