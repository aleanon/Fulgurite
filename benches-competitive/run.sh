#!/usr/bin/env bash
# Run one arm R times and keep the minimum per row.
#
# The arm already takes min-of-200 internally; this guards against a cold
# run, which is not hypothetical — the first `observe` measurement here read
# 46% slow and was pure noise. Re-running is what caught it, and is why no
# number in docs/results-competitive.md comes from a single run.
set -uo pipefail
export LC_ALL=C
R=${R:-5}
for _ in $(seq "$R"); do
  cargo run --locked --release -q --manifest-path "$(dirname "$0")/Cargo.toml" "$@" 2>/dev/null
done | python3 -c '
import re, sys
best = {}
order = []
for line in sys.stdin:
    m = re.match(r"^(\S+/.*?)\s{2,}([\d.]+) us", line)
    if not m: continue
    label, v = m.group(1).strip(), float(m.group(2))
    if label not in best:
        order.append(label)
    best[label] = min(best.get(label, 1e18), v)
for l in order:
    print(f"{l:<32}{best[l]:9.1f} us")
'
