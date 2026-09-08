# Reshape a BENCH5 harness's `key<TAB>value` output (harnesses/bench5/common.md)
# into competitors.sh's row format, carrying the UNIT the key actually has.
# `rss.*` is kilobytes and `frame.sustained_ms` is milliseconds; printing
# either as microseconds is the kind of quiet unit error a table never
# recovers from.
NF == 2 {
    key = $1; val = $2
    unit = "us"
    if (key ~ /^rss\./)                 unit = "kB"
    else if (key ~ /_ms$/)              unit = "ms"
    else if (key ~ /^(frame\.count|nodes\.)/) unit = "count"
    else if (val !~ /^-?[0-9.]+$/)      unit = ""
    printf "%s\t%s\t%s\t%s\t%s\t%s\n", arm, wl, n, key, val, unit
}
