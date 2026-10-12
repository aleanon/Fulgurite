#!/usr/bin/env bash
# Kill rule K1 of docs/plan-lss.md (M61): **a `.lss` sheet costs zero
# bytes.** The firmwares are linked twice -- once with the hand-written
# `SHEET` slice in fixtures/embedded/src/lib.rs, once with the same sheet
# authored as fixtures/embedded/src/sheet.lss through `include_lss!`
# (`FULGURITE_SHEET=lss`, a cfg the fixture's build.rs sets; it says why a
# feature could not be used) -- and each binary's `.text` and `.rodata` must
# be byte-identical between the two. Sizes are printed beside the verdict,
# so a match is visibly a match of something.
#
# Bytes, not sizes: a sheet lowered to different rules of the same size
# (a colour swapped) keeps every size and changes `.rodata`.
#
# Precondition, checked before the comparison is read: the firmwares that
# cascade must link `cascade_range` -- the code the sheet feeds. Without
# it both sides could agree about a binary in which no sheet is used.
#
# The `lss` side builds in its own target directory, so neither build
# relinks the other's (the default side is scripts/lib/firmware.sh's,
# shared with check_text.sh and check_bss.sh).
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

# shellcheck source=lib/firmware.sh
source scripts/lib/firmware.sh
firmware_build || exit 1
declare -A HAND
for name in $FIRMWARE_BINS; do HAND[$name]=$(firmware_path "$name"); done

LSS_DIR="${CARGO_TARGET_DIR:-target}/lss-parity"
bins=""; for b in $FIRMWARE_BINS; do bins="$bins --bin $b"; done
# shellcheck disable=SC2086
json=$(FULGURITE_SHEET=lss CARGO_TARGET_DIR="$LSS_DIR" cargo build --release -q -p embedded $bins \
    --features firmware --target "$FIRMWARE_TARGET" --message-format=json) || {
    echo "lss-parity: the include_lss! firmware build failed" >&2
    exit 1
}
declare -A LSS
while read -r name path; do LSS[$name]=$path; done < <(printf '%s\n' "$json" | python3 -c '
import json, sys
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    t = m.get("target", {})
    if m.get("reason") == "compiler-artifact" and "bin" in t.get("kind", []) and m.get("executable"):
        print(t["name"], m["executable"])
')

# section BIN NAME -> "<size> <sha256>" of that section's bytes. ELF32
# little-endian, as the thumb target links; host binutils' objcopy does
# not recognise the format, so the section table is read here.
section() {
    python3 - "$1" "$2" <<'PY'
import hashlib, struct, sys
data = open(sys.argv[1], "rb").read()
assert data[:4] == b"\x7fELF" and data[4] == 1 and data[5] == 1, "not an ELF32 LE file"
shoff, = struct.unpack_from("<I", data, 0x20)
shentsize, shnum, shstrndx = struct.unpack_from("<HHH", data, 0x2E)
def sh(i):
    return struct.unpack_from("<IIIIIIIIII", data, shoff + i * shentsize)
names = sh(shstrndx)
for i in range(shnum):
    h = sh(i)
    off = names[4] + h[0]
    name = data[off:data.index(b"\0", off)].decode()
    if name == sys.argv[2]:
        body = data[h[4]:h[4] + h[5]]
        print(len(body), hashlib.sha256(body).hexdigest())
        break
else:
    print("0 missing")
PY
}

rc=0
for name in $FIRMWARE_BINS; do
    hand=${HAND[$name]}; lss=${LSS[$name]:-}
    [ -n "$lss" ] && [ -f "$lss" ] || { echo "lss-parity: cargo reported no $name for the lss build" >&2; rc=1; continue; }
    case $name in
    firmware_interactive|firmware_cascade)
        n=$(nm -C "$lss" | grep -cF cascade_range || true)
        [ "$n" -gt 0 ] || { echo "lss-parity: $name (lss) links 0 cascade_range symbols -- no sheet is used, so the comparison measures nothing" >&2; rc=1; continue; }
        ;;
    esac
    line="$name:"
    for sec in .text .rodata; do
        read -r hs hh <<<"$(section "$hand" "$sec")"
        read -r ls lh <<<"$(section "$lss" "$sec")"
        if [ "$hh" = "$lh" ] && [ "$hh" != missing ]; then
            line="$line $sec $hs B hand / $ls B lss, identical;"
        else
            line="$line $sec $hs B hand / $ls B lss, DIFFERENT (delta $((ls - hs)) B);"
            rc=1
        fi
    done
    echo "lss-parity: $line"
done
[ "$rc" -eq 0 ] || echo "lss-parity: K1 fails -- a .lss sheet must cost zero bytes (docs/plan-lss.md §5)" >&2
exit "$rc"
