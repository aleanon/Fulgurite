#!/usr/bin/env bash
# M54/T54.0: a screen reader is a user, so it gets a live gate
# (docs/plan-a11y-platform.md §0 rule 1, R15-F1).
#
# Walks the platform's accessibility bus (AT-SPI, through
# scripts/a11y_drive.py) for two windows, each launched through
# scripts/live/launch.py so the walk is about the process it names:
#
#   counter-window --features a11y   the GOOD INPUT: its hand-written
#                                    adapter has reached the bus since
#                                    T7.5, so the walker is shown to see
#                                    what is there;
#   showcase --features agent        a `Program` application, the front
#                                    door since M32.
#
# Needs a display and an AT-SPI bus: exits 77 without either, which
# `just live` counts as not run, never as a pass (M43 §2.2).
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
LAUNCH="python3 scripts/live/launch.py --script a11y-drive"
$LAUNCH --display-only || exit $?
python3 - <<'EOF' || exit $?
import sys
try:
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi
except (ImportError, ValueError) as e:
    print(f"a11y-drive: skip -- no AT-SPI bindings: {e}"); sys.exit(77)
if Atspi.get_desktop(0).get_child_count() == 0:
    print("a11y-drive: skip -- the AT-SPI desktop lists no applications"); sys.exit(77)
EOF

cargo build --release -q -p counter-window --features a11y
$LAUNCH --leg counter-window --exe target/release/counter-window \
    -- python3 scripts/a11y_drive.py {port} --expect present --button "+1"

# The Program application. Expected ABSENT until T54.2 builds the
# adapter into the window; the walker fails as XPASS if it appears
# first, so the expectation cannot outlive the fix.
cargo build --release -q -p showcase --features agent
$LAUNCH --leg showcase --exe target/release/showcase \
    -- python3 scripts/a11y_drive.py {port} --expect absent --timeout 5

echo "a11y-drive: counter-window on the bus; showcase absent as expected until T54.2"
