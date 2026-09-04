# M14 — the desktop shell

*Planned 2026-09-05, after M13. Status: **not started**. Detail for
`docs/plan-architecture-v2.md` M14.*

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

## T14.0 — `lumen-static-winit`: extract the window — **DONE 2026-09-05**

**The largest gap, and the least interesting work.** There is no
windowing layer. `lumen-static-shell` has *no window dependency at all*
— it is `Host` + `Canvas` + text — and the winit/softbuffer glue lives
in an example, tangled with the agent endpoint, headless rendering and
the profiler. A second application copy-pastes 480 lines of event loop.

**Decided: a new crate `lumen-static-winit`**, not a feature on the
shell. The shell is the headless, portable half and every test depends
on it staying that way; adding `winit` there would put a windowing
system behind `cargo test`. The name states the dependency rather than
claiming generality — there is no second backend, and pretending
otherwise is how a premature abstraction gets written.

What moves: window creation, the `ApplicationHandler`, event
translation (`named_key` and its callers), the softbuffer present with
damage and buffer age, scale-factor changes, the `a11y` feature.

What stays behind: the agent endpoint (it belongs with
`lumen-static-agent`), `--headless`, `--drive`, the profiler and the
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

"Total lines in the example crate" was never the property worth
gating; "lines an application must write to get a window" was. The gate
should have said that, and this entry is the correction.

**And the split made the input path testable for the first time.**
`translate.rs` is pure — `WindowEvent` in, `Event` out — so
`crates/lumen-static-winit/tests/translate.rs` covers all twelve keys,
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
per platform in `lumen-static-winit`, so an application writes one
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
`crates/lumen-static/tests/modifiers.rs` asserts `Event` is still 16 B,
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

## T14.3 — mouse buttons and click counts

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

---

## T14.4 — text selection

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

---

## T14.5 — clipboard

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

**Gate:** pasting a string longer than `InputQueue::CAP` lands whole
and `dropped()` stays 0. Copy of an empty selection is `None`, not
`Some("")`.

---

## T14.6 — cursor icons

No I-beam over a text field, no pointer over a button. Small, and it is
most of what "feels native" means.

**Decided: `Style::cursor`, not an event and not a widget method.**
Style already cascades, is already per-node, and is already what a
stylesheet sets — so a cursor becomes a rule like any other, and a
custom widget gets one without implementing anything. The shell reads
the hovered node's style (C6 already tracks `hovered`) and sets the
window cursor.

Set: `Default`, `Text`, `Pointer`, `Grab`, `Grabbing`, `ColResize`,
`RowResize`, `NotAllowed`. `Style` is 72 B; a `u8` fits in existing
padding — **to be verified, not assumed**, since a growth here costs
every node of every tree.

**Gate:** `size_of::<Style>()` is unchanged. `ui_check.py` gains a
check: hovering `#field` reports an I-beam, hovering the background
does not.

---

## T14.7 — scrollbars

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

---

## T14.8 — IME — **OPEN, and possibly its own milestone**

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
