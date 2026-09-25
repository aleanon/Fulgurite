#!/usr/bin/env bash
# The self-test harness for every refusal (M43, docs/plan-gate-hygiene.md §2.4).
#
#   scripts/selftest/run.sh static      the no-compile checks   (just selftest)
#   scripts/selftest/run.sh build       cargo check/build/test  (just selftest-build)
#   scripts/selftest/run.sh slow        const-budget reruns     (just selftest-slow)
#   SELFTEST_ONLY=<name> ... run.sh T   one plant (and its check's baseline)
#
# WHY. A check that refuses something is only known to work if the thing it
# refuses has been planted and the check went red *by name*. M38 showed most
# of this repository's drift checks failing once, in scratch commits nothing
# re-runs (docs/results-deep-review.md §4). A plant here lives in the
# repository, so it re-runs.
#
# HOW. The harness keeps one scratch copy of the working tree at
# `target/selftest/wt` (tracked files plus untracked, unignored ones -- so
# an uncommitted change is what is tested, not HEAD). The copy is its own
# throwaway git repository, reset between plants; its `target/` survives,
# so the build tier stays warm between runs.
#
#   1. BASELINE, the accepting direction. Every distinct check named by a
#      plant of the tier runs on the UNMODIFIED copy and must be green. It
#      prints `baseline: N checks green`. A tightened check that now refuses
#      the real tree fails here, before any plant runs.
#   2. PLANTS. Each file in `plants/<tier>/*.sh` is sourced in a subshell,
#      in the copy's root. It sets:
#        CHECK   the gate, a shell command (run with `bash -c`);
#        EXPECT  an extended regex its red output must match (by name);
#        plant() the edit. It must FAIL when the edit did not apply -- use
#                the helpers in lib.sh, which check that their edit took;
#      and optionally:
#        applied()   a further check that the edit had the intended effect
#                    (rule 2: a plant must prove it applied);
#        run_check() how to run CHECK under the plant, when that differs from
#                    the baseline -- an environment "mode" (a fake `nm` first
#                    on PATH, an adapter hidden, fonts hidden). Default:
#                    `bash -c "$CHECK"`.
#
# Four outcomes per plant, the last three failures:
#
#   plant X: red as expected -- <the matching line>
#   plant X: stayed green -- expected red
#   plant X: edit did not apply
#   plant X: red, but not by name -- expected /EXPECT/
#
# The fourth matters as much as the second: a check that dies on a missing
# file is red, and proves nothing about the thing it was meant to refuse.
#
# Logs: target/selftest/logs/<tier>/<plant>.log (and baseline-<n>.log).
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
tier="${1:?usage: run.sh static|build|slow}"
PLANTS="$ROOT/scripts/selftest/plants/$tier"
[ -d "$PLANTS" ] || { echo "selftest: no tier '$tier' ($PLANTS)" >&2; exit 2; }

base="$ROOT/target/selftest"
WT="$base/wt"
LOGS="$base/logs/$tier"
mkdir -p "$WT" "$LOGS"
exec 9>"$base/lock"
flock 9

t0=$(date +%s)

# --- sync the copy ----------------------------------------------------------
cd "$ROOT"
find "$WT" -mindepth 1 -maxdepth 1 ! -name .git ! -name target -exec rm -rf {} +
git ls-files -co --exclude-standard -z \
  | rsync -a --from0 --files-from=- --ignore-missing-args ./ "$WT/"
cd "$WT"
if [ ! -d .git ]; then
    git init -q
    git config user.email selftest@localhost
    git config user.name selftest
fi
git add -A >/dev/null 2>&1
git commit -q --allow-empty -m "selftest base" >/dev/null
BASE_SHA=$(git rev-parse HEAD)
reset_copy() {
    git reset -q --hard "$BASE_SHA"
    git clean -fdq          # not -x: the copy's target/ is ignored and kept
}

# --- collect plants -----------------------------------------------------------
mapfile -t files < <(ls "$PLANTS"/*.sh 2>/dev/null | sort)
if [ -n "${SELFTEST_ONLY:-}" ]; then
    mapfile -t files < <(printf '%s\n' "${files[@]}" | grep -E "/${SELFTEST_ONLY}\.sh$")
fi
[ "${#files[@]}" -gt 0 ] || { echo "selftest $tier: no plants" >&2; exit 2; }

declare -a checks=()
for f in "${files[@]}"; do
    c=$( CHECK=""; source "$f"; printf '%s' "$CHECK" )
    [ -n "$c" ] || { echo "selftest: $f sets no CHECK" >&2; exit 2; }
    printf '%s\n' "${checks[@]}" | grep -qxF -- "$c" || checks+=("$c")
done

# --- 1. baseline: every check green on the unmodified copy ------------------
nb=0; bad=0
for c in "${checks[@]}"; do
    nb=$((nb + 1))
    reset_copy
    log="$LOGS/baseline-$nb.log"
    printf '$ %s\n' "$c" > "$log"
    if bash -c "$c" >>"$log" 2>&1; then
        echo "baseline: green  $c"
    else
        rc=$?
        echo "baseline: RED    $c   (exit $rc, log $log)"
        tail -5 "$log" | sed 's/^/    | /'
        bad=$((bad + 1))
    fi
done
if [ "$bad" -eq 0 ]; then
    echo "baseline: $nb checks green"
else
    echo "baseline: $bad of $nb checks RED on the unmodified tree"
fi

# --- 2. plants ----------------------------------------------------------------
n=0; ok=0; green=0; noapply=0; wrong=0
for f in "${files[@]}"; do
    name="$tier/$(basename "$f" .sh)"
    n=$((n + 1))
    reset_copy
    log="$LOGS/$(basename "$f" .sh).log"
    (
        set +e
        CHECK=""; EXPECT=""
        # shellcheck source=lib.sh
        source "$ROOT/scripts/selftest/lib.sh"
        unset -f applied run_check 2>/dev/null
        source "$f"
        if ! plant >"$log" 2>&1; then exit 10; fi
        if declare -F applied >/dev/null && ! applied >>"$log" 2>&1; then exit 10; fi
        printf '$ %s\n=== check output ===\n' "$CHECK" >> "$log"
        if declare -F run_check >/dev/null; then
            run_check >>"$log" 2>&1
        else
            bash -c "$CHECK" >>"$log" 2>&1
        fi
        rc=$?
        [ "$rc" -eq 0 ] && exit 11
        # Only the check's own output is searched: the plant's edit log and
        # the command line are above the marker, and a command that names
        # the very thing EXPECT looks for would otherwise match itself.
        if hit=$(sed -n '/^=== check output ===$/,$p' "$log" | grep -m1 -E -- "$EXPECT"); then
            printf '%s\n' "$hit" > "$log.hit"
            exit 0
        fi
        printf '%s\n' "$EXPECT" > "$log.hit"
        exit 12
    )
    case $? in
        0)  ok=$((ok + 1))
            echo "plant $name: red as expected -- $(head -c 160 "$log.hit" | head -1 | sed 's/^[[:space:]]*//')" ;;
        10) noapply=$((noapply + 1))
            echo "plant $name: edit did not apply"
            tail -3 "$log" | sed 's/^/    | /' ;;
        11) green=$((green + 1))
            echo "plant $name: stayed green -- expected red" ;;
        *)  wrong=$((wrong + 1))
            echo "plant $name: red, but not by name -- expected /$(cat "$log.hit" 2>/dev/null)/ (log $log)"
            tail -3 "$log" | sed 's/^/    | /' ;;
    esac
done
reset_copy

secs=$(( $(date +%s) - t0 ))
echo "selftest $tier: $n plants -- $ok red as expected, $green stayed green, $noapply edit did not apply, $wrong red for another reason; baseline $((nb - bad))/$nb green; ${secs} s"
[ "$ok" -eq "$n" ] && [ "$bad" -eq 0 ]
