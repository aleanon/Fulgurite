#!/usr/bin/env bash
# M48/T48.14 (R13-F11): the documented method set is the built one.
#
# Extracts every method-name string literal ("ui.x", "input.x", "app.x")
# that the agent's dispatch answers -- `crates/lumen-static-agent/src/lib.rs`
# (`dispatch`, `dispatch_input`, `handle`, `handle_input`) and the window's
# `answer_agent` (`crates/lumen-static-winit/src/lib.rs`) -- and diffs the
# set against the backticked names in the skill's "Methods that exist"
# paragraph and in `book/src/agent.md`. Either list non-empty fails.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

skill=.claude/skills/verifying-apps/SKILL.md
book=book/src/agent.md
name='(ui|input|app)\.[A-Za-z]+'

code=$( { sed -E 's@//.*$@@' crates/lumen-static-agent/src/lib.rs; \
          sed -E 's@//.*$@@' crates/lumen-static-winit/src/lib.rs; } \
        | grep -oE "\"$name\"" | tr -d '"' | sort -u)
# The skill's paragraph: from its bold heading to the next blank line.
skill_set=$(awk '/\*\*Methods that exist\*\*/{on=1} on&&/^$/{exit} on' "$skill" \
        | grep -oE "\`$name\`" | tr -d '`' | sort -u)
book_set=$(grep -oE "\`$name\`" "$book" | tr -d '`' | sort -u)

fail=0
report() { # label, documented set
    local missing extra
    missing=$(comm -23 <(echo "$code") <(echo "$2") | paste -sd, -)
    extra=$(comm -13 <(echo "$code") <(echo "$2") | paste -sd, -)
    echo "agent-methods: $1: missing: [${missing}], extra: [${extra}]"
    if [ -n "$missing$extra" ]; then fail=1; fi
}
echo "agent-methods: code $(echo "$code" | wc -l) methods, skill $(echo "$skill_set" | grep -c . || true), book $(echo "$book_set" | grep -c . || true)"
report skill "$skill_set"
report book "$book_set"
if [ "$fail" -ne 0 ]; then
    echo "agent-methods: the documented surface is not the built one -- fix the skill or the book (or the code)" >&2
    exit 1
fi
