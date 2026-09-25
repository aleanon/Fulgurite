#!/usr/bin/env bash
# Citation liveness: every repo artifact cited from code or docs must exist.
#
# Two kinds, same failure mode -- a pointer to something that was renamed or
# deleted, which nothing in the toolchain notices:
#   (1) `docs/foo.md` paths, cited from Rust comments, other docs, the
#       book, the justfile and Cargo.toml. The book is in scope from
#       2026-09-10: it cites results docs by rule rather than restating
#       their numbers, so its citations are the ones most likely to outlive
#       what they point at.
#   (2) `--bin NAME` / `-p CRATE` targets cited from docs, the book,
#       AGENT.md, the justfile and the scripts. A results doc that names a
#       bench binary which no longer exists makes its numbers
#       unreproducible -- the doc says how to rerun the measurement and the
#       command does not run.
#
# Cargo target names come from `cargo metadata`, so autodiscovered bins (the
# `fusion`/`repeater` case, which benches/Cargo.toml does not declare with a
# [[bin]] section) count as existing.
set -euo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
status=0

sources=$(find crates benches examples fixtures docs book scripts -type f \
    \( -name '*.rs' -o -name '*.md' -o -name '*.toml' -o -name '*.sh' \) \
    -not -path '*/target/*' -not -name 'check_*.sh' 2>/dev/null; echo justfile; echo AGENT.md; echo Cargo.toml)

# --- (1) docs/*.md paths ---
# Capture any leading path component too, so `Lumen/docs/foo.md` — a
# citation of another repo's tree — extracts whole and is dropped by the
# `^docs/` filter rather than masquerading as one of ours.
{ grep -hoE '[A-Za-z0-9._/-]*docs/[A-Za-z0-9._/-]+\.md' $sources 2>/dev/null || true; } \
    | grep -E '^docs/' | sort -u > "$tmp/paths"

# A plan may name a document it promises to create. Those live in
# scripts/doc-paths.allow with a reason, and come off the list as they
# are written.
allow="$(dirname "$0")/doc-paths.allow"
if [ -f "$allow" ]; then
    sed -e 's/#.*$//' -e 's/[[:space:]]*$//' "$allow" | grep -vE '^$' | sort -u > "$tmp/allow" || true
    comm -23 "$tmp/paths" "$tmp/allow" > "$tmp/paths.f" && mv "$tmp/paths.f" "$tmp/paths"
fi
while IFS= read -r p; do
    [ -n "$p" ] || continue
    if [ ! -f "$p" ]; then
        echo "dangling doc path: $p" >&2
        { grep -rlF "$p" $sources 2>/dev/null || true; } | head -4 | sed 's/^/    cited by /' >&2
        status=1
    fi
done < "$tmp/paths"

# --- (2) cargo targets cited from docs, the book, AGENT.md, the justfile
#         and the scripts ---
#
# M43/T43.2 (R17-F3). The target set used to be every `"name"` string in
# `cargo metadata`'s JSON -- which includes every DEPENDENCY's name, so
# `--bin serde` passed -- and only `--bin` was extracted, only from docs/*.md
# and the justfile. Now the metadata is parsed: a `--bin` must be a target
# of kind `bin`, and a `-p`/`--package` on a cargo command line must be a
# package, in either workspace. BOTH workspaces, because
# `benches-competitive` is deliberately its own workspace so foreign
# toolkits stay out of this one's Cargo.lock, and until 2026-09-08 its
# targets were invisible here -- so a doc citing `--bin bands` failed the
# gate for existing in the wrong workspace rather than for not existing.
#
# Exceptions live in scripts/doc-paths.allow as `bin NAME` or
# `package NAME`, each with a reason.
if ! command -v cargo >/dev/null; then
    echo "doc-paths: cargo not found, cannot check cited targets" >&2
    exit 1
fi
set +e
targets=$(python3 - "$allow" <<'PY'
import glob, json, os, re, subprocess, sys

allow = {"bin": set(), "package": set()}
if os.path.exists(sys.argv[1]):
    for line in open(sys.argv[1]):
        f = line.split("#")[0].split()
        if len(f) == 2 and f[0] in allow:
            allow[f[0]].add(f[1])

pkgs, bins = set(), set()
for ws in (".", "benches-competitive"):
    if not os.path.exists(f"{ws}/Cargo.toml"):
        continue
    r = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1",
         "--manifest-path", f"{ws}/Cargo.toml"],
        capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"doc-paths: cargo metadata failed in {ws}:\n{r.stderr}")
    for p in json.loads(r.stdout)["packages"]:
        pkgs.add(p["name"])
        bins.update(t["name"] for t in p["targets"] if "bin" in t["kind"])

srcs = sorted(set(
    glob.glob("docs/*.md") + glob.glob("book/src/**/*.md", recursive=True)
    + ["AGENT.md", "justfile", ".github/workflows/ci.yml"]
    + [f for f in glob.glob("scripts/**/*", recursive=True)
       if os.path.isfile(f) and not os.path.basename(f).startswith("check_")
       and "/selftest/" not in f and not f.endswith((".allow", ".pyc"))]))
cited = {"bin": set(), "package": set()}
bad = 0
for f in srcs:
    if not os.path.exists(f):
        continue
    for i, line in enumerate(open(f, errors="replace"), 1):
        found = [("bin", m.group(1)) for m in re.finditer(r"--bin[ =]([A-Za-z0-9_-]+)", line)]
        # -p / --package only on a cargo command, up to a shell separator:
        # `mkdir -p` and `ss -ltnp` are not package names.
        for m in re.finditer(r"\bcargo\b([^|;&`]*)", line):
            found += [("package", n) for n in
                      re.findall(r"(?:\s-p|--package)[ =]([A-Za-z0-9_-]+)", m.group(1))]
        for kind, name in found:
            cited[kind].add(name)
            live = name in (bins if kind == "bin" else pkgs)
            if not live and name not in allow[kind]:
                what = "--bin target" if kind == "bin" else "package"
                print(f"{f}:{i}: cited {what} does not exist: {name}", file=sys.stderr)
                bad += 1
nb, np_ = len(cited["bin"]), len(cited["package"])
lb = len([b for b in cited["bin"] if b in bins])
lp = len([p for p in cited["package"] if p in pkgs])
ab = len(cited["bin"] & allow["bin"])
ap = len(cited["package"] & allow["package"])
print(f"bins cited {lb} live / {nb} ({ab} allowed), packages cited {lp} live / {np_} ({ap} allowed)")
sys.exit(1 if bad else 0)
PY
)
rc=$?
set -e
[ "$rc" -eq 0 ] || status=1

if [ "$status" -ne 0 ]; then
    echo "" >&2
    echo "Fix the citation, or restore what it points at." >&2
    exit 1
fi
echo "doc-paths: $(wc -l < "$tmp/paths") doc paths, all live; $targets"
