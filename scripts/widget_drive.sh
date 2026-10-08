#!/usr/bin/env bash
# M27/T27.3: every widget, driven through every action it accepts,
# against the live window -- under BOTH invalidation strategies.
#
# `cargo test` drives a `Host` headlessly and `just agent-drive` proves
# the socket reaches a real window. Neither asks the question this does:
# for each widget, is every action a user can perform actually wired, does
# it do the specific thing it claims, and does the same input just outside
# the widget do nothing?
#
# Twice, because M26 found five defects and **four were invisible under
# the default `Everything`**: a clean subtree is skipped before a wrong
# answer can be observed. A protocol written after M26 that runs only the
# default has ignored M26's finding.
#
# **Each leg names the process it drove** (M42/T42.2,
# docs/plan-drive-identity.md): `scripts/live/launch.py` refuses a port
# that already answers, launches the binary itself, checks that the
# socket's owner is its child, and that the child's own `app.diagnostics`
# reports the strategy and features the leg declares below -- and prints
# `drove pid P (socket owner P), strategy S, ...`. Until then the strategy
# was a label this script set, and a leftover showcase on the port was
# driven twice and reported "both ways" (R18-F1).
#
# Needs a display. Skips with exit 77 without one (M43 §2).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
. scripts/drive_window.sh  # live windows on the monitor the user is not reading
LAUNCH="python3 scripts/live/launch.py --script widget-drive"
$LAUNCH --display-only || exit $?

# The manifest first, before anything is built or opened (M42/T42.5): a
# removed or added inventory row fails here, against a file this drive
# does not write.
python3 scripts/drive/run.py --manifest-only

ONLY="${1:-}"
status=0

# run_one LEG CARGO-FEATURES EXPECTED-FEATURES
#   LEG is the strategy the process must report, with a `+oracle` suffix
#   on the leg that builds the shadow oracle in; the expected features are
#   declared apart from the build's, so a leg built wrong is refused
#   rather than re-described. After the drive, pass or fail, `scripts/live/shadow.py`
#   asks the window's oracle the leg's question (M42/T42.4): `supported`,
#   `compared > 0` and no divergence where the leg declares it, and
#   `supported: false` everywhere else.
#
# LUMEN_DRIVE_LEGS (comma-separated leg names, e.g. `Boundaries`) runs only
# those legs -- `just ci-fast` drives one (docs/results-ci-time.md). A name
# that is not a leg is refused, so a typo cannot drive nothing and pass.
ALL_LEGS="Everything Boundaries Boundaries+oracle Boundaries+scale2 Everything+shaping Boundaries+shaping"
if [ -n "${LUMEN_DRIVE_LEGS:-}" ]; then
    for l in ${LUMEN_DRIVE_LEGS//,/ }; do
        case " $ALL_LEGS " in *" $l "*) ;; *) echo "widget-drive: LUMEN_DRIVE_LEGS: '$l' is not a leg ($ALL_LEGS)" >&2; exit 2 ;; esac
    done
fi
run_one() {
    local leg="$1" features="$2" expect="$3"
    if [ -n "${LUMEN_DRIVE_LEGS:-}" ] && [[ ",$LUMEN_DRIVE_LEGS," != *",$leg,"* ]]; then
        return 0
    fi
    cargo build --release -p showcase --features "$features" >/dev/null
    $LAUNCH --leg "$leg" --exe target/release/showcase \
        --expect-strategy "${leg%%+*}" --expect-features "$expect" \
        -- bash -c 'python3 scripts/drive/run.py "$1"; r=$?
                   python3 scripts/live/shadow.py widget-drive || exit 1; exit $r' \
           _ "$ONLY" || status=1
}

# The oracle leg is a third leg, not a feature of the second (M41/T41.7,
# docs/plan-boundaries-repair.md §0 rule 2): the oracle lays the live
# widgets out again after each frame it judges, so a `Boundaries` leg
# with it compiled in would be judging a repaired window.
run_one Everything         agent,everything     ""
run_one Boundaries         agent                ""
run_one Boundaries+oracle  agent,shadow-oracle  shadow-oracle
# M57/D6: a scale-2 leg, where fractional logical edges land on physical
# pixels, and every action's damaged frame is compared with a full
# repaint of the same state. A `Boundaries` process, so the leg is named
# for the strategy it must report, as the oracle's is.
WINIT_X11_SCALE_FACTOR=2 LUMEN_DRIVE_FULLCHECK=1 \
run_one Boundaries+scale2  agent                ""
# M81/T81.6: the catalogue laid out `Shaping::Complex` (the showcase's
# `shaping` feature), under both strategies -- every entry's Latin through
# the shaped path's measure and paint, and `TextInput RTL` right to left.
# Each leg declares `shaping`, so a build without it is refused by
# launch.py rather than driven as if shaped; M57's D5 parity checks that
# `--demo` laid the same entries out in the same mode.
run_one Everything+shaping agent,everything,shaping shaping
run_one Boundaries+shaping agent,shaping            shaping

exit "$status"
