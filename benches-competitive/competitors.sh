#!/usr/bin/env bash
# One runner for every competitive arm behind `docs/results-competitive.md`.
#
# Restored 2026-09-08. The arms themselves are unchanged; this script only
# builds them, records whether each build SUCCEEDED, and runs one at a chosen
# N and workload with a parseable line per number. It exists because the
# 2026-09-05 run was assembled by hand from five different invocations across
# two repositories, and three of those invocations have a trap in them that a
# hand-run reproduces only if the person remembers it (see TRAPS below).
#
#   ./competitors.sh list                       what exists, and whether it built
#   ./competitors.sh build [arm ...]            build all arms, or the named ones
#   ./competitors.sh run <arm> <n> <workload>   one arm, one size, one workload
#   ./competitors.sh size <arm>                 distributable size (measure.sh)
#   ./competitors.sh rss  <arm>                 idle RSS       (measure.sh)
#   ./competitors.sh plan <n> <workload>        print the commands, run nothing
#
# Output of `run` is TSV, one row per number, so a sweep is `for n in ...`:
#
#   arm <TAB> workload <TAB> n <TAB> metric <TAB> value <TAB> unit
#
# `run` REFUSES an arm whose artifact is missing rather than skipping it: a
# sweep with a silently absent arm is exactly the failure the harness comments
# in the sibling repo were all written to prevent — an opponent that does
# nothing reports a flattering number and no error.
#
# SOURCING THIS FILE RUNS NOTHING. It defines functions and returns.
#
# ---------------------------------------------------------------------------
# TRAPS this script exists to mechanise
#
# 1. LOCALE. `harnesses/gtk/frame.c` never calls `setlocale(LC_ALL, "C")` —
#    unlike `harnesses/bench5/gtk.c` and both Qt harnesses, which do. On this
#    machine `LC_NUMERIC=nb_NO.UTF-8`, so it prints `32,4 us`, and the decimal
#    comma silently truncates under the `([\d.]+) us` regex in `run.sh`.
#    Every path here exports `LC_ALL=C`.
# 2. CARGO FEATURE UNIFICATION. `cargo build --release` over the whole
#    benches-competitive workspace unifies features across the selected
#    packages, and `fulgurite-lean` comes out byte-identical to
#    `fulgurite-app` (measured 2026-09-08: 2 842 560 vs 2 842 576 built
#    together; 2 843 024 vs 2 683 712 built one at a time). The 159 KB
#    clipboard delta in `docs/results-competitive.md` §4 only exists if each
#    is built alone. `build` uses one `cargo` invocation per size arm.
# 3. BINARY PATH COLLISION. The default and `--features observe` builds of
#    `fulgurite-frame` write the SAME path. `build` copies each out to
#    `build/` immediately, as `harnesses/bench5/run.sh` does for its two
#    Lumen binaries.
# 4. SKIP_RENDER_TESTS. `benches/vs_toolkits.rs` sets it in its own `main`, so
#    it cannot drift — exported here as well, and asserted by that file's own
#    `skip_render_tests_stops_before_the_gpu` test.
set -uo pipefail
export LC_ALL=C

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
BUILD=$HERE/build
SIB=${SIB:-/mnt/Utvikling-linux/Lumen/benches-competitive}
STATUS=$BUILD/status.tsv
# Criterion sample count. 20 is what `docs/results-competitive.md` §1 used
# ("Criterion, 20 samples"); changing it changes comparability, not just noise.
SAMPLES=${SAMPLES:-20}
# Iteration count for the C/C++/fulgurite arms: min of 200 after 20
# warm-ups, the shape every harness in `harnesses/` was written to.
ITERS=${ITERS:-200}

# arm -> artifact that must exist before `run` will touch it.
declare -A ARTIFACT=(
  [fulgurite]="$BUILD/fulgurite-frame"
  [fulgurite-observe]="$BUILD/fulgurite-frame-observe"
  [gtk]="$BUILD/gtk_frame"
  [qt]="$BUILD/qt-frame/qt_frame"
  [gtk-bench5]="$BUILD/gtk_bench5"
  [qt-bench5]="$BUILD/qt-bench5/bench5_qt"
  [iced]="$SIB/target/release/deps/.vs_iced.stamp"
  [lumen]="$SIB/target/release/deps/.vs_iced.stamp"
  [lumen-memoized]="$SIB/target/release/deps/.vs_iced.stamp"
  [masonry]="$SIB/target/release/deps/.vs_toolkits.stamp"
  [lumen-semantics]="$SIB/target/release/deps/.vs_toolkits.stamp"
  [egui]="$SIB/target/release/deps/.vs_egui.stamp"
  [masonry-raw]="$SIB/target/release/masonry-raw-probe"
  # size/RSS-only arms
  [fulgurite-app]="$BUILD/fulgurite-app"
  [fulgurite-lean]="$BUILD/fulgurite-lean"
  [gtk-app]="$BUILD/gtk_app"
  [qt-app]="$BUILD/qt-app/qt_app"
  [iced-app]="$SIB/harnesses/apps/target/release/iced-app"
  [xilem-app]="$SIB/harnesses/apps/target/release/xilem-app"
  [slint-app]="$SIB/harnesses/apps/target/release/slint-app"
  [lumen-app]="$SIB/harnesses/apps/target/release/lumen-counter-app"
)

# arm -> which workloads it can actually run.
declare -A WORKLOADS=(
  [fulgurite]="point churn"        # one binary prints both
  [fulgurite-observe]="point churn"
  [gtk]="point"                       # harnesses/gtk/frame.c has no churn mode
  [qt]="point"                        # harnesses/qt/frame.cpp likewise
  [gtk-bench5]="point churn"
  [qt-bench5]="point churn"
  [iced]="point churn"
  [lumen]="point churn"
  [lumen-memoized]="point"
  [masonry]="point"                   # vs_toolkits has no churn group
  [lumen-semantics]="point"
  [egui]="point"
  [masonry-raw]="point"               # diagnostic: prints no time
)

FRAME_ARMS="fulgurite fulgurite-observe gtk qt gtk-bench5 qt-bench5 iced lumen lumen-memoized masonry lumen-semantics egui masonry-raw"
SIZE_ARMS="fulgurite-app fulgurite-lean gtk-app qt-app iced-app xilem-app slint-app lumen-app"

die() { echo "competitors.sh: $*" >&2; exit 2; }

# --- build ----------------------------------------------------------------

record() { printf '%s\t%s\t%s\n' "$1" "$2" "$(date -Is)" >> "$STATUS"; }

build_fulgurite() {
  # Trap 3: same output path for both feature sets, so copy immediately.
  cargo build --locked --release -q --manifest-path "$HERE/Cargo.toml" -p fulgurite-frame \
    && cp "$HERE/target/release/fulgurite-frame" "$BUILD/fulgurite-frame" \
    && record fulgurite ok || { record fulgurite FAILED; return 1; }
  cargo build --locked --release -q --manifest-path "$HERE/Cargo.toml" -p fulgurite-frame --features observe \
    && cp "$HERE/target/release/fulgurite-frame" "$BUILD/fulgurite-frame-observe" \
    && record fulgurite-observe ok || { record fulgurite-observe FAILED; return 1; }
}

build_size_arms() {
  # Trap 2: ONE package per cargo invocation, or the lean arm is not lean.
  local ok=0
  for p in fulgurite-app fulgurite-lean; do
    rm -f "$HERE/target/release/$p"
    if cargo build --locked --release -q --manifest-path "$HERE/Cargo.toml" -p "$p"; then
      cp "$HERE/target/release/$p" "$BUILD/$p"; record "$p" ok
    else record "$p" FAILED; ok=1; fi
  done
  return $ok
}

build_c_arms() {
  local ok=0 cflags
  cflags=$(pkg-config --cflags --libs gtk4) || { record gtk FAILED-no-gtk4; return 1; }
  # harnesses/gtk has no CMakeLists.txt or Makefile despite frame.c's header
  # comment claiming one; compiled directly instead.
  gcc -O2 -o "$BUILD/gtk_frame"  "$SIB/harnesses/gtk/frame.c"    $cflags && record gtk ok        || { record gtk FAILED; ok=1; }
  gcc -O2 -o "$BUILD/gtk_app"    "$SIB/harnesses/gtk/app.c"      $cflags && record gtk-app ok    || { record gtk-app FAILED; ok=1; }
  gcc -O2 -g -o "$BUILD/gtk_bench5" "$SIB/harnesses/bench5/gtk.c" $cflags && record gtk-bench5 ok || { record gtk-bench5 FAILED; ok=1; }
  cmake_arm qt        "$SIB/harnesses/qt"      "$BUILD/qt-frame"  Release        || ok=1
  cmake_arm qt-app    "$SIB/harnesses/qt-app"  "$BUILD/qt-app"    Release        || ok=1
  cmake_arm qt-bench5 "$SIB/harnesses/bench5"  "$BUILD/qt-bench5" RelWithDebInfo || ok=1
  return $ok
}

cmake_arm() {
  local arm=$1 src=$2 out=$3 type=$4
  if cmake -S "$src" -B "$out" -DCMAKE_BUILD_TYPE="$type" >"$out.log" 2>&1 \
     && cmake --build "$out" -j"$(nproc)" >>"$out.log" 2>&1; then
    record "$arm" ok
  else
    record "$arm" FAILED; echo "  see $out.log" >&2; return 1
  fi
}

build_sibling_benches() {
  local ok=0
  ( cd "$SIB" && cargo build --release -q --benches ) || { record iced FAILED; return 1; }
  # Criterion bench binaries are hash-named; stamp the ones that built so
  # `run` has something stable to require.
  for b in vs_iced vs_toolkits vs_egui; do
    if ls "$SIB"/target/release/deps/${b}-* >/dev/null 2>&1; then
      touch "$SIB/target/release/deps/.${b}.stamp"; record "$b" ok
    else record "$b" FAILED; ok=1; fi
  done
  # Diagnostic probe: masonry driven through the PUBLIC RenderRoot rather than
  # masonry_testing::TestHarness. Shares the sibling target dir so it reuses
  # the already-compiled masonry/vello.
  if ( cd "$HERE/probes/masonry-raw" && CARGO_TARGET_DIR="$SIB/target" cargo build --release -q ); then
    record masonry-raw ok
  else record masonry-raw FAILED; ok=1; fi
  return $ok
}

# One cargo invocation PER PACKAGE (T16.1).
#
# Not a style preference. Building this workspace in one `cargo build`
# unifies features across every member, and the four differential Lumen arms
# came out **byte-identical at 17,593,576 bytes** -- with the supposedly
# accessibility-free one still containing accesskit and atspi strings. Every
# size or RSS row derived from that build was measuring one binary four
# times.
#
# Reproduced on our own side before fixing it: one workspace build puts
# fulgurite-app and -lean 16 bytes apart; one package per invocation puts
# them 159,312 apart, which is the arboard stack the lean arm drops.
#
# This is the trap fulgurite's own justfile documents at length. The
# benchmark walked into it anyway, which is why the warning lives in the code
# that would repeat it rather than only in a commit message.
build_sibling_apps() {
  local ok=0 pkg
  for pkg in iced-app xilem-app slint-app lumen-counter-app \
             lumen-lean-app lumen-noa11y-app lumen-nodict-app; do
    if ( cd "$SIB/harnesses/apps" && cargo build --release -q -p "$pkg" ); then
      record "app:$pkg" ok
    else
      record "app:$pkg" FAILED; ok=1
    fi
  done
  return $ok
}

cmd_build() {
  mkdir -p "$BUILD"; : > "$STATUS"
  local arms=("$@"); local rc=0
  if [ ${#arms[@]} -eq 0 ]; then
    build_fulgurite || rc=1
    build_size_arms    || rc=1
    build_c_arms       || rc=1
    build_sibling_benches || rc=1
    build_sibling_apps || rc=1
  else
    for a in "${arms[@]}"; do
      case $a in
        fulgurite|fulgurite-observe) build_fulgurite || rc=1 ;;
        fulgurite-app|fulgurite-lean) build_size_arms || rc=1 ;;
        gtk|qt|gtk-app|qt-app|gtk-bench5|qt-bench5) build_c_arms || rc=1 ;;
        iced|lumen|lumen-memoized|masonry|lumen-semantics|egui|masonry-raw) build_sibling_benches || rc=1 ;;
        iced-app|xilem-app|slint-app|lumen-app) build_sibling_apps || rc=1 ;;
        *) die "unknown arm: $a" ;;
      esac
    done
  fi
  cmd_list
  return $rc
}

cmd_list() {
  printf '%-22s %-10s %s\n' ARM BUILT ARTIFACT
  for a in $FRAME_ARMS $SIZE_ARMS; do
    local p=${ARTIFACT[$a]:-}
    if [ -n "$p" ] && [ -e "$p" ]; then printf '%-22s %-10s %s\n' "$a" yes "$p"
    else printf '%-22s %-10s %s\n' "$a" NO "${p:-<none>}"; fi
  done
}

require_built() {
  # Two statements, not one: bash expands every word of a single `local`
  # before it assigns any of them, so `local a=$1 p=${ARTIFACT[$a]}` reads an
  # unset `a` and dies under `set -u`.
  local a=$1
  local p=${ARTIFACT[$a]:-}
  [ -n "$p" ] || die "unknown arm: $a"
  [ -e "$p" ] || die "arm '$a' is not built ($p is missing). Run: $0 build $a
                     Refusing to run: a sweep that silently drops an arm is worse than one that stops."
}

require_workload() {
  local a=$1 w=$2
  case " ${WORKLOADS[$a]:-} " in
    *" $w "*) : ;;
    *) die "arm '$a' has no '$w' workload (it has: ${WORKLOADS[$a]:-none}).
            Do NOT substitute another arm's number for it." ;;
  esac
}

# --- run ------------------------------------------------------------------

row() { printf '%s\t%s\t%s\t%s\t%s\tus\n' "$1" "$2" "$3" "$4" "$5"; }

# Criterion prints "  time:   [lo mid hi]" under the bench id. Keep the MEDIAN
# (mid), which is what criterion reports as the point estimate.
crit() {
  local arm=$1 workload=$2 n=$3 bench=$4 filter=$5
  # THE knob on this side of the comparison. The sibling harness's Cargo.toml
  # sets `default = ["observability"]`, which turns on `lumen-widgets/
  # dev-observability` — a per-frame ambient audit plus per-node ink collection
  # compiled straight into `Headless::pump()`, the function these benches time.
  # Its own comment says that is "the right default for `cargo run`, and the
  # wrong thing to compare against GTK and Qt, which run no such pass", and
  # offers `--no-default-features` for the release profile. An unqualified
  # `cargo bench` takes the dev build, so LUMEN_OBS defaults to `on` here to
  # reproduce that; `LUMEN_OBS=off` is the fair-to-Lumen setting and the one
  # matching fulgurite's own arm, whose `observe` feature is default-OFF.
  # (The sibling's own BENCH5 table, harnesses/bench5/results-2026-08-30.tsv,
  # is where the size of that difference is already written down.)
  local feat=()
  [ "${LUMEN_OBS:-on}" = off ] && feat=(--no-default-features)
  # stderr is kept, not discarded (T16.3). The sibling's `sizes()` helper
  # *panics* on a malformed BENCH_SIZES, and `2>/dev/null` turned that into an
  # empty run with a zero exit. A benchmark that refused to start now says so.
  local log; log=$(mktemp)
  ( cd "$SIB" && SKIP_RENDER_TESTS=1 BENCH_SIZES=$n BENCH_CHURN_SIZES=$n \
      cargo bench -q "${feat[@]}" --bench "$bench" -- --noplot --sample-size "$SAMPLES" "$filter" ) \
      >"$log" 2>&1
  local rc=$?
  if [ $rc -ne 0 ]; then
    echo "competitors: $arm: cargo bench exited $rc" >&2
    sed -n '/panicked\|^error/,+3p' "$log" >&2
    rm -f "$log"; return 1
  fi
  awk -v arm="$arm" -v wl="$workload" -v n="$n" -f "$HERE/crit.awk" <"$log"
  rc=$?
  rm -f "$log"
  return $rc
}

cmd_run() {
  local arm=${1:?arm} n=${2:?n} workload=${3:-point}
  require_built "$arm"; require_workload "$arm" "$workload"
  case $arm in
    fulgurite|fulgurite-observe)
      local bin=${ARTIFACT[$arm]}
      # One invocation prints BOTH workloads; select the rows asked for.
      "$bin" "$n" "$ITERS" | awk -v arm="$arm" -v wl="$workload" -v n="$n" '
        {
          line=$0
          sub(/[ \t]*\([0-9]+ rows\)[ \t]*$/,"",line)
          # "fulgurite/<metric>   <value> us"
          if (match(line, /^fulgurite\//)) {
            rest=substr(line, RLENGTH+1)
            if (match(rest, /[0-9.]+ us$/)) {
              val=substr(rest, RSTART); sub(/ us$/,"",val)
              metric=substr(rest, 1, RSTART-1); gsub(/[ \t]+$/,"",metric)
              isall = (metric ~ /^ALL rows: /)
              if (wl=="churn" && !isall) next
              if (wl=="point" && isall)  next
              sub(/^ALL rows: /,"",metric)
              printf "%s\t%s\t%s\t%s\t%s\tus\n", arm, wl, n, metric, val
            }
          }
        }' ;;
    gtk)
      DISPLAY=${DISPLAY:-:0} "$BUILD/gtk_frame" "$n" "$ITERS" \
      | awk -v n="$n" '{ sub(/[ \t]*\([0-9]+ rows\)[ \t]*$/,"");
          if (match($0,/[0-9.]+ us$/)) { v=substr($0,RSTART); sub(/ us$/,"",v);
            m=substr($0,1,RSTART-1); sub(/^gtk\//,"",m); gsub(/[ \t]+$/,"",m);
            printf "gtk\tpoint\t%s\t%s\t%s\tus\n", n, m, v } }' ;;
    qt)
      "$BUILD/qt-frame/qt_frame" "$n" "$ITERS" 2>/dev/null \
      | awk -v n="$n" '{ if (match($0,/[0-9.]+ us$/)) { v=substr($0,RSTART); sub(/ us$/,"",v);
            m=substr($0,1,RSTART-1); sub(/^qt\//,"",m); gsub(/[ \t]+$/,"",m);
            printf "qt\tpoint\t%s\t%s\t%s\tus\n", n, m, v } }' ;;
    gtk-bench5)
      DISPLAY=${DISPLAY:-:0} "$BUILD/gtk_bench5" "$n" "$ITERS" "$workload" \
      | awk -F'\t' -v arm=gtk-bench5 -v n="$n" -v wl="$workload" -f "$HERE/bench5.awk" ;;
    qt-bench5)
      "$BUILD/qt-bench5/bench5_qt" "$n" "$ITERS" "$workload" 2>/dev/null \
      | awk -F'\t' -v arm=qt-bench5 -v n="$n" -v wl="$workload" -f "$HERE/bench5.awk" ;;
    iced)            crit iced           "$workload" "$n" vs_iced     "$([ "$workload" = churn ] && echo churn_frame || echo build_frame)/iced/${n}_rows" ;;
    lumen)           crit lumen          "$workload" "$n" vs_iced     "$([ "$workload" = churn ] && echo churn_frame || echo build_frame)/lumen/${n}_rows" ;;
    lumen-memoized)  crit lumen-memoized point       "$n" vs_iced     "build_frame/lumen_memoized/${n}_rows" ;;
    masonry)         crit masonry        point       "$n" vs_toolkits "frame/masonry/${n}_rows" ;;
    lumen-semantics) crit lumen+semantics point      "$n" vs_toolkits 'frame/lumen\+semantics/'"${n}_rows" ;;
    egui)            crit egui           point       "$n" vs_egui     "build_frame/egui/${n}_rows" ;;
    masonry-raw)
      # Diagnostic only: proves masonry runs headlessly WITHOUT TestHarness and
      # that its AccessKit pass is an opt-in (`WindowEvent::EnableAccessTree`).
      # Prints counts, never a time.
      "$SIB/target/release/masonry-raw-probe" "$n" a11y
      "$SIB/target/release/masonry-raw-probe" "$n" no-a11y ;;
  esac
}

# --- size / RSS -----------------------------------------------------------

cmd_size() { local a=${1:?arm}; require_built "$a"; printf '%s\tsize\t%s\n' "$a" "$("$SIB/harnesses/measure.sh" size "${ARTIFACT[$a]}")"; }
cmd_rss()  { local a=${1:?arm}; require_built "$a"; printf '%s\trss\t%s\n'  "$a" "$("$SIB/harnesses/measure.sh" rss  "${ARTIFACT[$a]}")"; }

# --- plan (prints, runs nothing) ------------------------------------------

cmd_plan() {
  local n=${1:?n} wl=${2:-point}
  echo "# N=$n workload=$wl  ITERS=$ITERS SAMPLES=$SAMPLES  (LC_ALL=C)"
  for a in $FRAME_ARMS; do
    case " ${WORKLOADS[$a]:-} " in *" $wl "*) echo "$0 run $a $n $wl" ;; *) echo "# $a: no '$wl' workload" ;; esac
  done
}

usage() { sed -n '2,40p' "${BASH_SOURCE[0]}"; }

main() {
  local c=${1:-}; shift || true
  case $c in
    list)  cmd_list ;;
    build) cmd_build "$@" ;;
    run)   cmd_run "$@" ;;
    size)  cmd_size "$@" ;;
    rss)   cmd_rss "$@" ;;
    plan)  cmd_plan "$@" ;;
    ""|-h|--help|help) usage ;;
    *) die "unknown command: $c (try: list build run size rss plan)" ;;
  esac
}

# Sourcing defines the functions and does nothing else. Running is deliberate.
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
  main "$@"
fi
