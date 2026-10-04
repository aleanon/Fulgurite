#!/usr/bin/env bash
# The embedded firmwares' `.text` must stay under 80 KB, 15.6% of the
# T13.0 part's flash -- M13/T13.9's threshold (an eighth, 64 KB, enforced
# 2026-09-10), raised by the owner on 2026-09-30 (D5,
# docs/plan-iced-roadmap.md) -- and, since M52/T52.1, for FOUR binaries,
# each asserting before its number is read that the code it exists to
# price is linked (scripts/lib/firmware.sh says why four). All four share
# the one ceiling: an application that handles input is the ordinary
# case, not a larger product (docs/plan-embedded-footprint.md §2.5). And two symbol classes are
# forbidden outright, bound 0, per binary -- float formatting and soft
# `f64` (M52/T52.2) -- because the ceiling absorbed both regressions.
#
# **The number was written down on 2026-09-05 and nothing checked it.**
# `docs/results-embedded.md` stated `<= 64 KB` as a gate and recorded
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
# 80 KB: 15.6% of the T13.0 device's 512 KB flash, under the budget
# row's own "more than a fifth of the part" line (owner, D5, 2026-09-30).
# Raising it is a re-budget and belongs in docs/results-embedded.md with
# the argument -- so the unset default is checked against that row below,
# and the script and the doc cannot become two copies of one number.
CEILING_DEFAULT=81920
CEILING=${LUMEN_TEXT_CEILING:-$CEILING_DEFAULT}

# shellcheck source=lib/firmware.sh
source scripts/lib/firmware.sh
firmware_build || exit 1

rc=0
fail() { echo "text: $*" >&2; rc=1; }

# **The default agrees with the budget row** (M80/T80.0). Read the
# `Framework .text` threshold from the doc's budget table and compare it
# with CEILING_DEFAULT, by name, with both numbers printed. An override
# (`LUMEN_TEXT_CEILING`) changes the comparison below, not the budget, so
# it skips this check -- which is about the default.
row=$(python3 - "$DOC" <<'PY'
import re, sys
m = re.search(r"^\| Framework `\.text` \| \*\*≤ ([0-9]+) KB\*\*", open(sys.argv[1]).read(), re.M)
print(int(m.group(1)) * 1024 if m else "")
PY
)
if [ -z "${LUMEN_TEXT_CEILING:-}" ]; then
    if [ -z "$row" ]; then
        fail "ceiling: no 'Framework .text' budget row in $DOC"
    elif [ "$row" -ne "$CEILING_DEFAULT" ]; then
        fail "ceiling: the default $CEILING_DEFAULT B disagrees with $DOC's budget row ($row B) -- re-budget both together"
    else
        echo "text: ceiling $CEILING_DEFAULT B, the same as $DOC's budget row ($row B)"
    fi
fi

# count PATTERN -- symbols of the current binary matching a literal.
count() { printf '%s\n' "$NM" | grep -cF -- "$1" || true; }

# **Preconditions, M52/T52.1**: a binary exists to price some code, so that
# code must be linked before its number is read -- the Toggle check's rule
# (below), applied to input, binding and the cascade. Checked by symbol,
# because an optimisation that removed the dispatch a binary was built to
# price would leave a smaller, greener number and nothing else.
PRE_firmware=''
PRE_firmware_interactive='pass::dispatch on_event cascade_range update_marked'
PRE_firmware_bind='update_marked'
PRE_firmware_cascade='cascade_range'

# **What a binary must NOT link**, bound 0 (M52/T52.1). Each is a finding
# of docs/plan-embedded-footprint.md, committed red with an XFAIL naming
# the task that turns it green; an XFAIL whose count is already 0 fails,
# so the marker cannot outlive the fix.
#   bind-only hit-tests and dispatches nothing, so a `Node>::hit` or an
#   `on_event` is the vtable `bind::node_of`'s `&dyn Node` reached
#   (F-NEW-2; green since T52.5, which made the walk static);
#   cascade-only dispatches nothing, so an `on_event` or a `hit` is the
#   vtable the arena loop's `node_mut` reaches (R12-F4; green since T52.4,
#   which gates that loop on `alloc`).
# And the cascade-only binary's `.text` must sit within CASCADE_DELTA of
# the paint-only one: a three-rule sheet costs the cascade, not a vtable.
CASCADE_DELTA=3072
# The binding's own code in the bind-only binary -- `lumen_static::bind::`,
# `update_marked` and `TextTarget` symbols, summed -- must stay within
# BIND_OWN. NOT the whole-binary delta against the paint-only firmware,
# which the plan first named: that binary holds a different tree (one
# more label) and runs a second frame, and 2.2 KB of its 3.5 KB delta is
# that (docs/plan-embedded-footprint.md §6, T52.5). The delta is printed
# beside it as a record.
BIND_OWN=1024
FORBID_firmware_bind='Node>::hit on_event'
FORBID_firmware_cascade='on_event Node>::hit'

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

# **Two forbidden symbol classes, counted -- M52/T52.2 (R11-F1, R11-F2).**
# Properties, gated by the quantity they name, not by the ceiling
# (docs/plan-embedded-footprint.md §0 rule 2), which absorbed both
# regressions with room to spare:
#   float formatting -- one `f32::clamp` once linked 26.5 KB of
#     `core::num::flt2dec` through its panic message, in a GUI that
#     formats no floats (docs/results-embedded.md);
#   soft `f64` -- the FPU is single-precision, so any `f64` becomes
#     `compiler_builtins` calls: the `__aeabi_d*` ABI names, the
#     `__*df*` family (`__adddf3`, `__floatsidf`, `__truncdfsf2`,
#     `__extendsfdf2`, ...) and the `__aeabi_*2d` conversions.
# Kept separate so a failure says which property broke; both bound 0 in
# every binary. Matched on the demangled name anywhere, so the
# `compiler_builtins::float::add::__adddf3` form counts as well as the
# bare ABI alias.
FLOAT_FMT_RE='core::num::flt2dec|core::fmt::float'
SOFT_F64_RE='(^|[^A-Za-z0-9_])__(aeabi_d[a-z0-9]+|aeabi_[a-z0-9]*2d|[a-z]*df[a-z0-9]*)([^A-Za-z0-9_]|$)'
float_fmt=$(printf '%s\n' "$NM" | grep -cE -- "$FLOAT_FMT_RE" || true)
soft_f64=$(printf '%s\n' "$NM" | grep -cE -- "$SOFT_F64_RE" || true)
if [ "$float_fmt" -ne 0 ]; then
    fail "$name: float-fmt symbols $float_fmt (bound 0) -- float formatting is linked:"
    printf '%s\n' "$NM" | grep -E -- "$FLOAT_FMT_RE" | head -5 | sed 's/^/    /' >&2
fi
if [ "$soft_f64" -ne 0 ]; then
    fail "$name: soft-f64 symbols $soft_f64 (bound 0) -- double-precision arithmetic is linked:"
    printf '%s\n' "$NM" | grep -E -- "$SOFT_F64_RE" | head -5 | sed 's/^/    /' >&2
fi
# No `continue` here: the ceiling line below still prints, so a red
# count beside a green `.text` shows the count is not riding on it.

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
  echo "80 KB (15.6% of the device's flash, owner D5) is the budget" >&2
  echo "docs/results-embedded.md sets. Shrink it, or re-budget there with the argument." >&2
  continue
fi

[ "$name" = firmware ] && paint_text=$text
if [ "$name" = firmware_bind ]; then
    own=$(nm -C -S --radix=d "$BIN" | awk '$3 ~ /^[TtWw]$/ && /lumen_static::bind::|update_marked|TextTarget/ {s += $2} END {print s + 0}')
    echo "text: bind firmware: binding's own symbols $own B (bound $BIND_OWN); .text $text (paint-only ${paint_text:-?}, delta $((text - ${paint_text:-0})), a different tree and two frames -- record), Node>::hit symbols $(count 'Node>::hit') (bound 0)"
    [ "$own" -gt 0 ] || fail "bind firmware: no binding symbol found -- the pattern or the binary is wrong, so the bound proves nothing"
    [ "$own" -le "$BIND_OWN" ] || fail "bind firmware: the binding's own symbols are $own B, over $BIND_OWN"
fi
if [ "$name" = firmware_cascade ]; then
    d=$((text - ${paint_text:-0}))
    echo "text: cascade firmware: .text $text (paint-only ${paint_text:-?}, delta $d; bound $CASCADE_DELTA), on_event symbols $(count on_event) (bound 0)"
    [ -n "${paint_text:-}" ] && [ "$d" -le "$CASCADE_DELTA" ] || fail "cascade firmware: .text $text is $d B over the paint-only ${paint_text:-?}, bound $CASCADE_DELTA -- a stylesheet costs more than the cascade (R12-F4)"
fi
echo "text: $name: .text $text B of $CEILING ($pct% of budget), $toggles Toggle symbols linked, $tasks task symbols (bound 0), float-fmt symbols $float_fmt (bound 0), soft-f64 symbols $soft_f64 (bound 0)$pre_line$forbid_line"

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

# **A record, not a gate** (M52/T52.4, docs/plan-embedded-footprint.md §2.4):
# the cascade-only firmware at the M13 device's `alloc` tier, where arenas
# exist and the arena loop still asks every node through the vtable. Its
# own target directory, so the leaf builds above are not relinked.
if json=$(CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}/firmware-alloc" cargo build --release -q -p embedded \
        --bin firmware --bin firmware_cascade --features firmware,alloc-tier \
        --target "$FIRMWARE_TARGET" --message-format=json); then
    at=$(printf '%s\n' "$json" | python3 -c '
import json, sys
exe = {}
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") == "compiler-artifact" and m.get("executable"):
        exe[m["target"]["name"]] = m["executable"]
print(exe.get("firmware", ""), exe.get("firmware_cascade", ""))
')
    read -r ap ac <<<"$at"
    tp=$(size -A "$ap" | awk '$1 == ".text" {print $2}')
    tc=$(size -A "$ac" | awk '$1 == ".text" {print $2}')
    ev=$(nm -C "$ac" | grep -cF on_event || true)
    echo "text: record (alloc tier, not gated): cascade firmware .text $tc (paint-only $tp, delta $((tc - tp))), on_event symbols $ev -- the arena loop's vtable, until a has-arenas constant (M26 finding 9)"
else
    fail "the alloc-tier record build failed"
fi
exit "$rc"
