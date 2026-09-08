# Reshape criterion's console output into competitors.sh's row format.
#
# The MID of "time: [lo mid hi]" is criterion's point estimate and is the
# number `docs/results-competitive.md` §1 quotes.
#
# **Criterion prints the id in one of two shapes, and this parser handled
# only one of them (T16.3).** When the benchmark id fits the console's label
# column it shares a line with the timing:
#
#     frame/masonry/100_rows  time:   [32.788 us 32.842 us 32.906 us]
#
# and when it does not, it gets a line of its own and the timing is indented
# under it:
#
#     churn_frame/lumen/100_rows
#                             time:   [1.5410 ms 1.5481 ms 1.5550 ms]
#
# The old version matched only the second, so every arm whose id happened to
# be short — masonry, egui — parsed to nothing and `competitors.sh run
# masonry` exited 0 printing nothing at all, while the arm underneath was
# working and reporting 32.6 us. A silent skip in a script written to refuse
# rather than skip. Both shapes are handled now, and END fails loudly when a
# run produced no rows at all.
#
# The id is carried into the metric column on purpose: a filter that misses
# then shows up as a row naming the wrong benchmark, instead of being
# silently attributed to the arm the caller asked for.

# awk has no setlocale, so it inherits the environment's — and its number
# *parsing* is locale-sensitive, not just its printing. Under a comma-decimal
# locale "1.5481" reads as 1 and a 1548.1 us row silently becomes 1000.0.
# competitors.sh exports LC_ALL=C on every path, but this file is runnable on
# its own, so it checks rather than trusts. `gtk_init()` adopting the
# environment locale is the same hazard one layer down (T16.3).
BEGIN {
    # The probe must convert a *string*, not evaluate a source literal:
    # awk lexes its own program in the C locale whatever the environment
    # says, so `1.5 + 0 != 1.5` is true everywhere and tests nothing. It is
    # the string-to-number conversion of input data that follows LC_NUMERIC,
    # which is exactly what a criterion line is.
    probe = "1.5"
    if (probe + 0 != 1.5) {
        print "crit.awk: run me under LC_ALL=C — this locale reads \"1.5\" as " \
            (probe + 0) > "/dev/stderr"
        exit 1
    }
}

function emit(id, lo_unit_mid, _v, _u) {
    # $0 has already been reduced to "lo unit mid unit hi unit".
    _v = $3; _u = $4
    if (_u == "ns") _v /= 1000
    else if (_u == "ms") _v *= 1000
    else if (_u == "s")  _v *= 1000000
    printf "%s\t%s\t%s\t%s\t%.1f\tus\n", arm, wl, n, id, _v
    rows++
}

# Shape 1: id and timing on one line.
/^[^ \t]*\/[0-9]+_rows[ \t]+time:[ \t]*\[/ {
    id = $1
    gsub(/.*\[/, ""); gsub(/\].*/, "")
    emit(id)
    id = ""
    next
}

# Shape 2: id alone on its line, timing indented beneath it.
/\/[0-9]+_rows[ \t]*$/ { id = $0; sub(/^[^A-Za-z]*/, "", id); next }
/time:  *\[/ {
    if (id == "") next
    gsub(/.*\[/, ""); gsub(/\].*/, "")
    emit(id)
    id = ""
}

END {
    if (rows == 0) {
        printf "competitors: %s/%s/%s produced no parseable timing\n", arm, wl, n \
            > "/dev/stderr"
        exit 1
    }
}
