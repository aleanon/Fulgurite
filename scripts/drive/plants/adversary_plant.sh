#!/usr/bin/env bash
# `just adversary-plant` -- M42/T42.7 (R18-F11, docs/plan-drive-identity.md
# §1.5): the adversary's planted weakness, as a recipe that proves it is a
# weakness.
#
# docs/plan-widget-parity.md §5 tests the adversarial reviewer against a
# drive weakened on purpose. The one recorded run (T27.10) planted
# `_page(c) <= c._p0` for `== c._p0 - 1` in the Pagination inventory,
# and the plant existed only as prose. **Stored as `<`, not `<=`**: since
# the isolate check (3) was added, `<=` is caught by it -- an outside
# click leaves the page unchanged, which `<=` accepts, so the drive
# reports "the handler fires on everything" and the plant blinds nothing.
# `<` is the blind spot `<=` was meant to be (M42/T42.7). A plant makes the drive *blinder*,
# so "apply it and expect red" would refuse good input: with the plant and
# nothing else, the drive is green. What makes it a fair test of a
# reviewer is that it hides a real defect, so this runs two arms in a
# scratch copy, each over the widget-side defect the weakened assertion
# names ("make the first slot go to the window's first page"):
#
#   (A) the defect, with the real drive     -- must go RED, by name
#   (B) the defect, with the planted drive  -- must stay GREEN
#
# and prints `plant t27_10_pagination: real drive red (N failed), planted
# drive green`, or which arm was wrong. Patches next to this file:
#   t27_10_pagination.patch         the weakened assertion (the plant)
#   t27_10_pagination.defect.patch  the widget defect it hides
# A patch that does not apply fails as such, never as an arm's verdict.
# Needs a display; exits 77 without one (M43 §2).
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
HERE="$ROOT/scripts/drive/plants"
NAME=t27_10_pagination
if [ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    echo "adversary-plant: no display -- skipped (77)"
    exit 77
fi

WT="$ROOT/target/adversary/wt"
LOG="$ROOT/target/adversary"
mkdir -p "$WT"
exec 9>"$LOG/lock"
flock 9

# The working tree, by content (what is about to be committed), into a
# copy whose target/ survives between runs -- the same sync as
# scripts/selftest/run.sh.
cd "$ROOT"
find "$WT" -mindepth 1 -maxdepth 1 ! -name .git ! -name target -exec rm -rf {} +
git ls-files -co --exclude-standard -z \
  | rsync -rlpD --checksum --from0 --files-from=- --ignore-missing-args ./ "$WT/"
cd "$WT"
if [ ! -d .git ]; then
    git init -q
    git config user.email adversary@localhost
    git config user.name adversary
fi
git add -A >/dev/null 2>&1
git commit -q --allow-empty -m "adversary base" >/dev/null
BASE=$(git rev-parse HEAD)

# arm LABEL PATCH... -- reset, apply every patch (refusing one that does
# not apply), run the Pagination drive; echoes the drive's exit status.
arm() {
    local label="$1"; shift
    git reset -q --hard "$BASE"; git clean -fdq
    for p in "$@"; do
        if [ ! -s "$p" ]; then
            echo "adversary-plant: $(basename "$p") is empty -- applying it changes nothing" >&2
            continue
        fi
        if ! git apply --whitespace=nowarn "$p" 2>>"$LOG/$label.log"; then
            echo "adversary-plant: $(basename "$p") did not apply" >&2
            return 2
        fi
    done
    ./scripts/widget_drive.sh Pagination >"$LOG/$label.log" 2>&1
}

arm real "$HERE/$NAME.defect.patch"; real=$?
arm planted "$HERE/$NAME.defect.patch" "$HERE/$NAME.patch"; planted=$?
git reset -q --hard "$BASE"; git clean -fdq
[ "$real" = 2 ] || [ "$planted" = 2 ] && exit 2

failed=$(grep -c 'FAIL Pagination' "$LOG/real.log" || true)
named=$(grep -m1 'FAIL Pagination.*the previous arrow steps back' "$LOG/real.log" || true)
echo "adversary-plant: real drive exit $real, $failed failed leg(s)${named:+; first: ${named:0:160}}"
echo "adversary-plant: planted drive exit $planted ($(grep -E 'widget-drive \[.*\]: [0-9]+ widgets' "$LOG/planted.log" | grep -oE '[0-9]+ failed' | paste -sd/ -) per leg)"
bad=0
if [ "$real" -eq 0 ] || [ -z "$named" ]; then
    echo "adversary-plant: defect not caught by the real drive (log $LOG/real.log)"
    bad=1
fi
if [ "$planted" -ne 0 ]; then
    echo "adversary-plant: plant does not blind the drive (log $LOG/planted.log)"
    tail -5 "$LOG/planted.log" | sed 's/^/    | /'
    bad=1
fi
[ "$bad" -eq 0 ] || exit 1
echo "plant $NAME: real drive red ($failed failed), planted drive green"
