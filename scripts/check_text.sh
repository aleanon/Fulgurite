#!/usr/bin/env bash
# The embedded firmware's `.text` must stay under an eighth of flash —
# M13/T13.9's threshold, enforced 2026-09-10.
#
# **The number was written down on 2026-09-05 and nothing checked it.**
# `docs/results-embedded.md` states `<= 64 KB` as a gate and records
# 20,524 B against it; by 2026-09-10 the linked firmware was 22,458 B and
# the doc still said 20,524. A 9.4% drift, invisible, because a threshold
# in prose is not a threshold that was checked -- the same lesson
# `tests/banded.rs` taught the same week, and `AGENT.md` now records
# both.
#
# So this asserts the ceiling AND prints the measurement beside it, per
# `AGENT.md`: a gate that prints only "pass" cannot show a reader that
# the margin is closing.
#
# M43/T43.4, two more things:
#   * the binary is the one cargo reports having built
#     (scripts/lib/firmware.sh), so CARGO_TARGET_DIR cannot leave this
#     judging a stale ./target (R11-F6);
#   * the measurement is compared with the HEADLINE of
#     docs/results-embedded.md, and a delta above 1% fails (R17-F13).
#     Numbers are stated once, in the results doc -- and a headline a
#     milestone behind the firmware is how this doc was caught stale three
#     times. 1% lets a toolchain bump through and stops a milestone.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

DOC=docs/results-embedded.md
TOLERANCE_PCT=1
# An eighth of the T13.0 device's 512 KB flash. Raising this is a
# re-budget and belongs in docs/results-embedded.md with the argument,
# not in this line.
CEILING=${LUMEN_TEXT_CEILING:-65536}

# shellcheck source=lib/firmware.sh
source scripts/lib/firmware.sh
firmware_build || exit 1
BIN=$FIRMWARE

# The widgets a measurement is meant to judge must be IN the binary it
# measures (M25/T25.1). `Toggle` is generic: if the fixture's tree stops
# holding one, its code is never instantiated, `.text` cannot see it, and
# a threshold on a Toggle change passes whatever the change costs -- the
# setup-silently-did-not-happen shape AGENT.md records. Checked before
# the number is read, so the number cannot be quoted without it.
toggles=$(nm -C "$BIN" 2>/dev/null | grep -c 'toggle::Toggle' || true)
if [ "${toggles:-0}" -eq 0 ]; then
  echo "text: no Toggle code is linked into $BIN" >&2
  echo "" >&2
  echo "fixtures/embedded's tree must hold a CheckBox, a Radio and a Switch," >&2
  echo "or a .text threshold on toggle work measures nothing (docs/plan-appearance.md T25.1)." >&2
  exit 1
fi

# M28/T28.7: **no deferred-work code reaches a leaf profile.**
#
# `.text` is deliberately NOT the measure here. `inbox` and `spawn` are
# behind the `std` feature and fixtures/embedded does not enable it, so
# that number cannot move whatever M28 does -- a bound computed from the
# wrong quantity, which is the shape AGENT.md records twice. The
# falsifiable form of the same claim is the symbol count, and zero is a
# bound an addition can actually cross.
#
# Printed beside its bound like the Toggle count above, so a run that
# reports it is showing the quantity rather than asserting a pass.
tasks=$(nm -C "$BIN" 2>/dev/null | grep -cE 'lumen_static::(inbox|spawn)::' || true)
if [ "${tasks:-0}" -ne 0 ]; then
  echo "text: $tasks deferred-work symbols linked into $BIN, expected 0" >&2
  echo "" >&2
  nm -C "$BIN" 2>/dev/null | grep -E 'lumen_static::(inbox|spawn)::' | head -5 >&2
  echo "" >&2
  echo "M28's inbox and spawner are behind \`std\`; an embedded leaf profile" >&2
  echo "must link none of them (docs/plan-tasks.md T28.7)." >&2
  exit 1
fi

text=$(size -A "$BIN" | awk '$1 == ".text" {print $2}')
[ -n "$text" ] || { echo "text: could not read .text from $BIN" >&2; exit 1; }

pct=$(awk -v t="$text" -v c="$CEILING" 'BEGIN { printf "%.1f", 100 * t / c }')

if [ "$text" -gt "$CEILING" ]; then
  echo "text: .text is $text B, over the $CEILING B ceiling by $((text - CEILING)) B" >&2
  echo "" >&2
  echo "An eighth of the device's flash is the budget docs/results-embedded.md" >&2
  echo "set before measuring. Shrink it, or re-budget there with the argument." >&2
  exit 1
fi

# The headline: the first `| `.text` | ... | **N B** |` row of the doc,
# which is the headline table by construction (it precedes the history).
head=$(python3 - "$DOC" <<'PY'
import re, sys
m = re.search(r"^\| `\.text` \| [^|]*\| \*\*([0-9,]+) B\*\*", open(sys.argv[1]).read(), re.M)
print(m.group(1).replace(",", "") if m else "")
PY
)
[ -n "$head" ] || { echo "text: no headline .text row in $DOC" >&2; exit 1; }
delta=$(awk -v t="$text" -v h="$head" 'BEGIN { printf "%.2f", 100 * (t - h) / h }')
over=$(awk -v d="$delta" -v tol="$TOLERANCE_PCT" 'BEGIN { d = d < 0 ? -d : d; print (d > tol) ? 1 : 0 }')

echo "text: .text $text B of $CEILING ($pct% of budget) in $BIN, $toggles Toggle symbols linked, $tasks task symbols (bound 0)"
echo "text: $text B measured, $head B in the results doc's headline (delta $delta%, tolerance $TOLERANCE_PCT%)"
if [ "$over" -eq 1 ]; then
  echo "text: the headline in $DOC is $delta% off the linked firmware -- re-measure" >&2
  echo "and rewrite the headline (keeping the history below it, dated)." >&2
  exit 1
fi
