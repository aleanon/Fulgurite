#!/usr/bin/env bash
# No bare count in the justfile or ci.yml (M43/T43.14, R17-F11, R17-F14).
#
# Both files stated dependency and binary counts in their comments -- "wgpu
# is 37 crates", "95 -> 157 crates", "the dependency tree is six crates"
# while listing seven, "only `crossover` and `bindings`" against eight
# declared `[[bin]]`s -- and nothing checked any of them. Numbers are stated
# once, in a `docs/results-*.md` that says how it counted; everywhere else
# cites it. So a count of crates, packages, dependencies or `[[bin]]`s in
# either file is refused outright, whatever it cites: the citation is the
# claim.
#
# Comment lines are joined into blocks first, so a count broken across two
# lines ("37\n# crates") is still one claim.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

python3 - justfile .github/workflows/ci.yml <<'PY'
import re, sys

NUM = r"(?:\d+|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|a dozen)"
CLAIM = re.compile(
    rf"\b{NUM}(?:\s*(?:->|→)\s*{NUM})?\s+(?:unique\s+|more\s+|extra\s+)?"
    r"(?:crates?|packages|dependencies|deps|`?\[\[bin\]\]`?s?)\b",
    re.I,
)
bad = 0
for path in sys.argv[1:]:
    lines = open(path).read().split("\n")
    # Join each run of comment lines (and each other line alone) into one
    # string, remembering where every character came from.
    i = 0
    while i < len(lines):
        j = i
        if lines[i].lstrip().startswith("#"):
            while j + 1 < len(lines) and lines[j + 1].lstrip().startswith("#"):
                j += 1
        text, starts = "", []
        for k in range(i, j + 1):
            starts.append((len(text), k + 1))
            text += re.sub(r"^\s*#\s?", "", lines[k]) + " "
        for m in CLAIM.finditer(text):
            ln = max(l for off, l in starts if off <= m.start())
            print(f"{path}:{ln}: bare count \"{m.group(0)}\" -- cite the results doc that owns it", file=sys.stderr)
            bad += 1
        i = j + 1
if bad:
    sys.exit(1)
print(f"ci claims: 0 bare counts in {len(sys.argv) - 1} files")
PY
