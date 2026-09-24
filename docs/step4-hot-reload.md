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
2. **Reload surface is not yet cfg-gated.** serde/serde_json ride along
   in release builds; §9 says the dev surface must compile to nothing.
   Deferred to the observability step, which shares the gating.
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
