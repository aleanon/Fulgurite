# Handoff — 2026-09-23

*Session handoff, not a design document. `docs/00-handoff-static-tree.md`
is the design spec and is unrelated to this file. Delete this once you
have picked the thread back up.*

**The repository moved.** It is now `/mnt/Utvikling-linux/lumen-static`,
with a compatibility symlink at `~/lumen-static`. It used to be in the
encrypted home; the new volume is plain ext4, so **the source is no
longer encrypted at rest**. `cargo clean` freed 55.6 GiB during the move
— `target/` had reached 55 G of a 57 G repo, and the root filesystem was
at 100%.

---

## Where things stand

`main` is green: `just test`, `just static`, `just hot-demo`.

| branch | head | what it holds |
|---|---|---|
| `main` | `29c1238` | M31 complete, plus one prototype (below) |
| `proto/state-split` | `b7ddf07` | three prototypes, **not merged** |

Two milestones closed this session: **M29** (partly) and **M31** (fully).

---

## 1. What is decided and shipped

### M31 — `Application`, done (`docs/plan-application.md`)

An application had two construction paths and nothing checked they
agreed: its author wrote `Counter::new()`, and the framework rebuilt it
with `Default::default()` because `Reloadable::fresh()` derived to
exactly that.

- `Application::build()` is the one path; `fresh()` routes to it. **The
  derive needs no new attribute** — an application is the thing with a
  `#[tree]` field, and a field attribute is visible to every derive on
  the struct.
- `Application::style()` is re-asked on every swap, so an edited
  stylesheet reaches the frame. It returns `Stylesheet<'_>` (= `&[Rule]`)
  and **not** a generalised trait; §2.4 records why, and the reason that
  decides it is that `Vec<Rule>` in the signature would force the trait
  to need `alloc`.
- `Host::launch` / `adopt` / `reload` (the last behind a `reload`
  feature).
- A reloadable application that also derives `Default` is a compile
  error, via a coherence conflict — Rust has no negative bounds, so
  rustc answers E0119 and *prints the trait's name*, which is why the
  name is a sentence.

**Two of the four tasks were rewritten by building them**, and both
corrections are in the plan rather than quietly applied: T31.1's scope
(25 types → 4) and T31.3's premise (it specified a check against a
`Host` that `fixtures/hot-host` does not have).

### M29 — partly (`docs/plan-damage-precision.md`)

T29.0, T29.1 and T29.5 are done. **T29.2, T29.3, T29.4 are open**, and
§2.5 is the reason to think carefully before starting them — see
"decisions waiting" below.

The headline: narrowing a `#[bind]`'s mark is worth **20.7 µs, 36% of a
frame**, and **zero on any frame a click produces**, because
`pump_with` calls `mark_all()` for a consumed event at
`crates/lumen-static-shell/src/lib.rs:694`, eighty lines before
`App::update`.

### Benchmarks re-measured (`docs/results-competitive.md` §7)

Against iced, GTK, Qt, Xilem/masonry, egui and Lumen. **lumen-static is
9–14% faster than in September**; nothing regressed. The run's real
output was a harness defect: M30 had added `features = ["observe"]` to
the *plain* arm's dependency line, which cost it 25% of its frame and
read as a framework regression. Bisected, fixed, and two wrong
explanations are recorded beside the right one because both were
measured and discarded.

`docs/results-gpu.md` and `docs/results-vello.md` were re-run too.
`just raster-sweep` had been broken since M16 (`cargo run -p` went
ambiguous when the package gained a second binary) — fixed.

---

## 2. Three prototypes, none merged into a plan

### (a) `Program` builder — on `main`, `29c1238`

```rust
application(|| build(14.0))
    .title("glass").size(460, 620).page(PAGE)
    .style(|d| &d.sheet)
    .run();
```

**Why it exists:** M31 collapsed construction to one path and left
*styling* with two — `run()` never calls `Application::style()`, it uses
`Config::stylesheet` (`lumen-static-winit/src/lib.rs:340–343`). Same
defect shape, one field over.

**The insight:** `build` is an `fn` pointer, and a non-capturing closure
coerces to one. So parameterised constructors work with no associated
type, and rustc enforces the no-capture rule itself:
*"closures can only be coerced to `fn` types if they do not capture any
variables"*.

**The cost:** the borrowed `Stylesheet<'_>` means a computed sheet must
live in a field. `Counter` and `Demo` both carry `sheet: Vec<Rule>` now.

### (b) State/tree split — branch, `d861f4f`

`App` gains `type State` and `state_mut()`; `Tree: Handlers<Self::State>`;
dispatch calls `h(state.state_mut(), &ctx)`.

**Why:** with the tree out of a handler's reach *by type*, every tree
write comes from a binding — so `pump_with`'s `handled → mark_all` could
narrow, and M29's 36% becomes reachable on the frames that matter. The
constraint is already obeyed: **no handler in any example writes the
tree.**

Proof it holds:

```
t.ui.title.set("reached the tree");
error[E0609]: no field `ui` on type `&mut Tally`
              available field is: `count`
```

Backwards compatible: absent `#[state]`, the derive emits
`type State = Self`. **Cost: 26 hand-written `App` impls gained two
lines each.**

### (c) `#[bind]` source path — branch, `a1ded79`

```rust
#[state]
#[bind(ui.value, count, "count: {}")]
state: Tally,                            // → self.state.count
```

**This was the gate on (b).** Without it a split application could only
bind through `with =`, which `plan-damage-precision.md` §2.4 lists as
permanently unnarrowable — so the split would have bought M29's win on
the handler path and handed it straight back on the binding path.

The load-bearing test is `a_source_path_binding_still_reports_precisely`:
a second update that wrote nothing must answer `false`. Two injections,
both firing by name.

---

## 3. Decisions waiting for you

**1. Does the state/tree split ship?** The gate opened, so this is now a
straight trade: ~26 mechanical two-line additions against M29's 36% on
handler frames. If yes, it should land *before* T29.2–T29.4, because
§2.5 is what makes those worth building at all.

**2. Is `Boundaries` going to be the default?** `lumen_static_winit::Inval`
is `Everything` unless the non-default `boundaries` feature is set — so
**every invalidation win in M29 is worth zero in a turnkey window
today**. This compounds with §2.5 and is recorded in
`plan-damage-precision.md` §1. Flipping it is its own evidence-gathering
job: four of M26's five Phase A defects were invisible under
`Everything`.

**3. Does the `Program` builder become M32?** It closes the two-path
styling gap. The cost is the `sheet` field on every app with a computed
sheet.

**4. Paint culling.** Paint is 28.6 µs of a 32.3 µs narrowed frame, and
`plan-damage-precision.md` §4 says the reopen condition is **not** met —
that condition is off-surface node count, and the grid has 220 of 2,227.
Reopen when a fixture puts most of its nodes off the surface.

**5. The `observe` feature costs 24% of frame time when merely compiled
in**, with no agent attached and no journal running
(`results-competitive.md` §7). Unverified hypothesis: it adds
`probe`/`observe` to every widget's `Node` impl, and `probe` calls
`layout`, giving it a second call site. Worth a task — it is the largest
single frame-time item now on the table and applies to every frame.

**6. Panic recovery at the reload seam.** `hot_ice`
(`/mnt/Utvikling-linux/hot_ice`) wraps every hot call in `catch_panic`.
`just hot-demo` has no equivalent: a panic in reloaded code kills the
process, at exactly the moment you are mid-edit.

---

## 4. Things that will bite you

- **Another session works in this repo.** It committed with `git add -A`
  twice this session and swept one of my edits (`Host::timings3`) into
  its unrelated `fix(widgets)` commit `9f41a66`. **Stage by name.** I did
  the same to it earlier and had to `reset --soft`.
- **`cargo doc` runs only in `just test`, not `just static`**, so a
  commit can land with broken intra-doc links and nothing says so until
  someone runs the full suite. M30 did exactly that; `971040c` fixed five.
- **An intra-doc link in a module-level `//!` comment does not resolve
  against its own module's scope** on this toolchain — `[`Failure`]`
  fails fourteen lines above `pub struct Failure`. Use a `crate::`-prefixed
  path, or a plain code span.
- **A link to a `cfg`-gated item does not resolve** in a default-feature
  doc build. `Host::reload` is a code span for that reason.
- **`just static`'s doc-identifiers gate is strict and useful** — it
  caught two invented names in a test header this session, including a
  backticked *fragment* of one.
- Use `&&`, not `;`, between `just test` and `git commit`. I used `;`
  once and committed on a red suite.

---

## 5. Reproducing the numbers

| what | how |
|---|---|
| M29's probe | `just grid` |
| competitive comparison | `benches-competitive/competitors.sh run <arm> <n> <point\|churn>` |
| both backends | `just raster-sweep` |
| our rasteriser vs Vello's | `just vello` (and `just vello "" 0` for single-threaded) |
| the frame profile | `just profile` |

Every figure lives in exactly one `docs/results-*.md`; everything else
cites it. That rule is load-bearing and `just static` partly enforces it.
