#!/usr/bin/env bash
# The attribute table exists in two places. Keep them in agreement.
#
# `crates/lumen-static/src/lib.rs` says of its table: "This list is the
# only place they appear together." That stopped being true when the book
# gained `book/src/attributes.md`, and two copies of a table is the exact
# shape the doc-currency rule warns about -- so the second copy comes
# with a check rather than with a promise.
#
# It compares the ATTRIBUTE NAMES -- the identifier after `#[` in each row
# -- and, since M55/T55.3, the KEYS each parser accepts (below). Not the
# prose, which is allowed to differ (the appendix cross-links to chapters
# and the rustdoc does not), and not the row order.
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

# THE KEYS, NOT ONLY THE NAMES (M55/T55.3, R16-F10). Both tables listed
# every attribute name and still omitted `#[widget(wrap)]`,
# `#[widget(grid, columns = N)]` and `#[app(tick = f)]` for weeks, because
# a check that compares names cannot see a missing key. So every key the
# parser accepts -- each `meta.path.is_ident("..")` in the derive (the
# `#[widget]`, `#[app]` and `#[reload]` parsers) and `#[bind]`'s
# `kw != ".."` keyword -- must appear as a word in the first cell of some
# row of each table.
python3 - "$RUST" "$BOOK" <<'PY' || status=1
import re, sys
sys.path.insert(0, "scripts/lib")
import rustsrc
src = open("crates/lumen-static-macros/src/lib.rs", encoding="utf-8").read()
code, _ = rustsrc.split(src, keep_literals=True)
keys = sorted(set(re.findall(r'meta\.path\.is_ident\("(\w+)"\)', code))
              | set(re.findall(r'\bkw\s*!=\s*"(\w+)"', code)))
def cells(path):
    # The first cells of the reference table only -- the one headed
    # `| attribute | on | meaning |` -- so the book's "where each is
    # explained" table cannot supply a key the reference omits.
    out, inside = [], False
    for line in open(path, encoding="utf-8"):
        # `\|` is a pipe inside a cell (`row \| column`), not a column break.
        line = re.sub(r"^//! ?", "", line.rstrip("\n")).replace("\\|", "/")
        if re.match(r"^\|\s*attribute\s*\|\s*on\s*\|", line):
            inside = True
            continue
        if inside and not line.startswith("|"):
            break
        if inside:
            out.append(line.split("|")[1])
    return " ".join(out)
rust, book = cells(sys.argv[1]), cells(sys.argv[2])
has = lambda text, k: re.search(r"\b" + re.escape(k) + r"\b", text) is not None
miss = [f"{k} (rustdoc)" for k in keys if not has(rust, k)] + [f"{k} (book)" for k in keys if not has(book, k)]
print(f"attr-table: parser keys: {len(keys)}, in rustdoc table: {sum(has(rust, k) for k in keys)}, "
      f"in book table: {sum(has(book, k) for k in keys)}, missing: {', '.join(miss) or '(none)'}",
      file=sys.stderr if miss else sys.stdout)
if len(keys) < 8:
    print(f"attr-table: only {len(keys)} parser keys extracted -- the extractor lost the derive", file=sys.stderr)
    sys.exit(1)
sys.exit(1 if miss else 0)
PY

# A COUNT IN PROSE IS COMPUTED OR REMOVED (docs/plan-derive-dx.md §0 rule
# 4): if the book's gap list names a number of drawing commands, it is the
# number of variants of `enum Cmd`.
python3 - <<'PY' || status=1
import re, sys
disp = open("crates/lumen-static/src/display.rs", encoding="utf-8").read()
body = disp[disp.index("pub enum Cmd {"):]
depth, i = 0, body.index("{")
for j in range(i, len(body)):
    depth += {"{": 1, "}": -1}.get(body[j], 0)
    if depth == 0:
        body = body[i + 1:j]
        break
body = re.sub(r"//[^\n]*", "", body)
depth, variants, line_start = 0, 0, True
for m in re.finditer(r"[{}()]|^\s*([A-Z]\w*)", body, re.M):
    if m.group(1) and depth == 0:
        variants += 1
    elif m.group(0) in "{(":
        depth += 1
    elif m.group(0) in "})":
        depth -= 1
words = {w: n for n, w in enumerate("zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen".split())}
gaps = open("book/src/gaps.md", encoding="utf-8").read()
said = re.findall(r"\b(\d+|" + "|".join(words) + r")\s+commands\b", gaps, re.I)
for w in said:
    k = int(w) if w.isdigit() else words[w.lower()]
    print(f"attr-table: Cmd variants: {variants}, gaps.md says: {k}")
    if k != variants:
        print(f"attr-table: book/src/gaps.md says {w} commands; `enum Cmd` has {variants}", file=sys.stderr)
        sys.exit(1)
if not said:
    print(f"attr-table: Cmd variants: {variants}, gaps.md states no count")
PY

n() { grep -c . "$1" || true; }
if [ "$status" -ne 0 ]; then
    echo "attribute tables disagree with the derives: declared $(n "$tmp/decl"), rustdoc $(n "$tmp/rust"), book $(n "$tmp/book")" >&2
    echo "" >&2
    echo "Update the derive's list and both tables, in the same commit." >&2
    exit 1
fi
echo "attr-table: declared $(n "$tmp/decl"), rustdoc $(n "$tmp/rust"), book $(n "$tmp/book") -- the same $(n "$tmp/decl") attributes"
