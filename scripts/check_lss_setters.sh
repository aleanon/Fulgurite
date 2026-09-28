#!/usr/bin/env bash
# M61/T61.2 (docs/plan-lss.md): **every `Patch` setter has a `.lss`
# declaration**, and every declaration names a real setter.
#
# The `.lss` property table (crates/lumen-static-lss/src/lower.rs,
# `PROPERTIES`) cannot be checked against the core by the compiler: the
# lss crate cannot depend on lumen-static (its manifest says why), so a
# setter added to `Patch` later would simply have no `.lss` spelling, and
# nothing would say so. This reads both files and compares the sets, in
# both directions, and prints the counts beside the verdict.
#
# The same for the three enums the lowering mirrors -- `Align`, `Cursor`
# and `Role` -- whose variants a `.lss` keyword lowers to by name.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

python3 - crates/lumen-static/src/style.rs crates/lumen-static-lss/src/lower.rs <<'PY'
import re, sys
style, lower = (open(p).read() for p in sys.argv[1:3])

def block(src, start):
    """The text from `start` to the first line that is exactly `}`."""
    i = src.find(start)
    if i < 0:
        sys.exit(f"lss-setters: `{start}` not found -- the check cannot see what it compares")
    j = src.find("\n}", i)
    return src[i:j]

rc = 0
setters = set(re.findall(r"pub const fn (\w+)\(mut self", block(style, "impl Patch {")))
table = set(re.findall(r'\bsetter: "(\w+)"', lower)) | set(re.findall(r'role_setter: Some\("(\w+)"\)', lower))
missing, extra = sorted(setters - table), sorted(table - setters)
print(f"lss-setters: Patch has {len(setters)} setters, the .lss table names {len(table)}; "
      f"without a declaration: {missing} (bound []), naming no setter: {extra} (bound [])")
if not setters:
    print("lss-setters: found no Patch setter -- the pattern is wrong, so the check proves nothing", file=sys.stderr)
    rc = 1
if missing:
    print(f"lss-setters: Patch setters with no .lss declaration: {missing} -- add them to PROPERTIES "
          "in crates/lumen-static-lss/src/lower.rs", file=sys.stderr)
    rc = 1
if extra:
    print(f"lss-setters: .lss declarations naming no Patch setter: {extra}", file=sys.stderr)
    rc = 1

for enum in ("Align", "Cursor", "Role"):
    core = re.findall(r"^\s+([A-Z]\w*)\s*[,(]", block(style, f"pub enum {enum} {{"), re.M)
    m = re.search(r"\b" + enum + r"\s*\{([^}]*)\}", lower)
    mirror = re.findall(r"(\w+)\s*=\s*\"", m.group(1)) if m else []
    same = core == mirror
    print(f"lss-setters: {enum}: core {len(core)} variants, .lss mirror {len(mirror)}, "
          f"{'the same, in order' if same else 'DIFFERENT'}")
    if not core or not same:
        print(f"lss-setters: {enum} -- core {core}, mirror {mirror}", file=sys.stderr)
        rc = 1
sys.exit(rc)
PY
