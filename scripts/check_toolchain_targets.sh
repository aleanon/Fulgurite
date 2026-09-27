#!/usr/bin/env bash
# Every target triple the justfile, the scripts or ci.yml build for is
# declared in rust-toolchain.toml (M43/T43.14, R17-F10).
#
# rust-toolchain.toml declared no `targets`, so a clean runner failed at
# `just thumb` -- and the local runner never noticed, because it shares the
# developer's rustup home, where the target was added by hand once. A
# triple is anything shaped like `<arch>-none-eabi[hf]`,
# `<arch>-unknown-<os>`, `<arch>-linux-android[eabi]` (M54/T54.0: the
# pattern missed Android, so `just mobile-check` passed undeclared),
# `<arch>-apple-<os>` or `<arch>-pc-windows-<abi>`;
# the host's own is exempt only because it is always installed, and none
# is named today.
set -uo pipefail
cd "$(cd "$(dirname "$0")/.." && pwd)"

declared=$(python3 - <<'PY'
import re
s = open("rust-toolchain.toml").read()
m = re.search(r"^targets\s*=\s*\[([^\]]*)\]", s, re.M)
print("\n".join(re.findall(r'"([^"]+)"', m.group(1))) if m else "")
PY
)
host=$(rustc -vV 2>/dev/null | awk '/^host:/ {print $2}')

files=(justfile .github/workflows/ci.yml)
while IFS= read -r f; do files+=("$f"); done < <(find scripts -type f \( -name '*.sh' -o -name '*.py' \) -not -path 'scripts/selftest/*' | sort)

uses=$(grep -noE '\b[a-z0-9_]+-(none-eabihf|none-eabi|unknown-[a-z0-9_]+(-[a-z0-9_]+)?|linux-android(eabi)?|apple-[a-z]+|pc-windows-[a-z]+)\b' "${files[@]}" 2>/dev/null \
       | grep -v 'x86_64-unknown-linux-gnu$' || true)
status=0
used=$(echo "$uses" | awk -F: 'NF {print $3}' | sort -u | grep -v "^${host:-none}\$" || true)
for t in $used; do
    if ! echo "$declared" | grep -qxF "$t"; then
        echo "target $t is used but not declared in rust-toolchain.toml's targets:" >&2
        echo "$uses" | awk -F: -v t="$t" '$3 == t {print "    " $1 ":" $2}' | head -3 >&2
        status=1
    fi
done
nu=$(echo "$used" | grep -c . || true)
nd=$(echo "$declared" | grep -c . || true)
[ "$status" -eq 0 ] && echo "toolchain-targets: targets used $nu, declared $nd"
exit "$status"
