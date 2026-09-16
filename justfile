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
    ./scripts/check_book_includes.sh    # book includes, anchors and links must all resolve
    ./scripts/check_attr_table.sh       # the attribute table in lib.rs and the book must agree

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

# `--exclude lumen-static-wgpu` on all three: the GPU backend is a
# workspace member so it is type-checked and tested, but wgpu is 37
# crates and minutes of compile (docs/results-gpu.md), and this runs on
# every commit. It is built and tested in `check` instead, by the `gpu`
# recipe below. The crate being *in* the workspace is what keeps its
# Cargo.lock shared and its version drift visible; being out of this
# command is what keeps the gate under a minute.

# Dev surface: observe + reload, unified across the workspace by cargo.
dev-shape:
    cargo test --workspace --exclude lumen-static-wgpu
    cargo clippy --workspace --all-targets --exclude lumen-static-wgpu -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --exclude lumen-static-wgpu

# The four fixtures. Until today none of these ran in the gate; three of them
# assert. miri-smoke runs in `release-shape` above -- it contains zero
# `assert!`, so it proves only that no path panics or aborts, which is still
# the cheapest whole-core exercise available.

# The counter, the M8/M9 form, the alloc-free embedded frame, and the
# hot-reload swap. The form drives a
# `ScrollView` with real wheel events and renders a `TextInput` through the
# real rasteriser: unit tests count display-list commands, which is not the
# same as knowing a widget paints where it claims to.
fixtures:
    cargo run -p counter -- {{ justfile_directory() }}/target/counter.ppm
    cargo run -p counter-window -- --form {{ justfile_directory() }}/target/form.ppm
    cargo run -p embedded --bin embedded
    ./scripts/hot_reload_demo.sh

# ---------------------------------------------------------------------------
# Pre-push / CI
# ---------------------------------------------------------------------------

# Pre-push: everything in `test`, plus release-shape rustdoc and binsize,
# plus the live gate when there is a display to open a window on.
check: test check-extra
    #!/usr/bin/env bash
    if [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then ./scripts/live_gate.sh; else echo "live-gate: no display, skipped"; fi

# The `check`-only half, split out so CI can run it as its own step.
#
# `cargo fmt --all --check` is deliberately NOT here: it is red at 63 sites
# even with rustfmt.toml's `use_small_heuristics = "Max"`, because the
# remaining sites are hand-packed layouts stable rustfmt cannot express. See
# rustfmt.toml. Turning it on is a decision to accept a reformat, not a
# decision CI gets to make.

# Release-shape rustdoc, binsize, and the exhaustive f32 check.
check-extra: floats thumb gpu simd threads book
    # Release-shape rustdoc: `--workspace` hides broken links that point at
    # items which only exist behind `observe`/`reload`.
    RUSTDOCFLAGS="-D warnings" cargo doc -p lumen-static --no-deps
    cargo build --release -p binsize
    @ls -l target/release/binsize | awk '{print "binsize: " $5 " bytes"}'

# The GPU backend (T15.12), which `test` excludes for compile time.
# Its own tests are the backend-parity gate: the same display list
# rendered by `software::Canvas` and by wgpu, compared pixel by pixel
# against a stated tolerance, plus the partial-redraw property that
# `damage_oracle.rs` holds the software path to. **They skip rather than
# fail when no adapter is present** — a machine with no GPU is the
# configuration the CPU fallback exists for, so a red suite there would
# be reporting the wrong thing (docs/results-gpu.md).

# The rasteriser's SIMD path (M17/T17.1). Default-OFF, so `just test`
# builds the scalar one; this proves the vector one is byte-identical
# rather than merely close, which is the whole basis for offering it.

# The SIMD rasteriser: byte-identity against the scalar path, and lint.
simd:
    cargo test -p lumen-static-shell --features simd --test simd_shadow
    cargo test -p lumen-static-shell --features simd --test wide_stores
    cargo test -p lumen-static-shell --features simd --test damage_oracle
    cargo clippy -p lumen-static-shell --features simd --all-targets -- -D warnings

# Band-parallel rasterisation (M17/T17.4). Default-OFF and desktop-only.
# The comparison cannot happen inside one binary -- the split is a cargo
# feature -- so the same scenes are rendered by both builds and their
# checksums diffed.

# The threaded rasteriser: byte-identity against the single-band render.
threads:
    ./scripts/check_banded.sh
    cargo clippy -p lumen-static-shell --features threads,truetype --all-targets -- -D warnings
    cargo test -q -p lumen-static-shell --features threads,truetype --test damage_oracle

# The GPU backend: build, lint, and compare its pixels with the software canvas.
gpu:
    cargo test -p lumen-static-wgpu
    cargo clippy -p lumen-static-wgpu --all-targets -- -D warnings
    cargo clippy -p lumen-static-winit --features gpu --all-targets -- -D warnings

# Prove `num::floor`/`num::ceil` bit-exact with std over all 2^32 f32
# bit patterns (M13/T13.2). Release-only: 13 s optimized, minutes in the
# gate's debug build, which is why it is `#[ignore]`d there.

# Exhaustive f32 check: num::floor/ceil vs std over all 2^32 inputs.
floats:
    cargo test --release -p lumen-static --lib num -- --ignored

# The core, no_std, for the M13 device (M13/T13.1). `--features alloc`
# rather than bare `--no-default-features`: with the std > alloc > none
# tiers the bare form means NEITHER, and Vec/String/VecDeque are still in
# the core until T13.4 -- so the stricter build is that task's gate, not
# this one. Needs `rustup target add thumbv7em-none-eabihf`, which is why
# it is here and not in `just test`.

# Regenerate the embedded fixture's baked face (M13/T13.7). Only needed
# when the size or the source font changes; the committed face.rs is
# checked byte-for-byte by fixtures/embedded/tests/baked_face.rs.

# Re-bake fixtures/embedded/src/face.rs from a system TTF.
bake-font font="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf" px="16":
    cargo run -p bake-font -- {{ font }} {{ px }} fixtures/embedded/src/face.rs

# Link a real thumbv7em firmware and print its sections (M13/T13.9).
# `cargo check` proves none of this: it type-checks and stops, with no
# codegen, no link and no .text. The numbers in docs/results-embedded.md
# come from here.

# Section sizes of a linked Cortex-M4F firmware (docs/results-embedded.md).
firmware-size:
    cargo build --release -p embedded --bin firmware --features firmware --target thumbv7em-none-eabihf
    @size -A target/thumbv7em-none-eabihf/release/firmware | grep -E '\.text|\.rodata|\.bss|\.data|Total'

# Cortex-M4F no_std build of the core + fixture (docs/results-embedded.md).
thumb:
    # The middle tier: no std, an allocator present. This is the M13
    # device's shape (T13.0/T13.1).
    cargo check -p lumen-static --no-default-features --features alloc --target thumbv7em-none-eabihf
    cargo clippy -p lumen-static --no-default-features --features alloc --target thumbv7em-none-eabihf -- -D warnings
    # The strict tier: no std AND no alloc, where Vec, String, DisplayList
    # and the escape hatches cannot be *named* (T13.4). This is the strong
    # proof of the alloc-free path -- a panicking allocator only covers
    # paths that run; here the compiler says which line wanted one.
    cargo check -p lumen-static --no-default-features --target thumbv7em-none-eabihf
    cargo clippy -p lumen-static --no-default-features --target thumbv7em-none-eabihf -- -D warnings
    cargo check -p embedded --lib --target thumbv7em-none-eabihf
    # Zero framework statics, with a named exemption list. Needs a
    # linked binary, so it lives beside the firmware build rather than
    # in `just static`, which runs no cargo at all.
    ./scripts/check_bss.sh
    # And the flash ceiling, which docs/results-embedded.md has stated
    # since 2026-09-05 and nothing checked until 2026-09-10 -- by which
    # point the doc's figure had drifted 9.4% below reality.
    ./scripts/check_text.sh

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
    cargo run --release -p benches --bin cascade

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
# Run an example in a window — `just example glass`.
#
# Every directory under `examples/` is a package of the same name, so the
# name you pass is the directory you see. `just examples` lists them.
#
# Anything after the name goes to the binary, so the headless modes work
# too: `just example glass --shot frame.ppm 120 30`, or
# `just example counter-window --drive`.
#
# **Release, unlike the recipes below it.** These are demos somebody
# looks at and scrolls, and the software rasteriser is the thing being
# looked at -- `examples/glass` re-blurs its panel on every frame that
# moves (`docs/results-glass.md`). Plain `cargo run -p glass` is the
# debug build if you want one.
example name *args:
    @test -d examples/{{ name }} || { echo "no such example: {{ name }}" >&2; echo "try one of:" >&2; ls examples >&2; exit 1; }
    cargo run --release -p {{ name }} -- {{ args }}

# The glass home screen, and what it costs on both backends.
#
# Built `--features gpu`, which is the point: twenty-six backdrop filters
# at 1290x2796 is 153 ms on the software canvas and 2.1 ms on a GPU
# (docs/results-glass.md §0h). It also prints the damage the scene
# reports, which is `All` -- past MAX_DEPS every frame is a full redraw,
# and this example is deliberately past it.
glasswall-bench:
    cargo run --release -p glasswall --features gpu -- --bench

# Every built-in widget, one at a time — the showcase (docs/plan-widgets-gallery.md).
showcase:
    cargo run --release -p showcase

# Drive the showcase's live window over the agent socket (M23/T23.1c).
#
# The endpoint has existed since M15 and had no transport out of `run`;
# this is the gate that it does now. Needs a display; skips without one.
agent-drive:
    ./scripts/agent_drive.sh

# What `just example` accepts.
examples:
    @ls examples

# The tally in a window: click the buttons, or press up/down.
tally:
    cargo run -p counter-window

# One frame of the tally to a viewable image, at a given count.
tally-frame count="7":
    cargo run -p counter-window -- --headless target/tally.ppm {{ count }}

# Drive the tally through the agent surface and assert — rung 2 of the
# verification ladder, above a screenshot.
tally-drive:
    cargo run -p counter-window -- --drive

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

# The two backends compared through a real window — the half a headless
# test cannot see, because "rendered" and "shown" are different claims.
# This project has been caught by that gap before: the first partial
# redraw skipped the present and left another window's pixels on screen.
# Needs a display, an adapter and `xwd`; skips when any is missing.
# Manual rather than in `check`, because it builds the whole wgpu stack
# in release and opens two windows.

# Render one window on both backends and diff the pixels (docs/results-gpu.md).
gpu-parity:
    ./scripts/gpu_parity.sh

# Both backends over a resolution sweep — the table in docs/results-gpu.md.
raster-sweep:
    cd benches-competitive && cargo run --release -p raster-scaling

# Our software rasteriser against Vello's — docs/results-vello.md.
#
# The only arm in benches-competitive that rasterises on both sides:
# every other one stops at a display list, so none of them measure the
# renderer at all. `vello_cpu`'s thread count is a runtime setting, so
# one build sweeps it; ours is a cargo feature, so the recipe builds
# twice. Both frames are checked against each other every run — see the
# three bounds in the arm.
vello f="simd,threads" threads="16":
    cd benches-competitive && VELLO_THREADS={{ threads }} cargo run --release -p vello-cpu-arm --features "{{ f }}"

# Every stage of one frame, separately, over N x workload x retention.
#
# The frame arm reports four cumulative numbers because that is what is
# comparable with Qt and GTK; this one answers "which stage is it" —
# dispatch, update, layout, place, paint, damage, software raster, blit,
# and the GPU raster beside them. It prints the sum of the stages and an
# independently-measured whole frame so the accounting is checkable.
#
# Args: N list, iterations, warm-ups. LUMEN_NO_GPU=1 drops the GPU stage.

# Per-stage cost of one frame over N x workload x retention.
stages ns="100,1000,3000,10000" iters="200" warm="20":
    cd benches-competitive && cargo run --release -q -p lumen-static-stages -- {{ ns }} {{ iters }} {{ warm }}

# The Step 4 dylib swap demo; exit code is the verdict.
hot-demo:
    ./scripts/hot_reload_demo.sh

# The tally window with the agent endpoint on 127.0.0.1:9230 (override with
# LUMEN_AGENT_ADDR). Drive it with scripts/agent_client.py — see
# .claude/skills/verifying-apps/SKILL.md, rung 4.
run-agent:
    cargo run -p counter-window

# The tally with the platform screen-reader adapter (T7.5). Default-off:
# on Linux it pulls accesskit_unix and a D-Bus stack, 95 -> 157 crates,
# and `just test` builds this example every run.
tally-a11y:
    cargo run -p counter-window --features a11y

# Every live check against a fresh window (scripts/ui_check.py is the list
# — do not restate it here, it drifted to six once already). Rung 4's gate;
# needs a display. In `check` when one is present.
live-gate:
    ./scripts/live_gate.sh

# Per-stage cost of one tally frame, headless, median of 50. The numbers
# belong in docs/results-frame-cost.md.
profile:
    LUMEN_PROFILE=1 cargo run -q -p counter-window -- --headless target/tally.ppm 12

# The frame a popup opens on, damaged area against the window's
# (M26/T26.0). `profile` above is fixed to the tally and takes no
# arguments, so M26/T26.11 — a command-count change answering
# `Damage::All` — had no arm to be judged on.
profile-showcase:
    cargo run --release -q -p showcase -- --profile

# The same frame with the rasteriser's SIMD path on (M17/T17.1).
profile-simd:
    LUMEN_PROFILE=1 cargo run -q -p counter-window --features simd -- --headless target/tally.ppm 12

# And with band-parallel rasterisation as well (M17/T17.4).
profile-threads:
    LUMEN_PROFILE=1 cargo run -q -p counter-window --features simd,threads -- --headless target/tally.ppm 12

# ---------------------------------------------------------------------------
# The book
# ---------------------------------------------------------------------------

# `book/` is an mdBook. It lives in this repo because AGENT.md's
# doc-currency rule requires prose to move in the SAME COMMIT as the
# behaviour, which a second repository makes structurally impossible --
# and because both mechanisms below need one checkout.
#
# The split between here and `static`: check_book_includes.sh is pure
# bash and runs on every commit; these two need mdbook installed
# (`cargo binstall mdbook`) and a build, so they run pre-push.

# Build the book and compile every snippet in it.
book: book-deps
    ./scripts/build_book.sh

# The rlibs the book's snippets link against, in their own target dir.
# NOT target/debug/deps: that holds thousands of files and a dozen
# differently featured liblumen_static-*.rlib, and rustdoc refuses an
# ambiguous search path (E0464). This one holds exactly one of each.
#
# The wipe is not laziness. Cargo never deletes a superseded artifact, so
# building `-p counter` and later `-p counter -p minimal` -- different
# feature unification, different hash -- leaves BOTH rlibs here, and the
# book then fails to compile with an error that names neither the cause
# nor the fix. Eight seconds of rebuild buys a directory whose contents
# are a function of this line alone.
book-deps:
    rm -rf target/book
    cargo build -q -p counter -p minimal --target-dir target/book

# Read it locally, rebuilding on save.
book-serve: book-deps
    mdbook serve book --open
