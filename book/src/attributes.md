# Appendix: the attribute reference

The attributes of the three derives. This table mirrors the one in
`crates/lumen-static/src/lib.rs`, and a check in `just static` fails if
the two diverge, or if either omits an attribute a derive declares or a
key its parser accepts — see the note at the end. It states no count: a
count here was wrong twice.

| attribute | on | meaning |
|---|---|---|
| `#[widget(row \| column \| stack, spacing = 4.0)]` | a `StaticTree` struct | container mode, and the gap between children. `stack`'s children share its rect (M11/T11.6) and it takes no spacing |
| `#[widget(wrap)]` | a `StaticTree` struct | children flow left to right and wrap onto the next line when the width runs out (M27/T27.8) |
| `#[widget(grid, columns = 3)]` | a `StaticTree` struct | a wrap that breaks every `columns` children instead of on width; `columns` belongs to `grid` alone, and 0 is refused |
| `#[widget(fuse)]` | a `StaticTree` struct | emit static calls instead of `&dyn` — the erasure dial |
| `#[widget_id("save")]` | a field | stable author id: an agent selector, and `#save` in a stylesheet |
| `#[widget_class("row", "striped")]` | a field **or the struct** | style classes, matched by `.row`; on the struct they land on the container's own node |
| `#[reload(keep \| fresh \| reset)]` | a `Reloadable` field | survives a hot swap / comes from the new code / recomputed |
| `#[tree]` | an `App` field | which field is the widget tree |
| `#[state]` | an `App` field | what a handler receives, and the tree is not in it. Absent, a handler gets the whole application |
| `#[bind(ui.value, "count: {}")]` | an `App` field | how this state reaches the screen |
| `#[bind(ui.value, count, "count: {}")]` | an `App` field | the same, reading a path *inside* that field |
| `#[bind(ui.value, with = f)]` | an `App` field | a `fn(&mut Target, &Source)` projection; it cannot say whether it wrote, so it marks everything |
| `#[bind(ui.pane.w, blur)]` | an `App` field | the value form: copy a path inside that field into a non-text target, compared first |
| `#[app(after_update = f)]` | an `App` struct | `fn(&mut Self) -> bool`, run after the bindings; the `bool` says whether it changed anything |
| `#[app(tick = f)]` | an `App` struct | `fn(&mut Self, f32) -> bool`, called each frame with the elapsed ms; the `bool` says whether anything is still moving (M22/T22.4) |
| `#[app(wake = f)]` | an `App` struct | `fn(&Self) -> Option<f32>`, `App::wake_in_ms`: the ms until this application next needs a frame, which the window waits on; `None` waits for input (M84/T84.3) |
| `#[app(images = f)]` | an `App` struct | `fn(&Self) -> Option<&dyn ImageSource>`, `App::images`: the store the renderer reads through the frame, before the window's own; asked every present (M84/T84.6) |
| `#[app(requests = f)]` | an `App` struct | `fn(&mut Self) -> Option<&mut Requests>`, `App::requests` (std): the queue a handler pushes platform requests onto — a window to open (M89/T89.7) |
| `#[app(msg = T, apply = f)]` | an `App` struct | `type Msg = T` and `fn(&mut Self::State, T) -> bool`, `App::apply`: the message deferred work sends and what interprets it; either without the other is an error (M84/T84.8) |
| `#[app(subscriptions = f)]` | an `App` struct | `fn(&mut Self) -> Option<&mut Subs>`, `App::subscriptions`: what should be running, which the host diffs each frame (M84/T84.8) |
| `#[app(jobs = f)]` | an `App` struct | `fn(Descriptor) -> Option<JobFn<Msg>>`, `App::job_for`: the job body for a descriptor, from the build that is loaded; needs `msg` (M84/T84.8) |

**Each key once, and nothing ignored** (M55/T55.2). A repeated key —
two container modes, `spacing` twice, a second `#[widget_id]` on a
field, `tick` twice, two reload modes — is an error, not a silent
overwrite; so are `#[tree]` or `#[state]` with arguments, an empty or
struct-level `#[widget_id]`, and a `StaticTree` attribute on an enum.

`#[bind]` also takes `with = f` for a `fn(&mut Target, &Source)`
projection, and extra format arguments after the string — the annotated
field is always the first. Both are evaluated in your scope, never the
derive's: a function of yours called `changed` is yours (M55/T55.1).

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
doc-currency rule warns about. `scripts/check_attr_table.sh` reads the
derives' own attribute lists and every key their parsers accept, and
fails if either table omits one — so adding an attribute or a key to the
derive without documenting it, or documenting one that does not exist, is
a build failure rather than a discovery.

It was worth writing: the rustdoc table said `row | column` for four days
after `stack` shipped. And comparing names was not enough — both tables
listed every attribute and still omitted `wrap`, `grid`, `columns` and
`tick` until the check read the keys too (M55/T55.3).
