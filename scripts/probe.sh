#!/usr/bin/env bash
# Capability probe (M43/T43.13; docs/plan-m38-fixes.md §7 assigned it
# here). A recipe that needs a tool this machine may not have calls this
# FIRST, so a missing tool is a skip -- exit 77, M43 §2.2's code -- and
# not a failure halfway through with an error that names something else,
# nor a silent pass.
#
#   scripts/probe.sh thumb        the thumbv7em-none-eabihf std/core for the
#                                 pinned toolchain (rust-toolchain.toml)
#   scripts/probe.sh qemu         qemu-system-arm on PATH
#   scripts/probe.sh miri         `cargo +nightly miri` (just miri-reload)
#   scripts/probe.sh a b ...      all of them; 77 if any is absent
#
# Prints `probe: <cap> present (<evidence>)` or `probe: <cap> absent --
# skipped (77): <how to get it>`. What a 77 means to a caller is the
# caller's policy: `just ci` counts it as a failure unless
# LUMEN_ALLOW_SKIP=1, and records it as a skip either way.
#
# **The thumb probe asks the toolchain the build will use**, not rustup's
# default: `rustc --print sysroot` from the repository root resolves
# rust-toolchain.toml (and honours RUSTUP_TOOLCHAIN), and the target is
# present when that sysroot holds a `libcore` for it. `rustup target list
# --installed` answers for whichever toolchain rustup thinks is active.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
[ "$#" -gt 0 ] || { echo "usage: probe.sh thumb|qemu|miri ..." >&2; exit 2; }

THUMB=thumbv7em-none-eabihf
absent=0
for cap in "$@"; do
    case $cap in
        thumb)
            sysroot=$(rustc --print sysroot 2>/dev/null) || sysroot=""
            core=$(ls "$sysroot/lib/rustlib/$THUMB/lib/"libcore-*.rlib 2>/dev/null | head -1)
            if [ -n "$sysroot" ] && [ -n "$core" ]; then
                echo "probe: thumb present ($THUMB in $sysroot)"
            else
                echo "probe: thumb absent -- skipped (77): no $THUMB libcore in ${sysroot:-<no rustc>}; rustup target add $THUMB"
                absent=1
            fi ;;
        qemu)
            if q=$(command -v qemu-system-arm); then
                echo "probe: qemu present ($q, $(qemu-system-arm --version 2>/dev/null | head -1))"
            else
                echo "probe: qemu absent -- skipped (77): qemu-system-arm is not on PATH"
                absent=1
            fi ;;
        miri)
            if v=$(cargo +nightly miri --version 2>/dev/null); then
                echo "probe: miri present ($v)"
            else
                echo "probe: miri absent -- skipped (77): rustup +nightly component add miri"
                absent=1
            fi ;;
        *)  echo "probe: unknown capability '$cap'" >&2; exit 2 ;;
    esac
done
[ "$absent" -eq 0 ] || exit 77
