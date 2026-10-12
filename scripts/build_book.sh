#!/usr/bin/env bash
# Build book/ and compile every Rust snippet in it.
#
# Two things this wrapper exists for, both measured against mdbook v0.5.4
# on 2026-09-10:
#
#   1. `mdbook build` EXITS 0 after logging `ERROR` -- a chapter whose
#      include names a missing file is reported and then shipped. So the
#      exit status is not the gate; the absence of ERROR on stderr is.
#   2. `mdbook test` needs `-L` pointing at rlibs, and `target/debug/deps`
#      cannot be used: it holds many libfulgurite-*.rlib built with
#      different features and rustdoc will not choose between them.
#      `just book-deps` builds exactly one of each into target/book.
#
# Snippets are edition-2021 because book.toml says so. Without that line
# rustdoc compiles them as 2015, where `use fulgurite::prelude::*`
# cannot resolve and the error names the wrong cause.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

if ! command -v mdbook >/dev/null; then
    echo "mdbook not installed: cargo binstall mdbook   (or cargo install mdbook)" >&2
    exit 1
fi

deps=target/book/debug/deps
if [ ! -d "$deps" ]; then
    echo "$deps missing -- run \`just book-deps\` first" >&2
    exit 1
fi

log=$(mktemp); trap 'rm -f "$log"' EXIT

mdbook build book 2> >(tee "$log" >&2)
if grep -q '^ *ERROR' "$log"; then
    echo "" >&2
    echo "mdbook logged an ERROR and exited 0 anyway. Treating it as a failure." >&2
    exit 1
fi

mdbook test book -L "$deps"
