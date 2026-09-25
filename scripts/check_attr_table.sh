#!/usr/bin/env bash
# The attribute table exists in two places. Keep them in agreement.
#
# `crates/lumen-static/src/lib.rs` says of its table: "This list is the
# only place they appear together." That stopped being true when the book
# gained `book/src/attributes.md`, and two copies of a table is the exact
# shape the doc-currency rule warns about -- so the second copy comes
# with a check rather than with a promise.
#
# It compares the ATTRIBUTE NAMES: the identifier after `#[` in each row.
# Not the prose, which is allowed to differ (the appendix cross-links to
# chapters and the rustdoc does not), and not the row order.
#
# Worth writing: the rustdoc table said `row | column` for four days
# after `#[widget(stack)]` shipped in T11.6, because the commit that
# added a container mode did not touch the table that lists them.
#
# THREE SETS, NOT TWO (M43/T43.3, R17-F4). Comparing the two tables with
# each other passes when both omit an attribute the parser accepts. So
# the DECLARED set -- every helper attribute in the derives' own
# `attributes(..)` lists, read by scripts/lib/derive_attrs.py -- must
# equal the rustdoc table and the book table. And a missing book is a
# failure: it used to print "skipped" and exit 0.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

RUST=crates/lumen-static/src/lib.rs
BOOK=book/src/attributes.md
[ -f "$BOOK" ] || { echo "attr-table: no book table at $BOOK -- a missing book is a failure, not a skip" >&2; exit 1; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT

# Rows look like:  | `#[widget_id("save")]` | a field | ... |
# TABLE ROWS ONLY -- a line whose first non-comment character is `|`.
# Without that restriction the rustdoc's worked example at the top of
# lib.rs contributes `#[derive`, and the check fails for a reason that
# has nothing to do with the tables. Take the identifier between `#[`
# and the first `(`, `]` or backtick.
extract() {
    sed -E 's@^//! ?@@' "$1" \
      | grep -E '^\|' \
      | grep -oE '`#\[[a-z_]+' | sed 's/`#\[//' | sort -u
}

extract "$RUST" > "$tmp/rust"
extract "$BOOK" > "$tmp/book"
python3 scripts/lib/derive_attrs.py declared | cut -f2 | grep -v '^$' | sort -u > "$tmp/decl"

status=0
report() {  # report FILE-A FILE-B MESSAGE
    comm -23 "$1" "$2" | while IFS= read -r a; do echo "    $a: $3" >&2; done
    [ -z "$(comm -23 "$1" "$2")" ]
}
report "$tmp/decl" "$tmp/rust" "declared, not documented in the rustdoc table ($RUST)" || status=1
report "$tmp/decl" "$tmp/book" "declared, not documented in the book table ($BOOK)" || status=1
report "$tmp/rust" "$tmp/decl" "in the rustdoc table, but no derive declares it" || status=1
report "$tmp/book" "$tmp/decl" "in the book table, but no derive declares it" || status=1

n() { grep -c . "$1" || true; }
if [ "$status" -ne 0 ]; then
    echo "attribute tables disagree with the derives: declared $(n "$tmp/decl"), rustdoc $(n "$tmp/rust"), book $(n "$tmp/book")" >&2
    echo "" >&2
    echo "Update the derive's list and both tables, in the same commit." >&2
    exit 1
fi
echo "attr-table: declared $(n "$tmp/decl"), rustdoc $(n "$tmp/rust"), book $(n "$tmp/book") -- the same $(n "$tmp/decl") attributes"
