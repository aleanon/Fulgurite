# Step 5 — observability: design notes and verification

*2026-08-30. Handoff §9 implemented: AccessKit as the one tree for screen
readers and agents, source provenance in `NodeDesc`, all behind an
additive dev gate with measured-zero release cost. Also pays Step 4's

> **Condition, added 2026-09-02 (F2.5).** "Zero release cost" means: a
> binary that does not link `lumen-static-agent`. That crate depends on
> `lumen-static` with `features = ["observe"]`, and cargo unifies
> features across a selection, so linking it turns `observe` on for the
> whole graph — `NodeDesc` 48 B → 72 B, plus accesskit. This is the
> additive dev gate §9 blesses, used as intended, but the claim above was
> stated without the condition and two reviewers read it as unconditional.
> `fixtures/binsize` links neither the agent nor any feature, so the
> release figure it records is a release figure.
>
> **And checked, since M48/T48.12** (R14-F8): `just agent-symbols`
> counts `lumen_static_agent` and `serde_json` symbols in a release
> showcase built without the `agent` feature — 0 of each, against 66 and
> 192 in the agent build, its positive control. Inside an agent build, a
> window with no address set no longer runs the journal or the audit on
> any frame (`crates/lumen-static-winit/tests/agent_transport.rs`).
recorded debt: the reload surface is now gated too.*

## The gate: additive cargo features, not a mode

`observe` and `reload` are non-default features of `lumen-static`
(`observe = ["a11y", "dep:serde", "dep:serde_json"]`, `reload =
["std", "dep:serde", "dep:serde_json"]`).

> **Split since M54/T54.1** (`docs/plan-a11y-platform.md` §2.1): the
> accessibility tree is its own feature, `a11y = ["std",
> "dep:accesskit"]`, which `observe` implies. `a11y` gates every
> widget's `Node::observe`, the static walk and `observe::snapshot` —
> what a screen-reader adapter sends — and leaves `NodeDesc` at 48 B;
> `observe` adds the JSON dump, `probe`, `stats` and the `src` stamp
> (72 B). `crates/lumen-static-widgets/tests/desc_size.rs` asserts both
> sizes, and `scripts/check_feature_seam.sh` refuses the core's `a11y`
> without the widgets' as it refuses `observe`.
 §8 rejected features for *semantic* dials because
unification breaks third parties; a dev surface is the opposite case —
purely additive, so unification is harmless (someone enabling observe
anywhere just turns the lights on). Release builds without the features
compile none of it: no accesskit, no serde, no provenance field.

**Zero-cost, verified not asserted:** the Step 2 crossover benchmark
re-run after this step is unchanged (76 ns @ 10 nodes, 6.4 µs @ 1k —
within noise of the Step 2 record).

### The derive stays cfg-free

Emitted code must compile identically whether or not the app enables the
features (a proc macro cannot see downstream features). Both hooks are
always-present const builders whose *bodies* are gated inside
lumen-static: `NodeDesc::new(...).with_src(file!(), line)` (no-op without
`observe`) and `WidgetOps::new(...)` (fills the cfg-gated `observe` op
with a default). This replaced all struct-literal construction of
`NodeDesc`/`WidgetOps` — which is also what lets the structs grow gated
fields without breaking anyone.

## Provenance (§9's one addition beyond AccessKit)

`NodeDesc.src: SrcLoc { file: &'static str, line: u32 }` under `observe`.
The derive stamps it at table-build time (still const): the file comes
from `file!()` expanded at the derive site, per-field lines from
proc-macro2 span locations (`span-locations` feature). The ownership rule
that makes reports actionable:

- a child's **root** carries the *field* declaration site (where you put
  it in the parent);
- a nested component's **internals** carry the component's own
  declaration sites.

So "the label at `tests/observe.rs:42`" points at the line you would
edit. Tests pin the file and the declaration-order of lines rather than
absolute numbers, so they survive edits. Compact interning (`src_file:
u16` per the handoff sketch) is deferred — dev builds can afford the
pointer.

## The AccessKit tree

`observe::snapshot(base, table, kinds, st) -> accesskit::TreeUpdate`
(accesskit 0.24). The walker pre-fills role `GenericContainer`, absolute
bounds from `NodeState`, and children from the table's sibling chains;
each kind's cfg-gated `observe` op overrides what it knows better —
`Label` → role Label + value, `Repeat` → role List. No bespoke agent
channel: `observe::dump` is a JSON *serialization of the same snapshot*
(role/value/bounds/children per node) joined with provenance and kind
names — the agent view and the screen-reader view cannot drift because
one is derived from the other.

### Identity, including virtualized rows

Main-table nodes: `table index + 1` — positional, so ids survive hot
reload exactly as §9 argues (the id is an offset, not a generated key).
Repeater instances get synthetic ids keyed by **row**, not window slot:
`1<<63 | repeater_idx<<44 | row<<12 | template_node`. Scrolling by five
rows keeps the five still-visible rows' ids identical (tested) — an agent
watching "row 7" keeps watching row 7. Documented limits: repeater table
index < 2^19, template < 2^12 nodes, rows < 2^32, one nesting level of
synthetic ids.

## Deferred, deliberately

1. ~~**Action synthesis**~~ — **done 2026-08-31**: the event system
   landed (`docs/plan-events.md`), `Button` advertises
   `accesskit::Action::Click`, and `input.invokeAction` synthesizes it
   through the one input queue.
2. **Live TreeUpdate diffing**: `snapshot` rebuilds the full tree; real
   adapters want incremental updates driven by the same dirty tracking
   that invalidation needs. Same future work item as §11's invalidation
   emphasis.
3. **Platform adapter wiring** (winit + accesskit_winit): needs a shell,
   which this project composes rather than builds (§11); the `TreeUpdate`
   is the interface contract.

## Verification

4 new gated tests (`crates/lumen-static/tests/observe.rs`): snapshot
shape/roles/values/bounds against layout state; row-stable instance ids
across scroll; provenance file + declaration-order lines, including the
nested-component ownership rule; agent JSON dump content. Full gate:
`just test` (default + all-features runs — cargo silently skips
`required-features` tests, hence the justfile). Reload demo re-run: PASS.
