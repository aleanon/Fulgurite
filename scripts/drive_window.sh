# Sourced by the scripts that open live windows: put those windows on a
# monitor the user is not reading, and do not let them take focus.
#
# `LUMEN_WINDOW_POS=x,y` and `LUMEN_WINDOW_INACTIVE=1` are read by
# lumen-static-winit when it creates its window (both are requests to the
# window manager). Set either yourself to override; `LUMEN_WINDOW_POS=`
# (empty) leaves placement to the window manager.
#
# The default is the first monitor `xrandr` does not mark primary, 40 px
# in from its corner. With one monitor, or no `xrandr`, placement is left
# alone and only the focus request applies.
if [ -z "${LUMEN_WINDOW_POS+set}" ] && command -v xrandr >/dev/null 2>&1; then
    # " 1: +HDMI-0 1920/531x1080/299+0+0  HDMI-0" -- the geometry field's
    # trailing +X+Y is the monitor's origin; "*" marks the primary.
    _lumen_pos=$(xrandr --listmonitors 2>/dev/null | awk '
        NR > 1 && $2 !~ /\*/ {
            n = split($3, g, "+")
            if (n >= 3) { print g[n-1] + 40 "," g[n] + 40; exit }
        }')
    if [ -n "$_lumen_pos" ]; then
        export LUMEN_WINDOW_POS="$_lumen_pos"
    fi
    unset _lumen_pos
fi
export LUMEN_WINDOW_INACTIVE="${LUMEN_WINDOW_INACTIVE:-1}"
