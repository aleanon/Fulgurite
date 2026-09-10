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
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

RUST=crates/lumen-static/src/lib.rs
BOOK=book/src/attributes.md
[ -f "$BOOK" ] || { echo "attr-table: no $BOOK, skipped"; exit 0; }

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

if ! diff -q "$tmp/rust" "$tmp/book" >/dev/null; then
    echo "attribute tables disagree between $RUST and $BOOK:" >&2
    comm -23 "$tmp/rust" "$tmp/book" | sed 's/^/    only in rustdoc: /' >&2
    comm -13 "$tmp/rust" "$tmp/book" | sed 's/^/    only in the book: /' >&2
    echo "" >&2
    echo "Update both, in the same commit." >&2
    exit 1
fi
echo "attr-table: $(wc -l < "$tmp/rust") attributes, documented in both places"
