#!/usr/bin/env bash
# M42/T42.5 (R18-F2, docs/plan-drive-identity.md §2.4): every widget type
# `fulgurite-widgets` exports has a drive inventory, or a line in
# scripts/drive/undriven.allow with an owner and a reason.
#
# Until M42 nothing tied the crate's exports to the drive: the review
# found eighteen exported types with no inventory, Select, TextArea and
# ScrollView among them, and `just widget-drive` green. A new widget is
# now either driven or owed, visibly, in the diff.
#
# No compile, no display: it reads `pub use` from the crate root and the
# inventories scripts/drive/run.py discovers under scripts/drive/widgets/. Prints
#   widgets exported N, driven D, allowed A, missing 0
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
python3 - <<'PY'
import re, sys
from pathlib import Path

src = Path("crates/fulgurite-widgets/src/lib.rs").read_text()
src = src[: src.index("pub mod prelude")]
exported = []
for m in re.finditer(r"^pub use [a-z_:]+::(\{[^}]*\}|\w+);", src, re.M):
    for n in re.split(r"[,\s]+", m.group(1).strip("{}")):
        if re.match(r"^[A-Z][a-z]", n):
            exported.append(n)

# The inventories as run.py discovers them -- imported, not grepped,
# because some modules build their entries (nav.py, veil.py).
sys.path[:0] = ["scripts/drive", "scripts"]
from run import inventories  # noqa: E402
driven = {e[0] for e in inventories()}

allowed, aliases, bad = {}, {}, []
for i, ln in enumerate(Path("scripts/drive/undriven.allow").read_text().splitlines(), 1):
    ln = ln.strip()
    if not ln or ln.startswith("#"):
        continue
    if "=" in ln and "|" not in ln:
        name, entry = (x.strip() for x in ln.split("=", 1))
        aliases[name] = entry
        if entry not in driven:
            bad.append(f"undriven.allow:{i}: {name} = {entry}, but no inventory is named {entry}")
        continue
    parts = [x.strip() for x in ln.split("|")]
    if len(parts) != 3 or not all(parts):
        bad.append(f"undriven.allow:{i}: want `Name | owner | reason`: {ln!r}")
        continue
    allowed[parts[0]] = parts[1]

for n in exported:
    if n in driven and n in allowed:
        bad.append(f"{n}: has an inventory AND an undriven.allow entry -- remove the entry")
missing = [n for n in exported if n not in driven and n not in allowed and n not in aliases]
stale = [n for n in list(allowed) + list(aliases) if n not in exported]
for n in missing:
    bad.append(f"{n}: exported by fulgurite-widgets with no inventory and no undriven "
               f"entry -- write scripts/drive/widgets/<x>.py, or add `{n} | owner | reason` "
               f"to scripts/drive/undriven.allow")
for n in stale:
    bad.append(f"undriven.allow names {n}, which fulgurite-widgets does not export")

print(f"drive-coverage: widgets exported {len(exported)}, driven "
      f"{sum(1 for n in exported if n in driven or n in aliases)}, allowed "
      f"{sum(1 for n in exported if n in allowed)}, missing {len(missing)}")
for b in bad:
    print(f"drive-coverage: {b}", file=sys.stderr)
sys.exit(1 if bad else 0)
PY
