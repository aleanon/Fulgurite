#!/usr/bin/env bash
# Resource usage — every arm, every fulgurite configuration, ONE method
# per metric, so the rows are comparable with each other and with the
# numbers already in `docs/results-competitive.md` §3-§5.
#
# NOTHING here is a frame-cost benchmark. Frame cost lives in `run.sh` and
# `arms/frame`; this script measures what an application *occupies*:
# bytes on disk, bytes resident, time to a window, and allocations.
#
# ## The methods, one per metric
#
# 1. **Idle RSS** — delegated *verbatim* to the sibling harness's
#    `measure.sh rss`: median of 3 launches, `VmRSS` from
#    `/proc/<pid>/status` 4 s after launch. It is not reimplemented here.
#    Reimplementing it is how two tables stop being comparable, and this
#    number already appears in `docs/results-competitive.md` §3 and
#    `docs/results-gpu.md` §5 taken that way.
#
# 2. **Peak RSS** — `/usr/bin/time -v`'s "Maximum resident set size",
#    which is `getrusage`'s `ru_maxrss`: the same high-water mark
#    `/proc/<pid>/status` reports as `VmHWM`, read after exit instead of
#    before it. **Median of 3 runs.** Two workloads, two columns, because
#    they answer different questions and one number would answer neither:
#      * `peak(frame)` — the arm's standalone frame-workload binary, run
#        to completion at N rows (default 3000). This is the gap idle RSS
#        cannot speak to: idle RSS says nothing about what a 3000-row
#        list costs.
#      * `peak(idle)`  — the windowed app, SIGTERMed at 4 s. The
#        high-water mark behind the idle-RSS row, from its own launch.
#
# 3. **Distributable size** — delegated *verbatim* to `measure.sh size`:
#    the executable plus the sum of the unique shared objects `ldd`
#    resolves for it, excluding the C runtime every binary on the system
#    already has. Not reimplemented here, for the reason in (1).
#
# 4. **Startup** — `t_window`: wall time from `exec` until the window
#    manager lists a top-level window owned by that PID (`wmctrl -lp`,
#    matched on PID so no per-arm window-title guessing is needed).
#    **This is not time-to-first-frame and is not reported as one.**
#    See NOTES/STARTUP, printed under every table this script produces.
#
# 5. **Steady-state allocation** — `crates/fulgurite-shell/tests/alloc_gate.rs`,
#    a counting `#[global_allocator]` in its own test binary, asserting
#    the integer 0 per `Host::pump` (`docs/results-allocation.md`).
#    Reported pass/fail, not as a number, because the number is 0 and a
#    budget would only encode a bug. No competitor arm can be measured
#    this way; see NOTES/ALLOC.
#
# ## Honest failure
#
# Every arm in the table below is printed on every run. An arm that will
# not build, or has no binary for a metric, prints `n/a` and the reason
# in the notes column. There is no path through this script that drops a
# row silently — that is the difference between "we measured this and it
# is bad" and "we did not measure this".
#
# ## Usage
#
#   ./resources.sh --list                 # the arm table; measure nothing
#   ./resources.sh --dry-run              # every command it would run
#   ./resources.sh                        # everything
#   ./resources.sh --only fulgurite,fulgurite-gpu
#   ./resources.sh --skip-build           # measure what is already staged
#   ./resources.sh --reuse                # take competitors.sh's `build/`
#                                         # artifacts where it has one, so the
#                                         # two reports quote the same bytes
#   ./resources.sh --rows 3000 --iters 200
#   ./resources.sh --out /path/to/results
#
# Results land in `$OUT`: `resources.tsv` (machine-readable, one row per
# arm), `report.md` (the tables, as printed), `env.txt` (machine,
# session, toolchain, git rev) and `logs/` (every build and run log).
#
# ## Serialise it
#
# Every number here is contended: RSS moves with page cache pressure and
# `t_window` moves with anything else on the CPU. Run this on an
# otherwise idle machine, and never beside a compile.

set -uo pipefail
export LC_ALL=C   # printf %f is locale-sensitive; a comma decimal breaks it

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"

# The sibling harness that defines the size and idle-RSS methods. Called,
# not copied: a second implementation is a second method.
MEASURE_SH="${LUMEN_MEASURE_SH:-/mnt/Utvikling-linux/Lumen/benches-competitive/harnesses/measure.sh}"
LUMEN_ROOT="${LUMEN_ROOT:-/mnt/Utvikling-linux/Lumen}"
LUMEN_HARNESS="$LUMEN_ROOT/benches-competitive/harnesses"
LUMEN_APPS="$LUMEN_HARNESS/apps"

OUT="$HERE/out/resources-$(date +%Y%m%d-%H%M%S)"
ONLY=""
SKIP_BUILD=0
REUSE=0
DRY=0
LIST=0
ROWS=3000
ITERS=200
RUNS=3             # launches per metric; median. Matches measure.sh's 3.
IDLE_SETTLE=4      # measure.sh's read point; reused for peak(idle) only
WINDOW_TIMEOUT=20  # seconds to wait for an arm's window before giving up

usage() { sed -n '2,72p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit "${1:-0}"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --out)        OUT="$2"; shift 2 ;;
    --only)       ONLY="$2"; shift 2 ;;
    --rows)       ROWS="$2"; shift 2 ;;
    --iters)      ITERS="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --reuse)      REUSE=1; shift ;;
    --dry-run)    DRY=1; shift ;;
    --list)       LIST=1; shift ;;
    -h|--help)    usage 0 ;;
    *) echo "unknown option: $1" >&2; usage 2 ;;
  esac
done

mkdir -p "$OUT/bin" "$OUT/logs" "$OUT/build"
LOGS="$OUT/logs"
TSV="$OUT/resources.tsv"
ID="-"

# ---------------------------------------------------------------------------
# The arm table
#
# Eight `|`-separated fields:
#   1 id      row label, and the name `--only` matches
#   2 kind    app       windowed, idles: size, idle RSS, peak(idle), t_window
#             frame     headless, runs to completion: size, peak(frame)
#             bin       headless, runs to completion, takes no workload args
#             firmware  linked sections, no window, its own units
#             none      nothing to measure; field 8 says why
#   3 family  fulgurite | competitor    (which block it prints in)
#   4 build   shell command, eval'd. Empty = nothing to build.
#   5 exe     the built executable. `=<id>` means "the binary row <id>
#             already built", for runtime-fallback rows that must be the
#             same bytes.
#   6 env     environment prefix for every run of this arm; `-` for none
#   7 args    arguments for a `frame` arm; `-` for none
#   8 note    why it cannot be measured. NON-EMPTY MEANS THE ROW PRINTS
#             AS n/a WITH THIS REASON AND NOTHING IS BUILT.
#
# ONE PACKAGE PER `cargo` INVOCATION, DELIBERATELY. Cargo unifies features
# across a multi-package selection, which is the trap that made this
# repo's default and lean arms report byte-identical sizes until each was
# built alone (`docs/results-competitive.md` §4) — and the same trap has
# since flattened four of the sibling checkout's differential `lumen-*`
# arms, which were built in one invocation on 2026-09-05 and still link
# GTK they asked not to have. Every configuration below is its own
# invocation, and its executable is copied into `$OUT/bin/<id>` the
# moment it is built, because the next invocation overwrites the same
# path in `target/release`.
# ---------------------------------------------------------------------------

CW="--manifest-path $HERE/Cargo.toml --release"        # benches-competitive ws
CR="--manifest-path $REPO/Cargo.toml --release"        # the framework ws
CA="--manifest-path $LUMEN_APPS/Cargo.toml --release"  # sibling apps ws
CL="--manifest-path $LUMEN_ROOT/Cargo.toml --release"  # sibling root ws

arm_table() {
cat <<TABLE
fulgurite|app|fulgurite|cargo build $CW -p fulgurite-app|$HERE/target/release/fulgurite-app|-|-|
fulgurite-lean|app|fulgurite|cargo build $CW -p fulgurite-lean|$HERE/target/release/fulgurite-lean|-|-|
fulgurite-gpu|app|fulgurite|cargo build $CW -p fulgurite-app --features fulgurite-winit/gpu|$HERE/target/release/fulgurite-app|-|-|
fulgurite-gpu-sw|app|fulgurite||=fulgurite-gpu|FULGURITE_BACKEND=software|-|
fulgurite-observe|app|fulgurite|cargo build $CW -p fulgurite-app --features fulgurite/observe|$HERE/target/release/fulgurite-app|-|-|
fulgurite-frame|frame|fulgurite|cargo build $CW -p fulgurite-frame|$HERE/target/release/fulgurite-frame|-|$ROWS $ITERS|
fulgurite-frame-observe|frame|fulgurite|cargo build $CW -p fulgurite-frame --features observe|$HERE/target/release/fulgurite-frame|-|$ROWS $ITERS|
fulgurite-raster-gpu|bin|fulgurite|cargo build $CW -p raster-scaling|$HERE/target/release/raster-scaling|-|-|
fulgurite-rebuild|none|fulgurite|||-|-|Retention is a TYPE PARAMETER, not a feature — deliberately, so both modes stay available in one binary (crates/fulgurite-shell/src/lib.rs, \`Host<A, R: Retention = Incremental>\`). No build flag can therefore separate them on the size axis at all, and an RSS row needs a binary that runs exactly ONE mode: \`run()\` takes \`Host<A>\` so every windowed arm is Incremental, arms/frame constructs \`Incremental\` directly, and arms/stages runs \`Rebuild\` and \`Incremental\` in the same process. docs/results-competitive.md §5's \`Rebuild\` row is the \`+render (full)\` timing of the Incremental arm, not a separate build.
fulgurite-embedded|firmware|fulgurite|cargo build $CR -p embedded --bin firmware --features firmware --target thumbv7em-none-eabihf|$REPO/target/thumbv7em-none-eabihf/release/firmware|-|-|
iced|app|competitor|cargo build $CA -p iced-app|$LUMEN_APPS/target/release/iced-app|-|-|
xilem|app|competitor|cargo build $CA -p xilem-app|$LUMEN_APPS/target/release/xilem-app|-|-|
slint|app|competitor|cargo build $CA -p slint-app|$LUMEN_APPS/target/release/slint-app|-|-|
lumen|app|competitor|cargo build $CA -p lumen-counter-app|$LUMEN_APPS/target/release/lumen-counter-app|-|-|
lumen-lean|app|competitor|cargo build $CA -p lumen-lean-app|$LUMEN_APPS/target/release/lumen-lean-app|-|-|
lumen-noa11y|app|competitor|cargo build $CA -p lumen-noa11y-app|$LUMEN_APPS/target/release/lumen-noa11y-app|-|-|
lumen-nodict|app|competitor|cargo build $CA -p lumen-nodict-app|$LUMEN_APPS/target/release/lumen-nodict-app|-|-|
lumen-stubtext|app|competitor|cargo build $CA -p lumen-stubtext-app|$LUMEN_APPS/target/release/lumen-stubtext-app|LUMEN_STUBTEXT_WINDOW=1|-|
lumen-defaulttext|bin|competitor|cargo build $CA -p lumen-defaulttext-app|$LUMEN_APPS/target/release/lumen-defaulttext-app|-|-|
lumen-obs|app|competitor|cargo build $CL -p lumen-obs-app|$LUMEN_ROOT/target/release/lumen-obs-app|-|-|
lumen-obs-dev|app|competitor|cargo build $CL -p lumen-obs-app --features obs|$LUMEN_ROOT/target/release/lumen-obs-app|-|-|
qt-app|app|competitor|cmake -S $LUMEN_HARNESS/qt-app -B $OUT/build/qt-app -DCMAKE_BUILD_TYPE=Release && cmake --build $OUT/build/qt-app|$OUT/build/qt-app/qt_app|-|-|
gtk-app|app|competitor|cc -O2 -o $OUT/build/gtk-app $LUMEN_HARNESS/gtk/app.c \$(pkg-config --cflags --libs gtk4)|$OUT/build/gtk-app|-|-|
qt-frame|frame|competitor|cmake -S $LUMEN_HARNESS/qt -B $OUT/build/qt -DCMAKE_BUILD_TYPE=Release && cmake --build $OUT/build/qt|$OUT/build/qt/qt_frame|QT_QPA_PLATFORM=offscreen|$ROWS $ITERS|
gtk-frame|frame|competitor|cc -O2 -o $OUT/build/gtk-frame $LUMEN_HARNESS/gtk/frame.c \$(pkg-config --cflags --libs gtk4)|$OUT/build/gtk-frame|-|$ROWS $ITERS|
lumen-frame|frame|competitor|cargo build --manifest-path $LUMEN_ROOT/benches-competitive/Cargo.toml --release --bin probe_bench5|$LUMEN_ROOT/benches-competitive/target/release/probe_bench5|-|$ROWS $ITERS point patch|
iced-frame|none|competitor|||-|-|No standalone frame-workload binary. iced's 3000-row number in docs/results-competitive.md §1 comes from a criterion bench in the sibling checkout, and criterion's own harness allocates and resizes inside the measurement window, so a peak-RSS figure taken from it would be criterion's as much as iced's.
xilem-frame|none|competitor|||-|-|Same as iced-frame: masonry is driven through \`masonry_testing::TestHarness\` under criterion, which docs/results-competitive.md §1 already flags as an upper bound rather than a like-for-like.
slint-frame|none|competitor|||-|-|No frame-workload arm exists for Slint in either checkout — it appears in the idle-RSS and size tables only.
TABLE
}

# ---------------------------------------------------------------------------
# Plumbing
# ---------------------------------------------------------------------------

KIDS=()
cleanup() { for p in "${KIDS[@]:-}"; do [ -n "$p" ] && kill "$p" 2>/dev/null; done; }
trap cleanup EXIT INT TERM

have() { command -v "$1" >/dev/null 2>&1; }
NA="n/a"   # the one place a metric is allowed to be blank

# `competitors.sh` (the frame-cost runner beside this one) already builds
# eight of these arms and stages each one in `build/`, one package per
# `cargo` invocation, for the same feature-unification reason this script
# does. Under `--reuse` its artifact is taken instead of rebuilding —
# the two reports then quote **the same bytes**, which is the only way
# their size rows can be reconciled rather than merely compared.
reuse_path() {
  local id="$1" b="$HERE/build"
  case "$id" in
    fulgurite)               echo "$b/fulgurite-app" ;;
    fulgurite-lean)          echo "$b/fulgurite-lean" ;;
    fulgurite-frame)         echo "$b/fulgurite-frame" ;;
    fulgurite-frame-observe) echo "$b/fulgurite-frame-observe" ;;
    qt-app)                     echo "$b/qt-app/qt_app" ;;
    gtk-app)                    echo "$b/gtk_app" ;;
    qt-frame)                   echo "$b/qt-frame/qt_frame" ;;
    gtk-frame)                  echo "$b/gtk_frame" ;;
    *)                          echo "" ;;
  esac
}

# ---------------------------------------------------------------------------
# Metrics
# ---------------------------------------------------------------------------

# 3. Distributable size — the inherited definition, called not copied.
m_size() {
  local exe="$1"
  [ -x "$exe" ] || { echo "$NA"; return 1; }
  "$MEASURE_SH" size "$exe" 2>/dev/null || echo "$NA"
}

# 1. Idle RSS — the inherited method, called not copied. `measure.sh rss`
#    reports `0.0 MB` when there is no live process at the 4 s read, which
#    is a crash or an immediate exit, not a small footprint. It is turned
#    into a visible n/a here rather than quoted as a very good number.
m_idle_rss() {
  local exe="$1" envp="$2"
  local v
  if [ "$envp" = "-" ]; then
    v=$("$MEASURE_SH" rss "$exe" 2>/dev/null)
  else
    # shellcheck disable=SC2086
    v=$(env $envp "$MEASURE_SH" rss "$exe" 2>/dev/null)
  fi
  case "$v" in
    ""|"0.0 MB") echo "$NA" ;;
    *) echo "$v" ;;
  esac
}

# 2. Peak RSS. `/usr/bin/time -v`'s Maximum RSS is `ru_maxrss`, the same
#    high-water mark `/proc/<pid>/status` calls `VmHWM`, read after exit
#    instead of before it. Median of $RUNS.
#
#    `$1` exe, `$2` seconds before SIGTERM (0 = run to completion),
#    `$3` env prefix, rest = workload args.
#
#    `time` outside, `timeout` inside, and not the other way round: with
#    `timeout` outermost the SIGTERM lands on `time`, which then prints
#    no report at all. Nesting them this way works because Linux's
#    `wait4` fills the reaped child's rusage from RUSAGE_BOTH, so
#    `timeout`'s own waited-for child is folded in — verified once
#    against a process with a known allocation before this was relied on.
m_peak() {
  local exe="$1" limit="$2" envp="$3"; shift 3
  [ -x "$exe" ] || { echo "$NA"; return 1; }
  have /usr/bin/time || { echo "$NA"; return 1; }
  local vals=() i v log cmd=() e=()
  # Built as one array rather than a string prefix: an empty prefix
  # expanded with "${p[@]:-}" becomes an empty *argument*, which the
  # shell then tries to execute.
  [ "$envp" = "-" ] || { read -r -a e <<< "$envp"; cmd=(env "${e[@]}"); }
  cmd+=(/usr/bin/time -v)
  [ "$limit" = 0 ] || cmd+=(timeout -s TERM "$limit")
  cmd+=("$exe" "$@")
  for i in $(seq "$RUNS"); do
    log="$LOGS/$ID.peak.$i.log"
    "${cmd[@]}" >"$LOGS/$ID.run.$i.log" 2>"$log"
    v=$(awk -F': *' '/Maximum resident set size/{print $2}' "$log")
    [ -n "${v:-}" ] && vals+=("$v")
  done
  [ "${#vals[@]}" -ge 1 ] || { echo "$NA"; return 1; }
  printf '%s\n' "${vals[@]}" | sort -n | \
    awk -v n="${#vals[@]}" 'NR==int((n+1)/2){printf "%.1f MB", $1/1024}'
}

# 4. Startup — `t_window`, NOT time to first frame. See NOTES/STARTUP.
#    Wall time from exec until the window manager lists a top-level
#    window owned by this PID. PID-matched via `wmctrl -lp`, so no arm
#    needs its title guessed and no arm is advantaged by setting one.
#    One `wmctrl` spawn per poll; that spawn is the resolution of the
#    number, it is measured, and it is printed in env.txt — a resolution
#    nobody states is a resolution nobody can argue with.
STARTUP_RES="unmeasured"
m_startup_resolution() {
  have wmctrl || { STARTUP_RES="$NA (no wmctrl)"; return 1; }
  local a b
  wmctrl -lp >/dev/null 2>&1   # warm: the first spawn pays for page cache
  a=$EPOCHREALTIME; wmctrl -lp >/dev/null 2>&1; b=$EPOCHREALTIME
  STARTUP_RES=$(awk -v a="$a" -v b="$b" 'BEGIN{printf "%.1f ms/poll", (b-a)*1000}')
}

m_startup() {
  local exe="$1" envp="$2"
  [ -x "$exe" ] || { echo "$NA"; return 1; }
  have wmctrl || { echo "$NA (no wmctrl)"; return 1; }
  [ -n "${DISPLAY:-}" ] || { echo "$NA (no \$DISPLAY)"; return 1; }
  local vals=() i t0 pid found now
  for i in $(seq "$RUNS"); do
    t0=$EPOCHREALTIME
    if [ "$envp" = "-" ]; then
      "$exe" >/dev/null 2>&1 &
    else
      # shellcheck disable=SC2086
      env $envp "$exe" >/dev/null 2>&1 &
    fi
    pid=$!
    KIDS+=("$pid")
    found=""
    while :; do
      if wmctrl -lp 2>/dev/null | awk -v p="$pid" '$3==p{f=1} END{exit !f}'; then
        found=$EPOCHREALTIME; break
      fi
      kill -0 "$pid" 2>/dev/null || break           # died before mapping
      now=$EPOCHREALTIME
      awk -v a="$t0" -v b="$now" -v l="$WINDOW_TIMEOUT" 'BEGIN{exit !(b-a>l)}' && break
    done
    [ -n "$found" ] && \
      vals+=("$(awk -v a="$t0" -v b="$found" 'BEGIN{printf "%.1f", (b-a)*1000}')")
    kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
  done
  [ "${#vals[@]}" -ge 1 ] || { echo "$NA (no window within ${WINDOW_TIMEOUT}s)"; return 1; }
  printf '%s\n' "${vals[@]}" | sort -n | \
    awk -v n="${#vals[@]}" 'NR==int((n+1)/2){printf "%.1f ms", $1}'
}

# 5. Steady-state allocation. fulgurite only; see NOTES/ALLOC.
m_alloc_gate() {
  local log="$LOGS/alloc_gate.log"
  if [ "$DRY" = 1 ]; then
    printf '  $ cargo test %s -p fulgurite-shell --test alloc_gate\n' "$CR" >&2
    echo "(dry run)"; return 0
  fi
  if eval "cargo test $CR -p fulgurite-shell --test alloc_gate" >"$log" 2>&1; then
    echo "**0** — gate passes (6 tests, both retention modes)"
  else
    echo "GATE FAILED — see $log"
  fi
}

# ---------------------------------------------------------------------------
# The embedded row. Its own units: this profile has no window, so RSS,
# `t_window` and `ldd` mean nothing for it. Bytes of linked sections and
# the `.bss` shape check are what it has (`docs/results-embedded.md`).
# ---------------------------------------------------------------------------
FW_ROW=""
measure_firmware() {
  local build="$1" bin="$2"
  if ! rustup target list --installed 2>/dev/null | grep -qx thumbv7em-none-eabihf; then
    FW_ROW="$NA | $NA | $NA | $NA | $NA | target thumbv7em-none-eabihf not installed (\`rustup target add thumbv7em-none-eabihf\`)"
    return
  fi
  if [ "$DRY" = 1 ]; then
    printf '  $ %s\n  $ size -A %s\n  $ %s\n' "$build" "$bin" "$REPO/scripts/check_bss.sh" >&2
    FW_ROW="(dry run) | | | | |"
    return
  fi
  if [ "$SKIP_BUILD" = 0 ]; then
    eval "$build" >"$LOGS/firmware.build.log" 2>&1 || {
      FW_ROW="$NA | $NA | $NA | $NA | $NA | build failed, see $LOGS/firmware.build.log"; return; }
  fi
  [ -f "$bin" ] || { FW_ROW="$NA | $NA | $NA | $NA | $NA | no linked binary at $bin"; return; }
  local sizes text rodata data bss total bssnote
  sizes=$(size -A "$bin")
  text=$(awk '$1==".text"{print $2}'   <<< "$sizes")
  rodata=$(awk '$1==".rodata"{print $2}' <<< "$sizes")
  data=$(awk '$1==".data"{print $2}'   <<< "$sizes")
  bss=$(awk '$1==".bss"{print $2}'     <<< "$sizes")
  total=$(( ${text:-0} + ${rodata:-0} + ${data:-0} ))
  if [ -x "$REPO/scripts/check_bss.sh" ]; then
    bssnote=$("$REPO/scripts/check_bss.sh" 2>&1 | tail -1)
  else
    bssnote="scripts/check_bss.sh not found"
  fi
  FW_ROW="${text:-0} B | ${rodata:-0} B | ${data:-0} B | ${bss:-0} B | $total B | $bssnote"
}

# ---------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------

selected() {
  [ -z "$ONLY" ] && return 0
  case ",$ONLY," in (*",$1,"*) return 0 ;; esac
  return 1
}

if [ "$LIST" = 1 ]; then
  printf '%-28s %-9s %-13s %s\n' ID KIND FAMILY "BUILD / REASON IT CANNOT BE MEASURED"
  while IFS='|' read -r id kind fam build exe envp args note; do
    [ -n "$id" ] || continue
    printf '%-28s %-9s %-13s %s\n' "$id" "$kind" "$fam" "${note:-$build}"
  done < <(arm_table)
  rmdir "$OUT/bin" "$OUT/logs" "$OUT/build" "$OUT" 2>/dev/null
  exit 0
fi

# Preflight. A missing tool disables its metric everywhere and says so
# once, rather than producing a column of unexplained blanks.
MISSING=""
[ -x "$MEASURE_SH" ] || MISSING="$MISSING\n  - $MEASURE_SH (size + idle RSS: both are delegated to it)"
have /usr/bin/time   || MISSING="$MISSING\n  - /usr/bin/time (peak RSS)"
have wmctrl          || MISSING="$MISSING\n  - wmctrl (t_window)"
have ldd             || MISSING="$MISSING\n  - ldd (size)"
have cmake           || MISSING="$MISSING\n  - cmake (the Qt arms)"
have pkg-config      || MISSING="$MISSING\n  - pkg-config (the GTK arms)"
[ -n "${DISPLAY:-}" ] || MISSING="$MISSING\n  - \$DISPLAY (every windowed arm: idle RSS, peak(idle), t_window)"
if [ -n "$MISSING" ]; then
  # shellcheck disable=SC2059
  printf "preflight: not available on this machine:$MISSING\n" >&2
  echo "preflight: the metrics above report n/a with that reason; the run continues." >&2
fi

m_startup_resolution

{
  echo "date            $(date -Is)"
  echo "host            $(uname -srm)"
  echo "cpu             $(awk -F': ' '/model name/{print $2; exit}' /proc/cpuinfo)"
  echo "mem             $(awk '/MemTotal/{printf "%.0f GB", $2/1048576}' /proc/meminfo)"
  echo "governor        $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo unknown)"
  echo "session         XDG_SESSION_TYPE=${XDG_SESSION_TYPE:-unset} DISPLAY=${DISPLAY:-unset} WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-unset}"
  echo "wm              $(wmctrl -m 2>/dev/null | awk -F': ' '/^Name/{print $2}' || echo unknown)"
  echo "rustc           $(rustc -V 2>/dev/null)"
  echo "git             $(git -C "$REPO" rev-parse --short HEAD 2>/dev/null), $(git -C "$REPO" status --porcelain 2>/dev/null | wc -l) dirty file(s)"
  echo "measure.sh      $MEASURE_SH"
  echo "rows/iters      $ROWS / $ITERS"
  echo "runs per metric $RUNS (median)"
  echo "t_window res    $STARTUP_RES"
} > "$OUT/env.txt"

printf 'id\tkind\tfamily\tbuild\tsize\tidle_rss\tpeak_idle\tpeak_frame\tt_window\tnotes\n' > "$TSV"

emit() {  # id kind family build size idle peak_idle peak_frame t_window notes
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$@" >> "$TSV"
}

measure_arm() {
  local id="$1" kind="$2" fam="$3" build="$4" exe="$5" envp="$6" args="$7"
  ID="$id"
  local size=$NA idle=$NA peaki=$NA peakf=$NA tw=$NA status="ok"
  local staged="$OUT/bin/$id"

  case "$exe" in
    "="*) local parent="${exe#=}"
          # The runtime-fallback row must be the SAME bytes as its
          # parent — that is the whole claim it exists to test — so it is
          # copied, never rebuilt.
          if [ -x "$OUT/bin/$parent" ]; then cp -f "$OUT/bin/$parent" "$staged"
          else status="row \`$parent\` produced no binary to reuse"; fi ;;
    "")   status="no executable declared" ;;
    *)    local ru=""
          [ "$REUSE" = 1 ] && ru=$(reuse_path "$id")
          if [ -n "$ru" ] && [ -x "$ru" ]; then
            echo "== reusing competitors.sh artifact for $id: $ru" >&2
            cp -f "$ru" "$staged"
            build=""; exe="$ru"
          fi
          if [ "$SKIP_BUILD" = 0 ] && [ -n "$build" ]; then
            echo "== building $id" >&2
            if [ "$DRY" = 1 ]; then
              printf '  $ %s\n' "$build" >&2
            elif ! eval "$build" >"$LOGS/$id.build.log" 2>&1; then
              status="BUILD FAILED — $LOGS/$id.build.log: $(tail -3 "$LOGS/$id.build.log" 2>/dev/null | tr '\n' ' ' | cut -c1-200)"
            fi
          fi
          if [ "$status" = ok ] && [ "$DRY" = 0 ]; then
            if [ -f "$exe" ]; then cp -f "$exe" "$staged"
            elif [ -x "$staged" ]; then :          # --skip-build, already staged
            else status="no executable at $exe"; fi
          fi ;;
  esac

  if [ "$DRY" = 1 ]; then
    printf '  $ %s size %s\n' "$MEASURE_SH" "$staged" >&2
    case "$kind" in
      app)   printf '  $ %s rss %s\n  $ /usr/bin/time -v timeout -s TERM %s %s   (x%s, median)\n  $ launch %s, poll `wmctrl -lp` for its pid   (x%s, median)\n' \
               "$MEASURE_SH" "$staged" "$IDLE_SETTLE" "$staged" "$RUNS" "$staged" "$RUNS" >&2 ;;
      frame) printf '  $ /usr/bin/time -v %s %s   (x%s, median)\n' "$staged" "$args" "$RUNS" >&2 ;;
      bin)   printf '  $ /usr/bin/time -v %s   (x%s, median)\n' "$staged" "$RUNS" >&2 ;;
    esac
    emit "$id" "$kind" "$fam" "dry-run" "-" "-" "-" "-" "-" "-"
    return
  fi

  if [ "$status" = ok ]; then
    size=$(m_size "$staged")
    case "$kind" in
      app)
        idle=$(m_idle_rss "$staged" "$envp")
        peaki=$(m_peak "$staged" "$IDLE_SETTLE" "$envp")
        tw=$(m_startup "$staged" "$envp")
        peakf="$NA (windowed arm: no frame-workload binary)"
        if [ "$idle" = "$NA" ]; then
          status="built, but no live process at the ${IDLE_SETTLE}s read — see $LOGS/$id.peak.1.log"
        fi
        ;;
      frame)
        # shellcheck disable=SC2086
        peakf=$(m_peak "$staged" 0 "$envp" $args)
        idle="$NA (headless arm: no window to idle in)"
        peaki="$NA (headless arm)"
        tw="$NA (headless arm)"
        ;;
      bin)
        peakf=$(m_peak "$staged" 0 "$envp")
        peakf="$peakf (own default workload, not $ROWS rows)"
        idle="$NA (headless arm: prints and exits)"
        peaki="$NA (headless arm)"
        tw="$NA (headless arm)"
        ;;
    esac
  fi

  emit "$id" "$kind" "$fam" \
    "$([ "$status" = ok ] && echo ok || echo failed)" \
    "$size" "$idle" "$peaki" "$peakf" "$tw" \
    "$([ "$status" = ok ] && echo "-" || echo "$status")"
}

# A `=parent` row is the same bytes as another row, so selecting it
# without its parent would silently measure nothing. Say so instead.
while IFS='|' read -r id kind fam build exe envp args note; do
  case "$exe" in "="*)
    selected "$id" && ! selected "${exe#=}" && \
      echo "warning: --only selects $id but not ${exe#=}, whose binary it reuses; it will report n/a" >&2 ;;
  esac
done < <(arm_table)

while IFS='|' read -r id kind fam build exe envp args note; do
  [ -n "$id" ] || continue
  selected "$id" || continue
  if [ -n "$note" ]; then
    emit "$id" "$kind" "$fam" "not-applicable" "$NA" "$NA" "$NA" "$NA" "$NA" "$note"
    continue
  fi
  case "$kind" in
    firmware) measure_firmware "$build" "$exe" ;;
    none)     emit "$id" "$kind" "$fam" "not-applicable" "$NA" "$NA" "$NA" "$NA" "$NA" \
                "row declared unmeasurable but carries no reason — fix the arm table" ;;
    *)        measure_arm "$id" "$kind" "$fam" "$build" "$exe" "$envp" "$args" ;;
  esac
done < <(arm_table)

if selected fulgurite; then ALLOC=$(m_alloc_gate); else ALLOC="$NA (not selected)"; fi

# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------
report() {
  echo "# Resource usage — $(date -Is)"
  echo
  sed 's/^/    /' "$OUT/env.txt"
  echo
  for fam in fulgurite competitor; do
    case "$fam" in
      fulgurite) echo "## fulgurite configurations" ;;
      competitor)   echo "## Competitor arms" ;;
    esac
    echo
    echo "| arm | build | distributable size | idle RSS | peak RSS (idle, ${IDLE_SETTLE} s) | peak RSS (frame, $ROWS rows) | t_window | notes |"
    echo "|---|---|---|---|---|---|---|---|"
    awk -F'\t' -v f="$fam" 'NR>1 && $3==f {printf "| %s | %s | %s | %s | %s | %s | %s | %s |\n", $1,$4,$5,$6,$7,$8,$9,$10}' "$TSV"
    echo
  done

  echo "## Embedded profile — its own units"
  echo
  echo "No window, no \`ldd\`, no RSS: this row is a linked \`thumbv7em-none-eabihf\`"
  echo "image, the profile \`docs/results-embedded.md\` measures. Flash total is"
  echo "\`.text + .rodata + .data\`; \`.bss\` is RAM claimed before \`main\`."
  echo
  echo "| .text | .rodata | .data | .bss | flash total | .bss/.data check |"
  echo "|---|---|---|---|---|---|"
  echo "| ${FW_ROW:-$NA | $NA | $NA | $NA | $NA | not selected} |"
  echo
  echo "## Steady-state allocation"
  echo
  echo "| arm | allocations per steady-state frame |"
  echo "|---|---|"
  echo "| fulgurite (five tree shapes, both retention modes) | $ALLOC |"
  echo "| every other arm | $NA — see NOTES/ALLOC |"
  echo
  notes
}

notes() {
cat <<'NOTES'
## NOTES

### NOTES/STARTUP — what `t_window` is, and what it is not

`t_window` is wall time from `exec` to the window manager listing a
top-level window owned by that PID. It covers process start, dynamic
linking, toolkit initialisation and window creation. **It stops before
the first frame is painted**, and its resolution is one `wmctrl` spawn,
recorded in `env.txt` for the run that produced the table.

Time to *first frame* is not measured here, and is not estimated from
`t_window`, because it cannot be measured fairly across these arms with
the instruments available:

- The arms present through different paths — softbuffer's `XPutImage`,
  wgpu's swapchain, Qt's raster backend, GTK's GL renderer — so no
  syscall and no X request is a common definition of "presented".
- Reading it off the X server (poll the window's pixels until they stop
  being uniform) has a resolution set by an `xwd` round trip, tens of
  milliseconds against startups of the same order; and it mistakes
  "painted a uniform background" for "has not painted yet".
- The only fair definition is a timestamp the application takes at its
  own first present. That is a source change in **every** arm, and the
  competitor arms belong to the sibling checkout.

So: `t_window` for the part that is comparable, and no number at all for
the part that is not. An arm's `t_window` includes opening a GPU adapter
where it opens one, which is most of what separates the GPU-backed arms
here — the same effect `docs/results-gpu.md` §5 measures on RSS.

### NOTES/ALLOC — why only one arm has that column

The fulgurite entry comes from `crates/fulgurite-shell/tests/alloc_gate.rs`:
a counting `#[global_allocator]` in its own test binary, asserting **0**
allocations per `Host::pump` after warm-up, over five tree shapes and
both retention modes (`docs/results-allocation.md`).

That instrument does not transfer:

- It is in-process and Rust-only: it counts `GlobalAlloc::alloc` calls
  inside a binary that links the framework under test. There is no such
  binary for Qt, for GTK, or for Slint's C++ core.
- It needs a **steady-state frame loop that runs without a window**. Of
  the competitor arms only the Qt and GTK *frame* harnesses and Lumen's
  `probe_bench5` have one; the iced, Xilem and Slint arms are windowed
  apps with no workload loop, or criterion benchmarks whose own harness
  allocates inside the measurement window.
- The nearest cross-language substitute — an `LD_PRELOAD` shim counting
  `malloc`/`calloc`/`realloc` — measures a **different unit**: libc
  allocation calls, including ones a `GlobalAlloc` count never sees and
  excluding reuse inside an allocator's own free lists. Putting it in
  the same column as the gate's integer would be comparing two things
  because they are both numbers.

This is an asymmetry, not a win: fulgurite asserts an integer, and
the other arms are unmeasured on this axis. The script says so rather
than filling the cell.

### NOTES/METHOD — what is inherited, what is new here

Inherited verbatim, by calling the sibling script instead of
reimplementing it: **distributable size** and **idle RSS**
(`harnesses/measure.sh`). Those two columns are directly comparable with
`docs/results-competitive.md` §3-§4 and `docs/results-gpu.md` §5.

New here, and therefore with no prior numbers to be compared against:
**peak RSS** (both columns), **`t_window`**, and the per-configuration
breakout of the `gpu`, `observe` and lean builds as *resource* rows
rather than frame-cost rows.

`competitors.sh`, beside this file, is the *frame-cost* runner for the
same arms, and it also reports size and idle RSS for eight of them
through the same `measure.sh`. Where the two overlap they must agree;
run this one with `--reuse` and they are measuring the same bytes rather
than two builds that ought to be the same. Where they disagree, the
build is the thing to look at first — one package per `cargo`
invocation, which both scripts do and the 2026-09-05 hand run did not.

### NOTES/PROFILE — three workspaces, three release profiles

`benches-competitive/Cargo.toml` here and the sibling
`harnesses/apps/Cargo.toml` both set `lto = true`, `codegen-units = 1`,
`strip = true`, `panic = "abort"` — that is what makes their size rows
comparable. Two rows are built elsewhere and are **not** size-comparable
with them, which is recorded here rather than left for a reader to trip
over:

- `lumen-obs` / `lumen-obs-dev` build from the sibling **root**
  workspace, whose profile is `lto = "thin"` with no `panic = "abort"`,
  because that package inherits `workspace.package` keys the apps
  workspace does not define.
- `lumen-frame` (`probe_bench5`) builds from the sibling
  `benches-competitive` workspace, whose release profile sets
  `debug = true`. Its **size** column is therefore mostly debug info;
  its peak-RSS column is not affected, since debug sections are not
  resident.

For the same reason this script builds the `gpu` and `observe`
configurations from *this* `benches-competitive` workspace rather than
from `examples/minimal` in the framework workspace, which is where
`docs/results-gpu.md` §5's 5.29 MB / 12.26 MB pair came from: that
workspace sets no `[profile.release]` at all, so those binaries are
unstripped and un-LTO'd. Same code, different link, and the two size
figures must not be put in one column.

### NOTES/MOBILE — there is no mobile row, and none is invented

This repository has no mobile target. Searched: no `android`, `ios` or
mobile `aarch64` target anywhere in `crates/`, `examples/`, `fixtures/`,
`justfile`, `scripts/` or `.github/workflows/ci.yml`; no `cdylib` except
the hot-reload fixture; no `winit::platform::*` use; no NDK, Gradle,
Xcode or `cargo-apk` configuration. CI is `ubuntu-latest` plus one
non-blocking `macos-latest` job, which is desktop Apple Silicon.
`docs/competitive-feature-gaps.md` states the position outright:
fulgurite is desktop-only, plus a `no_std` core with no driver.

What *does* exist is machine-level, not project-level: `rustup` on this
box has `aarch64-linux-android`, `armv7-linux-androideabi`,
`x86_64-linux-android` and `wasm32-unknown-unknown` installed — from
other work, since nothing in this repository names any of them. There is
no `.cargo/config.toml` here or in `$HOME` pointing at an NDK linker, so
even a bare `cargo build --target aarch64-linux-android` of the core has
no linker to finish with.

A mobile number would therefore be a fabrication, and desktop RSS cannot
be scaled into one. The smallest honest mobile measurement would need,
at minimum: an NDK (or Xcode) toolchain and a linker configured for the
target; a `cdylib` entry point plus the `android-activity` glue winit
requires on Android, which nothing here has; the same label-and-button
app packaged as an APK or `.app`; and a device or emulator to read RSS
from — and `dumpsys meminfo` / Instruments do not report the same
quantity as `/proc/<pid>/status` here, so even then the number would
open its own column, not join one of these. That is a porting task, not
a measurement task.
NOTES
}

report | tee "$OUT/report.md"
echo >&2
echo "wrote $TSV" >&2
echo "      $OUT/report.md, $OUT/env.txt, $LOGS/" >&2
