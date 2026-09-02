# Everyday commands.
#
#   just test   -- the every-commit gate. Deterministic, no numbers, no
#                  network. Everything here either passes or names a file
#                  and a line.
#   just check  -- pre-push / CI-only. Slower, or produces artifacts to read
#                  rather than a pass/fail.
#   manual      -- bench, firewall, miri. Machine-dependent or broken
#                  tooling; never gates a commit.

# ---------------------------------------------------------------------------
# The gate
# ---------------------------------------------------------------------------

# The every-commit gate.
test: static release-shape dev-shape fixtures

# Static checks first: sub-second, no compile. A dead doc reference should
# not cost a 30-second build to discover.

# Static drift checks: no compile, sub-second.
static:
    ./scripts/check_no_unsafe.sh        # M0: no `unsafe` anywhere in crates/*/src
    ./scripts/check_doc_identifiers.sh  # backticked names in comments must exist in code
    ./scripts/check_derive_attrs.sh     # every derive helper attribute must have a user
    ./scripts/check_doc_paths.sh        # cited docs/*.md and bench binaries must exist

# Default features == the shipped shape. This is the ONLY build that compiles
# what docs/verification-notes.md scopes its argument to; `--workspace` cannot
# produce it, because lumen-static-agent, examples/counter and fixtures/hot-app
# turn on `observe`/`reload` and cargo unifies features across the selection.
# The old `cargo test -p lumen-static --all-features` line was redundant with
# `--workspace` and is gone.
#
# Grouped so the whole default-feature graph is built once. Interleaving these
# with the dev-shape commands recompiles lumen-static both ways every time.
#
# The steady-state allocation gate runs with the dev-shape tests, as
# `crates/lumen-static-shell/tests/alloc_gate.rs` — its own binary, since
# #[global_allocator] is per-binary. Every path asserts **zero**, not a
# budget: they are all at zero, so a budget would only encode a bug
# (docs/results-allocation.md).

# Default features: the shipped shape, and the only one docs/verification-notes.md covers.
release-shape: const-budget
    cargo test -p lumen-static
    cargo clippy -p lumen-static --all-targets -- -D warnings
    cargo run -p miri-smoke

# Dev surface: observe + reload, unified across the workspace by cargo.
dev-shape:
    cargo test --workspace
    cargo clippy --workspace --all-targets -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# The four fixtures. Until today none of these ran in the gate; three of them
# assert. miri-smoke runs in `release-shape` above -- it contains zero
# `assert!`, so it proves only that no path panics or aborts, which is still
# the cheapest whole-core exercise available.

# The counter and the hot-reload swap: the two fixtures that actually assert.
fixtures:
    cargo run -p counter -- {{ justfile_directory() }}/target/counter.ppm
    ./scripts/hot_reload_demo.sh

# ---------------------------------------------------------------------------
# Pre-push / CI
# ---------------------------------------------------------------------------

# Pre-push: everything in `test`, plus release-shape rustdoc and binsize.
check: test check-extra

# The `check`-only half, split out so CI can run it as its own step.
#
# `cargo fmt --all --check` is deliberately NOT here: it is red at 63 sites
# even with rustfmt.toml's `use_small_heuristics = "Max"`, because the
# remaining sites are hand-packed layouts stable rustfmt cannot express. See
# rustfmt.toml. Turning it on is a decision to accept a reformat, not a
# decision CI gets to make.
check-extra:
    # Release-shape rustdoc: `--workspace` hides broken links that point at
    # items which only exist behind `observe`/`reload`.
    RUSTDOCFLAGS="-D warnings" cargo doc -p lumen-static --no-deps
    cargo build --release -p binsize
    @ls -l target/release/binsize | awk '{print "binsize: " $5 " bytes"}'

# Line coverage, as an artifact to read -- not a threshold. A workspace
# percentage is dominated by whether the fixtures ran, not by core quality;
# the number worth watching is crates/lumen-static/src.

# Line coverage as an artifact, no threshold.
coverage:
    cargo llvm-cov --workspace --lcov --output-path target/lcov.info
    cargo llvm-cov --workspace --summary-only

# ---------------------------------------------------------------------------
# Manual: machine-dependent or broken tooling. Never in the gate.
# ---------------------------------------------------------------------------

# Benchmarks (release). Numbers are not portable: the reference machine is an
# i9-13900KF, governor `powersave`, turbo on. CI runs these for exit code and
# compile coverage only and archives the output; nothing asserts on the times.

# Benchmarks (release). Machine-dependent: never a gate.
bench:
    cargo run --release -p benches --bin crossover
    cargo run --release -p benches --bin fusion
    cargo run --release -p benches --bin repeater
    cargo run --release -p benches --bin bindings
    cargo run --release -p benches --bin agent

# Step 1 firewall regression experiment (its own generated workspace).
firewall:
    python3 scripts/gen_firewall.py
    ./scripts/measure_firewall.sh

# Whole-core UB check. Kept for the day the tooling works again; see
# docs/verification-notes.md.

# Whole-core UB check under miri (tooling currently broken).
miri:
    cargo +nightly miri run -p miri-smoke

# Individual fixtures, for when you want just one.

# Release-shaped whole-core smoke run (no asserts; crash-only).
smoke:
    cargo run -p miri-smoke
# The counter, rendered headlessly to a viewable frame.
counter:
    cargo run -p counter -- counter.ppm
# Compile-time budget for the const table check: 3,711 nodes must build
# in well under 5 s. A budget, not a comparison — the failure mode is a
# hard rustc limit (docs/results-const-eval.md).
const-budget:
    #!/usr/bin/env bash
    set -euo pipefail
    touch fixtures/const-budget/src/main.rs
    start=$(date +%s%N)
    cargo build -p const-budget
    ms=$(( ($(date +%s%N) - start) / 1000000 ))
    echo "const-budget: ${ms} ms for 3711 nodes"
    if [ "$ms" -gt 5000 ]; then
        echo "over the 5000 ms budget — see docs/results-const-eval.md" >&2
        exit 1
    fi

# The Step 4 dylib swap demo; exit code is the verdict.
hot-demo:
    ./scripts/hot_reload_demo.sh
