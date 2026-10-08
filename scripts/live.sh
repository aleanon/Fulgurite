#!/usr/bin/env bash
# `just live`: every live gate, aggregated (M43/T43.13,
# docs/plan-gate-hygiene.md §2.2). R17-F8: each of these used to exit 0
# on an empty display, so "green" and "never opened a window" looked the
# same, and `check` ran only one of them -- and only when DISPLAY or
# WAYLAND_DISPLAY happened to be set.
#
#   scripts/live.sh              all of them
#   scripts/live.sh NAME...      the named ones (as listed below)
#
# Each script exits 77 when a capability it needs is absent -- a display
# (scripts/live/launch.py, M42/T42.2), `xwd` or an adapter (gpu_parity.sh).
# This prints one line per script:
#
#   <name>: pass | FAIL (exit N) | skip(77) -- <reason>
#
# and a total. It FAILS on any FAIL. It also fails on any skip -- **a skip
# is not a pass** -- unless LUMEN_ALLOW_SKIP=1, in which case it prints
# `N skipped (allowed)` and exits 0. With no FAIL and a refused skip it
# exits 77, so an aggregate above this one (`just ci`) can tell "did not
# run" from "ran and failed".
#
# Ports: the launcher picks free ones from LUMEN_LIVE_PORT_RANGE when set.
# Logs: target/live/aggregate/<name>.log, one per script, overwritten.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

# name -> command. The order is cheapest first; the list is the one place
# `just live` names its scripts, and `check_live_list` below holds it to
# every live script that exists.
ALL=(
    "live-gate|./scripts/live_gate.sh"
    "wake-gate|./scripts/wake_gate.sh"
    "tasks-drive|./scripts/tasks_drive.sh"
    "panic-drive|./scripts/panic_drive.sh"
    "windows-drive|./scripts/windows_drive.sh"
    "motion-present|./scripts/motion_present.sh"
    "agent-drive|./scripts/agent_drive.sh"
    "widget-drive|./scripts/widget_drive.sh"
    "hot-window-drive|./scripts/hot_window_drive.sh"
    "gpu-parity|./scripts/gpu_parity.sh"
    "resume-drive|./scripts/resume_drive.sh"
    "windows-rss|./scripts/windows_rss.sh"
    "shader-drive|./scripts/shader_drive.sh"
    "idle-count|./scripts/idle_count.sh"
    "a11y-drive|./scripts/a11y_drive.sh"
    # On its own bus with the stand-in watcher, so `just ci` never puts an
    # item in the user's panel; the desktop's watcher is `just tray-drive`.
    "tray-drive|./scripts/tray_drive.sh --private-bus"
)

# Every script that goes through the live launcher is a live gate and must
# be listed above, or a new one would be written, pass alone and never be
# aggregated -- which is R17-F8's shape again.
check_live_list() {
    local f base missing=0
    for f in $(grep -l 'scripts/live/launch.py' scripts/*.sh); do
        base=$(basename "$f")
        [ "$base" = live.sh ] && continue
        if ! printf '%s\n' "${ALL[@]}" | grep -qE "\\|\\./scripts/$base( |\$)"; then
            echo "live: $f uses the live launcher but is not in scripts/live.sh's list" >&2
            missing=1
        fi
    done
    return $missing
}
check_live_list || exit 1

want=("$@")
LOGS=target/live/aggregate
mkdir -p "$LOGS"
n=0; pass=0; fail=0; skip=0
for entry in "${ALL[@]}"; do
    name=${entry%%|*}; cmd=${entry#*|}
    if [ "${#want[@]}" -gt 0 ] && ! printf '%s\n' "${want[@]}" | grep -qx "$name"; then
        continue
    fi
    n=$((n + 1))
    log="$LOGS/$name.log"
    t0=$(date +%s)
    $cmd >"$log" 2>&1 </dev/null
    rc=$?
    secs=$(( $(date +%s) - t0 ))
    last=$(grep -v '^[[:space:]]*$' "$log" | tail -1 | cut -c1-160)
    case $rc in
        0)  pass=$((pass + 1)); echo "$name: pass (${secs} s)" ;;
        77) skip=$((skip + 1)); echo "$name: skip(77) -- ${last:-no reason printed}" ;;
        *)  fail=$((fail + 1)); echo "$name: FAIL (exit $rc, ${secs} s) -- ${last:-no output} (log $log)" ;;
    esac
done
[ "$n" -gt 0 ] || { echo "live: no such script: ${want[*]}" >&2; exit 2; }

echo "live: $n scripts -- $pass passed, $fail failed, $skip skipped"
[ "$fail" -eq 0 ] || exit 1
if [ "$skip" -gt 0 ]; then
    if [ "${LUMEN_ALLOW_SKIP:-}" = 1 ]; then
        echo "live: $skip skipped (allowed)"
    else
        echo "live: $skip skipped -- a skip is not a pass; LUMEN_ALLOW_SKIP=1 accepts it" >&2
        exit 77
    fi
fi
exit 0
