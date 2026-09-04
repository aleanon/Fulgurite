# Streaming retention vs. a copied display list — M13/T13.8

**Ran 2026-09-05.** Reference machine: i9-13900KF, governor
`powersave`, turbo on — the same machine every other `docs/results-*.md`
number here was taken on. `cargo run --release -p benches --bin retain`,
median of blocked runs.

## The question

T13.8 replaced retention that kept the previous `DisplayList` with
retention that observes commands as they stream past a `Painter`
wrapper, keeping `(hash, ink)` per command. The motivation was that the
old shape *typed* `Retention` against a list — `record(&DisplayList)` —
so a host with no list could not have damage at all, which is the M13
device (`docs/results-embedded.md`).

But motivation is not permission. Streaming hashes every command where
copying compared stored ones, so it might simply be worse, and the plan
set the gate accordingly: **keep the copying implementation if streaming
is not faster.**

## The numbers

Commands are issued straight at a `Painter`, not painted from a tree:
that is the only way to reach a 2 000-command frame, and it keeps the
tree walk — which both forms pay identically — out of the measurement.
`build the list` is the shared floor; retention's own cost is the
difference from it.

| | 34 commands | 2 000 commands | ns/command |
|---|---:|---:|---:|
| build the list (floor) | 225.5 ns | 12 768 ns | — |
| **copying**: copy + diff | **88.8 ns** | **6 879 ns** | 3.44 |
| **streaming**: `Incremental` | **79.8 ns** | **5 421 ns** | 2.71 |
| **streaming**: `Streaming<4096>` | **33.9 ns** | **2 756 ns** | 1.38 |
| `Recording` wrapper alone (`Rebuild`) | ~0 | 155 ns | 0.08 |

And the row that only the new shape can produce at all:

| | 34 commands | 2 000 commands |
|---|---:|---:|
| streaming, **no list built** | 197.6 ns | 11 435 ns |

## Verdict

**Streaming is faster at both scales, so it replaces the copying form
rather than joining it.** No second `Retention` impl is needed for the
desktop.

Four things the measurement says that the plan did not predict:

1. **`Streaming<N>` beats `Incremental` by 2×** (1.38 vs 2.71 ns per
   command). Same algorithm; the difference is a fixed array indexed
   inline against a `Vec` that costs a pointer load and a second bounds
   check per command. The allocator-free implementation is the *fast*
   one, which is not the usual direction.
2. **`Recording` is free.** 0.08 ns per command for the wrapper, at the
   noise floor. Wrapping the painter costs nothing measurable, which is
   what makes the `paint → Recording(Scaled(sink))` composition
   affordable.
3. **Damage with no list is cheaper than a list with no damage.**
   11 435 ns against a 12 768 ns floor: a host that streams to an
   immediate-mode painter *and* computes damage does less work than one
   that merely builds a display list. That is the M10a seam paying off.
4. The desktop saving is ~4.1 µs a frame against a render measured in
   `docs/results-frame-cost.md` — real, but the reason to do this is the
   device, where the cost is the SPI bus rather than the raster.

## What it costs

**Equality became a 64-bit hash.** Two genuinely different commands that
collide would leave stale pixels. At ~2 000 commands per frame that is
a birthday probability around 10⁻¹³ per frame.
`crates/lumen-static-shell/tests/damage_oracle.rs` is the standing
check: whatever computes the damage, a partial redraw must stay
pixel-identical to a full one. It is parameterised over `Retention` as
of this task, so `Streaming<256>` gets the same 25-step pixel
comparison `Incremental` does — an implementation nothing renders
through is one nobody has checked. `Streaming<2>`, which cannot hold
the frame, is checked separately: it must report `All` on every step
and never a rect computed from mismatched slots.

**And one thing it gained.** `Cmd::Text` carries a *range* into a list's
text buffer, so two frames whose strings differ but sit at the same
offset and length — "12" becoming "13" — produced identical commands.
The copying diff needed a special case (`text_differs`) to catch that,
and a label that never repaints is what forgetting it looks like. A
stream is handed the `&str`, so the case does not exist to forget.
