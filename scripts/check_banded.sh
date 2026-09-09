#!/usr/bin/env bash
# M17/T17.4: a banded render must be byte-identical to a single-band one.
#
# The two cannot be compared inside one binary -- the split is a cargo
# feature, so it is compiled in or it is not. So the same scenes are
# rendered by both builds and their checksums diffed here, which is the
# only place the comparison can happen.
#
# `tests/banded.rs` prints one FNV-1a per (size, damage) case. A
# hard-coded golden would have to be regenerated on every legitimate
# rasteriser change and would rot into a rubber stamp; a cross-build
# diff cannot, because both sides move together or the gate fires.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
ck() {
    cargo test -q -p lumen-static-shell --features "$1" --test banded \
        -- --nocapture print_checksums 2>/dev/null \
      | grep -E '^[0-9]+x[0-9]+/' | sort
}

ck truetype          > "$tmp/single"
ck truetype,threads  > "$tmp/banded"

if [ ! -s "$tmp/single" ]; then
    echo "banded: the single-band build produced no checksums" >&2
    exit 1
fi

if diff -u "$tmp/single" "$tmp/banded" > "$tmp/diff"; then
    echo "banded: $(wc -l < "$tmp/single") scenes, byte-identical to the single-band render"
else
    echo "banded: the threaded render DIFFERS from the single-band one" >&2
    cat "$tmp/diff" >&2
    exit 1
fi
