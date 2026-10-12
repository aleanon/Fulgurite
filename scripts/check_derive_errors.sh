#!/usr/bin/env bash
# Every error the derives can emit is pinned by a `.stderr` (M55/T55.2,
# docs/plan-derive-dx.md §0 rule 2).
#
# WHY. R16-F4 found five error sentences with no `.stderr` anywhere, and
# the review disabled two of those checks at once -- the second `#[tree]`
# field and the zero-column grid -- with the `ui` suite staying green.
# A refusal nobody has watched fire is a refusal nobody knows works. So
# the untested path is a number printed on every run, not a finding the
# next review has to make.
#
# WHAT. Every `syn::Error::new_spanned(_, "..")`, `syn::Error::new(_, "..")`,
# `meta.error("..")` and `input.error("..")` in the derive crate's source
# contributes its sentence. A `format!("..")` sentence has each `{..}`
# replaced by a wildcard. Each sentence must appear in some
# `crates/fulgurite-macros/tests/ui*/*.stderr`, which trybuild compares
# against the compiler's real output -- so a pinned sentence is one a
# test has seen the derive emit.
#
# It reads the sentence, not the line: two sites that say the same thing
# are pinned by one case, which is right, since they are one message to
# the author.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

python3 - "$@" <<'PY'
import glob, re, sys

SRC = "crates/fulgurite-macros/src/lib.rs"
sys.path.insert(0, "scripts/lib")
import rustsrc  # noqa: E402

src = open(SRC, encoding="utf-8").read()
# Comments out, literals kept: a sentence quoted in a doc comment is not
# an error site.
code, _ = rustsrc.split(src, keep_literals=True)

site = re.compile(r'(?:syn::Error::new_spanned|syn::Error::new|\bmeta\.error|\binput\.error)\s*\(')
lit = re.compile(r'"((?:[^"\\]|\\.)*)"', re.S)

def unescape(s):
    # Rust's `\` + newline + leading whitespace is a continuation.
    s = re.sub(r'\\\n\s*', '', s)
    return s.replace('\\"', '"').replace("\\'", "'").replace('\\\\', '\\')

sentences = []  # (line, sentence, is_format)
for m in site.finditer(code):
    # The call's argument list: up to the matching close paren.
    depth, i = 1, m.end()
    while i < len(code) and depth:
        c = code[i]
        if c == '"':
            j = lit.match(code, i)
            i = j.end()
            continue
        depth += {'(': 1, ')': -1}.get(c, 0)
        i += 1
    args = code[m.end():i - 1]
    l = lit.search(args)
    if not l:
        continue
    line = code.count("\n", 0, m.start()) + 1
    fmt = "format!(" in args[:l.start()]
    sentences.append((line, unescape(l.group(1)), fmt))

stderr = ""
for p in sorted(glob.glob("crates/fulgurite-macros/tests/ui*/*.stderr")):
    stderr += open(p, encoding="utf-8").read() + "\n"

def pattern(sentence, fmt):
    # The whole sentence, as the compiler prints it: `error: <sentence>`
    # on a line of its own -- so a sentence that is a prefix of another
    # is not pinned by the longer one's case.
    if fmt:
        body = ".+?".join(re.escape(p) for p in re.split(r'\{[^{}]*\}', sentence))
    else:
        body = re.escape(sentence)
    return r"^error: " + body + r"$"

unpinned = []
seen = {}
for line, s, fmt in sentences:
    key = (s, fmt)
    if key not in seen:
        seen[key] = re.search(pattern(s, fmt), stderr, re.M) is not None
    if not seen[key]:
        unpinned.append((line, s))

n, m = len(sentences), len(sentences) - len(unpinned)
print(f"derive-errors: syn::Error sites: {n}, pinned by a .stderr: {m} (bound m == n)")
if unpinned:
    for line, s in unpinned:
        print(f"    {SRC}:{line}: no .stderr pins: {s!r}", file=sys.stderr)
    print("Add a tests/ui/fail_*.rs that triggers each, and commit its .stderr.", file=sys.stderr)
    sys.exit(1)
PY
