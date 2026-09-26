# M52 — the footprint gates price a firmware that does nothing, and a window that cannot recover

> **Written 2026-09-24, not started.** Closes R11-F1, R11-F2, R11-F3,
> R11-F4, R12-F4, F-NEW-1, F-NEW-2 and F-NEW-3. The R-numbered findings
> are from M38 (`docs/results-deep-review.md`). The three F-NEW findings
> are the re-triage's own: it found them against `main` at 1717808 on
> 2026-09-24 while re-running the review's firmware probes.
>
> **Phase 4** of the fix order. It depends on M43 in three places.
> R11-F6: `check_text.sh` and `check_bss.sh` read a hard-coded
> `./target` path, so with a relocated target directory they pass a
> stale binary. Every gate here reads those scripts' output. R17-F9:
> `check_bss.sh` passes when `nm` fails. R17-F13: the stale headline in
> `docs/results-embedded.md`. It also needs M43's CI runner, and that
> runner must have the thumbv7em target (R17-F10). T52.7 may run
> earlier, and says why.
>
> **This plan amends M36's plan** (`docs/plan-panic-recovery.md`,
> rule 3). See §2.2 and T52.7.
>
> **Why three regressions went unseen.** The gated firmware
> (`fixtures/embedded/src/bin/firmware.rs`) lays out and paints one
> frame. It has no `#[bind]` and never dispatches an event. M29 and M33
> then added a per-binding static, a vtable walk in `bind::node_of`, and
> a larger dispatch, and every embedded gate stayed green. That is
> R11-F4, and it is why **the first real task is putting an interactive
> firmware into `check_text.sh` and `check_bss.sh`** (T52.1).

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **The binary a gate measures must contain the code the gate is
   about.** `check_text.sh` already applies this to `Toggle`: it refuses
   to read `.text` unless Toggle code is linked. This milestone applies
   it to input, binding and the cascade. Each gated binary asserts that
   the symbols it exists to price are linked before its number is read.
2. **A property is gated by the quantity it names.** "No float
   formatting" and "no `f64`" are symbol counts with a bound of 0, as
   the task-symbol count in `check_text.sh` already is. They are not
   the `.text` ceiling, which absorbed both regressions with room to
   spare (R11-F1, R11-F2). "The stack fits 4 KB" is a measured stack,
   not `stack_bytes(DEPTH)` (R11-F3).
3. **Numbers are measured once, at the start, and stated once.** The
   figures in `docs/results-embedded.md` have drifted from the binary
   (M43/R17-F13 fixes the headline). T52.0 re-measures every budget row
   against the binaries, as they stand after M43, before any task
   changes them. It writes them into that doc once, and every later
   task cites that section.

## 1. What is wrong, at 1717808

The figures in this section are **the re-triage's own measurements at
1717808** (thumbv7em-none-eabihf, release, compared against the review's
base where it says so). They are stated here once. Figures the
re-triage re-ran and found identical to the review's are cited, not
restated.

### 1.1 The gated firmware never dispatches, so three regressions went unseen (R11-F4, WORSE; F-NEW-1; F-NEW-2)

`check_text.sh` and `check_bss.sh` build and read one binary,
`firmware` (`scripts/check_text.sh:21-27`, `scripts/check_bss.sh:19-22`).
It calls only `frame()`, so dispatch, cascade and binding are not in
it. The paint-only firmware is 27,024 B of `.text`, unchanged since the
review.

**R11-F4 is worse than the review found.** The review's interactive
probe firmware (hit test, dispatch, `Boundaries<16>`, cascade, one
`#[bind]`) went from 57,076 to **58,052 B** of `.text` between the
review's base and 1717808. That is 88.6% of the 64 KB ceiling, and
`.rodata` went from 18,366 to 18,818. The growth came from M29 and M33:
the renamed `pass::dispatch` at 3,170 B, `BindSite::get` at 730 B, and
two small helpers. No gate saw any of it.

Two of the three unseen regressions are in that growth:

- **F-NEW-1: a `.data` static per binding.** For a non-generic
  application, the derive emits `static __ln_site: BindSite` once per
  `#[bind]` (`crates/lumen-static-macros/src/lib.rs:1904`). `BindSite`
  starts at `UNKNOWN = usize::MAX` (`crates/lumen-static/src/bind.rs:240-244`).
  That is non-zero, so the static lands in `.data`: it is stored in
  flash and copied to RAM before `main`. The re-triage's one-binding
  probe (`cap_bind`) has a 4 B `.data` section at 1717808 and none at
  the review's base. `scripts/bss.allow` lists only `SINK`, so
  `check_bss.sh` would reject the static, if it ever linked a
  `#[bind]`. The per-site cache is deliberate (`bind.rs:230-236`). Its
  embedded RAM cost is recorded nowhere.
- **F-NEW-2: `bind::node_of` links the whole `Node` vtable.** `node_of`
  (`bind.rs:206-227`) calls `Node::node(tree, i)` for each table index.
  That returns `&dyn Node`, and then it calls `size_of_val` and
  `kind_name` through the vtable. Coercing to `&dyn Node` instantiates
  every method of every node type in the bound tree, including `hit`
  and `place`, which that firmware never calls. `cap_bind` grew **+3,480
  B `.text` and +764 B `.rodata`** between the review's base and
  1717808. This is the same shape M35 removed from `observe`. The walk
  runs once per binding site, so it costs flash, not CPU.

**Found while writing this plan:** `check_bss.sh` reads a symbol's name
as awk's `$4` (`scripts/check_bss.sh:29`). A demangled name with a
space in it, which is exactly the shape of the F-NEW-1 static
(`<… as lumen_static::event::App>::update_marked::__ln_site`), is cut at
its first space. The script would still reject the static, but it
would name only a prefix. No exemption pattern could match it.

### 1.2 The cascade links interaction code it cannot call (R12-F4)

`cascade_deep_with` (`crates/lumen-static/src/style.rs:1407-1417`),
`cascade_arenas_deep` (`:1434-1442`) and `cascade_subtree`'s loop
(`:1360-1362`) call `tree.node_mut(i)` for every table index, and pass
the `&mut dyn Node` to `cascade_arenas`. Without `alloc`, the body of
`cascade_arenas` is compiled out (`style.rs:1448-1456`), but the loop is
not. So a style-only leaf firmware links every widget's `on_event`,
`hit` and `compose` through the vtable. The re-triage measured the
review's style-only probe at **42,742 B** of `.text` at 1717808, against
a paint-only base that did not change. The verifier's one-line
narrowing, gating the loop on `alloc`, was measured by the review and
is not applied. The dispatch half of R12-F4 is the real price of input,
not dead code. The review's verifier established that, and T52.1's
interactive gate prices it.

### 1.3 Two forbidden symbol classes are prose (R11-F1, R11-F2)

`docs/results-embedded.md` records that one `f32::clamp` once cost
26.5 KB of float-formatting code, and its threshold table says "`f64`
in the frame path: zero" as a measured row (`results-embedded.md:20`,
`:44`). `check_text.sh` counts Toggle symbols and task symbols. It does
not count `core::num::flt2dec`, `core::fmt::float` or the soft-double
intrinsics. The re-triage re-applied both of the review's injections at
1717808 (`f32::clamp` in `Constraints::clamp`, scratch `2189c21`; an
`f64` round-trip, scratch `12e6d3d`). It got the review's figures to the
byte, with every embedded gate green.

### 1.4 The stack row reports a model as a measurement (R11-F3, slightly worse)

`stack_bytes` (`crates/lumen-static/src/desc.rs:167-168`) is depth ×
`STACK_BYTES_PER_LEVEL` (`desc.rs:150`). The fixture asserts
`stack_bytes(DEPTH) <= 4096` at compile time
(`fixtures/embedded/src/lib.rs:326-330`), and `docs/results-embedded.md`
reports that model's output as "Measured". The real stack, painted and
scanned under qemu (the review's `fwq` probe), exceeds the model in
release and exceeds the budget in the dev profile. A 4 KB local in a
widget's `layout` leaves the const assert green, because `DEPTH` did not
change. `desc.rs:149`'s rustdoc says a level's frame "does not vary with
the tree", and the review's `TextInput` sweep falsifies that.

**What the re-triage measured at 1717808 under qemu:** the frame
figures are unchanged. Dispatch and binding grew with M29 and M33: in
release, a typed character from 572 to **604 B** and a bound update
from 520 to **528 B**; in dev, from 2,600 to **2,708 B** and from 976
to **1,032 B**. `Boundaries<256>` does not reach the embedded profile,
because it is only `Host`'s default, and `Host` is `std`.

### 1.5 A release window links the panic-recovery code it never runs (F-NEW-3)

M36's plan rule 3 (`docs/plan-panic-recovery.md:47-52`) says recovery
is "compiled in" in debug builds and in release builds with `reload`,
and that a release build without `reload` keeps the default behaviour.
`lumen-static-winit` picks `RECOVER` as a `const` under exactly that
`cfg` (`crates/lumen-static-winit/src/lib.rs:248-251`). But it hands the
constant to `Local::from_host`, which stores it in a runtime field
(`crates/lumen-static-shell/src/session.rs:176`, `:196-208`). Every
guarded arm then tests the field: `run_guarded` (`session.rs:150-160`),
and `:243`, `:353`, `:360`, `:502`, `:518`. So every `recover::`
function is compiled and linked. The recover module itself is gated
only on `panic = "unwind"` (`crates/lumen-static-shell/src/lib.rs:26-27`).

The re-triage built `examples/minimal` in release with the window's
default features, no `reload` and no agent. `.text` grew **+14,992 B**
between the review's base and 1717808. **14 `lumen_static_shell::recover`
symbols totalling 13,964 B** are linked, where there were none: the
failure screen, the report, the wrap, the panic hook's closure, a static
and a thread-local. Behaviour is correct. Recovery is off, so no hook is
installed and nothing is caught. What is wrong is the link, and no gate
prices a release window binary: `fixtures/binsize` depends on neither
the shell nor the window crate.

`recover.rs`'s header (`recover.rs:4-5`) already reads "Compiled in
wherever panics unwind; **used** where the window crate decides". That
contradicts rule 3's wording, and nothing reconciled the two.

## 2. Decisions

### 2.1 A binding's cache goes to `.bss`, and is exempted by pattern, counted

The options for F-NEW-1:

- **(a) Encode zero as unknown.** Store the index plus two, with 1
  meaning nowhere, so that `BindSite::new` is all-zero. The static
  moves from `.data` to `.bss`: no flash image, only zeroed RAM. It is
  still a framework static, so `scripts/bss.allow` exempts it **by
  pattern** (`*::update_marked::__ln_site`), with the argument that it
  is 4 B per binding and bounded by the source text. `check_bss.sh`
  prints how many binding caches it exempted and their total bytes.
- **(b) No static on `no_std`.** The derive emits the generic arm, a
  walk per write, when the core is built without `alloc`. There are
  zero statics, and each write pays the walk.
- **(c) Exempt it as it is**, in `.data`.

**Recommendation: (a).** It keeps the cache, which M29 measured as worth
having, and it keeps `check_bss.sh`'s rule honest. A static is allowed
only when it is named with a reason and counted. (b) trades a
per-write walk on the slowest target for 4 B. (c) leaves flash paying
for an initialiser.

**What (a) breaks:** `check_bss.sh`'s exact-match allowlist becomes
exact-or-pattern. So the name extraction is fixed first (§1.1's `$4`
truncation). Otherwise a pattern could never match the name it is
written for.

### 2.2 Recovery becomes a type, not a value — an amendment to M36's rule 3

The options for F-NEW-3:

- **(a) A policy type.** `Local` becomes `Local<A, P>`, with
  `P: RecoverPolicy` carrying `const ON: bool`, and two marker types,
  one recovering and one crashing. `lumen-static-winit` picks the
  marker type under the `cfg` it already uses for `RECOVER`. Every
  guarded arm tests `P::ON`, so in a crashing window the arms and every
  `recover::` symbol they reach are monomorphised away. The hot
  library's session (`crates/lumen-static-reload/src/lib.rs:400`, which
  always recovers) names the recovering type.
- **(b) A `const` generic `bool`** on `Local` and `Live`. It has the
  same effect with less text, but a `true` at a call site says less
  than a type name, and the rustdoc has nothing to hang on.
- **(c) Keep the runtime field, and rely on inlining** of
  `run_guarded`. That leaves the result to the optimiser. **Rejected.**
- **(d) Keep the link, and reword rule 3** to "linked wherever panics
  unwind, enabled where…". It is acceptable at desktop scale only if it
  is written down. **Rejected,** because the M36 plan's reason for rule
  3 was that a shipped application should not carry recovery it does
  not use.

**Recommendation: (a),** as the re-triage proposed. Like (b), it takes
the choice out of the optimiser's hands. Unlike (b), it gives the
choice a name that shows up in error messages and in the rustdoc.

**The amendment to M36.** Rule 3 is reworded to say what the gate now
checks: "**Where it is linked**: debug builds, and release builds with
the `reload` feature. A release build without it links no `recover::`
symbol, and `just release-window-shape` asserts that." The M36 plan
gets a dated note under rule 3 citing this milestone. `recover.rs`'s
header, which currently says "compiled in wherever panics unwind", is
corrected in the same commit.

**What (a) breaks:** `Local`'s type gains a parameter. Its constructors
are called from `lumen-static-winit` and `lumen-static-reload` only. The
`Session` trait object is unchanged, so the dylib seam (M37) does not
move. `Live` (`session.rs:456-461`) carries `recover` too, and takes the
same parameter. `Host::guard` stays a plain method for headless
callers. It is linked only where it is called.

### 2.3 The bind walk becomes static, as M35 made the observe walk

For F-NEW-2, the derive generates the address walk: for each field, its
index range, its concrete type's `size_of`, and a constant kind name,
with no `&dyn Node`. Hand-written containers keep the current walk
through a provided default, exactly as M35's `observe_walk` does
(`docs/plan-observe-cost.md` T35.1, "Provided, not required"). Two
facts are recorded in rustdoc: the default drops nothing, and a
hand-written container that relies on it costs its vtable.

**Rejected:** comparing addresses against field-offset constants,
falling back to the `dyn` walk. It is cheaper to write, but it keeps the
coercion reachable in every tree that has one hand-written container.
The fallback's coercion keeps the vtable alive, which is exactly what
M35's T35.1 found when its fallback lived in the caller.

### 2.4 The cascade loop is gated on `alloc` now. The per-type constant stays M26's

For R12-F4, the three loops in §1.2 are wrapped in
`#[cfg(feature = "alloc")]`, since arenas exist only with `alloc`. That
removes the vtables from the leaf profile, which is the profile the
gated firmware builds. **An `alloc`-tier firmware still pays** until a
derive-computed "has arenas" constant lets the loop skip types with no
arena. That constant is M26 Phase D's finding 9, promoted and not
built, and M53's R6-F3 also touches `cascade_arenas_deep`. This plan
does not build it. It records the `alloc`-tier cost as T52.4's second
number.

### 2.5 The interactive firmware keeps the 64 KB ceiling

It measures 88.6% of 64 KB today (§1.1). The options are a separate,
higher ceiling for an interactive build, or the same one. **The same
one:** the ceiling is "an eighth of flash for the framework", and an
application that handles input is the ordinary case, not a larger
product. If a later task needs more, that is a re-budget with its
argument in `docs/results-embedded.md`, as `check_text.sh`'s own
comment requires.

### 2.6 The stack is measured under qemu, in release

`qemu-system-arm -M mps2-an386` (a Cortex-M4F) paints the stack and
scans it after each call. The alternative, `-Z emit-stack-sizes` plus a
call-graph tool, is static and cannot follow the `dyn Node` recursion,
so it would repeat the model's blind spot. The budget applies to
**release** (opt-level ≥ 1). The dev profile's figure is recorded
beside it, with a note that a dev firmware needs `opt-level = 1` for
the `embedded` package to fit. `stack_bytes` gains an intercept (a base
plus depth × per-level), fitted from the depth sweep. The compile-time
assert stays as a depth backstop, and its message says so.

## 3. Tasks

### T52.0 — re-measure every budget row, and commit the gates that are red today

1. **Re-measure**, after M43's R11-F6, R17-F9 and R17-F13 have landed.
   The gated firmware's `.text`, `.rodata`, `.bss`/`.data` symbols and
   stack (qemu, release and dev), and the review's probe firmwares
   (interactive, bind-only, cascade-only). All numbers go into one new,
   dated section of `docs/results-embedded.md`. Every later task cites
   that section and does not restate it.
2. **Commit the release-window gate, red:** a `just
   release-window-shape` step in `check-extra`. It builds `-p minimal`
   in release, counts `lumen_static_shell::recover::` symbols with a
   bound of 0, and prints the count beside the binary's `.text`. It
   also builds `-p minimal --features lumen-static-winit/reload` and
   asserts the count is **above** 0, so the `cfg` is shown switching
   both ways. The window crate already has the `reload` feature.
   **Red today** at the re-triage's 14 symbols (F-NEW-3), and marked
   expected to fail until T52.7.
3. **Record the rest.** F-NEW-1, F-NEW-2, R11-F4 and R12-F4 cannot be
   red in a committed gate until a gated binary contains a `#[bind]`, a
   dispatch and a cascade. **That is R11-F4 itself.** Their red states
   are committed in T52.1, with the binary that exhibits them. R11-F1,
   R11-F2 and R11-F3 hold at 1717808, because their defect is a missing
   gate. Their red states are the review's injections, recorded in the
   tasks that add the gates.

**Effort:** M. **Depends on:** M43 (R11-F6, R17-F9, R17-F10, R17-F13,
and the CI runner).

### T52.1 — an interactive firmware enters `check_text.sh` and `check_bss.sh`

**Closes:** R11-F4.

**Change:** promote the review's interactive probe to
`fixtures/embedded/src/bin/firmware_interactive.rs`. It has the same
tree plus one bound label, and it really dispatches a `PointerDown` and
`PointerUp` and a typed character (T13.5's path). It marks and clears
`Boundaries<16>`, cascades a three-rule sheet, and runs one `#[bind]`
update. Two single-purpose variants sit beside it: **bind-only**, with a
`#[bind]` and no pointer dispatch, and **cascade-only**, calling
`cascade_deep_with` with no dispatch. Both check scripts loop over the
binaries through M43's shared binary-resolution helper. Each binary
prints its own line. The interactive one asserts, before its number is
read, that `pass::dispatch`, an `on_event`, `cascade_range` and
`BindSite` are linked. That precondition is written in the style of
the Toggle check.

**Gate:** `check_text.sh` prints `.text N B of 65536 (P%)` and the
precondition counts for each binary. `check_bss.sh` runs the
zero-framework-statics check on each. The variants carry the symbol
checks that T52.3, T52.4 and T52.5 turn green, committed here as
expected to fail. **Committed red:**

- `check_bss.sh` on the interactive and bind-only binaries names the
  `__ln_site` static, with its full name, after the `$4` fix in this
  task (F-NEW-1);
- bind-only links `Node>::hit` symbols (F-NEW-2);
- cascade-only links `on_event` symbols (R12-F4).

**Injection:** (a) the review's `6697ad2`, a framework static in
`pass::dispatch`. `check_bss.sh` goes red on the interactive binary,
naming it, and stays green on the paint-only one. (b) The review's
`f32::clamp` edit takes the interactive binary over 65,536. Its ceiling
check fires. (c) Delete the dispatch call from the interactive binary.
The precondition fires, printing `0 pass::dispatch symbols linked`.
Accept: revert each, and the precondition and the ceiling pass.

**Docs in the same commit:** `docs/results-embedded.md` (the
interactive row, its ceiling decision from §2.5, citing T52.0's
section); the headers of `check_text.sh` and `check_bss.sh`; the
`[[bin]]` comments in `fixtures/embedded/Cargo.toml`; the fixture's
rustdoc that calls it "the M13 device's shape".

**Effort:** M. **Depends on:** T52.0; M43/R11-F6 (the helper this loops
through).

### T52.2 — float formatting and soft `f64` are symbol counts with a bound of 0

**Closes:** R11-F1, R11-F2.

**Change:** `check_text.sh` gains one "forbidden symbols" block with two
named counts, per binary. First, `core::num::flt2dec|core::fmt::float`.
Second, the soft-double intrinsics: `__aeabi_d*`, the `__*df*` family,
`__aeabi_f2d`, `__extendsfdf2` and `__truncdfsf2`. Each prints `N
(bound 0)`, and on failure lists the first five symbols. The two
counts are kept separate, so a failure says which property broke.

**Gate:** the summary line carries `float-fmt symbols N (bound 0),
soft-f64 symbols M (bound 0)` for each binary.

**Injection:** `2189c21` (both `Constraints::clamp` arms to
`f32::clamp`). The float-format count fires on the symbol count, while
the paint-only `.text` stays under the ceiling. That shows the check
does not ride on the ceiling. `12e6d3d` (the `f64` round-trip) makes
the soft-f64 count fire. Run both in one session, which also shows that
the patterns match the demangled `compiler_builtins` names as well as
the bare `__aeabi` ones. Accept: revert each, and both counts print 0
and pass. A third injection that only formats an `f32` with `Display`
must also fire the float-format count.

**Docs in the same commit:** `docs/results-embedded.md` (the threshold
table's `f64` row cites this gate; the "One `f32::clamp` cost 26.5 KB"
section says the property is now gated by a count); `check_text.sh`'s
header.

**Effort:** S. **Depends on:** T52.1 (the per-binary loop).

### T52.3 — a binding's cache claims no flash, and is exempted by pattern, counted

**Closes:** F-NEW-1.

**Change:** §2.1 (a). `BindSite` stores the index plus two, with 1 for
nowhere and 0 for unknown, and `BindSite::new` is all-zero.
`scripts/bss.allow` gains a pattern section with the argument, and
`check_bss.sh` matches exact names or patterns against the **full**
demangled name.

**Gate:** `check_bss.sh` prints `bss: N symbol(s), K binding cache(s)
(K×4 B, exempt by pattern)` for each binary. `check_text.sh` (or `size
-A`) asserts `.data` is 0 B on both firmwares and prints it.

**Injection:** go back to `UNKNOWN = usize::MAX`. The `.data` assertion
goes red at 4 B on the interactive binary. Remove the pattern from
`bss.allow`. `check_bss.sh` lists `__ln_site` by its full name. Accept:
restore both, and the gate is green with the count printed.

**Docs in the same commit:** `BindSite`'s rustdoc (its RAM cost on
embedded, and why it is exempt); `scripts/bss.allow`'s argument;
`docs/results-embedded.md` (the `.data` row); the T29.2/T29.3 entries
in `docs/results-damage-precision.md` (the embedded cost of the
per-site cache).

**Effort:** S. **Depends on:** T52.1.

### T52.4 — the cascade's arena walk is gated on `alloc`

**Closes:** R12-F4.

**Change:** §2.4. The three loops in §1.2 are wrapped in
`#[cfg(feature = "alloc")]`, with `cascade_arenas` gated to match.

**Gate:** the cascade-only firmware asserts 0 `on_event` and 0 `hit`
symbols linked, and a `.text` within 3,072 B of the paint-only
firmware. It prints `cascade firmware: .text N (paint-only M, delta D;
bound 3072), on_event symbols K (bound 0)`. It also prints, as a
record and not a gate, the same firmware's delta on the `alloc` tier,
which §2.4 says still pays.

**Injection:** remove the `cfg` from `cascade_deep_with`'s loop. The
gate goes red at the review's symbol count and delta. Accept: restored,
it is green at T52.0's measured figure.

**Docs in the same commit:** `cascade_deep_with`'s and
`cascade_arenas_deep`'s rustdoc; `docs/results-widget-crate.md` (a
qualifier on "`#[cfg(feature)]` and the linker … drop what a tree does
not name": not what a vtable reaches); `docs/results-embedded.md` (the
cascade row).

**Effort:** S. **Depends on:** T52.1. **Coordinate with M53**, whose
R6-F3 changes when `cascade_arenas_deep` runs. Whichever lands second
rebases onto the other, and this gate must stay green.

### T52.5 — the bind walk is static, as the observe walk is

**Closes:** F-NEW-2.

**Change:** §2.3. The derive generates the walk. Hand-written containers
take the provided default, and `node_of` dispatches statically where a
derived walk exists.

**Gate:** (1) the bind-only firmware asserts 0 `Node>::hit` symbols and
prints `.text N (bind-only; paint-only M, delta D; bound 1024)`. (2) A
unit test in `crates/lumen-static/src/bind.rs` (or its test file) keeps
`node_of`'s answers identical to the current implementation over the
existing T29.2 fixtures. It covers an enum's inactive variant and a
`None`, which resolve to their parent, and prints the first
disagreement.

**Injection:** route the derived walk back through `&dyn Node`. Gate
(1) goes red with `hit` symbols linked and a delta near the
re-triage's +3,480 B. Make the derived walk skip one field. Gate (2)
names the index. Accept: base passes both.

**Docs in the same commit:** `node_of`'s rustdoc (static where derived,
and the default's cost); `docs/results-damage-precision.md` T29.2 (the
flash cost, measured, then removed); `docs/results-embedded.md` (the
bound-firmware row).

**Effort:** M. **Depends on:** T52.1. Built after T52.4, because both
change what a firmware links through the vtable, and a delta measured
with both changes in flight cannot be attributed to either.

### T52.6 — the stack is measured, in release, under qemu

**Closes:** R11-F3.

**Change:** promote the review's `fwq` probe into
`fixtures/embedded/src/bin/fwq.rs` with its own linker script, and add
a `just stack` recipe to `check-extra` beside `thumb`. It paints the
stack, runs a cold and a warm frame, a typed character and a bound
update, and scans. `stack_bytes` gains an intercept fitted from the
depth sweep (§2.6), or its rustdoc says it is a per-level slope only.
The budget is stated as a release budget.

**Gate:** `just stack` first asserts the calibration: a known 2,000 B
local must measure within 2,000–2,100 B, so a broken painting method
fails loudly instead of reading 0. Then it asserts every measured call
≤ 4,096 B in release. It prints `stack: <call> X B / 4096 (model M)`
for each.

**Injection:** `937885b`, a 4 KB local in `TextInput::layout`. `just
stack` goes red naming the frame call, while the compile-time `STACK`
assert stays green. That is the finding, shown. A second injection
breaks the method itself: skip the paint. The calibration assertion
fires. Accept: revert, and green at T52.0's figures.

**Docs in the same commit:** `docs/results-embedded.md` (the stack row
reads "model M; measured X B release / Y B dev", citing T52.0's
section, plus §"The stack budget is enforced now"); `desc.rs:125-168`'s
rustdoc (the "does not vary with the tree" sentence is removed);
`fixtures/embedded/src/lib.rs:315-330`'s comment (the assert is a depth
backstop).

**Effort:** M. **Depends on:** T52.0; qemu-system-arm on the machine
M43's runner uses. M43 does not install it. Without it `just stack`
exits 77, and M43's skip policy (M43 §2.2) counts that as not run,
never as a pass.

### T52.7 — a release window without `reload` links no recovery code

**Closes:** F-NEW-3. **Amends:** `docs/plan-panic-recovery.md` rule 3.

**Change:** §2.2 (a). `RecoverPolicy`, the two marker types, and
`Local<A, P>` and `Live<'a, A, P>` in
`crates/lumen-static-shell/src/session.rs`. `lumen-static-winit` picks
the marker under its existing `cfg`. `lumen-static-reload` names the
recovering one. `RECOVER` and the `Recover` enum are removed in the
same commit, since nothing else reads them.

**Gate:** T52.0's `just release-window-shape` loses its expected-fail
marker. Without `reload` it counts 0 `recover::` symbols (bound 0). With
`lumen-static-winit/reload` it counts more than 0. Both counts are
printed beside `.text`. `just panic-drive` and the `fixtures/panic-app`
tests stay green, which shows recovery still works where it is on.
M39/T39.5's session test stays green too: its `Recover::Off` case,
where an application panic must still unwind out of `Session::agent`,
now runs under the crashing marker type.

**Injection:** hard-code the recovering marker in the release `cfg`
arm. The without-`reload` build reports the re-triage's 14 symbols
against a bound of 0, red. Put a runtime field back in `Local`. The
same. Accept: 0 without `reload`, above 0 with it, and the panic
fixtures green.

**Docs in the same commit:** `docs/plan-panic-recovery.md` rule 3 (the
dated amendment in §2.2); `recover.rs`'s header (lines 1-12);
`lumen-static-winit`'s rustdoc where `RECOVER` was (lines 240-251); the
`reload` feature's comment in `crates/lumen-static-winit/Cargo.toml`;
M39/T39.5's rustdoc on `Session::agent`, which names `Recover::Off`.
M55/T55.4 later adds a strategy parameter to `Local` after this one.

**Effort:** M. **Depends on:** T52.0 (its gate). **May run in Phase
3,** before the rest of this milestone, because it touches no embedded
code and needs no thumb toolchain. If it does, **it must be sequenced
with M49**, which also edits the session and reload paths, and it lands
after M49's session changes, not interleaved with them.

## 4. Boundaries of this plan

- **R11-F6** (the hard-coded `./target` path), **R17-F9** (`check_bss.sh`
  passes when `nm` fails), **R17-F10** (CI lacks the thumb target) and
  **R17-F13** (the stale headline) are M43's. This plan depends on all
  four and does not duplicate them.
- **R12-F2** (no gate executes core's `no_std`-only branches, such as
  bind's `SCRATCH_CAP` path) is M43's, and M43/T43.8 closes it with a
  host-side test in `fixtures/embedded`. T52.6's qemu firmware also
  **executes** a `#[bind]` update on `no_std`; that is a second
  executor, not the finding's gate, and this plan does not claim it.
- **The "has arenas" constant** (M26 Phase D, finding 9) is not built
  here. §2.4 records the `alloc` tier's remaining cost, and M53 owns the
  neighbouring R6-F3.
- **A trait split between `Node` and interaction methods** is not done.
  The review's verifier showed that the dispatch half of R12-F4 is the
  real price of input, and T52.1 prices it.
- **No won't-fix items.**

## 5. Kill rules and risks

This milestone moves bytes, not time. It has no speed-up to kill, but it
has two scope rules, written before the build:

- **T52.5.** If the derived walk does not bring the bind-only firmware
  within 1,024 B of paint-only, the remaining delta is attributed by
  symbol diff before anything else is built. The bound is not raised to
  fit.
- **T52.4.** If gating the loop on `alloc` leaves any `on_event` symbol
  in the cascade-only leaf firmware, something other than the arena loop
  reaches the vtable. It is found by symbol diff, and the task does not
  close until it is.

Risks, and what would reveal each:

- **The interactive firmware quietly stops being interactive,** because
  an optimisation removes the dispatch it was built to price. *Reveal:*
  T52.1's precondition counts, which read the symbols before the
  number.
- **qemu's stack figures differ from a real Cortex-M4F** by the
  exception frame or the FPU context. *Reveal:* the calibration line.
  §2.6 names a board run as owed. It is not claimed.
- **The policy type leaks into application code,** if an application
  ever names `Local`. *Reveal:* `cargo test --workspace` and every
  example building in `dev-shape`. Only the window crate and the reload
  crate construct it today.
- **`.data` returns through another derive-emitted static.** *Reveal:*
  T52.3's `.data == 0` assertion, now run over a firmware that has a
  `#[bind]`.

## 6. What building found (2026-09-27)

Dated notes, added as the tasks were built. The plan above is left as
written; where it was wrong, the note says so and says what was done.

- **T52.0 — the review's probe firmwares were not committed**, so the
  three binaries are rebuilt from §1.1's description
  (`fixtures/embedded/src/bin/firmware_{interactive,bind,cascade}.rs`).
  They land **with T52.0**, not T52.1, so that T52.0's figures are
  reproducible at its own commit; T52.1 puts them into the gates. The
  interactive build is **52,814 B** (80.6% of 64 KB), not §1.1's 58,052
  (88.6%): a different program with the same shape. Each finding shows
  in the binary that should exhibit it and in no other
  (`docs/results-embedded.md`, "The binaries a device actually is").
- **T52.0 — the tree is written once, as a macro.** `Button<A>` and the
  toggles are typed on their application, so the interactive
  application could not hold `Panel`'s `Ui`. `panel_tree!` expands both
  trees from one text; the paint-only `.text` is byte-identical
  (27,968 B before and after).
- **T52.0 — `check_bss.sh`'s `$4` cut is still there at HEAD.** M43/T43.4
  fixed the binary path and the `nm` failure, not the name extraction.
  T52.1 fixes it, as planned.
- **T52.0 — the stack rows move to T52.6's commit.** They need `fwq`,
  which is T52.6's deliverable, so T52.6 lands **before** T52.3–T52.5:
  T52.1 and T52.2 change no core code, so the stack T52.6 measures is the
  stack T52.0 would have.
- **T52.0 — the release window's recover count is 14 symbols, 13,964 B**,
  the re-triage's figures to the byte, and the same 14 with `reload` on.

- **T52.1 — the paint-only firmware's headline tolerance also catches
  the `f32::clamp` injection** (49,700 B against 27,968, +77.7%), so the
  review's "exit 0" for `2189c21` no longer holds since M43's headline
  check. The ceiling check is still what fires on the interactive build
  (73,356 B, 7,820 over). T52.2's counts are still needed: a smaller
  float leak inside 1% of the headline, or in a binary the headline does
  not cover, is invisible to both.
- **T52.1 — an XFAIL is a marker that must fail when its finding is
  fixed.** Each script's expected-red entry prints `expected red -- …`
  and passes while the finding stands, and fails with `remove the
  XFAIL` once the count reaches 0, so no marker can outlive its task.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R11-F4 | S2 | OPEN (worse) | T52.1 |
| R11-F1 | S2 | OPEN | T52.2 |
| R11-F2 | S2 | OPEN | T52.2 |
| F-NEW-1 | S3 | OPEN (new) | T52.3 |
| R12-F4 | S2 | OPEN | T52.4 |
| F-NEW-2 | S3 | OPEN (new) | T52.5 |
| R11-F3 | S2 | OPEN (slightly worse) | T52.6 |
| F-NEW-3 | S3 | OPEN (new) | T52.7 |
