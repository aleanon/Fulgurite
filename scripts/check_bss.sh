#!/usr/bin/env bash
# The embedded firmware must claim NO RAM before `main` — M13/T13.9,
# tightened 2026-09-07.
#
# `.bss` is zero-initialised globals: the file stores only their size,
# and startup code zeroes that much RAM. `.data` is globals with a
# non-zero initial value, stored in the image and copied to RAM. Both
# are memory a framework takes before an application runs, which is the
# thing a no-allocator profile cannot afford to have appear by accident.
#
# The old budget was `<= 2 KB` and measured 0 — a threshold nothing has
# approached. This checks the property that number was standing in for:
# zero framework statics, with a named exemption list.
#
# M43/T43.4: the binary judged is the one cargo reports having built
# (scripts/lib/firmware.sh), not a hard-coded ./target path that a
# CARGO_TARGET_DIR leaves stale (R11-F6); and `nm` failing is a failure,
# not an empty -- and therefore clean -- symbol list (R17-F9). At least one
# symbol must be found, because the fixture's own exempted `SINK` is
# always there: zero symbols means the listing is wrong, not the firmware
# clean.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

ALLOW=scripts/bss.allow
# shellcheck source=lib/firmware.sh
source scripts/lib/firmware.sh
firmware_build || exit 1
BIN=$FIRMWARE

allowed=$(grep -vE '^\s*(#|$)' "$ALLOW" | tr -d ' ')

if ! syms=$(nm -C --size-sort -S --radix=d "$BIN" 2>&1); then
    echo "bss: nm failed on $BIN:" >&2
    echo "$syms" | head -5 | sed 's/^/    /' >&2
    exit 1
fi

# B/b = bss, D/d = data, G/g and S/s = small-object variants of each.
found=$(echo "$syms" | awk '$3 ~ /^[BbDdGgSs]$/ {print $4}' | sort -u)

n_allowed=$(echo "$allowed" | grep -c . || true)
n_found=$(echo "$found" | grep -c . || true)

if [ "$n_found" -eq 0 ]; then
    echo "bss: nm listed no .bss/.data symbol at all in $BIN -- the fixture's exempted" >&2
    echo "static is always present, so the listing is wrong, not the firmware clean" >&2
    exit 1
fi

bad=""
for sym in $found; do
  echo "$allowed" | grep -qxF "$sym" || bad="$bad $sym"
done

if [ -n "$bad" ]; then
  echo "bss: framework statics in .bss/.data of $BIN, which must claim no RAM before main:" >&2
  for sym in $bad; do
    size=$(echo "$syms" | awk -v s="$sym" '$4 == s {print $2}')
    echo "  $sym  ($size bytes)" >&2
  done
  echo "" >&2
  echo "Remove the static, or add it to $ALLOW with the argument for it." >&2
  exit 1
fi

echo "bss: $n_found symbol(s) (nm: $BIN), all $n_found exempted ($n_allowed names in $ALLOW)"
