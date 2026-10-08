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
#   scripts/probe.sh xclip        xclip on PATH -- REQUIRED, see below
#   scripts/probe.sh nextest      cargo-nextest -- REQUIRED: `dev-shape` and
#                                 `gpu` run under it (docs/results-ci-time.md)
#   scripts/probe.sh a b ...      all of them; 1 if a required one is
#                                 absent, else 77 if any is absent
#
# Prints `probe: <cap> present (<evidence>)` or `probe: <cap> absent --
# skipped (77): <how to get it>`. What a 77 means to a caller is the
# caller's policy: `just ci` counts it as a failure unless
# LUMEN_ALLOW_SKIP=1, and records it as a skip either way.
#
# **`xclip` is a prerequisite, not a capability** (owner decision D7,
# docs/plan-iced-roadmap.md §3.4): the plan has decided the reference
# machine has it, for `just tasks-drive`'s independent read of the
# primary selection (M88/T88.4). Its absence prints `probe: xclip absent
# -- required (D7): sudo apt install xclip` and exits **1, never 77**, so
# no caller can count it as a skip.
#
# **The thumb probe asks the toolchain the build will use**, not rustup's
# default: `rustc --print sysroot` from the repository root resolves
# rust-toolchain.toml (and honours RUSTUP_TOOLCHAIN), and the target is
# present when that sysroot holds a `libcore` for it. `rustup target list
# --installed` answers for whichever toolchain rustup thinks is active.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
[ "$#" -gt 0 ] || { echo "usage: probe.sh thumb|qemu|miri|xclip|nextest ..." >&2; exit 2; }

THUMB=thumbv7em-none-eabihf
absent=0
required=0
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
        nextest)
            if v=$(cargo nextest --version 2>/dev/null | head -1); then
                echo "probe: nextest present ($v)"
            else
                echo "probe: nextest absent -- required: cargo binstall cargo-nextest (or cargo install cargo-nextest --locked)"
                required=1
            fi ;;
        xclip)
            if x=$(command -v xclip); then
                echo "probe: xclip present ($x, $(xclip -version 2>&1 | head -1))"
            else
                echo "probe: xclip absent -- required (D7): sudo apt install xclip"
                required=1
            fi ;;
        *)  echo "probe: unknown capability '$cap'" >&2; exit 2 ;;
    esac
done
[ "$required" -eq 0 ] || exit 1
[ "$absent" -eq 0 ] || exit 77
