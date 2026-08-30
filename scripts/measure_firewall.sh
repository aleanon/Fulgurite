#!/usr/bin/env bash
# Step 1 measurement (handoff §10): leaf-edit incremental rebuild times.
#
# Arms:
#   full      one cold `cargo build` of everything (context, not a criterion)
#   noop      `cargo build -p root` with nothing changed (fixed overhead floor)
#   firewall  edit TWEAK in one component crate -> `cargo build -p root`
#             (leaf rebuild + thin root + link: the "usable rebuild" path)
#   leaf-only edit TWEAK -> `cargo build -p compNNN` (rlib only: the
#             hot-reload-unit proxy, no relink)
#   monolith  edit the same component as a module -> `cargo build -p monolith`
#             (the no-firewall control arm)
#
# Success criterion (docs/00-handoff-static-tree.md §12.7): firewall arm < 2s.
set -euo pipefail

DIR="$(cd "$(dirname "$0")/.." && pwd)/experiments/firewall/generated"
cd "$DIR"

LEAVES=(comp007 comp100 comp191)
COUNTER=0

now_ms() { date +%s%N | cut -c1-13; }

tweak() { # tweak <file>  — change the TWEAK literal to a fresh value
    COUNTER=$((COUNTER + 1))
    sed -i "s/^pub const TWEAK: f32 = .*/pub const TWEAK: f32 = 1.0$COUNTER;/" "$1"
}

timed() { # timed <label> <cargo args...>
    local label="$1"; shift
    local t0 t1
    t0=$(now_ms)
    cargo build --quiet "$@"
    t1=$(now_ms)
    echo "$label: $((t1 - t0)) ms"
}

echo "== rustc: $(rustc --version) =="
echo "== full cold build =="
cargo clean --quiet 2>/dev/null || true
timed "full"

echo "== no-op build =="
timed "noop" -p root

echo "== firewall arm (edit comp -> build root bin) =="
for leaf in "${LEAVES[@]}"; do
    tweak "components/$leaf/src/lib.rs"
    timed "firewall/$leaf" -p root
done

echo "== leaf-only arm (edit comp -> build comp rlib only) =="
for leaf in "${LEAVES[@]}"; do
    tweak "components/$leaf/src/lib.rs"
    timed "leaf-only/$leaf" -p "$leaf"
done

echo "== monolith control arm (edit module -> build monolith) =="
for leaf in "${LEAVES[@]}"; do
    tweak "monolith/src/$leaf.rs"
    timed "monolith/$leaf" -p monolith
done

# Checksums legitimately diverge here: the arms received different TWEAK
# edits. Node count agreeing is the invariant.
echo "== sanity: both binaries agree on node count =="
./target/debug/root
./target/debug/monolith
