# Everyday commands. `test` is the full gate: default-feature tests plus
# the dev-surface tests (observe/reload are off by default per §9, and
# cargo silently skips required-features tests, so both runs are needed).

test:
    cargo test --workspace
    cargo test -p lumen-static --all-features
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy -p lumen-static --all-features --all-targets -- -D warnings

# The Step 4 dylib swap demo (also an integration test; exits nonzero on failure).
hot-demo:
    ./scripts/hot_reload_demo.sh

# Benchmarks (release): Step 2 crossover, Step 3 repeater scaling.
bench:
    cargo run --release -p benches --bin crossover
    cargo run --release -p benches --bin repeater

# Step 1 firewall regression experiment.
firewall:
    python3 scripts/gen_firewall.py
    ./scripts/measure_firewall.sh
