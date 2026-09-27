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
# COMMENTS ARE LEXED, NOT GREPPED (M43/T43.2, R17-F15). Candidates used to
# come only from lines that START with `//`, so a trailing comment and a
# `/* .. */` block were never read. scripts/lib/rustsrc.py lexes each file
# (string and char literals included, so a `//` inside a string is not a
# comment) and every comment counts: line, trailing and block.
#
# Allowlist: scripts/doc-identifiers.allow, one name per line, `#` comments.
# It is for names that are real but not ours (C functions, rustc features,
# upstream field names) and for names documented as deliberately REMOVED.
# **The allowlist is itself checked** (M43/T43.2): an entry that is now a
# live code token, or that nothing cites any more, fails -- an entry
# subtracted and never re-read is where drift hides. Six entries were live
# at 1717808, one of them saying it "comes off when it exists" long after
# it existed.
#
# MARKDOWN (M16/T16.6). `docs/*.md`, `book/src/*.md` and -- from M43 --
# AGENT.md are checked too, and markdown was not checked at all until
# 2026-09-08 -- which is how the design spec came to state as DECIDED that
# "rigidity is per axis (`RIGID_W`/`RIGID_H`)" when neither identifier has
# ever existed in the code. The spec is the most load-bearing document here
# and it was the one place the gate did not look.
#
# Markdown is filtered harder than Rust comments, and deliberately: prose
# legitimately backticks words that are not identifiers at all (`cargo`,
# `charts`, `accordion`), and 216 of 892 markdown candidates are dead under
# the Rust-comment rule -- an allowlist that size is where drift goes to
# hide, not where it is caught. So only **Rust-item-shaped** names count:
# CamelCase types and UPPER_SNAKE constants. That is the shape a claim about
# this codebase's API takes, it is the shape `RIGID_W` has, and it cuts the
# backlog to 84 of which half are foreign toolkit types matched by prefix.
#
# MANIFESTS (M43/T43.2, R12-F7). Cargo.toml comments are read for the same
# Rust-item-shaped names, and for the shape a comment takes when something
# stripped its backticked spans: a space before punctuation (` , `), two
# spaces between words, or trailing whitespace. That is how
# crates/lumen-static-widgets/Cargo.toml came to read "#  implies , the
# tiering  uses" -- every name gone, the sentence left.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

ALLOW=scripts/doc-identifiers.allow
LEX=scripts/lib/rustsrc.py
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
status=0

find crates benches examples fixtures -name '*.rs' -not -path '*/target/*' | sort > "$tmp/files"
mapfile -t files < "$tmp/files"

# Shape filters.
bare() {   # split on ::, trim ! and (), keep bare identifiers
    sed 's/`//g' | tr ':' '\n' | sed -E 's/!$//; s/\(\)$//' | grep -E '^[A-Za-z_][A-Za-z0-9_]*$' || true
}
itemish() {  # CamelCase (two or more humps) or UPPER_SNAKE / ALLCAPS
    grep -E '^([A-Z][a-z0-9]+([A-Z][A-Za-z0-9]*)+|[A-Z][A-Z0-9]+(_[A-Z0-9]+)*)$' || true
}

# The universe: every identifier appearing in code, comments removed
# (literals kept: a name that only a string carries, like an agent method,
# is still a name the code uses).
python3 "$LEX" nocomments "${files[@]}" \
  | grep -hoE '\b[A-Za-z_][A-Za-z0-9_]*\b' | sort -u > "$tmp/tokens"

# Candidates: backticked spans in any comment -- line (`///`, `//!`, `//`),
# trailing, or block.
python3 "$LEX" comments "${files[@]}" > "$tmp/comments"
for kind in line trailing block; do
    awk -F'\t' -v k="$kind" '$3 == k {print $4}' "$tmp/comments" \
      | { grep -oE '`[^`]+`' || true; } | bare | sort -u > "$tmp/cands.$kind"
done
sort -u "$tmp/cands.line" "$tmp/cands.trailing" "$tmp/cands.block" > "$tmp/cands"

# Markdown candidates, narrowed to Rust-item shape (see the header).
mdfiles=$(ls docs/*.md 2>/dev/null; find book/src -name '*.md' 2>/dev/null; ls AGENT.md 2>/dev/null)
: > "$tmp/mdcands"; : > "$tmp/mdall"
if [ -n "$mdfiles" ]; then
    # shellcheck disable=SC2086
    grep -hoE '`[^`]+`' $mdfiles | bare | sort -u > "$tmp/mdall"
    # Foreign toolkit types, by prefix. They appear only in the feature-gap
    # survey, they are real names in someone else's API, and listing four
    # hundred of them one per line would say nothing a prefix does not.
    itemish < "$tmp/mdall" | { grep -vE '^(Gtk|Qt)' || true; } > "$tmp/mdcands"
fi

# Manifest comments: item-shaped names, and the stripped-span shape.
mapfile -t tomls < <(find . -name Cargo.toml -not -path '*/target/*' -not -path './experiments/*' | sed 's@^\./@@' | sort)
: > "$tmp/tomlcands"; stripped=0
for t in "${tomls[@]}"; do
    grep -nE '^[[:space:]]*#' "$t" > "$tmp/tc" || true
    # Names only from the workspace whose code is the universe; the
    # stripped shape from every manifest, benches-competitive's included.
    case "$t" in benches-competitive/*) ;; *)
        { grep -oE '`[^`]+`' "$tmp/tc" || true; } | bare | itemish >> "$tmp/tomlcands" ;;
    esac
    while IFS= read -r l; do
        [ -n "$l" ] || continue
        echo "$t:${l%%:*}: comment looks stripped of its backticked names: ${l#*:}" >&2
        stripped=$((stripped + 1))
        status=1
    done < <(grep -E '#.*([A-Za-z0-9] [,.;:)]|[A-Za-z0-9]  +[A-Za-z0-9]|#  +[a-z]|[^[:space:]#][[:space:]]+$)' "$tmp/tc" || true)
done
# ci.yml (M43/T43.14): its comments cite names like any other prose.
if [ -f .github/workflows/ci.yml ]; then
    { grep -E '^[[:space:]]*#' .github/workflows/ci.yml || true; } | { grep -oE '`[^`]+`' || true; } \
      | bare | itemish >> "$tmp/tomlcands"
fi
sort -u -o "$tmp/tomlcands" "$tmp/tomlcands"

sort -u "$tmp/cands" "$tmp/mdcands" "$tmp/tomlcands" > "$tmp/cands.all"

# Rust keywords, attributes and prelude names: never project identifiers.
cat > "$tmp/builtin" <<'EOB'
None Ok Err Some Box String Vec Option Result Cow Rc Arc RefCell Cell
Default Clone Copy Debug Display Drop Iterator Into From Send Sync Sized
as async await break const continue crate dyn else enum extern false fn for
if impl in let loop match mod move mut pub ref return self static struct
super trait true type union unsafe use where while
derive doc cfg no_std no_mangle repr inline test allow deny warn
EOB
tr -s ' \n' '\n' < "$tmp/builtin" | grep -v '^$' | sort -u > "$tmp/builtin.sorted"

if [ -f "$ALLOW" ]; then
    sed -E 's/#.*$//' "$ALLOW" | tr -d '[:blank:]' | grep -v '^$' | sort -u > "$tmp/allow"
else
    : > "$tmp/allow"
fi

# --- the allowlist, checked ----------------------------------------------
# Live: an allowed name that code now defines needs no allowance, and its
# reason ("comes off when it exists", "deleted by ...") has become false.
# Uncited: nothing backticks it anywhere any more, so it allows nothing.
sort -u "$tmp/cands" "$tmp/mdall" "$tmp/tomlcands" > "$tmp/cited"
comm -12 "$tmp/allow" "$tmp/tokens" > "$tmp/stale.live"
comm -23 "$tmp/allow" "$tmp/cited" > "$tmp/stale.uncited"
while IFS= read -r name; do
    [ -n "$name" ] || continue
    echo "$ALLOW: \`$name\` is allowed but is a live code token -- remove the entry" >&2
    status=1
done < "$tmp/stale.live"
while IFS= read -r name; do
    [ -n "$name" ] || continue
    echo "$ALLOW: \`$name\` is allowed but cited by no comment or doc -- remove the entry" >&2
    status=1
done < "$tmp/stale.uncited"
nstale=$(cat "$tmp/stale.live" "$tmp/stale.uncited" | grep -c . || true)

# --- dead names ------------------------------------------------------------
sort -u "$tmp/tokens" "$tmp/builtin.sorted" "$tmp/allow" > "$tmp/known"
comm -23 "$tmp/cands.all" "$tmp/known" > "$tmp/dead"

if [ -s "$tmp/dead" ]; then
    echo "dead backticked identifiers -- documented but absent from workspace code:" >&2
    while IFS= read -r name; do
        echo "  \`$name\`" >&2
        # `|| true` on every stage: grep exits 1 on no match and head closes
        # the pipe early, and under `set -euo pipefail` either would abort the
        # loop -- the same bug class that inverted the old no-unsafe gate.
        { grep -rnE --include='*.rs' "\`$name(!|\(\))?\`" crates benches examples fixtures 2>/dev/null || true; } \
          | head -3 | sed 's/^/      /' >&2 || true
        # shellcheck disable=SC2086
        { grep -nE "\`$name(!|\(\))?\`" $mdfiles "${tomls[@]}" 2>/dev/null || true; } \
          | head -3 | sed 's/^/      /' >&2 || true
    done < "$tmp/dead"
    echo "" >&2
    echo "Fix the comment, or add the name to $ALLOW with a reason." >&2
    status=1
fi

# --- feature names (M54/T54.9, R15-F9) -------------------------------------
# A backticked name followed by "feature" in a comment -- Rust or manifest --
# must be a key in some `[features]` table (after a `crate/` prefix is cut).
# `image.rs` said decoding "lives behind" the shell's default-off `image`
# feature for a feature that never existed, and the identifier rule above
# passed it because `image` is a live token. "there is no `x` feature" is a
# negation and is skipped.
python3 - "$tmp/comments" "${tomls[@]}" > "$tmp/features.out" <<'PY' || status=1
import re, sys
comments, tomls = sys.argv[1], sys.argv[2:]
feats = set()
for t in tomls:
    s = open(t).read()
    m = re.search(r"^\[features\]\n(.*?)(?=^\[|\Z)", s, re.M | re.S)
    if m:
        feats.update(re.findall(r"^([A-Za-z0-9_-]+)\s*=", m.group(1), re.M))
pat = re.compile(r"`([^`]+)`\s+(?:Cargo\s+)?features?\b")
# "no `x` feature", the "no" possibly ending the previous comment line.
neg = re.compile(r"\bno\s*$", re.I)
sites = []
for line in open(comments):
    parts = line.rstrip("\n").split("\t")
    if len(parts) >= 4:
        sites.append((parts[0], parts[1], parts[3]))
for t in tomls:
    for i, l in enumerate(open(t), 1):
        if l.lstrip().startswith("#"):
            sites.append((t, str(i), l))
checked, missing = 0, []
prev = ("", "")
for f, ln, text in sites:
    for m in pat.finditer(text):
        before = text[: m.start()]
        if neg.search(before) or (not before.strip(" #/!*") and prev[0] == f and neg.search(prev[1])):
            continue
        checked += 1
        name = m.group(1).split("/")[-1]
        if name not in feats:
            missing.append(f"{f}:{ln}: `{m.group(1)}` is called a feature, and no [features] table has `{name}`")
    prev = (f, text)
for x in missing:
    print(x, file=sys.stderr)
print(f"feature refs checked: {checked}, missing: {len(missing)} (bound 0)")
sys.exit(1 if missing or checked == 0 else 0)
PY
cat "$tmp/features.out"

n() { grep -c . "$1" || true; }
if [ "$status" -ne 0 ]; then
    echo "" >&2
    echo "doc-identifiers: FAILED -- $(n "$tmp/dead") dead, $nstale stale allowlist entries, $stripped stripped manifest comments; $(tail -1 "$tmp/features.out")" >&2
    exit 1
fi
echo "doc-identifiers: candidates $(n "$tmp/cands.all") (line $(n "$tmp/cands.line"), trailing $(n "$tmp/cands.trailing"), block $(n "$tmp/cands.block"), markdown $(n "$tmp/mdcands"), toml+ci.yml $(n "$tmp/tomlcands")), allowlist $(n "$tmp/allow"), stale 0, all live"
