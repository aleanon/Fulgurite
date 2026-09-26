#!/usr/bin/env bash
# The embedded firmwares must claim NO RAM before `main` — M13/T13.9,
# tightened 2026-09-07; every gated binary since M52/T52.1, not only the
# paint-only one (scripts/lib/firmware.sh says why there are four).
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

# **Expected red until M52/T52.3** (F-NEW-1): the derive's per-binding
# `BindSite` static starts at a non-zero "unknown", so it lands in `.data`
# -- flash for its initialiser, RAM before `main`. While this is set, a
# binary whose ONLY unexempted statics match it reports them as expected
# and passes; any other static is still red, and a binary that links none
# fails the marker, so it cannot outlive the fix it waits for.
XFAIL_BINS="firmware_interactive firmware_bind"
XFAIL_RE='::update_marked::__ln_site$'
XFAIL_WHY="F-NEW-1: a binding's cache is in .data (until T52.3)"

allowed=$(grep -vE '^\s*(#|$)' "$ALLOW" | sed -E 's/^[[:space:]]+|[[:space:]]+$//g')
n_allowed=$(printf '%s\n' "$allowed" | grep -c . || true)

rc=0
for name in $FIRMWARE_BINS; do
    BIN=$(firmware_path "$name")
    if ! syms=$(nm -C --size-sort -S --radix=d "$BIN" 2>&1); then
        echo "bss: nm failed on $BIN:" >&2
        echo "$syms" | head -5 | sed 's/^/    /' >&2
        rc=1; continue
    fi

    # B/b = bss, D/d = data, G/g and S/s = small-object variants of each.
    # The name is EVERYTHING after the third field, not `$4`: a demangled
    # name has spaces in it (`<embedded::Device as lumen_static::event::App>
    # ::update_marked::__ln_site`), and `$4` cut it at the first -- a
    # rejection that named only a prefix, which no exemption could match
    # (M52/T52.1, docs/plan-embedded-footprint.md §1.1).
    table=$(printf '%s\n' "$syms" | awk '$3 ~ /^[BbDdGgSs]$/ { sz = $2 + 0; sub(/^[^ ]+ +[^ ]+ +[^ ]+ +/, ""); print sz "\t" $0 }' | sort -t$'\t' -k2 -u)
    n_found=$(printf '%s\n' "$table" | grep -c . || true)
    if [ "$n_found" -eq 0 ]; then
        echo "bss: nm listed no .bss/.data symbol at all in $BIN -- the fixture's exempted" >&2
        echo "static is always present, so the listing is wrong, not the firmware clean" >&2
        rc=1; continue
    fi

    bad=$(printf '%s\n' "$table" | while IFS=$'\t' read -r sz sym; do
        printf '%s\n' "$allowed" | grep -qxF -- "$sym" || printf '%s\t%s\n' "$sz" "$sym"
    done)

    xfail=""
    n_x=0
    case " $XFAIL_BINS " in *" $name "*) xfail=1 ;; esac
    if [ -n "$xfail" ]; then
        expected=$(printf '%s\n' "$bad" | grep -E -- "$XFAIL_RE" || true)
        bad=$(printf '%s\n' "$bad" | grep -vE -- "$XFAIL_RE" || true)
        n_x=$(printf '%s\n' "$expected" | grep -c . || true)
        if [ -z "$expected" ]; then
            echo "bss: $name: expected red ($XFAIL_WHY), but no such static is linked -- remove the XFAIL" >&2
            rc=1
        else
            printf '%s\n' "$expected" | while IFS=$'\t' read -r sz sym; do
                echo "bss: $name: expected red -- $sym ($sz bytes) ($XFAIL_WHY)"
            done
        fi
    fi

    if [ -n "$bad" ]; then
        echo "bss: framework statics in .bss/.data of $name ($BIN), which must claim no RAM before main:" >&2
        printf '%s\n' "$bad" | while IFS=$'\t' read -r sz sym; do
            echo "  $sym  ($sz bytes)" >&2
        done
        echo "" >&2
        echo "Remove the static, or add it to $ALLOW with the argument for it." >&2
        rc=1; continue
    fi

    echo "bss: $name: $n_found symbol(s), $((n_found - n_x)) exempted ($n_allowed names in $ALLOW), $n_x expected red (nm: $BIN)"
done
exit "$rc"
