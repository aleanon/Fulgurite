# Reshape criterion's console output into competitors.sh's row format.
#
# Criterion prints the benchmark id on its own line and then an indented
# "time:   [lo mid hi]" with a unit per number. The MID is criterion's point
# estimate and is the number `docs/results-competitive.md` §1 quotes.
#
# The id is carried into the metric column on purpose: a filter that misses
# then shows up as a row naming the wrong benchmark, instead of being silently
# attributed to the arm the caller asked for.
/\/[0-9]+_rows[ \t]*$/ { id = $0; sub(/^[^A-Za-z]*/, "", id); next }
/time:  *\[/ {
    if (id == "") next
    gsub(/.*\[/, ""); gsub(/\].*/, "")   # gsub on $0 re-splits the fields
    v = $3; u = $4                       # "lo unit mid unit hi unit"
    if (u == "ns") v /= 1000
    else if (u == "ms") v *= 1000
    else if (u == "s")  v *= 1000000
    printf "%s\t%s\t%s\t%s\t%.1f\tus\n", arm, wl, n, id, v
    id = ""
}
