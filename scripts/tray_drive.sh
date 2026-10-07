#!/usr/bin/env bash
# M90/T90.7 (docs/plan-desktop-integration.md): `just tray-drive` --
# examples/tray, a daemon whose only surface is a Linux tray item, driven
# through the **OS path**: the StatusNotifierWatcher's registry and the
# item's `com.canonical.dbusmenu` menu, over D-Bus with `gdbus`, under both
# invalidation strategies. What each leg asserts is `scripts/tray_drive.py`'s.
#
# Prerequisites and capabilities (docs/plan-iced-roadmap.md §3.1, §3.4):
#
# - **`gdbus` is a prerequisite**: the reference machine has
#   `/usr/bin/gdbus`, so without it the drive FAILS (exit 1) with the
#   install line, never 77 -- D7's rule, as M88/T88.4 applies it to
#   `xclip`. So is python3-gi, which parses `gdbus`'s GVariant text (and
#   is the private bus's watcher).
# - **A display, a session bus and a StatusNotifierWatcher on it are
#   capabilities**: without one the drive exits 77 with the reason. A skip
#   is not a pass; M90b closes only where the watcher is.
#
# `--private-bus` runs the whole drive inside `dbus-run-session`, on a bus
# nothing else is on, with `scripts/live/sni_watcher.py` as the watcher --
# so the item never appears in the user's own tray. The item, its menu and
# every click are still the application's, read over D-Bus as on the
# desktop; only the registry is the stand-in. Without it the drive uses
# the desktop's watcher (on the reference machine, Cinnamon's) and the item
# shows in the panel for the run.
#
# Each leg names the process it drove (`scripts/live/launch.py`), and must
# end by itself with status 0: Quit is `Request::Quit`, chosen from the
# tray, never `app.quit`.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

if [ "${1:-}" = "--private-bus" ]; then
    shift
    command -v dbus-run-session >/dev/null 2>&1 || {
        echo "tray-drive: FAIL -- --private-bus needs dbus-run-session: sudo apt install dbus-bin" >&2
        exit 1
    }
    exec dbus-run-session -- "$0" --inside-private-bus "$@"
fi
if [ "${1:-}" = "--inside-private-bus" ]; then
    shift
    log=target/live/tray-drive-watcher.log
    mkdir -p target/live
    python3 scripts/live/sni_watcher.py >"$log" 2>&1 &
    watcher=$!
    trap 'kill "$watcher" 2>/dev/null || true' EXIT
    for _ in $(seq 100); do
        grep -q 'sni-watcher: ready' "$log" 2>/dev/null && break
        kill -0 "$watcher" 2>/dev/null || { echo "tray-drive: FAIL -- the stand-in watcher died: $(cat "$log")" >&2; exit 1; }
        sleep 0.05
    done
    grep -q 'sni-watcher: ready' "$log" || { echo "tray-drive: FAIL -- the stand-in watcher never owned its name" >&2; exit 1; }
    echo "tray-drive: private session bus $DBUS_SESSION_BUS_ADDRESS, stand-in watcher pid $watcher"
fi

. scripts/drive_window.sh  # live windows on the monitor the user is not reading
LAUNCH="python3 scripts/live/launch.py --script tray-drive"
$LAUNCH --display-only || exit $?

command -v gdbus >/dev/null 2>&1 || {
    echo "tray-drive: FAIL -- gdbus is required (a prerequisite, docs/plan-iced-roadmap.md §3.4): sudo apt install libglib2.0-bin" >&2
    exit 1
}
python3 -c 'from gi.repository import GLib' 2>/dev/null || {
    echo "tray-drive: FAIL -- python3-gi is required to read gdbus's replies: sudo apt install python3-gi" >&2
    exit 1
}
if ! gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
        --method org.freedesktop.DBus.GetId >/dev/null 2>&1; then
    echo "tray-drive: skipped -- no session bus (exit 77; a skip is not a pass)"
    exit 77
fi
has=$(gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.NameHasOwner org.kde.StatusNotifierWatcher)
if [ "$has" != "(true,)" ]; then
    echo "tray-drive: skipped -- no org.kde.StatusNotifierWatcher on the session bus (exit 77; a skip is not a pass)"
    exit 77
fi

for leg in everything boundaries; do
    echo "== tray-drive: examples/tray, $leg =="
    case "$leg" in
    everything)
        cargo build --release -q -p tray --features tray,everything
        strategy=Everything ;;
    boundaries)
        cargo build --release -q -p tray --features tray
        strategy=Boundaries ;;
    esac
    # The drive's last step chooses Quit from the tray, so the process
    # ends by itself (`--expect-exit 0`).
    $LAUNCH --leg "$leg" --exe target/release/tray \
        --expect-strategy "$strategy" --expect-features "" --expect-exit 0 \
        -- python3 scripts/tray_drive.py {port}
done
echo "tray-drive: passed under both strategies (the item registered by the launched pid, its dbusmenu labels app.tray's, Open over dbusmenu [0] -> [0, k], Close -> [0] still registered, Lock flips Open's enabled, Quit exits 0 within 2 s)"
