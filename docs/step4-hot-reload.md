# Step 4 — hot reload: design notes and demo results

*2026-08-30. Handoff §7 implemented: `Reloadable` derive, save/fresh/
restore, `Msg`-as-data discipline, and a real dylib swap. Verified by 5
in-process tests plus `scripts/hot_reload_demo.sh`, which builds the same
app twice (v2 = "the edited code"), swaps live, and asserts every reload
class. The demo printed PASS on the reference machine.*

## The shape of the implementation

- **`Reloadable`** (`crates/lumen-static/src/reload.rs`): `save()` returns
  a `serde_json::Value` of the keep-half; `fresh()` is
  `Default::default()` — the constructor from whichever dylib is loaded;
  `restore()` grafts saved values onto a fresh tree.
- **`#[derive(Reloadable)]`** with three field classes:
  `#[reload(keep)]` (serde data, migrated), `#[reload(fresh)]` /
  `#[reload(reset)]` (untouched — the new build's value stands), and
  *unannotated* = nested `Reloadable`, which is what makes widget subtrees
  (`StaticTree` structs) compose: their keep-ness recurses.
- **`Vec<W: Reloadable>`: saved data drives arity** — `fresh()` is the
  zero-row list, restore rebuilds each element with fresh code and saved
  data. Tested with the handoff's own 47-row example.

> **Revised by M49** (`docs/plan-reload-hardening.md`). `fresh()` is gone
> (T49.4): an application is rebuilt by its program's **boot**, in a
> window swap and in `Host::reload` alike, and a container's fresh row is
> `Default`. And a kept widget saves **only its user's half** (T49.5):
> text and caret, values, scroll, open, selection, expansion, dividers —
> never a label's text, a button's label, a fixed size or a repeater's
> count and row height, which are code and must show an edit. The tree's
> default mode is keep, so `#[tree] #[reload(fresh)]` is no longer the
> taught pattern; `lumen-static-widgets`' `reload.rs` has the table, and
> `crates/lumen-static-shell/tests/reload_widget_state.rs` a row per
> widget (24 kept, 0 lost, where the taught pattern lost 23 of 23).

## Decisions (and two the handoff left open)

### Per-field fallback IS the §12.2 rule, generalized

`restore` pulls each keep field by name and deserializes it *alone*; any
failure — removed enum variant, changed type, missing field — keeps the
fresh value for that field only and logs through `reload::log_fallback`
(drainable via `take_log`, surfaced to the host). The demo deletes
`Mode::Fancy` in v2 and shows exactly one logged fallback while every
other field restores. This resolves §12.2 without an enum-specific
mechanism: "take the saved variant when it still exists" is just serde
succeeding; "fall back and log" is the per-field error path.

Corollary that matters for evolution: a field *added* in the new build is
simply missing from old bytes → fresh default + log, never a hard error.
JSON (rather than a positional binary format) is what buys this; it is a
dev-only path, so debuggability beats compactness.

**Two silent fallbacks closed (M49/T49.6, R5-F3).** A keep field whose
save failed was saved as `null`, and a `NaN` serializes as `null`; either
restored an `Option` as `None` over its fresh value with no log line —
and the save's own line is the old library's, drained before the
restore. `reload::save_keep` now leaves a failed value out (the restore
logs it missing), and `reload::restore_keep` refuses a `null` over a
non-null fresh value, logged "serialized as null".
`crates/lumen-static-shell/tests/reload_schema.rs` has the rows.

**A kept struct restores field by field (M49/T49.7, R5-F4).** A struct
that gained a field no longer deserializes whole, and used to fall back
whole: `Settings {a, b} → {a, b, c}` came back all defaults, and the
taught pattern keeps all state in one such struct. When a plain read
fails, `restore_keep` lays the saved object over the fresh value's own
serialization, key by key and recursively through objects, reads the
result, and logs each key the saved state lacked. Only after a plain read
fails, so an enum that changed variant is never merged into the fresh
variant's shape. `just window-reload-demo`'s `v-grow.so` adds a field to
the kept `Tally` in a live swap: the count survives and the warning names
`tally.step`.

### The no-code-pointers property is compile-time

Only `#[reload(keep)]` fields go through serde. A fn pointer or closure
marked `keep` does not implement `Serialize` and **fails to compile** —
stronger than the handoff's "the mirror type cannot contain those fields",
because here it is enforced at the annotation site. `fresh`/`reset`
fields can hold anything; restore never reads them. `Repeat`'s `bind` was
already a plain `fn` pointer (Step 3), so the §7 events-are-data
discipline held with no rework.

### §12.4, decided: the repeater window is disposable across reload

> **Amended 2026-09-02 (F1.7).** The `hot_take_log` shim is staged. It
> drains the core's log into a pending buffer and clears that buffer only
> after a successful copy, so an undersized host buffer refuses delivery
> instead of destroying the diagnostic — and the host uses the same
> two-call size probe as `hot_save` rather than guessing 64 KB and
> slicing past its own allocation. The demo exercises an undersized call
> deliberately.
>
> **Amended 2026-09-02 (F1.4).** Disposable, and *not trusted*. `count`
> and `item_extent` both come back from saved JSON, and both are ends of
> the division `visible_range` uses, so the reload path is the one place
> where an attacker-shaped value reaches core layout arithmetic. A
> subnormal extent cleared the old `<= 0.0` guard and overflowed the
> `as usize + 1`; `1e-6` with a large count asked for hundreds of
> millions of instances from forty bytes. `Repeat::restore` now rejects
> an extent below `MIN_ITEM_EXTENT` — keeping the fresh value, per this
> section's own per-field fallback rule — and `visible_range` caps the
> window at `MAX_WINDOW` regardless of how the fields were set.
> `crates/lumen-static/tests/reload_hostile.rs`.


`Repeat` keeps `count`, `scroll`, `item_extent` (the user's place in the
list survives the swap — visible in the demo as `rows=47 scroll=240`) and
drops the materialized window; it re-binds from data on the next frame
because it re-binds every frame anyway. Persistent *per-row* UI state
(e.g. an expanded row) does not exist yet; when it does, it must live in
the data model or in a keyed side store — not in window instances.

### Dev-mode leaks are policy, not accidents

Three deliberate leaks, all bounded per reload and all in the spirit of
§7's "dlopen without dlclose": the old `Library` is `Box::leak`ed (never
unmapped → no stale-code UB class), `Label::restore` leaks restored
strings (`&'static str` keeps leaves allocation-free in normal
operation), and the host keeps saved bytes in its own `Vec` (never
crossing allocators — §7's ABI rule).

## The demo (`fixtures/hot-app` + `fixtures/hot-host`)

The host follows the §7 sequence exactly: save while the old lib is
loaded (host-owned buffer, two-call length protocol) → drop the old tree
(old `Drop` impls run) → never `dlclose` → load new lib → `fresh()` →
`restore()`. The `extern "C"` surface is raw pointers and lengths only,
`unsafe`-annotated per clippy's FFI lint.

Observed across the swap (115 bytes of keep-state):

| class | field | before | after |
|---|---|---|---|
| keep | counter / title / rows / scroll | 3 / user text / 47 / 240 | unchanged ✓ |
| fresh | footer, theme | footer-v1, theme-v1 | **footer-v2, theme-v2** ✓ |
| reset | cache_hits | 2 | 1 (one post-swap frame) ✓ |
| §12.2 | mode | Fancy | Basic + logged fallback ✓ |

Reload latency budget: the swap itself is milliseconds; the wall-clock
cost is the leaf-crate rebuild, which Step 1 measured at ~90–250 ms —
comfortably inside the 2 s criterion end to end.

## Honesty list

1. **No ABI fingerprint.** Nothing verifies the dylib and host agree on
   `lumen-static`'s types; the fixture is safe because the boundary
   carries only bytes and opaque pointers *owned by one side*, which is
   the §7 rule — but a component system that passes framework types
   across the seam will need the fingerprint problem solved or avoided
   the same way. (Lumen's `hotpatch.rs` documents the same conclusion.)
2. **The reload surface is cfg-gated** (M49/T49.9). `#[derive(Reloadable)]`
   expands through the core's `__reloadable!`, which is nothing without
   `lumen-static/reload`, so an application that derives it builds with
   reload off and carries no serde_json: `fixtures/hot-window` forwards
   its `hot` feature to the reload features instead of enabling them
   always, and `just reload-gating` counts serde_json in its graph (0
   without `hot`, 2 with). The export line is `#[cfg(feature = "hot")]`.
3. **`fresh()` = `Default`** — sufficient until construction needs
   context (an allocator handle, §8's one type axis); revisit then.

## A panic, in a window and across the seam (M36)

An edit that panics is the ordinary case in a reload loop, so a panic is
caught, shown and survived — `docs/plan-panic-recovery.md` owns the
design; this is what it means for a reload.

**What is caught.** In a window: every path that runs application code
— input, applied messages, `update`, layout, paint, `tick`, the agent's
`Live::pump` — goes through one guard, `Host::guard`. That is compiled
in for debug builds and for release builds with
`lumen-static-winit/reload`; a release build without it dies, so a crash
reporter can see it. Across the dylib seam: every generated shim in
`export_reloadable_app!`, and any app verb that wraps its body in
`lumen_static_reload::panic::guard`, as `fixtures/hot-app`'s `hot_frame`
does. That catch has to be **inside the library**: a `cdylib` links its
own std, so it has its own panic hook, and a hook installed by the loader
never sees the library's panic — the location, which exists only inside
a hook, would be lost. The report crosses as bytes through
`hot_take_panic`. `just hot-demo` asserts both halves: v3's report names
its own line, and the loader's hook saw zero panics.

**What is kept.** The application's state, exactly as the panic left it
— possibly half-written, with no rollback, because a last-good copy would
cost every frame to protect against a once-per-bad-edit event. Focus is
kept. The host's own invariants are rebuilt: the display list and
overlays cleared, the next frame repaints in full, pointer capture and
hover/press released. After the demo's v3 panic, v4 restores the same
kept state and draws v2's frame exactly.

**What is not recovered.** State the panicking handler half-wrote; events
queued behind the trigger in the same pump (dropped — they were made
against a frame that never finished); input while the screen is up
(swallowed, except R, close and resize); a panic under `panic = "abort"`,
or one raised while already unwinding; and a library panic in a verb that
does not wrap itself in `guard`, which still aborts the loader — the
demo's script says so instead of leaving a bare `Aborted`. Task panics
are M28's `Failure::Panicked`, not this.

## Reloading inside a window: where the `Host` lives (M37)

hot_ice keeps the application state in the host and calls into the
library per function, which works because iced's `view` rebuilds the
widget tree every frame, so no code pointer outlives a call. **Here the
tree is retained and it holds code.** Handler `fn` pointers live in it,
and every widget's `layout` and `paint` is monomorphised over the
application's types. A `Host<A>` compiled into the loader would be the
loader's code forever. So **the `Host` lives in the library**, and the
window drives it through `lumen_static_shell::session::Session`.

The window itself is no longer generic over the application: input
goes in and a frame comes out, along with the title, the cursor and the
agent's requests. A statically linked application is a `Local<A>`
behind the same trait, so the static window and the reloading window are
one code path.

Four rules hold that path together:

- **A swap boots the new build's own `program()`** and grafts the saved
  state on through `Host::restore`. Nothing is constructed through a
  pointer held from the old build, which `just window-reload-demo`'s
  injection demonstrates. The window path therefore no longer uses
  `Reloadable::fresh()`.
- **Every session method that runs application code catches its panic
  inside the library**, which has its own std (M36/T36.4).
- **The swap happens between frames**, and the new session is booted
  before the old one is dropped. A refused library, or a boot that
  panics, therefore leaves the running one in place.
- **No library is unloaded** (`docs/plan-window-reload.md` §2.5). The
  memory this costs is measured below, and unloading is deferred on
  three conditions: every worker thread that may run old code is joined,
  no `&'static` from the old image survives, and no chained panic hook
  points into it.

## The window's boundary: pointers behind a stamp (M37/T37.0)

A window that reloads keeps its `Host` in the library, because the tree
holds code (`docs/plan-window-reload.md` §1). So each frame, the loader
either **borrows** the library's display list or **copies** it as bytes.
`docs/plan-window-reload.md` §2.3's kill rule was written before the
probe: bytes if copying costs less than the frame arm's fast-cluster
spread, pointers with an ABI stamp otherwise.

The probe is `examples/showcase`'s `boundary::measure_the_copy` (`cargo
test --release -p showcase measure_the_copy -- --ignored --nocapture`).
The frame arm is T34.2's instrument: a hover that moves, `Host::timings3`
(layout + place + paint), the median of 40 frames, eight runs. It
measures two copies. A `memcpy` of the commands and text into buffers
already allocated is the lower bound of any copy, but it is **not** a byte
boundary, because `repr(Rust)` promises no layout across builds. The
second is `lumen_static::wire`, a tagged little-endian encoding, which is
what a byte boundary would actually ship. It is encoded and then decoded
into a reused list.

| entry | cmds | text B | wire B | frame, 8 runs (µs) | spread (µs) | `memcpy` min (µs) | wire min (µs) |
|---|---:|---:|---:|---:|---:|---:|---:|
| Button | 15 | 200 | 697 | 0.663–0.732 | 0.069 | 0.020 | **0.153** |
| DataGrid | 32 | 380 | 1,458 | 8.215–9.871 | 1.656 | 0.025 | 0.370 |
| Markdown | 35 | 317 | 1,486 | 7.677–7.962 | 0.285 | 0.023 | **0.336** |
| ColorPicker | 12 | 232 | 622 | 0.640–0.647 | 0.007 | 0.017 | **0.120** |

**Verdict: pointers, behind a stamp.** The encoding exceeds the frame
arm's spread on three of the four entries, and only DataGrid's wide
spread covers it. The `memcpy` would pass on three, but it is not an
option. Two things the rule did not count are recorded here, so the
choice can be revisited on its own evidence. The absolute cost is
0.12–0.45 µs, which is nothing next to a raster pass. And the frame arm
is showcase's, whose frames are small enough that 0.15 µs is 23% of one.

**The stamp** (`lumen_static_reload::abi`): each image describes itself
as `name=value` lines covering:

- the compiler (from a build script);
- the target and the panic strategy;
- the core's version and **feature set**;
- the size and alignment of the core types that cross;
- **the source** (M49/T49.1): a hash of `src/` and `Cargo.toml` of each
  crate whose types cross — `lumen-static`, `lumen-static-shell`,
  `lumen-static-reload` — plus the locked versions of the external
  crates those manifests name, and the encoded rustflags. It is taken by
  `lumen-static-reload`'s build script, so it costs nothing at run time.

The loader refuses a library whose lines differ, naming the first field
that differs. `just abi-stamp` builds `fixtures/stamp-lib` twice: once as
`fixtures/stamp-host` was built, which is accepted with (at M49) 17 fields
agreeing, and once with the core's `observe` on, which is refused with
`lumen-static features` named.

**The feature field is load-bearing, and the sizes are not enough.** With
it removed, the `observe` build was *accepted*, because none of the seven
listed sizes moves even though every widget's vtable does. The stamp
cannot see a `#[global_allocator]`. A hot library must not set one,
because values are allocated on one side and freed on the other.

**Sizes could not see a reordering** (R4-F5). A library built after
`Cmd::Rect` was moved below `Cmd::Text` agreed on all 12 fields and was
then misread command by command. `just abi-stamp` now builds a copy of
the workspace three ways: unchanged (accepted — the hash is of content
and relative paths, not of where the workspace lives), with the two
variants reordered (refused by `source lumen-static`, and the message
says the framework was edited and the window must restart), and with
only `lumen-static-widgets` edited (accepted, because no widget type
crosses; `every_crossing_type_is_defined_in_a_hashed_crate` checks that
claim against `type_name`). *Injection:* the source lines left out of
the stamp, and the reordered library is accepted again.

What it deliberately does not cover, and why:

- **Any edit to the three crates refuses every later build** until the
  window restarts, a comment included. That is the right answer: the
  loader still runs the old framework.
- **Transitive dependency versions are not hashed**, only direct ones.
  Two wider forms were built and both refused an unchanged copy of the
  workspace: a lockfile lists a member's dev-dependencies, and resolves
  a registry crate's optional dependencies only where a member turns
  them on. No crossing type holds a third-party type today.
- **`lumen-static-macros` is not hashed.** A derive's output lands in
  the application's own types, which live in the library alone.

## Reloading inside a window: latency and memory (M37/T37.4)

`just hot-window-drive` (`scripts/hot_window_drive.sh`) runs the live
window on 2026-09-24. It is `fixtures/hot-window` built `--release
--features hot,agent`, in a copy of the crate that the drive edits with
its own writes. Plan rule 4 says the window must be measured before it
is called usable:

| quantity | measured | how |
|---|---|---|
| edit to swapped frame | **0.46–0.64 s** per reload (three reloads: a format edit, a panic introduced, the fix) | wall clock from the file write to `app.reloads` counting the swap, polled every 0.2 s, so up to 0.2 s of it is the poll |
| resident memory per reload | **+13.2 MB** (62,624 → 102,320 kB over three reloads) | `VmRSS` before the first edit and after the last |

The latency is almost all `cargo rustc` of one crate, incremental, into
`target/reload`. The swap itself is a boot of the new program plus a
restore. **The memory is not attributed.** Two things grow on every
reload: the libraries are never unloaded (§2.5), and each new session
loads the system face for its own text measurement. Neither has been
measured apart from the other, and splitting the figure is the first
step if it ever matters.

## How a descriptor crosses (M28/T28.7)

Deferred work in flight across a swap is the case item 1 above warns
about, so it crosses the way item 1 requires: **as primitives**.

A `Descriptor` is `(kind, step, payload)` — three `u64`s — and it
crosses as three arguments to an `extern "C"` verb, reassembled on the
far side. It is never passed as a `lumen_static::spawn::Descriptor`
value, even though both builds nominally have that type: v1 and v2 each
link their own copy and no layout agreement is promised, which is item
1's whole point.

**The chain's state lives in the loader process**, outside both dylibs —
the inbox, the epoch and the pending descriptor. That arrangement is
forced by §7's sequence rather than chosen: step 2 of the sequence drops
the entire app object while the old library is still loaded, so anything
that must survive the swap cannot live in the app.

The dylib contributes one thing: **what a step means**, through
`hot_step(kind, step, payload) -> u64`. It takes no session pointer,
deliberately — a step is a pure function of its descriptor, and handing
it the app would invite a step that mutates state the host is about to
restore over.

`just hot-demo` asserts the whole path, and
`docs/events-decision.md`'s async-boundary section has the numbers and
the injection.
