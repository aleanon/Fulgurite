# Appendix: glossary

Terms this book uses in a specific sense, with the chapter that explains
each.

**Arena** — storage beside the const table for instances the table cannot
describe: a `Repeat`'s window, a `List`'s items, a `Dyn`'s island. The
cascade reaches it by template, which is why per-instance styling does
not exist. → [Containers and escape hatches](./containers.md)

**Binding** — a `#[bind]` declaration carrying a state field to a widget
field. Applied by the frame loop between input and layout, so it cannot
be forgotten. → [State, bindings and handlers](./bindings.md)

**Component boundary** — the seam at which four things deliberately
coincide: the compile firewall, the hot-reload unit, the erasure dial and
the observability namespace. → [The component boundary](./component-boundary.md)

**Damage** — which *pixels* differ from the previous frame. A set of
regions, `None`, or `All`. Distinct from invalidation. →
[Damage and invalidation](./damage.md)

**Display list** — the recorded command stream a frame paints into.
Nine commands. A host with no allocator skips it and implements `Painter`
directly. → [The rasteriser](./rasteriser.md)

**Escape hatch** — `Either`, `List`, `Dyn` or `Repeat`: the four ways to
express a shape the type cannot know. Deliberately explicit. →
[Containers and escape hatches](./containers.md)

**Fuse** — `#[widget(fuse)]`, which makes a subtree emit static calls
instead of `&dyn`. The erasure dial, and it is local to a component
rather than global. → [The component boundary](./component-boundary.md)

**Invalidation** — which *nodes* must be laid out and painted again.
Currently conservative: a handler that ran marks everything. Pointer
and focus state — hover, press, focus becoming visible — marks only the
nodes it moved between. →
[What a frame does](./frame.md)

**`kind_name`** — the string `Selector::Kind` matches. Lowercase for
built-in leaves, the struct's own ident for derived containers. Since
M24 it forwards to the type-level `Widget::KIND`; see **Seed**. →
[Styling](./styling.md)

**Seed** — a node's kind and its own defaults, as a compile-time pair.
One per node in `Widget::SEEDS`, spliced the way the node table is. The
cascade reads these instead of the live tree, which is the only way a
*hidden* subtree can resolve its own style — it has no instance to ask.
→ [Styling](./styling.md), [Writing a widget](./custom-widget.md)

**Node table** — the `const` `[NodeDesc; N]` describing a tree type's
shape. Not used for traversal; used for index addressing. →
[The const node table](./node-table.md)

**`NodeState`** — the per-node entry in the flat state array: geometry
plus the paint attributes layout stamped there from the cascade. →
[Where state lives](./state-lives.md)

**Overlay phase** — a second paint pass that repaints deferred subtrees
outside every clip scope. Not a layout mode. → [Overlays](./overlays.md)

**Presentation copy** — the value a widget owns because it measures it,
kept in agreement with application state by a binding. →
[Where state lives](./state-lives.md)

**Relayout boundary** — a node whose own size provably cannot change, so
a dirty flag stops propagating there. →
[The retained-mode baseline](./baseline.md)

**Retention** — `Rebuild` or `Incremental`, a type parameter on `Host`.
`Rebuild` is the oracle `Incremental` is tested against. →
[Damage and invalidation](./damage.md)

**Rigidity** — the `RIGID` flag in the node table, const-folded, so a
static tree does not perform the check a dynamic one does per node per
frame. → [What it costs](./costs.md)

**Stack** — the third container mode, whose children share its rect
rather than dividing it. Not an `Axis` variant, because it has no main
axis. → [Layout](./layout.md)

**Wrapper rule** — a provided trait method is right for a leaf and wrong
for a wrapper, always. When a trait gains a method, every wrapper of it
is a call site the compiler will not show you. →
[Writing your own widget](./custom-widget.md)
