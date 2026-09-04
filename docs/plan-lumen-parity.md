# What feature parity with Lumen would take

*2026-09-01. A survey, not a commitment. Measured against the Lumen tree
at `/mnt/Utvikling-linux/Lumen` (~62k lines of framework across 20
crates) and its own two readiness assessments, which are unusually candid
about where it is and is not done.*

> **Superseded as a plan, 2026-09-04.** Parity stopped being a
> destination when two of the figures below stopped being true. **"About
> 10k is adoptable" is false**: `lumen-render` and `lumen-text` were
> measured and declined (74 and 84 transitive crates, both hard-depending
> on `lumen-core` — `docs/text-render-adoption-decision.md`), and
> `lumen-style` was checked on 2026-09-04 and depends on `lumen-core`,
> `lumen-layout` *and* `lumen-text`, so it is in the same position.
> Nothing here is adoptable, which means parity is ~35–40k lines of
> someone else's library written from scratch. **And "~4,300 lines, 4
> widgets" is stale by 2.5×**: 11,101 lines and eleven widget types as of
> 2026-09-04, with a third of Phase B already built.
>
> Phases A–E became milestones instead, and only where they earned it:
> Phase A shipped as M5/M7, Phase B is now M11, Phase E is M12, Phase C
> is unscheduled, and **M10 is the embedded profile — which is not in
> this document at all**, because it is the thing this architecture can
> do that Lumen cannot. See `docs/plan-architecture-v2.md`.
>
> The survey below is left as written. It is still the best map of
> someone else's territory, and useful for orientation; it is no longer
> useful for navigation.

## The scale, stated plainly

| subsystem | Lumen | ours | verdict |
|---|---:|---:|---|
| widgets | 17,534 | ~250 (4 widgets) | **build** — architecture-bound |
| app runtime | 12,292 | ~700 | **mostly N/A** — see below |
| render | 7,997 | 0 | **adopt** |
| core | 7,926 | ~1,500 | partly N/A, partly done |
| style | 4,322 | 0 | adopt the parser, build the application |
| shell (desktop) | 2,780 | 0 | build (small, but OS-facing) |
| text | 2,604 | 0 | **adopt** |
| agent | 1,611 | 1,320 | **done** |
| cli / dev server | 1,602 | ~100 (scripts) | later |
| layout | 946 | ~200 | **build** — and a decision to make first |
| test harness | 931 | 0 | build |
| macros | 633 | 563 | done |
| mobile/web shells | ~830 | 0 | later |
| **total** | **~62,000** | **~4,300** | |

The raw ratio (14×) overstates the work, and understates it in one place.
Both corrections matter more than the number.

## Two large chunks are *not* parity targets

**`lumen-app` (12.3k lines) is mostly machinery we deliberately do not
have.** It is `Element` construction, rebuild/reconcile, the cascade
application pass, memoized subtree reuse, and scope bookkeeping — the
runtime cost of a framework that re-runs a `build` function every frame.
Our tree is a struct that is never rebuilt. Roughly two-thirds of that
crate has no counterpart here, and its absence is the architecture
working as designed, not a gap.

**Signals and the keyed state store** (a large part of `lumen-core`) are
the same story: state lives in our tree's fields, so there is nothing to
key, hash, or invalidate. This is also why `ui.getDeps` was recorded as
out-of-scope-by-architecture in `docs/plan-agent-parity.md` rather than
as a TODO.

Netting those out, the honest target is closer to **~35–40k lines**, of
which ~10k is adoptable rather than writable.

## Two large chunks should be adopted, not rebuilt

§11 of the handoff is explicit: *compose rather than build; make the
contribution the node-table architecture.* Two Lumen crates are
architecture-neutral and can be taken nearly as-is:

- **`lumen-text` (2.6k)** — a parley/swash wrapper: shaping, font
  fallback, a text editor model, rich text, glyph rasterization. It
  touches `lumen-core` in exactly one place. This is the crate §11 warns
  takes *years* to write, and it is sitting there finished.
- **`lumen-render` (8.0k)** — display list, tiny-skia CPU backend, wgpu
  GPU backend, gradients, atlas, SVG, images, blend modes, backdrop
  filters, and an APCA contrast analyzer. Depends on `lumen-core` only
  for value types.

Adopting these is what converts our paint checksum into pixels. It is
also the single highest-leverage move available, and it is mostly
integration work rather than invention.

`lumen-shell` (2.8k, winit + arboard) is *not* directly adoptable: it
depends on `lumen-widgets` and drives Lumen's frame loop. But it is small
and the pattern is clear — our version feeds the existing `InputQueue`
and calls our passes.

## Architectural decisions that must be made *first*

> **All four decided 2026-09-01** — `docs/layout-and-storage-decisions.md`
> (text storage, `Length` instead of taffy/flex, style array, §6 build
> order) plus the traversal result in `docs/results-traversal.md`. The
> work is sequenced in `docs/plan-architecture-v2.md`; Phase A below is
> its M5.

These are not effort items; they are forks in the road that later work
depends on. Each is a gap in our own spec, not just versus Lumen.

1. **Text storage.** `Label.text` is `&'static str`. No application can
   display a computed value — a counter, a name, a formatted number —
   without leaking. This blocks essentially every real app and it is the
   first thing to fix. It interacts with §3 (values in fields), with
   `no_std` (an arena, not `String`, on that profile), and with hot
   reload (already handled: `Label::restore` leaks deliberately).

2. **Layout engine: taffy or our own?** Lumen wraps taffy (946 lines
   because taffy does the work). Taffy builds *its own* node tree — a
   second tree beside ours, which is exactly the duplication our flat
   table exists to avoid, and Lumen's own CP5 measured the cost of
   maintaining it. Writing flex directly over the flat table is more work
   but is arguably the point of the architecture. **This decision gates
   the entire widget library**, because every widget's `up`/`down` is
   written against whichever model wins.

3. **Style values live where?** Spacing is currently a macro attribute
   constant, baked into a generated `fn`. Real styling needs per-instance
   values, which means fields, which means the derive must thread them —
   and then `.lss` (or an equivalent) needs a cascade that writes into
   those fields. Lumen's `lumen-style` parser (4.3k) is adoptable; the
   *application* half is architecture-bound.

4. **§6's escape hatches are DECIDED but unimplemented.** `Option<W>`,
   `Either<A, B>`, `enum` variants, and `Box<dyn Widget>` islands all
   appear in the spec as settled, and none exist. Only `Repeat` was
   built. Every non-trivial UI needs conditional subtrees, so this is
   load-bearing for parity and is *our own* spec debt.

## Ordered plan

### Phase A — make it real (the credibility milestone)

Nothing is demonstrable until pixels appear in a window.

1. Text storage decision + a `Text` type that can hold owned strings.
2. Adopt `lumen-render`: replace the `u64` paint checksum with a real
   `DisplayList`. Ops signatures already have the right shape
   (`out: &mut u64` → `out: &mut DisplayList`).
3. Adopt `lumen-text`: real measurement in `Label::up`, via the concrete
   `LayoutCx` the renderer/executor answer already sketched (a context
   struct threaded through the ops, `dyn` inside, no new type parameter).
4. A minimal winit shell feeding the existing `InputQueue`.

**Exit:** the `counter` example, on screen, with real glyphs, driven by
the event system that already exists.

### Phase B — a usable widget vocabulary

The layout decision from above lands first, then the widgets it enables.
Lumen's 17.5k is a fair proxy for the *whole* library; a credible v1 is
perhaps a quarter of it: text input (with IME and selection — the
expensive one), scrollable, checkbox/radio/switch, slider, text area,
container/card, tabs, dropdown/select, tooltip, modal, and the §6 escape
hatches as first-class widgets.

### Phase C — styling

Adopt the `.lss` lexer/parser/AST; write the cascade against our fields;
animation and transitions after that (Lumen has springs and motion, and
those interact with the invalidation work §11 keeps flagging as the
actually-effective optimization).

### Phase D — platform and content

Multi-window, clipboard, drag and drop, file dialogs, images (png works
in Lumen; jpeg/webp were still follow-on there), SVG, i18n. Mobile and
web shells last — Lumen's own assessment grades those as one-shot render
cores assembled by hand, so there is less to inherit than the crate list
suggests.

### Phase E — the test harness

`lumen-test` (931 lines): locators, snapshots, traces. We have the agent
protocol and the one input queue, which is most of the substrate — a
session already replays. This is small and should ride along.

## What we should *not* copy

- The `Element`/rebuild machinery (see above).
- `ui.getDeps`/`whatDependsOn` — no dependency graph exists here, and
  `ui.lastChange` already answers the agent-facing question by diffing
  state.
- Lumen's tier-2 hot patch. It is **off by default** in Lumen because its
  ABI fingerprint is a documented placeholder; ours works because the
  boundary carries only bytes and host-owned pointers.

## What we have that Lumen does not

Worth protecting through any parity push, since these are the reasons the
architecture exists:

- **Source provenance** on every node, in findings, lint output and agent
  responses — Lumen's semantics tree has nothing equivalent.
- **Const `NODE_COUNT`, zero allocation, no-allocator viability** — the
  microcontroller profile Lumen cannot serve.
- **`#[fuse]`** — a local erasure dial with measured numbers.
- **Ambient audit with per-field fallback logging**, built from Lumen's
  own review scars rather than re-earning them.
- **A measured unsafe-free path** (`docs/results-safe-prototype.md`) and
  therefore a credible route to formal verification.

## Honest summary

Parity is roughly **35–40k lines of framework**, of which about **10k is
adoptable** from Lumen's architecture-neutral crates and the rest is
genuinely ours to write — dominated by the widget library, which cannot
be shared because every widget is written against the node-table model.

The gating decisions are text storage and the layout engine, both of
which should be settled before any widget beyond the current four is
written, because every widget encodes both.

Phase A is the one that changes the project's character: it is mostly
integration, it is the smallest slice that produces a running app with
real text on screen, and everything after it is incremental.
