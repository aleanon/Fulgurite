#!/usr/bin/env bash
# M83/T83.0 (docs/plan-gpu-extension.md): lumen-static-shell names no
# window and no GPU. D1(b) moves `Backend` and `Frame` into the shell
# (T83.1) on the premise that the shell can hold the trait without
# pulling in what implements it -- winit, softbuffer, wgpu. Nothing
# failed to compile if that premise broke: a dependency edge is
# invisible until someone counts it, so this counts it.
#
# Two builds, because T83.1's `backend` module lives behind the shell's
# `truetype` (its `FrameSource`, `Screen` and `FontStack` are there): the
# default (`default = []`) and `--features truetype`. Each is read with
# `cargo tree -e normal --locked --offline` and its unique-crate count
# printed the way K2 counts (`--prefix none`, `(*)` stripped, `sort -u`;
# docs/plan-iced-roadmap.md K2). A crate of the winit, softbuffer or wgpu
# families in either build fails by name. A `cargo tree` that itself
# fails -- a stale lock under `--locked`, a crate not in the offline
# cache -- is reported as that, never as a pass.
#
# No compile: `cargo tree` reads the lock and the manifests only, so this
# runs in `just static` before anything builds.
set -uo pipefail
cd "$(dirname "$0")/.."
export LC_ALL=C

FORBIDDEN='^(winit|softbuffer|wgpu)(-[a-z0-9-]+)? v'
fail=0

for build in default truetype; do
    args=(-p lumen-static-shell -e normal --locked --offline --prefix none)
    [ "$build" = truetype ] && args+=(--features truetype)
    if ! out=$(cargo tree "${args[@]}" 2>&1); then
        echo "check_shell_headless: cargo tree failed for the $build build -- not a pass:" >&2
        printf '%s\n' "$out" | tail -5 >&2
        exit 1
    fi
    crates=$(printf '%s\n' "$out" | sed 's/ (\*)$//' | sort -u)
    n=$(printf '%s\n' "$crates" | grep -c .)
    bad=$(printf '%s\n' "$crates" | grep -E "$FORBIDDEN" | sed 's/ (.*$//' | tr '\n' ' ')
    if [ -n "$bad" ]; then
        echo "check_shell_headless: the shell's $build build depends on ${bad% } -- the backend seam (D1 b) needs a shell with no window or GPU crate" >&2
        fail=1
    else
        echo "check_shell_headless: $build build, $n crates, none of winit/softbuffer/wgpu"
    fi
done

exit $fail
