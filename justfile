# Everyday commands.
#
#   just test   -- the every-commit gate. Deterministic, no numbers, no
#                  network. Everything here either passes or names a file
#                  and a line.
#   just check  -- before calling work done. Slower: `test`, the cross,
#                  GPU, SIMD, thread and book builds, and `live` -- every
#                  live gate, which needs a display and FAILS without one
#                  unless LUMEN_ALLOW_SKIP=1 (a skip is not a pass).
#   just ci     -- the executed gate (M43/T43.13): `check`'s closure step by
#                  step, plus the ci-only steps, against a COMMIT in a clean
#                  worktree, with a record `just ci-verify` reads. There is
#                  no remote; .github/workflows/ci.yml is a dormant spec.
#   manual      -- bench, firewall. Machine-dependent; never gates a commit.
#                  Time them under `just bench-lock`.

# ---------------------------------------------------------------------------
# The gate
# ---------------------------------------------------------------------------

# The every-commit gate.
test: static selftest release-shape dev-shape fixtures observe-inlining firmware

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
    ./scripts/check_one_agent_server.sh # M39: one agent transport; nothing else binds a listener
    ./scripts/check_locks.sh            # both Cargo.lock files match their manifests (--locked)
    ./scripts/check_toolchain_targets.sh  # every --target used is declared in rust-toolchain.toml
    ./scripts/check_ci_claims.sh        # no bare dependency/[[bin]] count in the justfile or ci.yml
    ./scripts/check_drive_coverage.sh   # M42: every exported widget is driven, or owed with an owner
    ./scripts/check_idle_citations.sh   # M47: a comment citing idle numbers cites a file that has them
    ./scripts/check_agent_env.sh        # M48/T48.11: LUMEN_AGENT_ADDR is read by the window alone
    python3 scripts/ui_check.py selftest # M48/T48.3: an unmeasured app.perf is NOT MEASURED, not a pass

# Default features == the shipped shape. This is the ONLY build that compiles
# what docs/verification-notes.md scopes its argument to; `--workspace` cannot
# produce it, because lumen-static-agent, examples/counter and fixtures/hot-app
# turn on `observe`/`reload` and cargo unifies features across the selection.
# Nor can a plain `-p lumen-static` on its own: a dev-dependency's features
# unify into the crate under test, and until M43/T43.7 core's dev edge asked
# for the widgets' `observe`. scripts/release_shape.sh builds under
# `--cfg lumen_release_shape`, where core refuses `observe`/`reload` (the
# tripwire), prints core's resolved features and its test count, and fails
# on zero tests. The `observe` tests live in fixtures/observe-tests.
#
# The steady-state allocation gate runs with the dev-shape tests, as
# `crates/lumen-static-shell/tests/alloc_gate.rs` — its own binary, since
# #[global_allocator] is per-binary. Every path asserts **zero**, not a
# budget: they are all at zero, so a budget would only encode a bug
# (docs/results-allocation.md).

# Default features: the shipped shape, and the only one docs/verification-notes.md covers.
release-shape: const-budget
    ./scripts/release_shape.sh

# `--exclude lumen-static-wgpu` on all three: the GPU backend is a
# workspace member so it is type-checked and tested, but wgpu is the
# largest dependency tree in the workspace and minutes of compile
# (docs/results-gpu.md §5 counts it, and says how), and this runs on
# every commit. It is built and tested in `check` instead, by the `gpu`
# recipe below. The crate being *in* the workspace is what keeps its
# Cargo.lock shared and its version drift visible; being out of this
# command is what keeps the gate under a minute.

# Dev surface: observe + reload, unified across the workspace by cargo.
dev-shape:
    cargo test --workspace --exclude lumen-static-wgpu
    # M34/T34.1: the shadow oracle is a feature, so its test needs it on.
    cargo test -p lumen-static-shell --features shadow-oracle --test shadow
    # M43/T43.9: the golden gates. Both need `truetype` OFF (a golden with
    # system text records the machine's fonts), and `--workspace` turns it
    # on, so above they compile to zero tests (golden.rs) or lose their
    # golden test (failure_screen.rs). Here the shell is built alone.
    cargo test -p lumen-static-shell --test golden --test failure_screen
    # M43/T43.11: text tests shape a committed face; the discovery test
    # must find a system one (not skip) unless LUMEN_ALLOW_SKIP=1.
    just fonts
    cargo clippy --workspace --all-targets --exclude lumen-static-wgpu -- -D warnings
    # A fresh doc dir: rustdoc 1.94 panics in its search index
    # (`search_index.rs:478`) on a stale `target/doc` after large edits —
    # seen twice on 2026-09-24, gone both times with the directory. An
    # ICE in the gate is a gate that cries wolf.
    rm -rf target/doc
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --exclude lumen-static-wgpu
    # M48/T48.9: the core's `observe` without the widgets' must not
    # build, and the matched build must. Compiles, so not in `static`.
    ./scripts/check_feature_seam.sh
    # Last: no test binary above may have been emptied by a cfg without
    # saying why (scripts/zero-tests.allow).
    ./scripts/check_test_counts.sh

# Every library under crates/ tested ALONE, so no manifest passes by
# leaning on a feature a neighbour turns on (M43/T43.6, R13-F7 -- the
# agent's tests passed in `--workspace` only). Not in `test`: it re-resolves
# features nine times. `just ci` (T43.13) runs it.
test-crates *names:
    ./scripts/test_crates.sh {{ names }}

# The text tests against the committed face, and system font discovery
# under a requirement (M43/T43.11, docs/plan-gate-hygiene.md §2.2) --
# with the fallback faces (M45/T45.3), which are the system's too. The
# shaping tests used to return early -- passing -- with no system face;
# now only `font_discovery` needs one, and here it must find it unless
# LUMEN_ALLOW_SKIP=1. In `dev-shape`.
fonts:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "${LUMEN_ALLOW_SKIP:-}" = 1 ]; then echo "fonts: LUMEN_ALLOW_SKIP=1 -- discovery may skip"; else export LUMEN_REQUIRE_FONTS=1; fi
    cargo test -q -p lumen-static-shell --features truetype --test shaping --test glyph_placement --test font_discovery --test font_fallback
    cargo test -q -p lumen-static-shell --features truetype --test font_discovery -- --nocapture | grep '^fonts:'
    cargo test -q -p lumen-static-text --test shaping
    cargo test -q -p embedded --test baked_face

# Security advisories, offline, against deny.toml's accepted list: a NEW
# advisory fails (M43/T43.12). `cargo deny fetch` refreshes the database.
deny:
    ./scripts/check_deny.sh

# The firmware's two linked-binary gates (M43/T43.13): zero framework
# statics (check_bss.sh) and .text under its ceiling AND within 1% of
# docs/results-embedded.md's headline (check_text.sh). In `test`, not only
# in `thumb`: a merge of two branches that each grew .text left the
# headline 1.14% stale with every every-commit gate green, because these
# ran only under `check` (c0c2cfd). One thumb release build of core and
# the fixture -- under a second warm, and the plan's "What building found"
# records the measurement.
firmware:
    ./scripts/probe.sh thumb
    ./scripts/check_bss.sh
    ./scripts/check_text.sh

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
    ./scripts/abi_stamp.sh
    ./scripts/window_reload_demo.sh
    cargo run -q -p miri-reload
    ./scripts/reload_gating.sh

# Every refusal, planted (M43, docs/plan-gate-hygiene.md §2.4): each check
# runs green on an unmodified copy of the tree, then each plant under
# scripts/selftest/plants/<tier>/ edits the copy and its check must go red
# BY NAME. scripts/selftest/run.sh's header is the format. Wired in by
# M43/T43.15: `selftest` in `test` (inside §5's 30 s budget), `selftest-build`
# in `check`, `selftest-slow` and `selftest-live` in `just ci`. A check
# replaced by `exit 0` now fails `just test` with `stayed green`.

# The no-compile plants: the drift checks in `static`, and the locks.
selftest:
    ./scripts/selftest/run.sh static

# The plants that need cargo: forbid(unsafe), firmware, goldens, skips.
selftest-build:
    ./scripts/selftest/run.sh build

# The const-budget plants, each built three times (minutes).
selftest-slow:
    ./scripts/selftest/run.sh slow

# The live plants (M42): the drive harness's refusals, each shown red in a
# scratch copy. Needs a display; exits 77 without one.
selftest-live:
    ./scripts/selftest/live.sh

# M42/T42.7: the adversary's planted weakness proves it is one -- the real
# drive catches the widget defect, the planted drive does not. Needs a display.
adversary-plant:
    ./scripts/drive/plants/adversary_plant.sh

# ---------------------------------------------------------------------------
# Pre-push / CI
# ---------------------------------------------------------------------------

# Before calling work done: everything in `test`, the `check-extra` builds,
# and every live gate. **No display is a failure**, not "skipped" and green
# (R17-F8): `live` exits 77, and so does `check`, unless LUMEN_ALLOW_SKIP=1.
check: test check-extra selftest-build live

# Every live gate, one line each and a total (M43/T43.13, §2.2 of
# docs/plan-gate-hygiene.md): `name: pass | FAIL | skip(77) -- reason`.
# Fails on a FAIL; a skip fails too (exit 77) unless LUMEN_ALLOW_SKIP=1.
# `just live widget-drive tasks-drive` runs those two. Ports come from
# LUMEN_LIVE_PORT_RANGE when set (scripts/live/launch.py).
live *names:
    ./scripts/live.sh {{ names }}

# The whole gate set against a commit, in the clean worktree target/ci/wt,
# with a record at target/ci/<sha>.json (scripts/ci.sh's header is the
# design). `just ci`, `just ci <sha>`, `just ci --only static,firmware`.
ci *args:
    ./scripts/ci.sh {{ args }}

# Is there a green `just ci` record for this commit? A milestone is not
# done until there is (docs/plan-m38-fixes.md §5). `--allow-skip` accepts
# a record whose skips LUMEN_ALLOW_SKIP=1 let through.
ci-verify *args:
    ./scripts/ci_verify.py {{ args }}

# Opt in to a pre-commit hook that runs `just static`. A convenience that
# `--no-verify` bypasses; `just ci` is the gate. Sets core.hooksPath, which
# every worktree of this repository shares.
install-hooks:
    git config core.hooksPath scripts/hooks
    @echo "install-hooks: core.hooksPath=scripts/hooks (pre-commit runs just static)"

# Run a timing under the one bench lock every worktree shares, with
# `uptime` before and after, marked LOADED when the load average says the
# machine was busy (scripts/bench_lock.sh). M44, M45, M46 and M55 cite it.
# `just bench-lock cargo run --release -p benches --bin grid`.
bench-lock +cmd:
    ./scripts/bench_lock.sh {{ cmd }}

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
# `damage_oracle.rs` holds the software path to. The tests themselves skip
# when no adapter is present — a machine with no GPU is the configuration
# the CPU fallback exists for — but THIS recipe is the gate that claims a
# comparison happened, so it requires one (LUMEN_REQUIRE_GPU=1) unless
# LUMEN_ALLOW_SKIP=1 says otherwise (docs/results-gpu.md).

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
    #!/usr/bin/env bash
    # **A skip is not a pass** (M43/T43.10, docs/plan-gate-hygiene.md §2.2).
    # With no adapter the parity tests return early, and libtest swallows
    # their "skipping" line on a pass -- so this recipe used to be green on
    # a machine that compared nothing (R17-F6). It requires the adapter
    # unless LUMEN_ALLOW_SKIP=1, and prints the one it used.
    set -euo pipefail
    if [ "${LUMEN_ALLOW_SKIP:-}" = 1 ]; then echo "gpu: LUMEN_ALLOW_SKIP=1 -- an absent adapter skips"; else export LUMEN_REQUIRE_GPU=1; fi
    cargo test -p lumen-static-wgpu
    cargo test -q -p lumen-static-wgpu --test parity the_gpu_half_ran -- --nocapture | grep '^adapter:'
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
# this one. The thumb target is declared in rust-toolchain.toml (M43/T43.14),
# so no manual `rustup target add` is needed; it is here and not in
# `just test` for the cross build's cost.

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
    #!/usr/bin/env bash
    ./scripts/probe.sh thumb || exit $?
    # The path is cargo's report, not a guess (scripts/lib/firmware.sh),
    # so a CARGO_TARGET_DIR cannot leave this printing a stale binary.
    set -euo pipefail
    source scripts/lib/firmware.sh
    firmware_build
    echo "firmware: $FIRMWARE"
    size -A "$FIRMWARE" | grep -E '\.text|\.rodata|\.bss|\.data|Total'

# Cortex-M4F no_std build of the core + fixture (docs/results-embedded.md).
thumb:
    ./scripts/probe.sh thumb
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
    # The fixture's host tests, built ALONE so core stays no_std: this is
    # the only run of core's no_std `#[bind]` branch (M43/T43.8). The
    # requirement refuses the std answer a unified build would give.
    LUMEN_REQUIRE_NOSTD=1 cargo test -p embedded
    # The linked-firmware gates (.bss statics, .text ceiling and headline)
    # moved to `firmware`, in `test` (M43/T43.13). The flash ceiling
    # docs/results-embedded.md stated on 2026-09-05 went unchecked until
    # 2026-09-10, by which point the doc was 9.4% below reality; under
    # `check` alone, a merge left its headline 1.14% stale in 2026-09.

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
    cargo run --release -p benches --bin grid

# M29's spreadsheet fixture: the shape assertions T29.0 built and the
# three-arm probe T29.1 measured. **A recipe because the gates live in
# its `fn main`** — until this existed they had been shown to fail once,
# by hand, and nothing re-ran them
# (`docs/results-damage-precision.md`).
grid:
    cargo run --release -p benches --bin grid

# Step 1 firewall regression experiment (its own generated workspace).
firewall:
    python3 scripts/gen_firewall.py
    ./scripts/measure_firewall.sh

# Whole-core UB check. Kept for the day the tooling works again; see
# docs/verification-notes.md.

# Whole-core UB check under miri (tooling currently broken).
miri:
    cargo +nightly miri run -p miri-smoke

# M49/T49.3: lumen-static-reload's raw-pointer code under miri. Works where
# `miri` does not: the fixture has no dependencies and no build script, and
# includes the reload crate's raw.rs and panic.rs by path.
# In `just ci` (M43/T43.13); a machine without nightly miri skips (77).
miri-reload:
    ./scripts/probe.sh miri
    cargo +nightly miri run -p miri-reload

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
# Every widget, every action it accepts, driven against the live window
# under BOTH invalidation strategies (M27/T27.3). `just widget-drive
# Switch` drives one. Needs a display; skips without one.
widget-drive name="":
    ./scripts/widget_drive.sh {{ name }}

# this is the gate that it does now. Needs a display; skips without one.
agent-drive:
    ./scripts/agent_drive.sh

# M28/T28.12: drive examples/tasks-demo through the agent endpoint --
# spawn, batch, chain, cancel and subscribe -- under BOTH invalidation
# strategies, asserting each action landed AND its consequence. Needs a
# display; skips without one.
tasks-drive:
    ./scripts/tasks_drive.sh

# M28/T28.1: a completion posted by a worker wakes an idle window. The
# fixture owns its own deadline and asserts itself, because the failure
# it catches is "no wake ever arrives" — which under `cargo test` would
# hang rather than fail. Needs a display; skips without one.
wake-gate:
    ./scripts/wake_gate.sh

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
#
# **A clean, non-incremental build every run** (M43/T43.5): `cargo clean
# -p const-budget` in its own target directory, CARGO_INCREMENTAL=0. The
# old recipe touched main.rs and timed an INCREMENTAL rebuild, which
# reuses the last build's const-eval: a quadratic went red on the first
# run and green on every rerun (R11-F5). The fixture must use `wid_index!`
# so `count_wid` is evaluated at all; the grep below holds it to that.
const-budget:
    #!/usr/bin/env bash
    set -euo pipefail
    src=fixtures/const-budget/src/main.rs
    uses=$(grep -c 'wid_index!(' "$src" || true)
    if [ "$uses" -lt 1 ]; then
        echo "const-budget: $src uses no wid_index!, so count_wid is never evaluated" >&2
        exit 1
    fi
    export CARGO_TARGET_DIR=target/const-budget CARGO_INCREMENTAL=0
    # Its dependencies, untimed (the same default features the fixture
    # asks for, so the timed build reuses them); then only the fixture.
    cargo build -q -p lumen-static -p lumen-static-widgets
    cargo clean -q -p const-budget
    start=$(date +%s%N)
    cargo build -q -p const-budget
    ms=$(( ($(date +%s%N) - start) / 1000000 ))
    echo "const-budget: ${ms} ms for 3711 nodes (non-incremental, bound 5000; wid_index uses: $uses)"
    if [ "$ms" -gt 5000 ]; then
        echo "over the 5000 ms budget — see docs/results-const-eval.md" >&2
        exit 1
    fi
    "$CARGO_TARGET_DIR"/debug/const-budget

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

# M47/T47.8: the Idle section of docs/results-frame-cost.md -- frames,
# wakes, context switches and threads of a still window and of one with a
# 120 ms timer, over SECONDS. Needs a display (run it on a private one).
idle-count seconds="60":
    ./scripts/idle_count.sh {{ seconds }}

# M47/T47.7 (R15-F6): examples/glasswall's images survive a suspend and
# resume round trip (a debug-only LUMEN_TEST_RESUME hook runs it), on the
# software and GPU backends, screenshot before and after. Needs a display.
resume-drive:
    ./scripts/resume_drive.sh

# **`--locked` on every benches-competitive build** (M43/T43.12): its
# lock went stale at M30 and every recipe that built there rewrote the
# committed file, `just test` included. `check_locks.sh` in `static` now
# refuses a stale lock, and these refuse to write one.

# Both backends over a resolution sweep — the table in docs/results-gpu.md.
# `--bin` is required: the package gained a second binary (`bands`,
# M16/T16 threading) and `cargo run -p` then refuses as ambiguous. This
# recipe was broken from that day until 2026-09-22, which is how long it
# had been since anyone ran it.
raster-sweep:
    cd benches-competitive && cargo run --locked --release -p raster-scaling --bin raster-scaling

# Our software rasteriser against Vello's — docs/results-vello.md.
#
# The only arm in benches-competitive that rasterises on both sides:
# every other one stops at a display list, so none of them measure the
# renderer at all. `vello_cpu`'s thread count is a runtime setting, so
# one build sweeps it; ours is a cargo feature, so the recipe builds
# twice. Both frames are checked against each other every run — see the
# three bounds in the arm.
vello f="simd,threads" threads="16":
    cd benches-competitive && VELLO_THREADS={{ threads }} cargo run --locked --release -p vello-cpu-arm --features "{{ f }}"

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
    cd benches-competitive && cargo run --locked --release -q -p lumen-static-stages -- {{ ns }} {{ iters }} {{ warm }}

# The Step 4 dylib swap demo; exit code is the verdict.
hot-demo:
    ./scripts/hot_reload_demo.sh

# M37/T37.0: a hot library built with a different feature set is refused by
# name, and one built the same way is accepted -- two real builds.
abi-stamp:
    ./scripts/abi_stamp.sh

# M37/T37.1: a window library swapped v1 -> v2 headlessly; the swapped
# frame equals v2 run from the start, byte for byte.
window-reload-demo:
    ./scripts/window_reload_demo.sh

# M37/T37.4: a live window that reloads itself -- edits a COPY of
# fixtures/hot-window with sed and drives it over the agent: new text with
# the old count, a failed build kept out, a panic shown and cleared by the
# next swap. Needs a display.
hot-window-drive:
    ./scripts/hot_window_drive.sh

# M49/T49.0 (R5-F8): an application deriving `Reloadable` builds without
# hot reload and carries none of it; with `hot`, it does.
reload-gating:
    ./scripts/reload_gating.sh

# The tally window with the agent endpoint on 127.0.0.1:9230 (override with
# LUMEN_AGENT_ADDR). Drive it with scripts/agent_client.py — see
# .claude/skills/verifying-apps/SKILL.md, rung 4.
run-agent:
    cargo run -p counter-window

# The tally with the platform screen-reader adapter (T7.5). Default-off:
# on Linux it pulls accesskit_unix and a D-Bus/async stack
# (docs/plan-architecture-v2.md, T7.5, has the measured count), and
# `just test` builds this example every run.
tally-a11y:
    cargo run -p counter-window --features a11y

# Every live check against a fresh window (scripts/ui_check.py is the list
# — do not restate it here, it drifted to six once already). Rung 4's gate;
# needs a display. In `check` through `live`.
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
    cargo build -q -p counter -p minimal -p hot-window --target-dir target/book

# Read it locally, rebuilding on save.
book-serve: book-deps
    mdbook serve book --open

# M35/T35.0: does compiling `observe` in stop a row widget being inlined
# into its List/Repeat loop? The symbol table answers (docs/plan-observe-cost.md).
# `--aa` builds plain twice (must pass); `--clock` adds the frame arm's clock.
observe-inlining *args:
    ./scripts/check_observe_inlining.sh {{ args }}

# M36/T36.3: a panic in a live window shows where it happened and the
# window survives it, under both strategies (--release WITH `reload`);
# and a release build WITHOUT `reload` must still die. Needs a display.
panic-drive:
    ./scripts/panic_drive.sh
