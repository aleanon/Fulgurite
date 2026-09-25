# R17-F13: docs/results-embedded.md's headline was a milestone behind the
# firmware and nothing compared them. Moving the headline .text by ~5%
# must be named (T43.4, tolerance 1%).
CHECK='./scripts/check_text.sh'
EXPECT='headline.*delta'
plant() {
    python3 - <<'PY'
import re, sys
p = "docs/results-embedded.md"
s = open(p).read()
m = re.search(r"^\| `\.text` \| [^|]*\| \*\*([0-9,]+) B\*\*", s, re.M)
if not m:
    sys.exit("no headline .text row")
v = int(m.group(1).replace(",", ""))
w = f"{int(v * 1.05):,}"
s = s[:m.start(1)] + w + s[m.end(1):]
open(p, "w").write(s)
print(f"headline {v} -> {w}")
PY
}
