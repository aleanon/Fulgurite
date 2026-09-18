# Appendix: the attribute reference

Seven attributes across three derives. This table mirrors the one in
`crates/lumen-static/src/lib.rs`, and a check in `just static` fails if
the two diverge — see the note at the end.

| attribute | on | meaning |
|---|---|---|
| `#[widget(row \| column \| stack, spacing = 4.0)]` | a `StaticTree` struct | container mode, and the gap between children. `stack`'s children share its rect (M11/T11.6) and it takes no spacing |
| `#[widget(fuse)]` | a `StaticTree` struct | emit static calls instead of `&dyn` — the erasure dial |
| `#[widget_id("save")]` | a field | stable author id: an agent selector, and `#save` in a stylesheet |
| `#[widget_class("row", "striped")]` | a field **or the struct** | style classes, matched by `.row`; on the struct they land on the container's own node |
| `#[reload(keep \| fresh \| reset)]` | a `Reloadable` field | survives a hot swap / comes from the new code / recomputed |
| `#[tree]` | an `App` field | which field is the widget tree |
| `#[bind(ui.value, "count: {}")]` | an `App` field | how this state reaches the screen |
| `#[app(after_update = f)]` | an `App` struct | `fn(&mut Self) -> bool`, run after the bindings; the `bool` says whether it changed anything |

`#[bind]` also takes `with = f` for a `fn(&mut Target, &Source)`
projection, and extra format arguments after the string — the annotated
field is always the first.

## Where each is explained

| attribute | chapter |
|---|---|
| `#[widget]` container modes | [Layout](./layout.md) |
| `#[widget(fuse)]` | [The component boundary](./component-boundary.md) |
| `#[widget_id]`, `#[widget_class]` | [Styling](./styling.md), [The const node table](./node-table.md) |
| `#[tree]`, `#[bind]`, `#[app]` | [State, bindings and handlers](./bindings.md) |
| `#[reload]` | [Hot reload](./hot-reload.md) |

## Two things that are easy to get wrong

**`#[widget_id]` is not `Selector::Kind`.** The id matches `#save`; the
kind matches the widget's `kind_name`, and those follow two different
spelling conventions. [Styling](./styling.md) has the detail.

**`#[reload(keep)]` on a `fn` pointer does not compile.** That is
deliberate and structural — saved state cannot contain code pointers,
and the type system enforces it rather than a rule someone has to
remember. See [Hot reload](./hot-reload.md).

## Why there is a check

This table exists in two places, which is exactly the shape the project's
doc-currency rule warns about. `scripts/check_attr_table.sh` extracts the
attribute names from both and fails if the sets differ, so adding an
attribute to the derive without documenting it — or documenting one that
does not exist — is a build failure rather than a discovery.

It was worth writing: the rustdoc table said `row | column` for four days
after `stack` shipped.
