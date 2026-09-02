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
