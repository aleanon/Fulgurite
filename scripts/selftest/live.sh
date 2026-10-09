#!/usr/bin/env bash
# The live plants (M42, docs/plan-drive-identity.md §0 rule 5 and §2.6):
# every refusal the drive harness makes, planted in a scratch copy and
# shown red by name, on M43's harness (`scripts/selftest/run.sh live`).
# Needs a display; exits 77 without one (M43 §2's skip code).
#
#   plant               finding    red output once fixed
#   stranger            R18-F1     port already answering on 127.0.0.1:P
#   leg-drop  (I5a)     R18-F1     expected Everything, got ...Boundaries<256>
#   shadow-absent       I-NEW-3    expected the shadow oracle in this build
#   shadow-unexpected   I-NEW-3    shadow oracle present in a leg that does not expect it
#   hover-noop          R18-F3     Button/hovering is visible...: changed in subtree 0
#   quit-panic (I6)     R18-F9     live-gate: app exited 101
#   late-panic          R18-F9     app log: 1 panic
#
# Two more need no display and run in `just selftest` (plants/static/):
#   inventory-drop (I5b/I5c)  R18-F2  manifest expects Radio / Switch: missing action
#   export-unowned            R18-F2  check_drive_coverage.sh names the widget
#
# The harness's own injection -- a plant whose edit matches nothing --
# reports `edit did not apply`, never `stayed green`.
set -uo pipefail
if [ -z "${DISPLAY:-}" ]; then
    echo "selftest-live: no X display -- skipped (77)"
    exit 77
fi
# The launcher picks a free port per leg (scripts/live/launch.py); these
# are the ones it may pick from here. The stranger plant needs one port
# OUTSIDE that range, so a range given by the caller (`just ci` runs with
# the one it was given, M43/T43.13) gives its top port to the stranger and
# the rest to the launcher; with none, 7880-7889 and 7890.
if [ -n "${LUMEN_LIVE_PORT_RANGE:-}" ] && [ -z "${LUMEN_SELFTEST_STRANGER_PORT:-}" ]; then
    lo=${LUMEN_LIVE_PORT_RANGE%-*}; hi=${LUMEN_LIVE_PORT_RANGE#*-}
    if [ "$hi" -le "$lo" ]; then
        echo "selftest-live: LUMEN_LIVE_PORT_RANGE=$LUMEN_LIVE_PORT_RANGE needs two ports or more" >&2
        exit 2
    fi
    export LUMEN_SELFTEST_STRANGER_PORT=$hi LUMEN_LIVE_PORT_RANGE="$lo-$((hi - 1))"
fi
export LUMEN_LIVE_PORT_RANGE="${LUMEN_LIVE_PORT_RANGE:-7880-7889}"
export LUMEN_SELFTEST_STRANGER_PORT="${LUMEN_SELFTEST_STRANGER_PORT:-7890}"
echo "selftest-live: launcher ports $LUMEN_LIVE_PORT_RANGE, stranger on $LUMEN_SELFTEST_STRANGER_PORT"
# The stranger plant starts its window itself, not through the launcher.
. "$(dirname "$0")/../drive_window.sh"  # live windows on the monitor the user is not reading
exec "$(dirname "$0")/run.sh" live
