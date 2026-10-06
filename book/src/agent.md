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
`ui.lastChange`, `ui.lastDamage`, `ui.lint`, `ui.explain`, and
`ui.contrast` — the WCAG ratio of each painted text run against the fill
under it, read off the display list, within a selector's box
(M54/T54.5) — and `ui.regionHash`, a hash of the presented frame's
pixels over a rect, which is how a drive asserts that an action's
consequence *shows* rather than that a value moved (M57/D1).

**Synthesise** — `input.click`, `input.hover`, `input.key`,
`input.type`, `input.fill`, `input.paste`, `input.scroll`, `input.scrollIntoView`,
`input.pointerDown`, `input.pointerUp`, `input.invokeAction`,
`input.compose` (an input-method step: `{text, commit}` or `{cancel:
true}`, through the same route the window's IME takes).
`input.key` takes `mods` (`["ctrl", "shift"]`), held for that key only,
and `input.click` takes `clicks` (2 or 3) for a double or triple click.

**Observe** — `app.diagnostics`, `app.logs`, `app.perf`,
`ui.waitSettled`, and after a panic `app.failure` and `app.retry`
(M36); `app.shadow` is the shadow oracle's report, from a build with it.

**The window's own** — `app.quit` replies and then exits the process
with status 0 (every turnkey window since M48/T48.13); `app.reloads`
counts hot swaps; `app.presents` counts frames presented and redraws
that presented nothing because nothing changed (M53/T53.4), and asking
does not draw a frame; `app.copy` copies the focused node's selection to
the system clipboard and reads it back, replying `{copied, clipboard}`
(M80/T80.4) — a drive's `input.key` Ctrl+C reaches the session, not the
window's clipboard shortcut, so this is how a drive copies;
`app.imeArea` answers the IME cursor area the window last passed to the
platform after a frame, in physical pixels, or `null` (M80/T80.5) — the
focused field's caret, which is where an input method opens its
candidate window. `app.windows` lists the open windows — each one's
`key`, the `program` it runs, its `title`, and whether it is
`windowless` — with `posts`, the `Request::Post`s delivered and those
dropped because no open window had the key (M89/T89.4), and `app.close`
closes the window a request names
(closing the primary ends the process; M89/T89.2). The list above, the
skill's and the code are held to one another by
`scripts/check_agent_methods.sh`.

**One port, windows by key** (M89/T89.2, `docs/plan-windows.md`). A
request may carry a top-level `"window"` member beside `"method"`: the
key of the window it is for, as `app.windows` reports it. Without one it
is the **primary** window's (key 1) — never the focused window's, since
every live drive opens its windows without focus. A key no open window
has answers `-32602`, and the message names `app.windows`. **Every reply
names the window that answered** with its own top-level `"window"`, so a
drive's output says who answered; the `-32602` for a window nobody has
is the one reply without it. A call whose drain opened windows — a click
on a button whose handler pushed `Request::OpenWindow` — also carries
`"opened": [keys]`, the windows that drain created (M89/T89.3): the one
observation that can tell a window opened inside the call from one
opened just after it. `app.quit` still ends the process whichever
window it names, and closing one window is `app.close`, never `app.quit`
with a field: a method whose meaning flipped on an optional member would
let a stale script close a window where it meant to exit.

**A synthesised input reports what arrived.** Everything above goes
through the host's input queue, which has a fixed size, so `input.type`
and `input.fill` feed long text in chunks and pump between them. The
`input.type` reply's `chars` is the number of characters that reached
the queue — `sent` less `dropped` — and `app.diagnostics` carries the
host's `dropped_inputs` count, ever. A fill the queue cut short is an
error that says so, rather than blaming the control.

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

**Steer** — `app.repaintAll` marks everything and repaints the whole
frame, so a drive can compare the frame damage produced with a full
repaint of the same state (M57/D6). `app.setPalette` swaps a running window's palette, by name
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

`W1012` names a rule that sets a length the cascade cannot lay out — a
NaN spacing, an infinite padding, a NaN font size — or a negative padding
edge. The cascade repairs it where it enters (the field takes its type
default; a bad padding edge becomes 0) and this says that it did, by rule
and field. Before, one NaN left the window blank and unclickable with no
diagnostic at all (M55/T55.5).

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

**The two damages are different quantities.** `ui.lastDamage` is which
nodes the journal saw change, in logical pixels; `app.perf { since }`
answers what the surface actually presented over those frames, in
physical pixels. The widget drive prints both for every action, and an
action that repaints must show some of each (M48/T48.4).

**An `input.*` reply's `frame` is the first journal frame that includes
the input**, so `since = frame - 1` finds what it changed once the window
has drawn that frame. It used to be the host's own counter, which moves
twice per input — the pump inside the request and the redraw after it —
so from the second click on, `since = frame - 1` pointed past every change
the click made (M48/T48.1). The host's number is still in the reply, as
`host_frame`.

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
The same answer carries `"reissued"` and `"reissue_dropped"`: how many
jobs that were in flight at the last swap the new build started again,
and how many it had no job for (M49/T49.10). Treat a non-zero
`reissued` as a reason to wait before reading the screen. `ui.waitSettled`
counts those jobs like any others.

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

## What each widget tells a screen reader

The agent's tree is the screen reader's tree, so a widget's role, name
and state are one answer for both. The table the role test holds every
widget to (`crates/lumen-static-widgets/tests/a11y_roles.rs`, M54/T54.3)
— `row(id, impl, role, named)`, where `named` means the control carries
a name when it is built with one (its `label`, or an image's `alt`):

```text
{{#include ../../crates/lumen-static-widgets/tests/a11y_roles.rs:rows}}
```

A control with no visible text of its own — a checkbox, a slider, a
field — is announced by its `label`, which is empty by default and then
not published: an application says what the control is for, with
`set_label`. The field exists only with `a11y`, so a build nobody reads
aloud — the firmware — carries no name at all. The test
counts `impl Node for` in the widgets' source, so a new widget without a
row fails it by name, and `just widget-drive` checks each driven
widget's role live (`scripts/drive/roles.py`).

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

**It is two features, and the build insists on both.** A widget's
`observe` impl is gated on `lumen-static-widgets/observe`, not on the
core's, so an agent build that turned on only the core's reported every
widget as `GenericContainer`. The widgets crate now refuses to compile in
that graph and names the feature to add (M48/T48.9,
`docs/results-widget-crate.md`).

`docs/results-agent-cost.md` measures what it costs when it is on.
