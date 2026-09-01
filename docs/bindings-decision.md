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

## Measurement — and the verdict is *not yet* in its favour

`cargo run --release -p benches --bin bindings`. Medians, same run
(absolute numbers drift with frequency scaling between runs; only
within-run comparisons are meaningful):

| | ns |
|---|---|
| 1 binding, unchanged (compare, skip write) | 18–27 |
| 1 binding, changed (compare, then write) | 32–46 |
| 1 update, no compare (always write) | 14–26 |
| label layout, memo warm | 13–29 |
| label layout, memo cold (re-measure) | 15–34 |
| 1000 bindings, unchanged | 18 µs |

**The compare costs ~4–13 ns per binding and currently saves ~2 ns.** It
is a net loss today, by roughly an order of magnitude.

The reason is that `BlockMetrics` — the stand-in text measurer — is
`char_count × advance` arithmetic, so the memo it protects has almost
nothing in it. The whole design is a bet on *real* shaping, which M5's
remaining `lumen-text` adoption brings.

**The threshold, stated so the gate can say no:** the compare pays for
itself once re-measuring a short label costs more than **~13 ns**. Real
shaping of a short run is conventionally quoted in microseconds, which
would make the bet pay off by two to three orders of magnitude — but that
number is *not measured here* and must be re-run against the real text
stack. If it comes back under 13 ns, `format_into` should drop the
comparison and always write, and this file should say so.

Two things are already settled regardless:

- **In absolute terms the cost is irrelevant.** 1000 bindings cost 18 µs
  per frame, 0.1% of a 16.6 ms budget. Nothing here needs optimizing for
  its own sake.
- **A meaningful slice of the compare path is the thread-local scratch
  buffer**, not the comparison — `take`/`replace` plus moving a `String`
  twice. If the threshold above ever looks marginal, that is the thing to
  attack first, not the compare.

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
- **`Label<&'static str>` bindings leak** on every change, because that
  storage cannot own. It is the documented dev-only path (§7's
  dlopen-without-dlclose spirit); bindings should use `String` or `Cow`.
