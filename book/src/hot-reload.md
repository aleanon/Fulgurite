# Hot reload

The model in one line: **throw the tree away, rebuild from new code,
graft the old values back on.**

## Three functions

* `save()` — returns a value holding only the *keep* half of the state.
* `fresh()` — the constructor from whichever dylib is currently loaded.
* `restore()` — grafts saved values onto a fresh tree, **per field, with
  fallback**.

Per field with fallback is the important phrase. Any incompatibility —
a missing field, a removed enum variant, a changed type — keeps the
**fresh** value for that field alone and logs it. One field going stale
does not abort the reload, and a reload does not silently produce a state
that no version of the code could have written.

## `#[reload(...)]` on each field

```text
#[reload(keep)]    survives the swap
#[reload(fresh)]   comes from the new code
#[reload(reset)]   recomputed after the swap
```

Choosing between them is a design question about the field, not a
mechanical one: a scroll position and a counter are `keep`; a theme
constant or a new field is `fresh`; a cache is `reset`, because carrying
a cache across a code change is how you get a stale answer that looks
plausible.

## Code pointers cannot be kept, structurally

A `fn` pointer or a vtable marked `#[reload(keep)]` **fails to compile** —
`Serialize` is not implemented for it.

That matters more than it sounds. The rule "saved state cannot contain
code pointers" is a real requirement (the old dylib is unloaded; a
retained pointer into it is a crash waiting for the next call), and here
it is enforced by the type system rather than by discipline. You cannot
write the bug.

This is also why several things in the framework are `fn` pointers
rather than closures — a `Repeat`'s row configuration function, a
handler. A closure could capture, and a capture would be exactly the
thing that cannot survive.

## What a swap actually does

`fixtures/hot-app` and `scripts/hot_reload_demo.sh` run the whole cycle
and assert each clause of it. Reading its output is the fastest way to
understand the semantics — it checks that a counter is kept, that a
renamed title is kept, that a repeater's row count is kept, that a scroll
position is kept, that a new field is fresh, that a cache is recomputed,
that a derived value is re-derived, and that an enum variant removed in
the new code falls back to the fresh value and **logs** it.

That last one is the interesting case, and it is the one people assume
will simply work.

## Why the boundary matters

A reload is one dylib and one save/fresh/restore cycle, and it is fast
because the rebuild is one crate. That is the same seam as the compile
firewall, the erasure dial and the observability namespace — see [The
component boundary](./component-boundary.md), which is where the argument
for making all four coincide lives.

If reloads are slow, the answer is usually that the component boundary is
in the wrong place, not that reload is slow.
