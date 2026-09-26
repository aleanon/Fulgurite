#!/usr/bin/env bash
# The embedded firmwares' `.text` must stay under an eighth of flash —
# M13/T13.9's threshold, enforced 2026-09-10 — and, since M52/T52.1, for
# FOUR binaries, each asserting before its number is read that the code
# it exists to price is linked (scripts/lib/firmware.sh says why four).
# The interactive one keeps the same 64 KB: an application that handles
# input is the ordinary case, not a larger product
# (docs/plan-embedded-footprint.md §2.5).
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

rc=0
fail() { echo "text: $*" >&2; rc=1; }

# count PATTERN -- symbols of the current binary matching a literal.
count() { printf '%s\n' "$NM" | grep -cF -- "$1" || true; }

# **Preconditions, M52/T52.1**: a binary exists to price some code, so that
# code must be linked before its number is read -- the Toggle check's rule
# (below), applied to input, binding and the cascade. Checked by symbol,
# because an optimisation that removed the dispatch a binary was built to
# price would leave a smaller, greener number and nothing else.
PRE_firmware=''
PRE_firmware_interactive='pass::dispatch on_event cascade_range BindSite'
PRE_firmware_bind='BindSite'
PRE_firmware_cascade='cascade_range'

# **What a binary must NOT link**, bound 0 (M52/T52.1). Each is a finding
# of docs/plan-embedded-footprint.md, committed red with an XFAIL naming
# the task that turns it green; an XFAIL whose count is already 0 fails,
# so the marker cannot outlive the fix.
#   bind-only hit-tests nothing, so a `Node>::hit` is the vtable
#   `bind::node_of`'s `&dyn Node` reaches (F-NEW-2);
#   cascade-only dispatches nothing, so an `on_event` is the vtable the
#   arena loop's `node_mut` reaches (R12-F4).
FORBID_firmware_bind='Node>::hit'
FORBID_firmware_cascade='on_event'
XFAIL_firmware_bind='F-NEW-2: bind::node_of walks through &dyn Node (until T52.5)'
XFAIL_firmware_cascade='R12-F4: the arena loop runs without alloc (until T52.4)'

for name in $FIRMWARE_BINS; do
BIN=$(firmware_path "$name")
if ! NM=$(nm -C "$BIN" 2>&1); then
    fail "nm failed on $BIN"; continue
fi

# The widgets a measurement is meant to judge must be IN the binary it
# measures (M25/T25.1). `Toggle` is generic: if the fixture's tree stops
# holding one, its code is never instantiated, `.text` cannot see it, and
# a threshold on a Toggle change passes whatever the change costs -- the
# setup-silently-did-not-happen shape AGENT.md records. Checked before
# the number is read, so the number cannot be quoted without it.
toggles=$(count 'toggle::Toggle')
if [ "${toggles:-0}" -eq 0 ]; then
  fail "$name: no Toggle code is linked into $BIN"
  echo "" >&2
  echo "fixtures/embedded's tree must hold a CheckBox, a Radio and a Switch," >&2
  echo "or a .text threshold on toggle work measures nothing (docs/plan-appearance.md T25.1)." >&2
  continue
fi

pre_var="PRE_$name"; pre_line=""; pre_ok=1
for pat in ${!pre_var}; do
    n=$(count "$pat")
    pre_line="$pre_line, $n $pat"
    if [ "$n" -eq 0 ]; then
        fail "$name: 0 $pat symbols linked -- this binary exists to price that code, so its .text measures nothing without it"
        pre_ok=0
    fi
done
[ "$pre_ok" -eq 1 ] || continue

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
tasks=$(printf '%s\n' "$NM" | grep -cE 'lumen_static::(inbox|spawn)::' || true)
if [ "${tasks:-0}" -ne 0 ]; then
  fail "$name: $tasks deferred-work symbols linked into $BIN, expected 0"
  echo "" >&2
  printf '%s\n' "$NM" | grep -E 'lumen_static::(inbox|spawn)::' | head -5 >&2
  echo "" >&2
  echo "M28's inbox and spawner are behind \`std\`; an embedded leaf profile" >&2
  echo "must link none of them (docs/plan-tasks.md T28.7)." >&2
  continue
fi

forbid_var="FORBID_$name"; xfail_var="XFAIL_$name"; forbid_line=""
for pat in ${!forbid_var:-}; do
    n=$(count "$pat")
    forbid_line="$forbid_line, $n $pat (bound 0)"
    xf=${!xfail_var:-}
    if [ "$n" -ne 0 ] && [ -n "$xf" ]; then
        echo "text: $name: expected red -- $n $pat symbols linked, bound 0 ($xf)"
    elif [ "$n" -ne 0 ]; then
        fail "$name: $n $pat symbols linked, bound 0:"
        printf '%s\n' "$NM" | grep -F -- "$pat" | head -5 | sed 's/^/    /' >&2
    elif [ -n "$xf" ]; then
        fail "$name: expected red ($xf), but 0 $pat symbols are linked -- remove the XFAIL"
    fi
done

text=$(size -A "$BIN" | awk '$1 == ".text" {print $2}')
[ -n "$text" ] || { fail "$name: could not read .text from $BIN"; continue; }

pct=$(awk -v t="$text" -v c="$CEILING" 'BEGIN { printf "%.1f", 100 * t / c }')

if [ "$text" -gt "$CEILING" ]; then
  fail "$name: .text is $text B, over the $CEILING B ceiling by $((text - CEILING)) B"
  echo "" >&2
  echo "An eighth of the device's flash is the budget docs/results-embedded.md" >&2
  echo "set before measuring. Shrink it, or re-budget there with the argument." >&2
  continue
fi

echo "text: $name: .text $text B of $CEILING ($pct% of budget), $toggles Toggle symbols linked, $tasks task symbols (bound 0)$pre_line$forbid_line"

# The headline: the first `| `.text` | ... | **N B** |` row of the doc,
# which is the headline table by construction (it precedes the history).
# It states the paint-only firmware; the others' rows are cited, not held
# to a tolerance, because the ceiling above is their gate.
[ "$name" = firmware ] || continue
head=$(python3 - "$DOC" <<'PY'
import re, sys
m = re.search(r"^\| `\.text` \| [^|]*\| \*\*([0-9,]+) B\*\*", open(sys.argv[1]).read(), re.M)
print(m.group(1).replace(",", "") if m else "")
PY
)
[ -n "$head" ] || { fail "no headline .text row in $DOC"; continue; }
delta=$(awk -v t="$text" -v h="$head" 'BEGIN { printf "%.2f", 100 * (t - h) / h }')
over=$(awk -v d="$delta" -v tol="$TOLERANCE_PCT" 'BEGIN { d = d < 0 ? -d : d; print (d > tol) ? 1 : 0 }')

echo "text: $text B measured, $head B in the results doc's headline (delta $delta%, tolerance $TOLERANCE_PCT%)"
if [ "$over" -eq 1 ]; then
  fail "the headline in $DOC is $delta% off the linked firmware -- re-measure"
  echo "and rewrite the headline (keeping the history below it, dated)." >&2
fi
done
exit "$rc"
