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
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

TARGET=thumbv7em-none-eabihf
BIN=target/$TARGET/release/firmware
ALLOW=scripts/bss.allow

cargo build --release -q -p embedded --bin firmware --features firmware --target $TARGET || exit 1
[ -f "$BIN" ] || { echo "bss: $BIN not built" >&2; exit 1; }

allowed=$(grep -vE '^\s*(#|$)' "$ALLOW" | tr -d ' ')

# B/b = bss, D/d = data, G/g and S/s = small-object variants of each.
found=$(nm -C --size-sort -S --radix=d "$BIN" 2>/dev/null \
        | awk '$3 ~ /^[BbDdGgSs]$/ {print $4}' | sort -u)

bad=""
for sym in $found; do
  echo "$allowed" | grep -qxF "$sym" || bad="$bad $sym"
done

n_allowed=$(echo "$allowed" | grep -c . || true)
n_found=$(echo "$found" | grep -c . || true)

if [ -n "$bad" ]; then
  echo "bss: framework statics in .bss/.data, which must claim no RAM before main:" >&2
  for sym in $bad; do
    size=$(nm -C --size-sort -S --radix=d "$BIN" | awk -v s="$sym" '$4 == s {print $2}')
    echo "  $sym  ($size bytes)" >&2
  done
  echo "" >&2
  echo "Remove the static, or add it to $ALLOW with the argument for it." >&2
  exit 1
fi

echo "bss: $n_found symbol(s) in .bss/.data, all $n_allowed exempted by name"
