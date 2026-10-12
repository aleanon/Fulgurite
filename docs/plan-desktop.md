# M14 — the desktop shell

*Planned 2026-09-05, after M13. **Status: done 2026-09-05,
T14.0–T14.8.** Detail for `docs/plan-architecture-v2.md` M14.*

> **What building it changed.** Four of this plan's decisions were
> wrong and the work found them, not review: the click count belonged
> in `InputState` rather than on `PointerUp`; `Button` collided with the
> widget of that name; the T14.2 gate could not be built because winit's
> `KeyEvent` has no public constructor; and a preedit has to be *in* the
> buffer, not beside it. Each is marked at its task. The one thing the
> plan got exactly right was measuring `Event`'s size before designing
> the vocabulary — it is still 16 B, and the queue is still 1048 B.

## The premise

M13 asked whether the seams held from the other side, and they did: a
second `Painter`, a second `TextMeasure`, a third `Retention`, a second
`TextBuffer` all plugged in without touching the first.

The desktop question is the opposite one. Not *"can another
implementation plug in"* but *"is anything missing that no
implementation can supply"* — and the answer is yes. **None of the gaps
below is a swap.** Each is a capability the framework does not have, in
the core vocabulary or in a crate that does not exist.

## What is already swappable, and therefore not in scope

| Seam | Desktop default | Proven by |
|---|---|---|
| `Painter` | `DisplayList` | `Scaled`, `Recording`, the M13 checksum sink |
| `Renderer` | `Canvas` (software) | — |
| `TextMeasure` | `FontMetrics` | `BlockMetrics`, `BakedFace` (T13.7) |
| `Retention` | `Incremental` | `Rebuild`, `Streaming<N>` (T13.8) |
| `TextBuffer` | `String` | `Cow`, `InlineText<N>` (T13.5) |
| feature tier | `std` | `alloc`, neither (T13.1/T13.4) |

Desktop is the default type argument of every one of those, and M13
*improved* it rather than costing it anything: retention got faster at
every scale (`docs/results-retention.md`), and `num::clamp` removed a
panic path from every node of every frame. **Nothing in M14 is about
restoring desktop capability. It is all addition.**

## The measured starting position, 2026-09-05

Measured before planning, so the estimates below are anchored:

- `Event` **16 B**, `Key` **1 B** (12 variants), `EventCtx` **56 B**,
  `InputQueue` **1048 B** (`CAP` × 16 + bookkeeping).
- **9 `pass::dispatch` call sites** across the workspace — the cost of
  changing its signature.
- `examples/counter-window/src/main.rs` is **1160 lines**, of which
  roughly **480 are the window** (`WinApp` and its
  `ApplicationHandler`). The rest is the agent endpoint (~150), the
  headless render, the profiler, `--drive`, and the `--form` fixture
  (~265).
- The glue handles **8 `WindowEvent` variants**. `MouseButton::Left`
  only. No `Ime`. No `ModifiersChanged`.
- **The full M14 vocabulary measures 16 B** — `Key` with `Char(char)`
  and `F(u8)`, pointer events carrying a button, plus a `Modifiers`
  event. Measured 2026-09-05, *before* the design was written, because
  the queue is `CAP` × `size_of::<Event>()` and a vocabulary that grew
  the enum would grow every host's queue with it. **It does not: the
  queue stays 1048 B.**

## What this milestone is not

- **Not overlays.** Dropdowns, tooltips, modals and context menus are
  `docs/plan-widgets.md` T11.5, which is a *layout* milestone. T14.3
  unblocks the right-click that a context menu needs; it does not build
  one.
- **Not a GPU backend.** The software raster measures 0.31 ms a frame
  in the live gate. `Renderer` is where one would go, and nothing here
  narrows that seam.
- **Not a second windowing backend.** T14.0 extracts what exists; it
  does not abstract over winit.
- **Not complex-script shaping**, which
  `docs/text-render-adoption-decision.md` records as deferred twice.
- **Not multi-window, menus, or file dialogs.**

---

## T14.0 — `fulgurite-winit`: extract the window — **DONE 2026-09-05**

**The largest gap, and the least interesting work.** There is no
windowing layer. `fulgurite-shell` has *no window dependency at all*
— it is `Host` + `Canvas` + text — and the winit/softbuffer glue lives
in an example, tangled with the agent endpoint, headless rendering and
the profiler. A second application copy-pastes 480 lines of event loop.

**Decided: a new crate `fulgurite-winit`**, not a feature on the
shell. The shell is the headless, portable half and every test depends
on it staying that way; adding `winit` there would put a windowing
system behind `cargo test`. The name states the dependency rather than
claiming generality — there is no second backend, and pretending
otherwise is how a premature abstraction gets written.

What moves: window creation, the `ApplicationHandler`, event
translation (`named_key` and its callers), the softbuffer present with
damage and buffer age, scale-factor changes, the `a11y` feature.

> **The `a11y` feature did not move — until M54/T54.2 (2026-09-27).** It
> stayed in `examples/counter-window` as a hand-written adapter, so no
> application started through `run` could reach a screen reader
> (R15-F1). It is `fulgurite-winit`'s `a11y` feature and
> `fulgurite_winit::a11y` now, used by `run`, every `Program` and
> counter-window alike; default-off by a measured kill rule
> (`docs/results-a11y-platform.md`).

What stays behind: the agent endpoint (it belongs with
`fulgurite-agent`), `--headless`, `--drive`, the profiler and the
`--form` fixture. Those are harness and example concerns, and mixing
them into the window is what made this 1160 lines.

**Gate:** `scripts/ui_check.py` passes **unchanged** through the new
crate — all eight checks, including click-to-presented latency and
contrast. Behavioural equivalence on a real window is the only gate
worth having for a move like this. Plus: `counter-window` drops below
**400 lines** and a second example builds a window in **under 30**.

**Done. The live gate passes unchanged** — eight checks, frame p50
0.06 ms and click-to-presented 5.3 ms against a 100 ms budget, the same
numbers as before the move.

**One half of the line gate was the wrong measure, and the result says
so.** `examples/minimal` opens a real window — tree, event loop,
software raster, damage, buffer age, scale factor, system font — in
**24 lines of code**, which is the half that mattered. But
`counter-window` did *not* drop below 400 lines in total: it went from
1160 to **1255**, because splitting one file into five adds five module
headers, and the doc comments explaining what each module is for are
worth more than the lines they cost.

What actually moved is the *windowing* code, and the honest numbers are:

| | before | after |
|---|---:|---:|
| `main.rs` | 1160 | **43** |
| generic window code in the example | ~480 | **0** |
| `window.rs` (agent socket, a11y, sign cascade, app keys) | — | 384 |
| a second app's window | ~480 | **24** |

> **M60 (2026-09-27): `window.rs` is gone.** What it kept — the agent
> socket, the a11y adapter, the sign cascade and the app keys — was the
> window crate's already, except the keys and the keyed sheet, which
> became `Program::on_key` and `Program::style_by`. The tally runs on
> `Program::run`; `docs/plan-tally-slim.md` has the line counts.

"Total lines in the example crate" was never the property worth
gating; "lines an application must write to get a window" was. The gate
should have said that, and this entry is the correction.

**And the split made the input path testable for the first time.**
`translate.rs` is pure — `WindowEvent` in, `Event` out — so
`crates/fulgurite-winit/tests/translate.rs` covers all twelve keys,
the cursor-position-on-click rule, wheel lines versus pixels, and the
logical/physical conversion, with **no display**. None of that had a
test before, because it lived inside an event loop that cannot run
headless.

Three things the extraction turned up:

- **`Surface` must not own the `Canvas`.** The first design had it, and
  the tally's agent endpoint answers `ui.screenshot` before `resumed`
  has created a surface — which would have made it `None`. The canvas
  is the caller's; `Surface` is the softbuffer half alone.
- **The `a11y` feature broke and the default build did not notice**,
  because a default-off feature is not in `just test`'s path. Found by
  building it explicitly, which is now worth doing after any change to
  that file.
- **The 0RGB→RGBA readback moved into the crate** as
  `Surface::fetch_rgba`. It is the same for every window, and it was in
  the example only because the example was where windows lived.

---

## T14.1 — modifier state — **DONE 2026-09-05**

Nothing in the event vocabulary carries Shift, Ctrl, Alt or Cmd. That
single absence blocks keyboard shortcuts, Ctrl+C/V/A/Z, Shift+Arrow
selection and Shift+click range selection — which is why it comes
first among the vocabulary tasks.

**Decided: a `Modifiers` event in the queue, and `mods` on
`EventCtx`.** Three designs were possible:

1. A field on every event variant. Rejected: it duplicates the state on
   events that do not need it, and risks the 16-byte budget.
2. A side channel the host holds outside the queue. **Rejected on E0
   grounds** — `docs/plan-events.md` E0 is "one queue, so anything an
   agent does replays as a test", and a modifier state that never
   enters the queue makes a recorded session unreplayable exactly when
   modifiers matter.
3. `Event::Modifiers(Mods)` in the stream; `dispatch` tracks the
   current value and puts it in every `EventCtx`. **Chosen.** It keeps
   `Event` at 16 B, it replays, and it mirrors winit's own
   `ModifiersChanged` so translation is a forward.

`Mods` is a `u8` bitflag: `SHIFT | CTRL | ALT | META`. **`META`, not
`CMD` or `SUPER`** — one name for the platform's own modifier, mapped
per platform in `fulgurite-winit`, so an application writes one
shortcut table. Whether Ctrl or Meta is "the" shortcut modifier is the
*application's* choice; the framework reports both truthfully.

**A signature change: `dispatch` now needs a third piece of caller-owned
state** beside `focus` and `capture`. Six arguments becoming seven is
the point at which this stops being readable.

**Decided: bundle them.** `pass::dispatch(state, style, st, &mut
input, ev)` where `InputState { focus, capture, mods }` is a plain
struct the host owns. 9 call sites, all in this repository. It also
gives the buffer-age and recording work one thing to snapshot instead
of three.

**Gate:** `size_of::<Event>()` is asserted to be 16 in a test, so the
budget is a gate rather than a memory. A recorded session that includes
modifier changes replays identically (T12.3's `Recording` already has
the machinery).

**Done, as designed.** `Event::Modifiers(Mods)` in the queue, `mods` on
`EventCtx`, `InputState { focus, capture, mods }` bundled.
`crates/fulgurite/tests/modifiers.rs` asserts `Event` is still 16 B,
`Mods` 1 B and `InputQueue` 1048 B — the budget is a gate now, not a
memory.

The bundle cost more churn than the plan's "9 call sites" implied:
`Host.focus` and `Host.capture` were **public fields** with readers in
five test files and the window, so it was closer to 30 edits. Worth it
anyway — the alternative was `Host` holding focus, capture *and* mods as
three fields while `dispatch` took an `InputState`, which is two
representations of one thing and a sync bug waiting to be written.

---

## T14.2 — the key set — **DONE 2026-09-05**

`Key` has 12 variants and no way to name a shortcut's letter.
Printable characters arrive only as `Event::Text`, which is the typing
path — so Ctrl+C would type a "c".

**Decided, and scoped to the common keys rather than a keyboard map:**

| Added | For |
|---|---|
| `PageUp`, `PageDown` | scrolling and text navigation |
| `Insert` | completes the editing set beside Delete/Backspace |
| `Char(char)` | shortcuts: Ctrl+A/C/V/X/Z/S/F and every application's own |
| `F(u8)` | F1 help, F2 rename, F5 refresh, F11 fullscreen — one variant, twelve keys |

**Deliberately excluded**, so the omission is a decision and not an
oversight: media and browser keys, numpad-vs-row distinctions, and the
modifier keys themselves. Modifiers are *state* (T14.1), not keys; a
`Key::Shift` would let a widget see a shortcut half-pressed.

**And a rule the shell must follow:** `Event::Text` is **suppressed
when any modifier but Shift is held**. Shift is excluded because
Shift+A *is* how a capital arrives. Without this rule Ctrl+C both fires
the shortcut and types a letter — the specific bug this task exists to
prevent, and one that is invisible until a text field has focus.

`Key` grows from 1 B to 8 B; `Event` stays 16 B (measured).

**Gate:** a table test walks every added key from a winit `KeyEvent`
through translation to `dispatch`, so a key that translates to nothing
fails rather than being silently dropped — which is what `named_key`
returning `Option` does today.

**Done.** `PageUp`, `PageDown`, `Insert`, `Char(char)`, `F(u8)`;
`Key` is 8 B and `Event` is still 16.

**The gate as written could not be built, and the reason improved the
code.** `winit::event::KeyEvent` has a `platform_specific` field with no
public constructor, so a test cannot synthesize a keyboard event at all.
So the keyboard branch is now `translate::keyboard(logical, text, down,
held)` — a function of what a key event *says* rather than of winit's
type — and `window_event` calls it. That is the branch most worth
testing, because it is where the shortcut/typing rule lives, and it was
the one branch the original design left untestable.

Thirteen tests now cover translation, including the two rules that
matter: **Ctrl+C produces the key and no text**, and **Shift+A still
types**. Both were written as the bug they prevent.

---

## T14.3 — mouse buttons and click counts — **DONE 2026-09-05**

Only `MouseButton::Left` is handled. Right-click reaches nothing, so
there are no context menus; and nothing counts clicks, so there is no
word-select or line-select.

**Decided:** `Button { Left, Right, Middle }` on `PointerDown` /
`PointerUp`, and a `clicks: u8` on `EventCtx` (1, 2, 3) rather than
separate `DoubleClick` events — a widget that does not care reads the
same `PointerDown` it reads today, and one that cares reads a field.
Back-mouse buttons are excluded for the same reason as media keys.

**Thresholds, stated rather than guessed: 500 ms and 5 logical px.**
Both are the platform-conventional values; both are constants on the
host so an application can change them, and both are *logical* px so a
scale factor does not change the feel (M10b).

**Gate:** a right-click reaches a handler that a left-click does not;
three clicks inside the threshold report 1, 2, 3 and a fourth outside
it reports 1; a click 6 px away restarts the count.

**Superseded in part by M90/T90.2 (2026-10-06):** a right press no
longer reaches any handler as a press. It is one `Event::ContextRequest`
at its point, and a right or middle release activates nothing
(`docs/plan-desktop-integration.md`, "A secondary button never reaches
the press machinery"; `docs/results-desktop-integration.md` T90.2).

**Done**, with two corrections to the design as planned.

**The type is `PointerButton`, not `Button`** — the prelude already
exports `Button`, the *widget*, and a framework whose prelude has two
`Button`s is a framework that gets one import wrong per file. The events
are `PointerDown`/`PointerUp`, so the name matches them.

**The click count is tracked in `InputState`, not carried on
`PointerUp`.** Planned as a field on both pointer events; the test found
why that is wrong within a minute — a `Button` activates on the
*release*, and the count is known at the *press*, so `ctx.clicks` was
always 0 at the moment a handler could read it. `dispatch` now carries
the press's count forward to its release, which also means a host that
*synthesizes* a release — the agent, a screen reader — does not have to
know the count of the press it is completing.

**And 60 call sites wanted `Event::down(x, y)`.** Adding two fields to
`PointerDown` broke every literal construction in the workspace, most of
them tests that mean "click here" and have no opinion about buttons or
click counts. `Event::down`/`Event::up` are those tests' vocabulary;
the literal form is for the host that actually knows.

Counting lives in `fulgurite-winit`, not `dispatch`: it needs a
clock, and the core is `no_std`. `MULTI_CLICK_TIME` is 500 ms and
`MULTI_CLICK_SLOP` is 5 **logical** px, both public constants, both the
platform-conventional values.

---

## T14.4 — text selection — **DONE 2026-09-05**

`TextInput` and `TextArea` cannot select. `input.rs` says why:
selection needs *"modifier state on `Event::Key` (for Shift+Arrow), or
pointer drag (`PointerMove` is deliberately not routed)"*.

**Half of that is now stale, and finding it is part of this task.**
T11.2 added pointer capture, and `pass.rs` routes `PointerMove`
directly to a captured node — which is exactly the mechanism a
drag-select needs, and is how `Slider` already works.
`docs/plan-widgets.md` says so ("selection needs … pointer drag
(T11.2's capture)"); `input.rs` still says the opposite. **The comment
must be corrected in the same commit that acts on it** — it is a
comment describing machinery in another file, which the doc-currency
rule in `AGENT.md` already names as the fragile kind.

So selection needs T14.1 for Shift+Arrow, and nothing at all for
drag-select.

**Decided:** an `anchor: usize` beside the caret — the field B6/B7
refused to reserve while nothing could move it, which T14.1 and capture
now can. Rendering is a `Cmd::Rect` behind the run at the selection's
cluster bounds; B4 already decided the rectangle.

**Gate:** Shift+Arrow and drag produce byte-identical selections for
the same span. `damage_oracle.rs` stays pixel-identical — a moving
selection rect is a new damage shape and the oracle is what proves it
is covered. Typing over a selection replaces it.

**Done, in both widgets**, with the stale comment corrected in the same
commit as the rule says. 13 tests, including the equivalence the task
turns on: Shift+Arrow and drag select the same span.

**The selection colour is derived, not a `Style` field — and that is a
budget decision, recorded rather than defaulted.** A selection colour
*is* a style property, but paint only has `NodeState`, so a real one
>
> **Both figures below are M14-era and have since moved**: M27/T27.1's
> border fields took `Style` to **80 B** and `NodeState` to **84 B**,
> asserted in `crates/fulgurite/tests/cursor.rs` and
> `tests/focus.rs`. The tests are the live answer; these paragraphs
> record what the decision was taken against.

would have to live there too. `NodeState` was 76 B and asserted at that
size by `fixtures/embedded` against the RAM budget in
`docs/results-embedded.md`; four bytes times every node of every tree
is a poor trade for a colour that can be derived. `selection_tint` is
the foreground at alpha 64, which works in light and dark themes
because it tints with the colour that by construction contrasts with
the field, and the canvas blends it so the glyphs stay readable.

**`TextArea`'s selection is one rect per line**, not one rectangle: a
multi-line selection is a partial first line, whole middle lines and a
partial last one. `selection_runs` yields per line so `paint` does not
have to know that.

**A test that needed a measurer to exist at all.** `BlockMetrics`
reports *no* clusters — `TextMeasure::shape`'s default measures and says
nothing — so a field laid out with it has no geometry to hit-test and
every click lands at the end of the run. The drag test supplies a
monospace stand-in, which is what a real host does too. Anything
testing a caret's *position* has to; that is worth knowing before the
next such test is written.

---

## T14.5 — clipboard — **DONE 2026-09-05**

> **M45 (2026-09-26).** The modifier is the platform's — Cmd on macOS,
> Ctrl elsewhere — where either had been accepted on every OS, so
> Super+C copied on Linux; and a shortcut with nothing to act on
> (nothing focused, nothing selected, an empty clipboard) routes the key
> on to the application instead of consuming it (T45.9, R10-F13). A
> paste is one edit through `Node::paste`, no longer per-character
> typing (T45.7). `docs/results-text.md` T45.7, T45.9.

There is none. For a text field this is table stakes.

**Decided, and the design avoids a vocabulary change.** `Event` is
`Copy` and 16 bytes; a `Paste { text: &str }` variant would put a
lifetime on the whole enum and a borrow in the queue. Instead:

- **Paste** is `Host::paste(&str)`, which pushes `Event::Text` per
  character. No new variant, and it replays as ordinary typing.
- **Copy** is `Host::copy() -> Option<String>`, backed by a new
  `Node::selection(&self) -> Option<&str>` (default `None`) asked of
  the focused node. Needs T14.4.

**The queue is `CAP` = 64 and a paste is not**, so `Host::paste` fills
to the free space, pumps, and repeats. That is the whole reason
`InputQueue::push` returns `bool` and counts drops (T13.4) — this is
its first real caller, and a paste that silently lost its tail is
exactly the failure that counter exists to make visible.

**The agent's `input.type` and `input.fill` did not chunk, and the
counter had no reader** (R3-F2, closed by M44/T44.5). A 100-character
`input.type` left 64 in the field and a reply saying 100; nothing outside
`event.rs` read `dropped()`. They now chunk the same way (half the queue,
then pump), reply with the characters that *arrived* beside `sent` and
`dropped`, and `Host::dropped_inputs` puts the counter in
`app.diagnostics`. The same milestone made `Host::post` merge a move into
a trailing move and let a release evict a queued move rather than be
refused (`InputQueue::push_merged`), so a burst of pointer input can no
longer strand a capture.

**Gate:** pasting a string longer than `InputQueue::CAP` lands whole
and `dropped()` stays 0. Copy of an empty selection is `None`, not
`Some("")`.

**Done**, in three pieces that stay on their own side of the seam:

- `Node::selection() -> Option<&str>` in the core, defaulting to
  `None` — borrowed, so the core never allocates to answer.
- `Host::copy()` and `Host::paste()` in the shell. Paste is **per
  character into the one queue**, so it replays as ordinary typing and
  gets selection-replacement, caret movement and bindings by the paths
  typing already uses. No second edit path to get subtly different.
- `Clipboard` in `fulgurite-winit`, because a clipboard *is* a
  platform and neither the core nor the shell has one.

*Amended 2026-09-25, M41/T41.5:* "no second edit path" was false in
one respect. `Host::paste` pumped with the `BlockMetrics` stand-in, and
the window's session called it although the session owns the real
measurer, so a pasted field was laid out by a different text stack than
a typed one — and kept that box, since the shaped memo is not keyed on
the measurer (R2-F1, R10-F4). `Host::paste_with` takes the caller's
measurer, `Session::paste` passes the session's, and `paste` is
`paste_with` with the stand-in, for headless callers.
`crates/fulgurite-shell/tests/paste_measurer.rs` is the gate.

**Default-on, and the difference from `a11y` is measured.** `arboard`
costs **11 crates** (207 → 218, 2026-09-05) against `a11y`'s 62
(95 → 157). A text field without copy and paste is not a working text
field, and a fifth of `a11y`'s cost is worth paying by default. Without
the feature the type still exists and every operation is a no-op, so a
host needs no `cfg` around its Ctrl+C.

Two details the writing settled:

- **The handle is held open, not opened per copy.** On X11 a clipboard
  owner must stay alive to answer paste requests, so a handle dropped
  after each copy puts text on the clipboard that vanishes when the
  user switches windows.
- **Cut is copy plus the Backspace the field already implements**,
  posted as an event rather than called as a method. One edit path, so
  the caret and the shaped cache cannot disagree with a bespoke one.

---

## T14.6 — cursor icons — **DONE 2026-09-05**

No I-beam over a text field, no pointer over a button. Small, and it is
most of what "feels native" means.

**Decided: `Style::cursor`, not an event and not a widget method.**
Style already cascades, is already per-node, and is already what a
stylesheet sets — so a cursor becomes a rule like any other, and a
custom widget gets one without implementing anything. The shell reads
the hovered node's style (C6 already tracks `hovered`) and sets the
window cursor.

Set: `Default`, `Text`, `Pointer`, `Grab`, `Grabbing`, `ColResize`,
`RowResize`, `NotAllowed`. `Style` was 72 B then; a `u8` fits in existing
padding — **to be verified, not assumed**, since a growth here costs
every node of every tree.

**Gate:** `size_of::<Style>()` is unchanged. `ui_check.py` gains a
check: hovering `#field` reports an I-beam, hovering the background
does not.

**Done, and it was free as predicted**: `Style` was still 72 B and
`NodeState` still 76 B at the time, both asserted in
`crates/fulgurite/tests/cursor.rs`. The `Cursor` fits in the tail
padding beside `align` and `visible`.

**And `NodeState` needed nothing at all.** The plan assumed paint would
have to carry the cursor; it does not — the *host* resolves it, and the
host has the style array. `Host::cursor()` finds the deepest hovered
node and reads `styles[i].cursor`. Deepest, because C6 marks every node
on the hover chain and an ancestor's cursor must not override the leaf
the pointer is on: a button inside a text field's row has to read as a
button.

**The row case this names was not true until M44/T44.8 (2026-09-26).**
A *repeater's* row is not in the main table, so the scan over hovered
bits found the repeater and read its cursor: a `Button` in a list row
read as `Default` (R15-F7). `Host::cursor` now reads the node the last
move hit, and when that hit was on a row it asks the repeater for the
row's resolved cursor (`Node::inner_cursor`).
`crates/fulgurite-shell/tests/cursor_rows.rs` gates it: a table
button, a button row and a label row read `Pointer`, `Pointer`,
`Default`.

`TextInput` and `TextArea` default to `Text`, `Button`, `Slider` and
`Toggle` to `Pointer`, so an application gets the right shapes without
writing a rule. `Cursor` is a closed enum rather than a string because
the windowing layer has to map every variant to a platform cursor, and
an unmappable name is a silent no-op.

The window only calls `set_cursor` when the shape *changes*: setting it
is a round trip to the display server, and a frame loop that set it
unconditionally would make one per frame for a cursor standing still.

---

## T14.7 — scrollbars — **DONE 2026-09-05**

`ScrollView` scrolls but draws nothing. A scroll region a user cannot
see the extent of is a usability defect, not a missing decoration.

**Decided:** a track and thumb painted by `ScrollView` when
`measured_overflow > 0`, styled from `Style`, and **draggable** —
which is pointer capture again (T11.2), the third widget to use it.

**Gate:** no overflow paints no scrollbar (so a short list is
unchanged); dragging the thumb to the bottom equals scrolling to the
bottom; `damage_oracle.rs` stays pixel-identical with the thumb moving,
which is the interesting case because the thumb and the content damage
different regions in the same frame.

**Painted, not yet draggable.** A track and a thumb appear when there
is overflow and not otherwise — a bar that is always present says "this
scrolls" about something that does not. The thumb is as long a fraction
of the track as the viewport is of the content, floored at 24 px so a
very long list still has something to grab.

**The bar is painted outside the clip**, and that is the part worth
asserting: it belongs to the viewport, not the content, so clipping it
would be right by accident. The test checks the paint *order* as much
as the geometry.

**Dragging the thumb is deliberately not done here.** It is the fourth
user of pointer capture and it needs a hit region that is *not* the
content's — `ScrollView::hit` currently forwards to the content and
falls back to itself, so a press on the bar reaches the content
underneath. That is a hit-testing change, not a paint one, and bundling
it into a paint task is how it would get underestimated. Recorded as
the remaining half rather than left implied.

---

## T14.8 — IME — **DONE 2026-09-05** (was OPEN)

> **M45/T45.8 (2026-09-26): key text that is not one character is a
> commit.** winit's `KeyEvent::text` can hold several characters without
> any IME — a dead key that did not combine (`"^a"`), a decomposed
> accent, a flag — and the window kept the first and dropped the rest
> (R10-F8). It now routes such text through `Session::compose` as a
> commit, the path this task built; one character is still an
> `Event::Text`. `docs/results-text.md` T45.8.
>
> **M45/T45.3 (2026-09-26): what IME commits is now drawable.** Until
> then the font stack was one face and substituted `?` for every
> character it lacked, so a CJK commit reached the buffer and rendered
> as `??` (R10-F12). `docs/results-text.md` T45.3.

Without it, Chinese, Japanese and Korean input are *impossible*, not
merely awkward. `WindowEvent::Ime` is not handled and the vocabulary
has no preedit.

This is the one item I do not think should be scoped inside M14 on the
evidence available. Done properly it needs: a preedit string that is
*provisional* (rendered, underlined, not in the buffer), a commit that
replaces it, cursor reporting so the platform places its candidate
window, and a composition seam in `TextBuffer` — which is a trait three
storages now implement, one of them fixed-capacity. `docs/text-and-
scrolling-decisions.md` B6 named it and decided it out of the first cut.

**Recommendation:** take T14.0–T14.7 first, then decide whether IME is
a task or a milestone with what those taught. Specifically: T14.4's
anchor is the machinery a preedit range would reuse, and whether that
reuse is clean is the question that decides the size. Committing to a
shape now would be guessing.

**If it is taken:** the gate is that a preedit renders underlined
without entering the buffer, a commit replaces it atomically, and
`InlineText` — which can *refuse* an insert — reports failure rather
than stranding a composition, which is the T13.5 bug in a new place.

**Decided after T14.0–T14.7, as this task said to: it is a task.** The
question was whether T14.4's anchor machinery would be reusable, and
the answer is that it does not need to be — a preedit is a *different*
range with a different lifetime, and the two coexist. Once that is
clear the work is bounded: one non-`Event` type, one `Node` method, one
field per widget, one underline, and a twenty-line platform handshake.

**One correction to the gate as written.** It says "renders underlined
*without entering the buffer*". That is the wrong model, and building it
showed why: provisional text that is not in the buffer cannot be
measured, positioned, scrolled or hit-tested, so every one of those
paths would need a parallel implementation. **The text goes in the
buffer**; what makes it provisional is a recorded range that the next
`Preedit` replaces. That is also what the platform protocol assumes —
winit sends an empty `Preedit` immediately before every `Commit`, which
is only coherent if the preedit is really there.

**The T13.5 bug did reappear, and was caught by the design rather than
by a test.** A bounded storage can refuse the provisional text, so the
range is recorded from what the buffer **took** — `insert` moved the
caret by exactly that much — not from what was offered. A range taken
from the offered string would point past the end of a full
`InlineText`. Two tests cover it.

**Not done, and named rather than implied:** `set_ime_cursor_area`, so
the platform places its candidate window at the caret rather than at the
window's origin. It needs the caret rect in physical pixels, which the
host can produce but does not expose; the composition itself works
without it.

**Done by M80/T80.5 (2026-10-04):** `Node::caret_rect` answers the
focused editor's caret, `Host::caret_rect` exposes it in physical pixels,
and the window passes it to `set_ime_cursor_area` after each frame in which
it changed (`app.imeArea` reports what it passed). Gated against the
painted caret, live and headless (`docs/results-text.md` M80/T80.5).

**Amended 2026-09-25, M41/T41.1: the routing is gated now, and it was
wrong under `Boundaries`.** The tests above call the widgets' inherent
`compose`; nothing called `Host::compose`, the path the window uses, so
both `Node::compose` impls could be no-ops with every test green
(R10-F9). And `Host::compose` marked nothing, so once `Boundaries<256>`
became the default (M34/T34.2) a commit was painted over last frame's
layout (R1-F2). It marks the focused subtree now, and
`crates/fulgurite-shell/tests/compose_marks.rs` routes Tab, a preedit,
a commit and a cancel through the host and compares every frame with
`Everything` (`docs/plan-boundaries-repair.md` T41.0–T41.1).

---

## Sequence

T14.0 first, so every later change lands in the windowing crate once
instead of in an example that is about to move. Then T14.1, because
modifiers unblock more than anything else in the list. Then:

```
T14.0 → T14.1 → T14.2 → T14.4 → T14.5
                  ↓
                T14.3 → (context menus wait on T11.5)
T14.6, T14.7 are independent; take them when convenient
T14.8 decided after T14.4
```

T14.6 and T14.7 depend on nothing here and could be done first if a
visible improvement is wanted early — T14.7 in particular is small and
`ScrollView` has shipped without it since M8.

## Risks

- **T14.0 is a move, and moves lose behaviour silently.** The live gate
  is the mitigation and the reason its eight checks are the stated
  gate rather than "it compiles".
- **`dispatch`'s signature change touches 9 sites** and every one is a
  test that constructs input state by hand. Cheap, but it is the kind
  of churn that invites "while I'm here".
- **T14.2's `Event::Text` suppression rule is a behaviour change to
  existing typing**, and the failure mode — a shortcut that also types
  a letter — appears only when a text field has focus. It needs a test
  with focus set, not a unit test on translation.
- **`Style::cursor` must not grow `Style`.** 72 B × every node of every
  tree; if the `u8` does not fit in padding, this becomes a decision
  rather than an addition.
- **Nothing here needs `alloc` in the core**, and the two host-side
  additions that do (`Host::copy` returning a `String`, the clipboard
  itself) live in the shell and the windowing crate, which are `std`
  by nature. M13's tiers must still build: `just thumb` is part of
  every gate in this milestone.
