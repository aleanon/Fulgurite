#!/usr/bin/env bash
# `just ci`: the whole gate set, run against a COMMIT in a clean worktree,
# with a record of what ran (M43/T43.13, docs/plan-gate-hygiene.md §2.1).
#
#   scripts/ci.sh [--only a,b,...] [--fail-fast] [SHA]    (default HEAD)
#
# WHY. There is no git remote, so `.github/workflows/ci.yml` has never run
# and a pre-push hook would never fire. Until this existed, "the gates are
# green" meant whatever somebody remembered running, in a working tree
# that may not have been the commit, with skips that looked like passes.
#
# HOW.
#
#   1. One persistent worktree per checkout, `target/ci/wt`, so its own
#      `target/` stays warm between runs. It is detached at SHA and cleaned
#      of everything but directories named `target` (`git clean -ffdx -e
#      target`): an ignored directory a recipe depends on -- say
#      benches-competitive/build/ -- does not survive into the next run.
#   2. The steps. **They are derived, not listed**: every recipe in the
#      dependency closure of `check`, in just's order, each run alone with
#      `just --no-deps` so the record names the stage that failed -- read
#      from the justfile AT SHA (`just --dump`), so `ci` cannot drift from
#      `check` the way ci.yml drifted from the justfile. Recipes with no
#      body (`test`) are aggregates and are not steps. Then the ci-only
#      steps in CI_ONLY below, and the capability probe first of all.
#   3. Each step's exit status: 0 pass; 77 skip (M43 §2.2) -- a FAILURE
#      unless LUMEN_ALLOW_SKIP=1, recorded as a skip either way; anything
#      else a failure. Under LUMEN_ALLOW_SKIP=1 a step that exits 0 but
#      says it accepted a skip is recorded as a skip too. All steps run
#      unless --fail-fast.
#   4. **Rule 5, the tree is left clean**: `git status --porcelain` of the
#      worktree AFTER the run must be empty. A recipe that rewrites a
#      committed file (M43/T43.12 found `just test` rewriting a lock) fails
#      the run as `tree changed during the run: <path>`.
#   5. The record, `target/ci/<sha>.json` in the INVOKING checkout (an
#      `--only` run writes `<sha>.only.json`, which ci-verify ignores): each
#      step's name, status, exit code and seconds; the capabilities the
#      probe found; LUMEN_ALLOW_SKIP; the port range; the tree check. And
#      one line for the closing commit's body:
#
#        ci <sha>: S steps, F failed, K skipped, tree clean
#
# `just ci-verify [sha]` (scripts/ci_verify.py) checks a record exists
# and is green -- and refuses a record with an allowed skip unless told
# `--allow-skip`, so a green record never hides a skip.
#
# What it is not: a test of the working tree. Uncommitted changes are NOT
# in the run, and it says so when there are any. Commit, then `just ci`.
#
# Environment: DISPLAY (the live gates open windows on it),
# LUMEN_LIVE_PORT_RANGE (the ports the live launcher may pick; recorded),
# LUMEN_ALLOW_SKIP. CARGO_TARGET_DIR is UNSET for the steps: several
# recipes name `target/...` paths, and the worktree's own target/ is the
# warm cache this design depends on.
#
# Logs: target/ci/logs/<sha>/<step>.log in the invoking checkout.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Steps that `check` does not reach, and why each is ci-only:
#   test-crates     nine feature graphs, one per library (M43/T43.6, §2.3)
#   deny            advisories against deny.toml's accepted list (T43.12)
#   selftest-slow   const-budget's plants, each built three times (T43.0)
#   selftest-live   M42's live plants; windows (M42/T42.0)
#   adversary-plant M42/T42.7's planted weakness is still one; windows
#   miri-reload     M49/T49.3: the reload crate's raw pointers under miri
CI_ONLY=(test-crates deny selftest-slow selftest-live adversary-plant miri-reload)

only=""; fail_fast=0; sha_arg=""
while [ "$#" -gt 0 ]; do
    case $1 in
        --only) only=$2; shift 2 ;;
        --only=*) only=${1#--only=}; shift ;;
        --fail-fast) fail_fast=1; shift ;;
        -*) echo "ci: unknown option $1" >&2; exit 2 ;;
        *) sha_arg=$1; shift ;;
    esac
done
sha=$(git rev-parse --verify "${sha_arg:-HEAD}^{commit}") || { echo "ci: no such commit: ${sha_arg:-HEAD}" >&2; exit 2; }
short=${sha:0:12}

base="$ROOT/target/ci"
WT="$base/wt"
LOGS="$base/logs/$sha"
mkdir -p "$base"
exec 9>"$base/lock"
if ! flock -n 9; then echo "ci: another run holds $base/lock -- waiting"; flock 9; fi

if [ -z "$sha_arg" ] && [ -n "$(git status --porcelain)" ]; then
    echo "ci: NOTE the working tree has uncommitted changes; they are NOT in this run, which tests $short"
fi

# --- 1. the worktree ---------------------------------------------------------
if [ -e "$WT/.git" ]; then
    git -C "$WT" checkout -q --detach -f "$sha" || { echo "ci: could not check out $short in $WT" >&2; exit 2; }
else
    rm -rf "$WT"
    git worktree prune
    git worktree add -q --detach "$WT" "$sha" || { echo "ci: could not create $WT" >&2; exit 2; }
fi
git -C "$WT" clean -ffdxq -e target
git -C "$WT" reset -q --hard "$sha"
if [ -n "$(git -C "$WT" status --porcelain)" ]; then
    echo "ci: $WT is not clean at $short before the run:" >&2
    git -C "$WT" status --porcelain >&2
    exit 2
fi
rm -rf "$LOGS"; mkdir -p "$LOGS"

# --- 2. the steps ---------------------------------------------------------------
cd "$WT"
unset CARGO_TARGET_DIR
mapfile -t CHECK_STEPS < <(just --dump --dump-format json | python3 -c '
import json, sys
r = json.load(sys.stdin)["recipes"]
seen, out = set(), []
def visit(n):
    for d in r[n]["dependencies"]:
        if d["recipe"] not in seen:
            seen.add(d["recipe"]); visit(d["recipe"])
    if r[n]["body"]:
        out.append(n)
visit("check")
print("\n".join(out))
') || { echo "ci: could not read the justfile at $short" >&2; exit 2; }
[ "${#CHECK_STEPS[@]}" -gt 0 ] || { echo "ci: the justfile at $short has no check closure" >&2; exit 2; }

steps=(probe "${CHECK_STEPS[@]}" "${CI_ONLY[@]}")
if [ -n "$only" ]; then
    IFS=, read -r -a want <<<"$only"
    for w in "${want[@]}"; do
        printf '%s\n' "${steps[@]}" | grep -qx -- "$w" || { echo "ci: --only $w: not a step (steps: ${steps[*]})" >&2; exit 2; }
    done
    steps=("${want[@]}")
fi

echo "ci $short: ${#steps[@]} steps in $WT${only:+ (--only $only)}"
echo "ci: DISPLAY=${DISPLAY:-<unset>} LUMEN_LIVE_PORT_RANGE=${LUMEN_LIVE_PORT_RANGE:-<unset>} LUMEN_ALLOW_SKIP=${LUMEN_ALLOW_SKIP:-<unset>}"
t_all=$(date +%s)
: > "$LOGS/steps.tsv"
nfail=0; nskip=0
for s in "${steps[@]}"; do
    log="$LOGS/$s.log"
    t0=$(date +%s)
    if [ "$s" = probe ]; then
        probe=./scripts/probe.sh; [ -x "$probe" ] || probe="$ROOT/scripts/probe.sh"
        "$probe" thumb qemu miri xclip >"$log" 2>&1 </dev/null
    else
        just --no-deps "$s" >"$log" 2>&1 </dev/null
    fi
    rc=$?
    secs=$(( $(date +%s) - t0 ))
    last=$(grep -v '^[[:space:]]*$' "$log" | tail -1 | cut -c1-140)
    case $rc in
        0)  st=pass ;;
        77) st=skip; nskip=$((nskip + 1)) ;;
        *)  st=fail; nfail=$((nfail + 1)) ;;
    esac
    # An ACCEPTED skip exits 0 -- `live` prints `N skipped (allowed)`, and
    # `gpu`/`fonts` print `LUMEN_ALLOW_SKIP=1 -- ...` before running tests
    # that may then return early. Counted as a skip, so the record cannot
    # call a run green that did not run everything (found when the first
    # LUMEN_ALLOW_SKIP=1 run recorded a display-less `live` as a pass).
    if [ "$st" = pass ] && [ "${LUMEN_ALLOW_SKIP:-}" = 1 ] \
        && grep -qE 'skipped \(allowed\)|LUMEN_ALLOW_SKIP=1 --' "$log"; then
        st=skip; nskip=$((nskip + 1))
    fi
    printf '%s\t%s\t%s\t%s\n' "$s" "$st" "$rc" "$secs" >> "$LOGS/steps.tsv"
    case $st in
        pass) echo "ci: $s: pass (${secs} s)" ;;
        skip) echo "ci: $s: skip(exit $rc) (${secs} s) -- $last" ;;
        fail) echo "ci: $s: FAIL (exit $rc, ${secs} s) -- $last (log $log)" ;;
    esac
    if [ "$st" != pass ] && [ "$fail_fast" = 1 ]; then
        { [ "$st" = fail ] || [ "${LUMEN_ALLOW_SKIP:-}" != 1 ]; } && break
    fi
done
total=$(( $(date +%s) - t_all ))

# --- 3. rule 5: the tree is left clean ------------------------------------------
changes=$(git -C "$WT" status --porcelain)
if [ -n "$changes" ]; then
    echo "ci: tree changed during the run: $(printf '%s\n' "$changes" | awk '{print $2}' | paste -sd' ' -)"
    printf '%s\n' "$changes" > "$LOGS/tree-changes.txt"
fi

# --- 4. the record ------------------------------------------------------------------
cd "$ROOT"
record="$base/$sha.json"
# A partial run never replaces, or passes for, a full one.
[ -z "$only" ] || record="$base/$sha.only.json"
python3 - "$record" "$sha" "$LOGS" "$total" "$only" <<'PY'
import json, os, socket, sys, time
out, sha, logs, total, only = sys.argv[1:]
steps = []
for line in open(os.path.join(logs, "steps.tsv")):
    name, status, code, secs = line.rstrip("\n").split("\t")
    steps.append({"name": name, "status": status, "exit": int(code), "seconds": int(secs)})
probe = os.path.join(logs, "probe.log")
caps = [l.strip() for l in open(probe)] if os.path.exists(probe) else []
tc = os.path.join(logs, "tree-changes.txt")
changes = [l.rstrip("\n") for l in open(tc)] if os.path.exists(tc) else []
allow = os.environ.get("LUMEN_ALLOW_SKIP") == "1"
failed = [s["name"] for s in steps if s["status"] == "fail"]
skipped = [s["name"] for s in steps if s["status"] == "skip"]
green = not failed and not changes and (allow or not skipped)
rec = {
    "sha": sha, "date": time.strftime("%Y-%m-%dT%H:%M:%S%z"), "host": socket.gethostname(),
    "only": only.split(",") if only else None, "allow_skip": allow,
    "display": os.environ.get("DISPLAY"), "port_range": os.environ.get("LUMEN_LIVE_PORT_RANGE"),
    "capabilities": caps, "steps": steps, "failed": failed, "skipped": skipped,
    "tree_clean": not changes, "tree_changes": changes, "seconds": int(total), "green": green,
}
json.dump(rec, open(out, "w"), indent=1)
PY
nsteps=${#steps[@]}
[ -z "$only" ] || nsteps="$nsteps (--only)"
echo "ci: record $record, ${total} s"
echo "ci $short: $nsteps steps, $nfail failed, $nskip skipped, tree $([ -z "$changes" ] && echo clean || echo CHANGED)${LUMEN_ALLOW_SKIP:+ (LUMEN_ALLOW_SKIP=$LUMEN_ALLOW_SKIP)}"
[ "$nfail" -eq 0 ] && [ -z "$changes" ] || exit 1
if [ "$nskip" -gt 0 ] && [ "${LUMEN_ALLOW_SKIP:-}" != 1 ]; then
    echo "ci: $nskip skipped -- a skip is not a pass; LUMEN_ALLOW_SKIP=1 accepts it (and the record says so)" >&2
    exit 1
fi
exit 0
