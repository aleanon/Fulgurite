# Build the embedded fixture's firmwares and say where cargo put them
# (M43/T43.4; four binaries since M52/T52.1). Sourced by
# scripts/check_text.sh and scripts/check_bss.sh.
#
#   source scripts/lib/firmware.sh
#   firmware_build || exit 1
#   # FIRMWARE          the paint-only binary (`firmware`)
#   # FIRMWARE_BINS     every gated binary's name, paint-only first
#   # firmware_path N   the linked binary for name N
#
# WHY THE PATH COMES FROM CARGO. Both scripts used to read a hard-coded
# `target/thumbv7em-none-eabihf/release/firmware`. With CARGO_TARGET_DIR
# set, cargo built somewhere else and the scripts judged whatever stale
# binary sat in ./target -- and passed (R11-F6). The path now comes from
# cargo's own report of the artifact it produced
# (`--message-format=json`, the `compiler-artifact` record for each bin),
# so it follows CARGO_TARGET_DIR, a `[build] target-dir` in a config
# file, or anything else that moves it.
#
# WHY FOUR (M52/T52.1, R11-F4). `firmware` lays out and paints one frame.
# It never dispatches, binds or cascades, so M29 and M33 grew all three
# with every gate green. The other three link what a device does
# (fixtures/embedded/src/bin/, and docs/results-embedded.md for what each
# costs). One cargo call builds all four.

FIRMWARE_TARGET=thumbv7em-none-eabihf
FIRMWARE_BINS="firmware firmware_interactive firmware_bind firmware_cascade"

firmware_build() {
    local json bins="" b
    for b in $FIRMWARE_BINS; do bins="$bins --bin $b"; done
    # shellcheck disable=SC2086
    json=$(cargo build --release -q -p embedded $bins --features firmware \
        --target "$FIRMWARE_TARGET" --message-format=json) || {
        echo "firmware: cargo build failed" >&2
        return 1
    }
    FIRMWARE_PATHS=$(printf '%s\n' "$json" | python3 -c '
import json, sys
want = sys.argv[1].split()
exe = {}
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    t = m.get("target", {})
    if m.get("reason") == "compiler-artifact" and t.get("name") in want \
            and "bin" in t.get("kind", []) and m.get("executable"):
        exe[t["name"]] = m["executable"]
missing = [w for w in want if w not in exe]
if missing:
    sys.exit("firmware: cargo reported no executable for " + ", ".join(missing))
for w in want:
    print(w, exe[w])
' "$FIRMWARE_BINS") || return 1
    local name path
    while read -r name path; do
        [ -f "$path" ] || { echo "firmware: cargo reported $path, which does not exist" >&2; return 1; }
    done <<<"$FIRMWARE_PATHS"
    FIRMWARE=$(firmware_path firmware)
    export FIRMWARE FIRMWARE_PATHS FIRMWARE_BINS
}

# The linked binary for one name, from the last firmware_build.
firmware_path() {
    printf '%s\n' "$FIRMWARE_PATHS" | awk -v n="$1" '$1 == n {print $2}'
}

# firmware_build_one NAME [cargo args...] -- build one fixture bin for the
# target (e.g. `fwq`, with `--release` or without for the dev profile) and
# print cargo's path to it. M52/T52.6.
firmware_build_one() {
    local name=$1; shift
    cargo build -q -p embedded --bin "$name" --features firmware \
        --target "$FIRMWARE_TARGET" --message-format=json "$@" | python3 -c '
import json, sys
name = sys.argv[1]
exe = None
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    t = m.get("target", {})
    if m.get("reason") == "compiler-artifact" and t.get("name") == name and m.get("executable"):
        exe = m["executable"]
if not exe:
    sys.exit("firmware: cargo reported no executable for " + name)
print(exe)
' "$name"
}
