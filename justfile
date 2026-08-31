# Everyday commands. `test` is the full gate: default-feature tests plus
# the dev-surface tests (observe/reload are off by default per §9, and
# cargo silently skips required-features tests, so both runs are needed).

test: no-unsafe
    cargo test --workspace
    cargo test -p lumen-static --all-features
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy -p lumen-static --all-features --all-targets -- -D warnings

# M0 gate: the release core contains no `unsafe` at all
# (docs/plan-architecture-v2.md, docs/results-safe-prototype.md).
no-unsafe:
    #!/usr/bin/env bash
    set -euo pipefail
    # Actual constructs, not the word in prose.
    pat='unsafe (\{|fn |impl |trait )'
    n=$(grep -rnE "$pat" crates/lumen-static/src/*.rs | wc -l)
    if [ "$n" -ne 0 ]; then
        echo "release core must contain no unsafe; found $n:" >&2
        grep -rnE "$pat" crates/lumen-static/src/*.rs >&2
        exit 1
    fi
    echo "no unsafe in the release core"

# Whole-core smoke run (and the entry point for cargo miri).
smoke:
    cargo run -p miri-smoke

# The Step 4 dylib swap demo (also an integration test; exits nonzero on failure).
hot-demo:
    ./scripts/hot_reload_demo.sh

# Benchmarks (release): Step 2 crossover, Step 3 repeater scaling.
bench:
    cargo run --release -p benches --bin crossover
    cargo run --release -p benches --bin fusion
    cargo run --release -p benches --bin repeater

# Step 1 firewall regression experiment.
firewall:
    python3 scripts/gen_firewall.py
    ./scripts/measure_firewall.sh
