#!/usr/bin/env bash
# M47/T47.0 (R7-F11): a comment that says it keeps "the idle ... numbers in
# `docs/X.md`" true must cite a file that HAS idle numbers -- a heading
# with the word "Idle" in it. check_doc_paths.sh only checks that the file
# exists, and four comments cited `docs/results-frame-cost.md` for idle
# figures it did not contain.
#
# A comment block (consecutive `//`, `///` or `//!` lines) is one unit, so
# a citation wrapped across lines is still seen. It matches "idle" followed
# within the same sentence by "numbers" and a backticked docs path, or the
# path followed by "'s idle ... numbers". A comment citing the file without
# the word "idle" is not this rule's business.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
python3 - <<'PY'
import pathlib, re, sys

cite = re.compile(
    r"idle[^.;]*?numbers[^.;]*?`(docs/[\w./-]+\.md)`"
    r"|`(docs/[\w./-]+\.md)`'s idle[^.;]*?numbers",
    re.I | re.S)
heading = re.compile(r"^#+ .*\bidle\b", re.I | re.M)

sites, bad = [], []
for path in sorted(pathlib.Path(".").glob("crates/*/src/**/*.rs")) + sorted(pathlib.Path(".").glob("examples/*/src/**/*.rs")):
    lines = path.read_text(errors="replace").splitlines()
    block, start = [], 0
    for i, line in enumerate(lines + [""]):
        s = line.strip()
        if s.startswith("//"):
            if not block:
                start = i + 1
            block.append(s.lstrip("/!").strip())
            continue
        if block:
            text = " ".join(block)
            for m in cite.finditer(text):
                doc = m.group(1) or m.group(2)
                sites.append((f"{path}:{start}", doc))
            block = []

for site, doc in sites:
    p = pathlib.Path(doc)
    ok = p.exists() and heading.search(p.read_text(errors="replace"))
    print(f"  {'ok  ' if ok else 'MISS'} {site} -> {doc}")
    if not ok:
        bad.append(site)

print(f"idle-citations: {len(sites)} sites, {len(bad)} citing a file with no Idle heading")
if bad:
    print("Add the idle figures under an 'Idle' heading in the cited doc, or cite the doc that has them.",
          file=sys.stderr)
    sys.exit(1)
PY
