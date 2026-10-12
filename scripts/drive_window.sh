# Sourced by the scripts that open live windows: put those windows on a
# monitor the user is not reading, and do not let them take focus.
#
# `FULGURITE_WINDOW_POS=x,y` and `FULGURITE_WINDOW_INACTIVE=1` are read by
# fulgurite-winit when it creates its window (both are requests to the
# window manager). Set either yourself to override; `FULGURITE_WINDOW_POS=`
# (empty) leaves placement to the window manager.
#
# The default is the first monitor `xrandr` does not mark primary, 40 px
# in from its corner. With one monitor, or no `xrandr`, placement is left
# alone and only the focus request applies.
#
# scripts/live/launch.py applies this file itself for a script that did
# not source it, and then checks it: a leg whose window opened off the
# monitor asked for fails. A script that starts a window any other way
# (Xephyr, a selftest's stranger, `just a11y-cost`) sources this file.
if [ -z "${FULGURITE_WINDOW_POS+set}" ] && command -v xrandr >/dev/null 2>&1; then
    # " 1: +HDMI-0 1920/531x1080/299+0+0  HDMI-0" -- the geometry field's
    # trailing +X+Y is the monitor's origin; "*" marks the primary.
    _fulgurite_pos=$(xrandr --listmonitors 2>/dev/null | awk '
        NR > 1 && $2 !~ /\*/ {
            n = split($3, g, "+")
            if (n >= 3) { print g[n-1] + 40 "," g[n] + 40; exit }
        }')
    if [ -n "$_fulgurite_pos" ]; then
        export FULGURITE_WINDOW_POS="$_fulgurite_pos"
    fi
    unset _fulgurite_pos
fi
export FULGURITE_WINDOW_INACTIVE="${FULGURITE_WINDOW_INACTIVE:-1}"
