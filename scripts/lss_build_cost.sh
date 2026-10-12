#!/usr/bin/env bash
# Kill rule K2 of docs/plan-lss.md (M61): what `include_lss!` adds to a
# warm incremental build of the crate that invokes it. A MEASUREMENT, not
# a gate -- a time is machine-dependent -- run as
#
#   just bench-lock ./scripts/lss_build_cost.sh [runs]
#
# and recorded in docs/results-lss.md. Two crates, the tally's three
# `include_lss!` (examples/counter-window/src/app.rs, `cargo build`) and
# the showcase's 49-rule mirror (examples/showcase/src/main.rs, `cargo
# build --tests`), each built `runs` times (8 by default) per arm after a
# `touch`, the arms interleaved so drift lands on all of them:
#
#   hand   each `include_lss!(..)` replaced by the rules it expands to,
#          written out -- **the sheet written by hand**, which is what K2
#          compares with;
#   const  the same, plus everything else the macro emits -- the tree
#          checks and the `include_bytes!` -- so `const - hand` is what
#          those cost rustc, and `macro - const` what running the macro
#          costs;
#   macro  the source as committed.
#
# The written-out forms are the macro's own output: the crate is expanded
# once with `-Zunpretty=expanded` (RUSTC_BOOTSTRAP, measurement only) and
# each expansion is cut out of it, so no arm is a hand-copy that could
# drift from what the macro emits. The assertions are re-spelled as
# `assert!` because the expanded form names an unstable `core` function.
#
# The source files are edited in place and restored on exit, whatever
# happens (trap), and checked against git afterwards.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"
export LC_ALL=C
RUNS=${1:-8}

tmp=$(mktemp -d)
FILES=(examples/counter-window/src/app.rs examples/showcase/src/main.rs)
for f in "${FILES[@]}"; do mkdir -p "$tmp/orig/$(dirname "$f")"; cp "$f" "$tmp/orig/$f"; done
restore() { for f in "${FILES[@]}"; do cp "$tmp/orig/$f" "$f"; done; }
trap 'restore; rm -rf "$tmp"' EXIT

# write_arms CRATE FILE EXPAND_ARGS... -- expand CRATE, and write FILE's
# `hand` and `const` forms to $tmp/<crate>.{hand,const}.rs.
write_arms() {
    local crate=$1 file=$2; shift 2
    RUSTC_BOOTSTRAP=1 cargo rustc -q -p "$crate" "$@" -- -Zunpretty=expanded > "$tmp/$crate.expanded.rs" 2>"$tmp/$crate.log" || {
        echo "lss-build-cost: expanding $crate failed:" >&2; tail -5 "$tmp/$crate.log" >&2; exit 1; }
    python3 - "$tmp/$crate.expanded.rs" "$file" "$tmp/$crate" <<'PY'
import re, sys
expanded, src_path, out = sys.argv[1], sys.argv[2], sys.argv[3]
x = open(expanded).read()
src = open(src_path).read()
HEAD = "const __FULGURITE_LSS: &[::fulgurite::style::Rule] ="
blocks = []
pos = 0
while True:
    i = x.find(HEAD, pos)
    if i < 0:
        break
    tail = re.compile(r"\n\s*__FULGURITE_LSS\n").search(x, i)
    rules = x[i:tail.start()]
    start = x.rfind("const _: &[u8] =", 0, i)
    # The tracking line and the tree checks, verbatim but for the
    # assertions: the expanded form calls `core::panicking::panic_fmt`,
    # which is unstable, so each is spelled as the `assert!` it came from.
    checks, n = re.subn(
        r'const _: \(\) =\s*if !(.*?)\s*\{\s*\{\s*::core::panicking::panic_fmt\(format_args!\("((?:[^"\\]|\\.)*)"\)\);\s*\}\s*\};',
        lambda m: f'const _: () = assert!({m.group(1)}, "{m.group(2)}");', x[start:i], flags=re.S)
    blocks.append((rules, checks, n))
    pos = tail.end()
calls = list(re.finditer(r"fulgurite::include_lss!\(([^)]*)\)", src))
assert calls and len(calls) == len(blocks), f"{src_path}: {len(calls)} include_lss! calls, {len(blocks)} expansions"
for arm in ("hand", "const"):
    s, shift = src, 0
    for m, (rules, checks, _) in zip(calls, blocks):
        body = [checks] if arm == "const" else []
        body.append(rules + "\n__FULGURITE_LSS")
        text = "{ " + "\n".join(body) + " }"
        a, b = m.start() + shift, m.end() + shift
        s = s[:a] + text + s[b:]
        shift += len(text) - (m.end() - m.start())
    open(f"{out}.{arm}.rs", "w").write(s)
print(f"lss-build-cost: {src_path}: {len(blocks)} expansions cut out, "
      f"{sum(n for _, _, n in blocks)} tree assertions")
PY
}

write_arms counter-window examples/counter-window/src/app.rs --bin counter-window || exit 1
write_arms showcase examples/showcase/src/main.rs --profile test --bin showcase || exit 1

# time_build FILE CMD... -- touch FILE, build, print milliseconds.
time_build() {
    local file=$1; shift
    touch "$file"
    local t0 t1
    t0=$(date +%s%N)
    "$@" >/dev/null 2>"$tmp/build.log" || { echo "lss-build-cost: build failed: $*" >&2; tail -20 "$tmp/build.log" >&2; exit 1; }
    t1=$(date +%s%N)
    echo $(( (t1 - t0) / 1000000 ))
}

set_arm() {  # crate file arm
    case $3 in
        macro) cp "$tmp/orig/$2" "$2" ;;
        *) cp "$tmp/$1.$3.rs" "$2" ;;
    esac
}

for crate in counter-window showcase; do
    if [ "$crate" = counter-window ]; then
        file=examples/counter-window/src/app.rs; cmd=(cargo build -q -p counter-window)
    else
        file=examples/showcase/src/main.rs; cmd=(cargo build -q --tests -p showcase)
    fi
    declare -A runs=([hand]="" [const]="" [macro]="")
    # One unmeasured build of each arm, so every arm starts warm.
    for arm in hand const macro; do set_arm "$crate" "$file" "$arm"; time_build "$file" "${cmd[@]}" >/dev/null; done
    for _ in $(seq "$RUNS"); do
        for arm in hand const macro; do
            set_arm "$crate" "$file" "$arm"
            runs[$arm]="${runs[$arm]} $(time_build "$file" "${cmd[@]}")"
        done
    done
    python3 - "$crate" "${runs[hand]}" "${runs[const]}" "${runs[macro]}" <<'PY'
import statistics, sys
crate = sys.argv[1]
arms = {k: [int(v) for v in s.split()] for k, s in zip(("hand", "const", "macro"), sys.argv[2:5])}
med = {k: statistics.median(v) for k, v in arms.items()}
for k, v in arms.items():
    print(f"{crate} {k:>5}: median {med[k]:.0f} ms, spread {min(v)}-{max(v)}; runs {v}")
print(f"{crate}: include_lss! adds {med['macro'] - med['hand']:+.0f} ms over the hand-written sheet (K2 bound < 100 ms) "
      f"= tree checks {med['const'] - med['hand']:+.0f} ms + running the macro {med['macro'] - med['const']:+.0f} ms")
PY
    unset runs
done
restore
git diff --quiet -- "${FILES[@]}" || { echo "lss-build-cost: the sources were not restored" >&2; exit 1; }
