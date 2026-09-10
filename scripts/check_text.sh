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
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

TARGET=thumbv7em-none-eabihf
BIN=target/$TARGET/release/firmware
# An eighth of the T13.0 device's 512 KB flash. Raising this is a
# re-budget and belongs in docs/results-embedded.md with the argument,
# not in this line.
CEILING=${LUMEN_TEXT_CEILING:-65536}

cargo build --release -q -p embedded --bin firmware --features firmware --target $TARGET || exit 1
[ -f "$BIN" ] || { echo "text: $BIN not built" >&2; exit 1; }

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

echo "text: .text $text B of $CEILING ($pct% of budget)"
