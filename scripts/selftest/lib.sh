# Edit helpers for plants (sourced by run.sh). Every helper FAILS when its
# edit did not take -- a plant that matched nothing must report
# `edit did not apply`, never a green check for the wrong reason
# (docs/plan-gate-hygiene.md §0 rule 2; AGENT.md, "the injection itself can
# be wrong").

# replace FILE OLD NEW -- literal, every occurrence; fails if OLD is absent
# or if NEW is not present afterwards.
replace() {
    python3 - "$1" "$2" "$3" <<'PY'
import sys
path, old, new = sys.argv[1:4]
s = open(path, encoding="utf-8").read()
if old not in s:
    sys.exit(f"replace: {path}: text not found: {old!r}")
s = s.replace(old, new)
open(path, "w", encoding="utf-8").write(s)
if new and new not in open(path, encoding="utf-8").read():
    sys.exit(f"replace: {path}: replacement not present afterwards")
PY
}

# append FILE TEXT -- adds TEXT as a new line and checks it is there.
append() {
    [ -f "$1" ] || { echo "append: no such file: $1" >&2; return 1; }
    printf '%s\n' "$2" >> "$1"
    grep -qF -- "$2" "$1"
}

# write FILE CONTENT -- creates (or overwrites) FILE, making directories.
write() {
    mkdir -p "$(dirname "$1")"
    printf '%s\n' "$2" > "$1"
    grep -qF -- "$(printf '%s' "$2" | head -1)" "$1"
}

# drop_lines FILE LITERAL -- deletes every line containing LITERAL; fails if
# none did.
drop_lines() {
    grep -qF -- "$2" "$1" || { echo "drop_lines: $1: no line contains: $2" >&2; return 1; }
    grep -vF -- "$2" "$1" > "$1.selftest.tmp" && mv "$1.selftest.tmp" "$1"
    ! grep -qF -- "$2" "$1"
}

# remove FILE -- deletes FILE; fails if it did not exist.
remove() {
    [ -e "$1" ] || { echo "remove: no such file: $1" >&2; return 1; }
    rm -f "$1"
    [ ! -e "$1" ]
}

# sub FILE REGEX REPLACEMENT -- Python `re.sub` with MULTILINE, first match
# only; fails if the pattern matched nothing or the file did not change.
sub() {
    python3 - "$1" "$2" "$3" <<'PY'
import re, sys
path, pat, rep = sys.argv[1:4]
s = open(path, encoding="utf-8").read()
t, n = re.subn(pat, rep, s, count=1, flags=re.M)
if n == 0 or t == s:
    sys.exit(f"sub: {path}: pattern matched nothing: {pat!r}")
open(path, "w", encoding="utf-8").write(t)
PY
}
