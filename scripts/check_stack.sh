#!/usr/bin/env bash
# `just stack`: the embedded stack, MEASURED -- M52/T52.6
# (docs/plan-embedded-footprint.md §2.6, R11-F3).
#
# docs/results-embedded.md reported `stack_bytes(DEPTH)` -- a model -- as
# "Measured", and the fixture asserts that model at compile time. A 4 KB
# local in a widget's `layout` leaves the assert green, because DEPTH did
# not change. This runs fixtures/embedded/src/bin/fwq.rs under qemu
# (mps2-an386, a Cortex-M4F): it paints the stack, runs a cold and a warm
# frame, a typed character and a bound update, scans what each used, and
# prints one line per call over semihosting. Here:
#
#   1. the CALIBRATION first: a known 2,000 B local must measure
#      2,000-2,100 B, so a broken painting method fails loudly instead of
#      reading 0 (or the whole region);
#   2. `landed:` -- qemu exits 1 unless the typed character reached the
#      field and the binding reported where it wrote, so the figures are
#      not the price of no-ops;
#   3. every `stack:` line <= 4,096 B, in RELEASE -- the budget is a
#      release budget; the dev profile's figures are printed as a record;
#   4. the model `stack_bytes(depth)` bounds every release FRAME it is
#      printed beside (the four-depth sweep and the device's frames), so
#      the constant cannot drift into a claim again without this saying so.
#
# Needs qemu-system-arm: without it this exits 77 (scripts/probe.sh), which
# M43's skip policy counts as not run, never as a pass.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

BUDGET=4096
CAL_LO=2000
CAL_HI=2100

./scripts/probe.sh thumb qemu || exit $?
# shellcheck source=lib/firmware.sh
source scripts/lib/firmware.sh

run() {  # run BIN -> qemu's output; exit status is qemu's
    timeout 60 qemu-system-arm -M mps2-an386 -nographic -monitor none \
        -semihosting-config enable=on,target=native -kernel "$1" </dev/null 2>&1
}

rel=$(firmware_build_one fwq --release) || exit 1
out=$(run "$rel"); qrc=$?
printf '%s\n' "$out" | sed 's/^/  release| /'

rc=0
fail() { echo "stack: $*" >&2; rc=1; }

cal=$(printf '%s\n' "$out" | sed -nE 's/^calibrate: ([0-9]+) B.*/\1/p')
if [ -z "$cal" ]; then
    fail "no calibration line from $rel (qemu exit $qrc)"
    exit 1
fi
if [ "$cal" -lt "$CAL_LO" ] || [ "$cal" -gt "$CAL_HI" ]; then
    fail "calibration measured $cal B for a known 2000 B local, outside $CAL_LO-$CAL_HI -- the painting method is broken, so no figure below means anything"
    exit 1
fi
echo "stack: calibration $cal B for a 2000 B local (bound $CAL_LO-$CAL_HI)"

if [ "$qrc" -ne 0 ]; then
    fail "fwq exited $qrc: the calls did not land (see 'landed:'), or it faulted or panicked"
fi

n=0
while IFS='|' read -r call used model; do
    n=$((n + 1))
    echo "stack: $call $used B / $BUDGET (model $model)"
    [ "$used" -le "$BUDGET" ] || fail "$call used $used B, over the $BUDGET B release budget by $((used - BUDGET)) B"
    case $call in
        *frame) [ "$used" -le "$model" ] || fail "$call used $used B, above the model stack_bytes = $model B -- a frame outgrew the model; find what grew before refitting desc::STACK_BASE_BYTES (docs/results-embedded.md)" ;;
    esac
done < <(printf '%s\n' "$out" | sed -nE 's/^stack: (.+) ([0-9]+) B \/ [0-9]+ \(model ([0-9]+)\)$/\1|\2|\3/p')
[ "$n" -ge 4 ] || fail "read $n stack lines from fwq, expected 4 (cold, warm, typed, bound)"

while read -r depth used model; do
    echo "stack: sweep depth $depth frame $used B (model $model)"
    [ "$used" -le "$model" ] || fail "depth-$depth frame used $used B, above the model stack_bytes($depth) = $model B -- a frame outgrew the model"
done < <(printf '%s\n' "$out" | sed -nE 's/^sweep: depth ([0-9]+) cold frame ([0-9]+) B \(model ([0-9]+)\)$/\1 \2 \3/p')

# The dev profile, recorded: not the budget's profile (§2.6).
if dev=$(firmware_build_one fwq); then
    dout=$(run "$dev")
    printf '%s\n' "$dout" | grep -E '^(calibrate|stack|sweep):' | sed 's/^/  dev (record, not gated)| /'
else
    fail "the dev fwq did not build"
fi

exit "$rc"
