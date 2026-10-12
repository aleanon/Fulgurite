#!/usr/bin/env bash
# M61/T61.5, T61.0's item 5 (docs/plan-lss.md): **every `.lss` in the
# repository is read both ways by some test** -- through `include_lss!`
# and through `lss::load` -- so the two lowerings are held to each other
# on every sheet that exists, not on the ones someone remembered.
#
# A sheet counts as covered when one `.rs` file names it on an
# `include_lss!` line, calls `load(`, and names it again on a line that is
# not the `include_lss!` -- the path the load is given. The refusal cases
# under crates/fulgurite-macros/tests/ui-lss/ (`fail_*`, `tree_fail_*`)
# are not sheets the macro accepts; `tests/lss_parity.rs` compares load's
# refusals with their pinned .stderr instead, and this counts them apart.
#
# No compile: it reads files. Prints every sheet's verdict beside the
# count, so a new file with no test is named.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

python3 - <<'PY'
import os, re, subprocess, sys
files = subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard"],
                       capture_output=True, text=True, check=True).stdout.split("\n")
sheets = [f for f in files if f.endswith(".lss")]
refusals = [f for f in sheets if re.search(r"tests/ui-lss/(fail|tree_fail)_[^/]*\.lss$", f)]
accepted = [f for f in sheets if f not in refusals]
rs = [f for f in files if f.endswith(".rs") and os.path.exists(f)]

covered = {}
for f in rs:
    text = open(f, encoding="utf-8", errors="replace").read()
    if "load(" not in text:
        continue
    inc, other = set(), set()
    for line in text.split("\n"):
        for n in re.findall(r'([\w.-]+\.lss)"', line):
            (inc if "include_lss!" in line else other).add(n)
    for n in inc & other:
        covered.setdefault(n, set()).add(f)

rc, both_ways = 0, 0
for s in sorted(accepted):
    where = covered.get(os.path.basename(s), set())
    if where:
        both_ways += 1
        print(f"lss-coverage: {s}: both ways in {', '.join(sorted(where))}")
    else:
        print(f"lss-coverage: {s}: NOT compared -- no .rs file names it in an include_lss! and again for a load(",
              file=sys.stderr)
        rc = 1
print(f"lss-coverage: {len(accepted)} sheets, {both_ways} compared both ways (bound: all {len(accepted)}); "
      f"{len(refusals)} refusal cases, compared with their .stderr by crates/fulgurite-shell/tests/lss_parity.rs")
if not accepted:
    print("lss-coverage: found no .lss sheet -- the listing is wrong, so the check proves nothing", file=sys.stderr)
    rc = 1
sys.exit(rc)
PY
