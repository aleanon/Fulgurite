# The agent protocol

This is the chapter that describes something other frameworks do not
have, so it is worth stating the shape before the detail:

> **The accessibility tree, the test harness and the agent surface are
> the same object.**

An agent needs a serialisable tree with roles, labels, geometry and
state, plus the ability to synthesise events and observe what changed.
That is precisely an accessibility tree. One implementation serves screen
readers and agents both — and the accessibility work, which is required
anyway, stops being a tax.

## JSON-RPC over an already-laid-out tree

The protocol is JSON-RPC 2.0. Methods divide into three groups:

**Query** — `ui.getTree`, `ui.getLayout`, `ui.getAppliedStyles`,
`ui.resolve`, `ui.probe`, `ui.probeRegion`, `ui.screenshot`,
`ui.lastChange`, `ui.lastDamage`, `ui.lint`, `ui.explain`.

**Synthesise** — `input.click`, `input.hover`, `input.key`,
`input.type`, `input.fill`, `input.scroll`, `input.scrollIntoView`,
`input.pointerDown`, `input.pointerUp`, `input.invokeAction`.

**Observe** — `app.diagnostics`, `app.logs`, `app.perf`,
`ui.waitSettled`.

**What the endpoint refuses.** The socket speaks one JSON object per
line, and **a line that is anything else ends the connection** after a
single error reply. That is the whole defence against a web page the
developer happens to have open: every way a page can reach a local port
— a `no-cors` `fetch`, a form, a beacon, a WebSocket upgrade — begins
with an HTTP request line, which cannot be a JSON object, so nothing a
page sends after it runs. Until M39 the endpoint skipped a line it could
not parse and read the next, so a page's request body executed; what a
real Firefox and Chrome send, and what each executed before and after,
is `docs/results-agent-transport.md`. Pointing `curl` at the port gets
a reply that says it is not HTTP.

**Steer** — `app.setPalette` swaps a running window's palette, by name
(`light`, `dark`), and the host re-resolves every stylesheet rule that
names a role (M25/T25.11) — so `ui.getAppliedStyles` answers in the new
colours on the next call. It answers `applied: false` from a host that
has no palette to set, rather than succeeding at nothing (M25/T25.3).

`ui.lint` reports stylesheet rules too, when the host supplies its sheet
— which a host built with the `agent` feature does, and one without does
not lint at all (M26/T26.18):
`W1010` names a rule whose selector matches no node, or whose fields no
node it matches reads — by rule, because the rule is what is wrong
(M25/T25.2).

`W1011` names two nodes carrying the same `#[widget_id]`. Both are then
**unaddressable**: `ui.resolve` refuses the ambiguity and so does every
`input.*` that takes a selector. It is easy to write by accident, because
an id lives on a *type's* field — a composite instantiated twice
duplicates every id inside it. `examples/showcase` had eight, from three
labelled-pair types used two and three times each, and nothing said so
until a drive script could not click either checkbox (M27/T27.3).

## What changed, and what it damaged

`ui.lastChange` answers which nodes moved or repainted since a frame;
`ui.lastDamage` answers what that costs in pixels.

**Read `regions`, not `rect`.** `regions` is the damaged *set*, capped
the way the renderer caps it; `rect` is that set's bounding box and is
kept only because existing callers read it. A bound computed against
`rect` is a bound against the wrong quantity — two small changes at
opposite corners bound to the whole window, which is the union M18
replaced and which this surface went on reporting until M27/T27.3. The
numbers that found it are in `docs/damage-amplification-decision.md`.

## Selectors

Space-separated terms, all of which must match:

| term | matches |
|---|---|
| `#save` | author id, from `#[widget_id("save")]` |
| `label` | widget kind name |
| `~text` | the text it **shows** contains "text" — a value or a label |
| `@app.rs:42` | source provenance — file suffix, optional line |
| `7` | table index |
| `#a > #b` | the `#b` **inside** `#a` |

`#a > #b` is a **descendant step**, and it exists because a part id
lives on a *type's* table: a composite used twice carries every id inside
it twice, and neither copy can be addressed on its own. A step means the
part only has to be unique within its owner, and the owner is named by
the `#[widget_id]` the application gave its own field. `W1011` reports
only what a step cannot separate — two copies under the *same* owner.

`~text` is the one term that reaches inside a virtualized list. Every
other term is matched against the const `NodeDesc`, and a repeater's rows
are not on the const table — N materialized rows share the template's
single entry, so they share its id, its classes and its kind. `~text`
matches the node the widget *published* instead, which each row has of
its own. It is how a menu row is clicked by name rather than by
multiplying a row height (`docs/plan-widget-parity.md` T27.0).

`@app.rs:42` is the one worth dwelling on. `NodeDesc` carries a `SrcLoc`
in `observe` builds, so a report is not "a button has zero hit area" but
a file and a line. It composes with hot reload, because a node is
identified by an offset in a table rather than by a generated key — patch
that line, swap the dylib, and the identity survives **as long as the
edit keeps the tree's shape**. An edit that adds, removes or reorders a
field moves every offset after it, and the window's `app.reloads` answer
then carries `"shape_changed": true`: resolve again, by author id or a
`#a > #b` path, which are the names that survive any edit (M49/T49.11).

## `ui.explain`

The method that earns the protocol its keep. It answers the question the
other selectors are usually asked in service of — *"I clicked it and
nothing happened"* — by running **the real hit test**, not by describing
the tree.

That distinction is the whole value. A description of the tree tells you
what you already believe; running the hit test tells you what the frame
actually does, which is where the disagreement lives.

## Why synthesised input replays as a test

Every synthesised event enters the **same `InputQueue`** as OS input —
see [Events, focus and keyboard navigation](./events.md). There is no
second path, so an agent session and a test are the same kind of thing,
and a recorded session replays by construction.

`examples/counter` uses this on itself: it asks the agent where a widget
is, clicks the centre of it, pumps, and asserts the count. Coordinates
come from the laid-out tree rather than from guesses.

## The trap that makes an agent test vacuous

Read this before writing one. In M16/T16.7 an invalidation oracle drove
an application by clicking through this protocol, compared two frames,
and passed — while **every click had missed**. A `Fixed(220)` card in a
300 px window had put the button at x = 256, outside its own parent, so
the clicks landed on the root and the handler never ran. The test was
comparing two frames in which nothing had happened.

**Assert that the action landed.** A changed count, a `handled` flag, a
hit on the node you meant — not only that two results match. The same
advice appears in [Layout](./layout.md) from the layout side; this is the
side it usually bites from.

## What is gated, and what that costs

The whole development surface is behind `observe`, which compiles to
nothing in release. The const tables exist either way — only *exposure*
is gated — which is what makes the release cost genuinely zero rather
than small.

`docs/results-agent-cost.md` measures what it costs when it is on.
