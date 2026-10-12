#!/usr/bin/env bash
# M82/T82.3 (docs/plan-paint-vocabulary.md): a node's fill is painted by
# `ops::fill`, which every kind that draws a border must call before its
# flat background -- `Style.background_fill` is honoured nowhere the call
# is missing, and nothing fails to compile. So this pairs them, per call
# site and not per file: `Label` and `Button` share widgets/src/ops.rs,
# and a per-file rule would let `Label`'s call keep `Button` green.
#
# 1. Every `border(` call in production code (crates/*/src, tests
#    excluded), qualified (`ops::border(`, `::fulgurite::ops::border(`)
#    or bare through an import, must be preceded by a `fill(` call in the
#    same `fn`, unless listed in FILL_ALLOW with a reason. A failure names
#    the enclosing `impl ... for` type.
# 2. A file whose paint calls `fill(` must name `BACKGROUND_FILL` in a
#    seed mask, unless its kind is listed in SEED_ALLOW with a reason. The
#    per-kind version of this direction is measured, not grepped:
#    crates/fulgurite-widgets/tests/style_reads.rs renders every kind
#    with and without a fill and compares the result with its mask.
#
# Prints `sites N, filled N, allowed A`; N was 29 in 18 files at landing.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 - <<'PY'
import glob, re, sys

# file:Type  reason   (a border site that may go without a fill)
FILL_ALLOW = {
}
# Type  reason   (a kind that calls fill( and does not declare the bit)
SEED_ALLOW = {
    "Image": "its layout calls no record_painted, so NodeState::fill (like bg) is never stamped; "
             "style_reads.rs measures it reading neither",
}

call_border = re.compile(r'(?<![.\w])(?:(?:::)?[A-Za-z_]\w*::)*border\(')
# A paint call takes the state array first (`fill(st, i, out)`), which is
# what tells it from `fill(2)` in a `.lss` length or a doc string.
call_fill = re.compile(r'(?<![.\w])(?:(?:::)?[A-Za-z_]\w*::)*fill\(\s*[a-z_]\w*\s*,')
fn_start = re.compile(r'^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?fn\s+\w+')
impl_line = re.compile(r'^\s*impl\b(.*)')

def impl_type(header):
    m = re.search(r'\bfor\s+([A-Za-z_#][\w]*)', header)
    if m:
        return m.group(1)
    m = re.match(r'\s*(?:<[^>]*(?:<[^>]*>[^>]*)*>)?\s*([A-Za-z_]\w*)', header)
    return m.group(1) if m else "?"

sites = filled = allowed = 0
files = set()
bad = []
fill_types = {}  # file -> set of types calling fill(
for path in sorted(glob.glob("crates/*/src/**/*.rs", recursive=True)):
    lines = open(path, encoding="utf-8").read().split("\n")
    for n, line in enumerate(lines):
        code = line.split("//", 1)[0]
        is_border = bool(call_border.search(code)) and not re.search(r'\bfn\s+border\(', code)
        is_fill = bool(call_fill.search(code)) and not re.search(r'\bfn\s+fill\(', code)
        if not (is_border or is_fill):
            continue
        start = next((k for k in range(n, -1, -1) if fn_start.match(lines[k])), 0)
        imp = next((impl_type(impl_line.match(lines[k]).group(1)) for k in range(start, -1, -1) if impl_line.match(lines[k])), "?")
        if is_fill:
            fill_types.setdefault(path, set()).add(imp)
        if not is_border:
            continue
        sites += 1
        files.add(path)
        body = "\n".join(l.split("//", 1)[0] for l in lines[start:n])
        if call_fill.search(body):
            filled += 1
        elif f"{path}:{imp}" in FILL_ALLOW:
            allowed += 1
        else:
            bad.append(f"{path}:{n + 1}: `{imp}` draws a border with no `fill(` before it in the same fn")

for path, types in sorted(fill_types.items()):
    text = open(path, encoding="utf-8").read()
    if "BACKGROUND_FILL" in text:
        continue
    for t in sorted(types):
        if t not in SEED_ALLOW:
            bad.append(f"{path}: `{t}` paints a fill and no seed in its file declares Fields::BACKGROUND_FILL")

print(f"fill-paint: sites {sites} in {len(files)} files, filled {filled}, allowed {allowed}")
if bad:
    print("\n".join(bad))
    print("\nCall fulgurite::ops::fill(st, i, out) before the node's background, or list the")
    print("site in scripts/check_fill_paint.sh with the reason it has no fill.")
    sys.exit(1)
PY
