#!/usr/bin/env bash
# The embedded firmwares must claim NO RAM before `main` — M13/T13.9,
# tightened 2026-09-07; every gated binary since M52/T52.1, not only the
# paint-only one (scripts/lib/firmware.sh says why there are four). And
# `.data` -- the initialised half, which costs flash too -- is 0 B in each
# (M52/T52.3).
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

# Two kinds of entry (M52/T52.3): an exact name, and `glob: <pattern>`, a
# shell glob matched against the FULL demangled name. A pattern exists for
# one static the derive emits per `#[bind]` -- the binding's node cache --
# whose name carries the application's type, so no exact line could list
# it. Pattern-exempted symbols are counted and their bytes printed: an
# exemption by pattern is allowed only if it is visible.
allowed=$(grep -vE '^\s*(#|$)' "$ALLOW" | grep -v '^glob:' | sed -E 's/^[[:space:]]+|[[:space:]]+$//g')
globs=$(grep -E '^glob:' "$ALLOW" | sed -E 's/^glob:[[:space:]]*//; s/[[:space:]]+$//')
n_allowed=$(printf '%s\n' "$allowed" | grep -c . || true)
n_globs=$(printf '%s\n' "$globs" | grep -c . || true)

# Does SYM match one of the patterns?
by_glob() {
    local g
    while IFS= read -r g; do
        [ -n "$g" ] || continue
        # shellcheck disable=SC2053
        [[ $1 == $g ]] && return 0
    done <<<"$globs"
    return 1
}

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

    bad=""; n_glob=0; b_glob=0
    while IFS=$'\t' read -r sz sym; do
        if printf '%s\n' "$allowed" | grep -qxF -- "$sym"; then
            :
        elif by_glob "$sym"; then
            n_glob=$((n_glob + 1)); b_glob=$((b_glob + sz))
        else
            bad="$bad$sz"$'\t'"$sym"$'\n'
        fi
    done <<<"$table"
    bad=${bad%$'\n'}

    # `.data` is the half that costs flash as well as RAM: an initialiser
    # stored in the image and copied before `main`. Zero bytes in every
    # binary, printed -- F-NEW-1's binding cache sat there until T52.3.
    data=$(size -A "$BIN" | awk '$1 == ".data" {print $2}')
    data=${data:-0}
    if [ "$data" -ne 0 ]; then
        echo "bss: $name: .data is $data B (bound 0) -- a static with a non-zero initialiser:" >&2
        printf '%s\n' "$syms" | awk '$3 ~ /^[DdGg]$/' | head -5 | sed 's/^/    /' >&2
        rc=1
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

    [ "$data" -eq 0 ] || continue
    echo "bss: $name: $n_found symbol(s), .data $data B (bound 0), $n_glob binding cache(s) ($b_glob B, exempt by pattern), $((n_found - n_glob)) by name ($n_allowed names, $n_globs patterns in $ALLOW) (nm: $BIN)"
done
exit "$rc"
