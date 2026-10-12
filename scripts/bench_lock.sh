#!/usr/bin/env bash
# `just bench-lock <cmd>`: run a timing under THE bench lock, and say
# whether the machine was quiet (M43/T43.13; docs/plan-m38-fixes.md §7
# assigned it here, and M44, M45, M46 and M55 cite it).
#
# M38's reviewers ran this protocol by hand (docs/plan-deep-review.md §1):
# one timing at a time, `uptime` before and after, and a run taken while
# other work was compiling marked LOADED rather than quoted as if it were
# quiet. By hand it was only as good as whoever remembered it.
#
#   scripts/bench_lock.sh cargo run --release -p benches --bin grid
#
# 1. Takes an exclusive `flock` on one lock file, waiting for it -- so two
#    timings never overlap, from this checkout or any other worktree of the
#    same repository. The file is `<git common dir>/fulgurite-bench.lock`,
#    which every worktree shares; a lock under one worktree's `target/`
#    (the plan's first wording) would serialise nothing across worktrees,
#    which is where the parallel work is. FULGURITE_BENCH_LOCK overrides it.
# 2. Prints `uptime` before and after the command.
# 3. Marks the run LOADED when the one-minute load average, before OR
#    after, exceeds FULGURITE_BENCH_LOAD_FRACTION (default 0.25) of the
#    online cores. A quarter of 32 cores is load 8: past that, other work
#    competes for the cores and the boost clock, and a figure taken then
#    is evidence about the load, not the code.
#
# The last line is `bench-lock: quiet|LOADED (load1 before B, after A;
# bound F x N = L) exit S`. The exit status is the command's. LOADED does
# not fail the command: a loaded count is still a count (prefer counts),
# but a loaded time must be marked so where it is quoted.
set -uo pipefail
export LC_ALL=C
[ "$#" -gt 0 ] || { echo "usage: bench_lock.sh <command> [args...]" >&2; exit 2; }

if [ -n "${FULGURITE_BENCH_LOCK:-}" ]; then
    lock=$FULGURITE_BENCH_LOCK
else
    common=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || {
        echo "bench-lock: not in a git checkout; set FULGURITE_BENCH_LOCK" >&2; exit 2; }
    lock="$common/fulgurite-bench.lock"
fi
frac=${FULGURITE_BENCH_LOAD_FRACTION:-0.25}
cores=$(nproc)
load1() { cut -d' ' -f1 /proc/loadavg; }

exec 8>>"$lock"
if ! flock -n 8; then
    echo "bench-lock: waiting for $lock"
    flock 8
fi
echo "bench-lock: holding $lock"
before=$(load1)
echo "bench-lock: before: $(uptime)"
"$@"
status=$?
after=$(load1)
echo "bench-lock: after:  $(uptime)"
verdict=$(awk -v b="$before" -v a="$after" -v f="$frac" -v n="$cores" \
    'BEGIN { l = f * n; print ((b > l || a > l) ? "LOADED" : "quiet"), l }')
echo "bench-lock: ${verdict% *} (load1 before $before, after $after; bound $frac x $cores = ${verdict#* }) exit $status"
exit $status
