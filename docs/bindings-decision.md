# Decision: derived presentation via `#[bind]`, and what the change
# detection actually costs

*Settled 2026-09-01 (plan M6, tasks T6.1–T6.4). Supersedes the "explicit
update step" sentence in `layout-and-storage-decisions.md` §1.*

## The problem

`docs/layout-and-storage-decisions.md` §1 settled that application state
is the source of truth in sibling fields and widget fields hold the
*presentation copy*, because only owned storage can invalidate a
measurement cache. It left the copying to "an explicit update step".

The counter example showed what that costs in practice:

```rust
host.click(x, y);
host.pump();
host.app.update();   // forget this and the screen silently lies
host.pump();
```

Two failure modes, both silent. The body of `update` has to be kept in
step with the fields by hand, and the *call* has to be remembered at
every site that changes state. Neither is checkable, and the penalty for
getting either wrong is a frame that renders stale data — the worst kind
of bug to notice, because everything still runs.

## Decision

Two separate changes, because they are two separate problems.

**1. The frame loop owns *when*.** `App::update` (empty default body) is
called by `pump` after input is drained and before layout. A handler has
just changed state and layout is about to measure what that state renders
as; that is the one correct moment, and it is not a decision each call
site should be making.

**2. The field owns *what*.** `#[derive(App)]` builds the body of
`update` from `#[bind(...)]` attributes:

```rust
#[derive(App, Default)]
struct Counter {
    #[tree]
    ui: Ui,
    #[bind(ui.value, "count: {}")]
    count: i64,
}
```

The derive emits the whole `App` impl — accessors *and* `update` —
because a derive that emitted only a binding body would leave the author
to call it from `update`, which is the same wiring one level up.

Three forms: a format string (the annotated field is the first argument,
further arguments written out in full), `with = f` for
`fn(&mut Target, &Source)`, and struct-level `#[app(after_update = f)]`
for logic that will not fit a declaration.

**A binding is generated straight-line code.** No registration, no
runtime graph, no scheduling, nothing captured — the same properties that
made handlers `fn` pointers rather than closures
(`docs/events-decision.md`), and for the same reason: a hot reload
re-supplies code from the new dylib, so a binding needs no migration and
appears nowhere in the saved bytes. The `hot-app` fixture now derives its
`status` label this way, and the swap demo asserts it comes back
correct from the kept counter.

## The interesting part: what to compare against

A binding must not re-measure text that did not change. Three ways to
know:

| | Defeatable? | Stored state |
|---|---|---|
| dirty bit on the source | **yes** — mutate the field directly and the screen lies | 1 bool |
| shadow copy of the source | no | `size_of::<T>()`, and a derive cannot add a field |
| compare the formatted **output** | no | none |

The third was chosen. There is no setter to route around and no bit to
forget, it stores nothing, and it keys on exactly what the memo protects:
equal text measures equally, so two sources that render identically —
`1.44` and `1.44999` at one decimal — are correctly treated as no change.

The cost is running the formatter every frame.

## Measurement — corrected 2026-09-02 after review

**The first version of this section was wrong, and the way it was wrong
is worth keeping.** It reported that the output comparison "costs ~4–13
ns per binding and saves ~2 ns — a net loss by roughly an order of
magnitude", and set ~13 ns as the threshold at which the comparison
would start to pay. Review found three problems, each independently
verified before this rewrite:

1. **The harness could not resolve the question.** It timed one call
   per sample. An empty closure through it measured 10–27 ns — the
   timer itself — and every row of the table sat within a few ns of
   that floor. The "~4–13 ns" range was the *same quantity sampled five
   times*: 4, 2, 6, 6, 13. A range of noise was presented as a range of
   costs, and the top of it became a threshold.
2. **The arm difference was misattributed.** "Unchanged" minus "always
   write" isolates two things, not one: the comparison *and* the
   thread-local scratch buffer. Decomposed, the buffer is the whole
   cost and the comparison is none of it.
3. **Both the original and the review's first re-measurement were
   measuring a folded format.** With a literal `41`, the compiler
   constant-folds the entire `format_args!` and the arms time a 9-byte
   `memcmp`. Under that folding the comparison looked like a 3–6 ns
   *saving*. Passing the value through `black_box` so the formatter
   runs, that saving vanishes too.

`benches --bin bindings`, rewritten to batch (21 blocks, 200k calls,
floor ~0.2 ns, machine: i9-13900KF, governor `powersave`, turbo on,
`tsc`). Five runs, **minimum** per arm — the least-perturbed estimator;
the median is within ~1 ns except where noted:

| arm | ns | |
|---|---|---|
| B — local buffer, always write | 12.4–13.6 | format + `set_str` |
| C — local buffer, compare then skip | 11.8–12.9 | format + `memcmp` |
| **C − B** | **≈ −0.5** | **the comparison: a wash** |
| A — `format_into` as shipped (TLS + compare + skip) | 14.9–26.3 | bimodal between runs |
| **A − C** | **+3 to +14** | **the thread-local scratch, alone** |
| D — `format_into`, value alternates every frame | 18.0–18.4 | the changed case |
| `Label::layout`, memo warm / cold | 3.8 / 5.4 | what the memo protects, under `BlockMetrics` |
| 1000 bindings, unchanged | 25.2–25.4 µs | 0.15% of a 16.6 ms frame |

**What the evidence now supports:**

- **The comparison costs nothing to speak of.** Comparing-and-skipping
  is within half a nanosecond of writing, because the ~12 ns integer
  format dominates both. There is no threshold question, because there
  is no cost to cross a threshold. **The ~13 ns threshold is deleted.**
- **The thread-local scratch is the only real overhead in `format_into`**,
  and it is bimodal — ~3 ns in some runs, ~14 ns in others, presumably
  by TLS fast-path and cache placement. It is an implementation detail:
  a `&mut String` threaded from the host, or a small pool, removes it.
  Not done now, because 25 µs per thousand bindings does not justify a
  signature change.
- **The comparison's purpose is unchanged and unmeasured here:** keeping
  the memo warm so a real shaper is not re-run on identical text.
  `BlockMetrics` is `char_count × advance`, so the warm/cold gap above
  (1.5 ns) says nothing about that. It is the claim that M7's `lumen-text`
  adoption makes measurable — see the rewritten T7.3 — and the direction
  of the bet is not in doubt: a compare at ~0.5 ns against a shape at
  hundreds of nanoseconds to microseconds.
- One caution for whoever runs that measurement: a shaper with its own
  content-keyed cache (cosmic-text's `ShapeRunCache`) makes a memo miss
  cost a hash lookup rather than a shape. The framework memo still wins,
  but by ~200× rather than ~4000×, and §1's "re-shaping is what the
  cache exists to avoid" quietly assumes the memo is the only thing
  standing between the label and a shape. It will not be.

**Two further findings from the same review, both fixed:**

- `Label`'s default storage is `&'static str`, and `set_from_str`'s
  default body routed to `from_string`, which for that storage is
  `Box::leak`. So a `#[bind]` on a plain `Label` — the default the
  examples used — leaked ~10 bytes per changed frame without bound.
  Measured: 1000 updates, 9,906 bytes never freed. The repeated-write
  method now lives in `TextBuffer`, implemented for `String` and `Cow`
  and deliberately not for `&'static str`; binding a default `Label` is
  a compile error. Leaking once per hot reload and once per frame are
  different things and the type system now says so.
- A `Display` impl that re-enters `format_into` is panic-free and
  correct, but not amortized: the outer call's `replace` drops the inner
  call's warmed buffer, so the inner allocates on every frame forever
  (measured: 3 allocations per frame in steady state). Recorded; not
  fixed, since no binding in the tree nests today. A small pool instead
  of a single slot is the fix if one ever does.

## T7.3 — what the memo is worth against a real face (2026-09-04)

The measurement above ran against `BlockMetrics`, where a re-measure is
`char_count × advance` and the memo saves 1.3 ns — a number that says
nothing about the memo's purpose. T7.3 asked for the same comparison
against a real shaper, four numbers, and a verdict: *"if it is under
~1 ns the memo is pointless and `Measured` should go."*

Machine and method as elsewhere in this file (i9-13900KF, `powersave`,
`benches --bin bindings`, ≥20k iterations in blocks, median of blocks).
The face is fontdue with the system Noto Sans, warmed by 100 prior
calls at each size — fontdue caches rasterized glyphs per (glyph, px)
inside `Font`, so an unwarmed first call pays for the whole alphabet.
Lumen's parley stack was measured and declined
(`docs/text-render-adoption-decision.md`); fontdue has **no
content-keyed shaping cache**, so a miss here is a real re-layout. That
retires the trap T7.3 named — a warm shaper cache making a miss cost a
hash lookup — rather than working around it: the numbers below are the
honest upper bound, not a cache hit in disguise.

| `Label::layout` | memo hit | memo miss | saved |
|---|---:|---:|---:|
| `"count: 41"`, unwrapped | **4.82 ns** | **2 459 ns** | 2.45 µs (510×) |
| 60 chars, wrapped at 200 px | **4.78 ns** | **16 712 ns** | 16.7 µs (3 500×) |

**Verdict: the memo stays, and it is not close.** The threshold was
1 ns; the margin is three to four orders of magnitude above it.

Two things follow that are larger than the memo itself:

1. **Text measurement dominates layout once text is real.** The whole
   tally lays out in 1 µs and a 901-node tree in 7.3 µs
   (`docs/results-agent-cost.md`); *one* wrapped 60-character label
   costs 16.7 µs to measure. A screenful of unmemoized wrapped labels
   is milliseconds — a dropped frame from layout alone, in a framework
   whose layout traversal was never the cost. This is the handoff's
   §"Performance expectations" calibration arriving a second time, from
   the other side: after the raster fix (`docs/results-frame-cost.md`)
   the next thing that can blow a frame budget is shaping, not
   traversal.
2. **It raises the value of M8c, not just of the memo.** The memo is
   keyed on (dirty, limits), so a *resize* invalidates every label at
   once and pays the miss column for all of them. Dirty-scoped layout
   does not help there, but the `Shaped` cache (M8/B2) and per-axis
   rigidity (M8c/T8c.2) both do: a width-rigid node's children keep
   their memo through a height change.

## Correction to `layout-and-storage-decisions.md` §1

§1 rejected the getter design partly because it "would have to re-shape
or re-hash the string every frame, **which is exactly the cost the cache
exists to avoid**". The second clause was too broad, and this design
makes that visible: re-hashing or comparing is nanoseconds, re-*shaping*
is what the cache exists to avoid. Those are different costs by orders of
magnitude and the sentence conflated them.

§1's conclusion is unchanged and in fact strengthened: this design *is*
the getter's "compare instead of re-shape" variant, and it is only
possible because the storage is owned by the widget. The getter could not
have done it — it has nowhere to keep the previous value to compare
against.

## Not decided here

- **Multi-source bindings** beyond extra format arguments. An author who
  needs one writes `#[app(after_update = f)]`, which is one line and
  honest. A dependency-tracking form would need a runtime graph, which is
  the thing this design exists to avoid.
- **Skipping layout entirely when no binding changed.** `format_into`
  already returns whether it wrote, so `update` could return a bool and
  `pump` could skip layout on a quiet frame. Deferred: it interacts with
  style changes, resizes and animation, and the frame loop is not yet
  under measurement pressure.
- ~~`Label<&'static str>` bindings leak on every change~~ — **no longer
  possible**: they do not compile (see Measurement, above).
