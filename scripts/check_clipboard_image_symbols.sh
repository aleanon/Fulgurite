#!/usr/bin/env bash
# M88/T88.5 (docs/plan-platform-requests.md; K2, docs/plan-iced-roadmap.md):
# a window built without `clipboard-image` links no `image` crate.
#
# Modelled on scripts/check_agent_symbols.sh. Builds examples/showcase for
# release twice -- as it ships, then with `fulgurite-winit/clipboard-image`
# -- and counts symbols from the `image` crate in each with `nm -C`. The
# first must be 0; the second is the positive control and must not be, or
# the count is blind and would pass anything. The pattern is anchored at
# the crate root (`image::` at the start of a path, after a space or a
# `<`), because `fulgurite_shell::image::` is this workspace's own
# module and is not the crate counted.
#
# It also prints the stripped size of each and the delta beside K2's 3 MB
# bound, measured as docs/results-a11y-platform.md measured `a11y` (GNU
# strip on a copy). The feature is default-off either way; the size is a
# record (docs/results-tasks.md T88.5), not this script's verdict.
#
# Compiles two release builds, so it is not in `static`; `just
# clipboard-image-symbols` runs it, and `check-extra` includes it.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
out=target/clipboard-image-symbols
mkdir -p "$out"
count() { nm -C "$1" 2>/dev/null | grep -cE '(^|[ <(&*,])image::' || true; }
cargo build -q --release -p showcase
cp target/release/showcase "$out/plain"
cargo build -q --release -p showcase --features fulgurite-winit/clipboard-image
cp target/release/showcase "$out/image"
plain=$(count "$out/plain")
image=$(count "$out/image")
strip -o "$out/plain.stripped" "$out/plain"
strip -o "$out/image.stripped" "$out/image"
sp=$(stat -c %s "$out/plain.stripped")
si=$(stat -c %s "$out/image.stripped")
echo "clipboard-image-symbols: image:: symbols in the default showcase: $plain / bound 0 (clipboard-image build: $image)"
echo "clipboard-image-symbols: stripped showcase $sp B -> $si B with clipboard-image, delta $((si - sp)) B / K2 bound 3000000 B (default-off either way)"
if [ "$image" -eq 0 ]; then
    echo "clipboard-image-symbols: the clipboard-image build links no image:: symbols -- the count is blind" >&2
    exit 1
fi
if [ "$plain" -ne 0 ]; then
    echo "clipboard-image-symbols: a build without clipboard-image links the image crate" >&2
    exit 1
fi
