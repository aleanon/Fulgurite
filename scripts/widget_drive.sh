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
LAUNCH="python3 scripts/live/launch.py --script widget-drive"
$LAUNCH --display-only || exit $?

# The manifest first, before anything is built or opened (M42/T42.5): a
# removed or added inventory row fails here, against a file this drive
# does not write.
python3 scripts/drive/run.py --manifest-only

ONLY="${1:-}"
status=0

# run_one LEG CARGO-FEATURES EXPECTED-FEATURES
#   LEG is the strategy the process must report; the expected features are
#   declared apart from the build's, so a leg built wrong is refused
#   rather than re-described.
run_one() {
    local strategy="$1" features="$2" expect="$3"
    cargo build --release -p showcase --features "$features" >/dev/null
    $LAUNCH --leg "$strategy" --exe target/release/showcase \
        --expect-strategy "$strategy" --expect-features "$expect" \
        -- python3 scripts/drive/run.py "$ONLY" || status=1
}

run_one Everything  agent,everything  everything
run_one Boundaries  agent             ""

exit "$status"
