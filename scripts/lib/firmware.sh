# Build the embedded fixture's firmware and say where cargo put it
# (M43/T43.4). Sourced by scripts/check_text.sh and scripts/check_bss.sh.
#
#   source scripts/lib/firmware.sh
#   firmware_build || exit 1     # sets FIRMWARE to the linked binary
#
# WHY. Both scripts used to read a hard-coded
# `target/thumbv7em-none-eabihf/release/firmware`. With CARGO_TARGET_DIR
# set, cargo built somewhere else and the scripts judged whatever stale
# binary sat in ./target -- and passed (R11-F6). The path now comes from
# cargo's own report of the artifact it produced
# (`--message-format=json`, the `compiler-artifact` record for the
# `firmware` bin), so it follows CARGO_TARGET_DIR, a `[build] target-dir`
# in a config file, or anything else that moves it.
#
# M52's embedded tasks build on this rather than on a path of their own.

FIRMWARE_TARGET=thumbv7em-none-eabihf

firmware_build() {
    local json
    json=$(cargo build --release -q -p embedded --bin firmware --features firmware \
        --target "$FIRMWARE_TARGET" --message-format=json) || {
        echo "firmware: cargo build failed" >&2
        return 1
    }
    FIRMWARE=$(printf '%s\n' "$json" | python3 -c '
import json, sys
exe = None
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    if m.get("reason") == "compiler-artifact" and m.get("target", {}).get("name") == "firmware" \
            and "bin" in m["target"].get("kind", []) and m.get("executable"):
        exe = m["executable"]
if not exe:
    sys.exit("firmware: cargo reported no executable for the firmware bin")
print(exe)
') || return 1
    [ -f "$FIRMWARE" ] || { echo "firmware: cargo reported $FIRMWARE, which does not exist" >&2; return 1; }
    export FIRMWARE
}
