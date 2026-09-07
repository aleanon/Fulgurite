# The documents, and which of them to trust

*Index, 2026-09-02. Twenty-four files use six different conventions for
"this is out of date" — token markers, bold DONE lines, strikethrough,
italic parentheticals, header blockquotes, and prose headings. You cannot
grep for what to distrust, which is what this table is for.*

**Class** — `live` a decision still in force · `open` a question not yet
answered · `queue` a work plan · `historical` a step that shipped and was
superseded, kept for its reasoning · `stale` claims something the code no
longer does.

**Reproducible** — whether the numbers in it can be re-run from this
tree today.

| file | answers | class | reproducible |
|---|---|---|---|
| [`00-handoff-static-tree.md`](00-handoff-static-tree.md) | why the whole design is shaped this way | **live** (the spec) | n/a |
| [`plan-architecture-v2.md`](plan-architecture-v2.md) | what is built and what is next (M0–M12) | **queue** | n/a |
| [`plan-review-fixes.md`](plan-review-fixes.md) | the F0–F7 fix pass from the 2026-09-02 review | **queue** | n/a |
| [`text-and-scrolling-decisions.md`](text-and-scrolling-decisions.md) | how scrolling and text input will work | **live**, all 23 answered (2026-09-04) | n/a |
| [`layout-and-storage-decisions.md`](layout-and-storage-decisions.md) | the `Length` model, text storage, the style array | **live** (most-cited from code) | n/a |
| [`events-decision.md`](events-decision.md) | why handlers are `fn` pointers, not `Msg` | **live** | n/a |
| [`bindings-decision.md`](bindings-decision.md) | why `#[bind]` compares output, and what it costs | **live** | yes — `benches --bin bindings` |
| [`either-reopened.md`](either-reopened.md) | whether `Either` should use a merged table | **decided** 2026-09-04: Option A, built as T8.0 | n/a |
| [`plan-lumen-parity.md`](plan-lumen-parity.md) | how far this is from Lumen | **superseded 2026-09-04** as a plan; kept as a map. Its counts are dated 2026-09-01 and stale by 2.5× | n/a |
| [`plan-embedded.md`](plan-embedded.md) | M13 — no_std, and proving "zero allocation". Reviewed and corrected 2026-09-04; its errors are marked inline | **done** 2026-09-05, T13.0–T13.9 | yes — `just firmware-size`, `just thumb` |
| [`plan-desktop.md`](plan-desktop.md) | M14 — the desktop gaps M13 made visible: windowing crate, modifiers, clipboard, selection, cursors, IME | **done** 2026-09-05, T14.0–T14.8 | vocabulary size gated at 16 B |
| [`plan-widgets.md`](plan-widgets.md) | M11 — the widget vocabulary, with each item's real blocker | **queue**, T11.1–T11.4 done; T11.5 (overlays) deliberately not started | n/a |
| [`plan-harness.md`](plan-harness.md) | M12 — locators, goldens, session replay | **queue**, T12.3–T12.4 done | n/a |
| [`plan-agent-parity.md`](plan-agent-parity.md) | the agent surface, P0–P7 | **queue**, P0–P7 done | n/a |
| [`plan-events.md`](plan-events.md) | the event system, E0–E5 | **queue**, E0–E4 done | n/a |
| [`verification-notes.md`](verification-notes.md) | what could be formally verified, and the `unsafe` audit | **live** | n/a |
| [`results-embedded.md`](results-embedded.md) | the embedded footprint, and the 26.5 KB `f32::clamp` finding | **live** — measured 2026-09-05, thresholds recorded first | yes — `just firmware-size` |
| [`plan-capabilities.md`](plan-capabilities.md) | M15 — the thirteen capabilities all six others have, and the gating policy for each | **done** 2026-09-07, T15.0–T15.12 (T15.12 deferred and un-deferred the same day; built 2026-09-07) | sizes and crate counts asserted per task |
| [`competitive-feature-gaps.md`](competitive-feature-gaps.md) | what Qt, GTK, iced, Xilem, Slint and Lumen have that this does not, per framework | **live**, surveyed 2026-09-06 | n/a |
| [`results-competitive.md`](results-competitive.md) | frame time, RSS and size against the same six — and the two defects it found | **live**, measured 2026-09-05 | yes — `benches-competitive/` |
| [`results-raster-scaling.md`](results-raster-scaling.md) | does the software rasteriser scale to a real display? (not at 4K) | **live**, measured 2026-09-07 | yes — `benches-competitive -p raster-scaling` |
| [`results-gpu.md`](results-gpu.md) | what the wgpu backend buys (20× at 4K), what it costs, and how closely it matches the software canvas | **live**, measured 2026-09-07 | yes — `benches-competitive -p raster-scaling`, `cargo test -p lumen-static-wgpu` |
| [`results-retention.md`](results-retention.md) | streaming retention vs a copied display list (M13/T13.8) | **live** | yes — `benches --bin retain` |
| [`results-cascade-cost.md`](results-cascade-cost.md) | what a stylesheet swap costs, and why M8c inverted | **live** | yes — `benches --bin cascade` |
| [`results-frame-cost.md`](results-frame-cost.md) | frame cost and input latency, before/after M8b | **live** | yes — `just profile`, `just live-gate` |
| [`text-render-adoption-decision.md`](text-render-adoption-decision.md) | why Lumen's text/render crates were measured and declined | **live** | n/a |
| [`results-allocation.md`](results-allocation.md) | does a steady-state frame allocate? (no) | **live** | yes — `shell/tests/alloc_gate.rs` |
| [`results-const-eval.md`](results-const-eval.md) | what the const table check costs to compile | **live** | yes — `just const-budget` |
| [`results-agent-cost.md`](results-agent-cost.md) | what observing a frame costs | **live** | yes — `benches --bin agent` |
| [`results-traversal.md`](results-traversal.md) | recursion vs a flat table walk | **live** conclusion | **no — harness deleted** |
| [`results-safe-prototype.md`](results-safe-prototype.md) | is the `unsafe` needed? (no) | **live** conclusion | **no — harness deleted** |
| [`results-step1-firewall.md`](results-step1-firewall.md) | does the compile-time firewall hold? | **live** | yes — `just firewall` |
| [`results-step2-crossover.md`](results-step2-crossover.md) | static vs `Box<dyn Widget>` | **historical**, header inverts its own conclusion | superseded; re-measured in the header |
| [`results-step6-fuse.md`](results-step6-fuse.md) | what `#[fuse]` buys | **historical** | superseded — see its header |
| [`step2-design-notes.md`](step2-design-notes.md) | the node table's design | **historical**, SUPERSEDED per section | n/a |
| [`step3-repeaters.md`](step3-repeaters.md) | virtualized lists | **live** substance, drifted mechanism | n/a |
| [`step4-hot-reload.md`](step4-hot-reload.md) | the swap sequence and what survives | **live** | n/a |
| [`step5-observability.md`](step5-observability.md) | the AccessKit tree and provenance | **live** doctrine | n/a |

## Two things a new reader should know

**Two results docs cannot be re-run.** `results-traversal.md` and
`results-safe-prototype.md` cite a `benches` binary that no longer
exists. They are the documents that overturned handoff §5 and justified
deleting the entire `unsafe` surface, so their conclusions are
load-bearing and their evidence is not reproducible. Both say so at the
top. Under this project's own rule — a gate must be able to say no — a
gate whose harness has been deleted cannot say anything.

**The oldest numbers are the least trustworthy.** Anything measured
before 2026-09-02 was taken with a per-iteration benchmark harness whose
own floor was 10–27 ns, so any figure under ~100 ns in an older document
is at or below the noise. The harness is batched now and prints its floor
on every run; a result within a small multiple of it is not a result.

## Where a decision is recorded

Per `AGENT.md`, a decision backed by measurement gets its own
`results-*.md` — the number, the threshold it was judged against, and the
verdict — so a later reader can see why the gate said no. A decision that
is an argument rather than a measurement gets a `*-decision.md`. Plans
are `plan-*.md` and carry task ids that appear in commit subjects.
