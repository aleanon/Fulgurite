# M44 — input semantics and keyboard reach

> **T44.0–T44.10 DONE 2026-09-27** (branch `impl/m44`):
> - T44.0: every gate committed red first, in two landings; `overlay_stack.rs` closes R18-F7 green, with its three mutants red.
> - T44.1: `Node::owns_arrows` for the eight arrow-driven widgets; the group is walked, not capped at 16 (R3-F1, R10-F3, R3-F9).
> - T44.2: a click is a press and a release on the same `(node, row, row_generation)` (R3-F3).
> - T44.3: a captured node that leaves the screen loses the capture (R3-F4).
> - T44.4: the primary touch drives the pointer; `Event::PointerCancel` (R15-F4).
> - T44.5: moves merge, a release evicts a move, the drop count is published; the agent chunks input (R3-F2, R3-F11).
> - T44.6: an event writes at most the nodes it changed — 2 writes per move on 1,728 nodes, was 3,457; focus is published after the event (R3-F12).
> - T44.7: `Repeat`'s handler goes through the ring slot (the same fix as M56's `af902b9`), and rows past the addressable ceiling are cut (R3-F5, R3-F10).
> - T44.8: the cursor over a repeater row is the row's (R15-F7).
> - T44.9: only a modal traps Tab; a menu is dismissed by it; focus returns when an overlay closes (R3-F7).
> - T44.10: rows a keyboard can choose — `Repeat`'s active row, `Node::active_row`, the choosers, pickers, `Tree` and `DataGrid`; `Select` and `ScrollView` are driven live, both strategies (R3-F6).
>
> **Written 2026-09-24.** Closes R3-F1, R3-F2, R3-F3,
> R3-F4, R3-F5, R3-F6, R3-F7, R3-F9, R3-F10, R3-F11, R3-F12, R10-F3,
> R15-F4, R15-F7 and R18-F7 from M38's deep review
> (`docs/results-deep-review.md`), as re-triaged against `main` at
> `1717808` on 2026-09-24. Where the review and the re-triage differ,
> the re-triage is what this plan follows, and where this plan found the
> re-triage wrong at `1717808` it says so in place (§2.2, §2.3).
>
> **Phase 3** of the fix queue: after M41 and M42. Every task here
> changes what an event consumes, and under the `Boundaries<256>`
> default what an event consumes is what gets redrawn. So the evidence
> for each task is a `Boundaries` frame compared against an
> `Everything` frame, and M41 is what makes that comparison trustworthy
> (its shadow oracle repairs the frames it inspects today, R1-F6). M42
> is what makes a live drive's "green both ways" name the binary it
> drove. T44.7's one-line ring fix is the exception and says why.
>
> The review found the input path correct pass by pass and wrong in
> assembly. The group rule is right for a toolbar and wrong for a field
> beside a button. Activation is right for a click and wrong for a drag
> that ends somewhere else. Capture is right for a visible slider and
> wrong for one that was hidden mid-drag. Each piece had a passing test
> for the case it was written for.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **Every input fix is checked against T29.6's narrowing.** Since
   M29/T29.6, a split application redraws only the nodes in the
   `Consumed` set, plus what its bindings write
   (`crates/lumen-static-shell/src/lib.rs:1064-1071`). A widget that
   takes an event without being pushed into `Consumed` is a stale frame
   under the default. So every task that changes what consumes an event
   adds that action to the session in
   `crates/lumen-static-shell/tests/handled_narrowing.rs`, whose oracle
   requires every frame to be byte-identical under `Everything` and
   `Boundaries`. **The review's prototype for R3-F1 (`11492f9`) is the
   example of what this rule catches:** it offers the arrow to the
   widget directly and never calls `consumed.push`.
2. **No new call site dispatches to `on_event` without pushing.** Where
   a fix needs a widget to see a key it does not see today, the fix
   routes the key into Pass 1 of `dispatch_at`
   (`crates/lumen-static/src/pass.rs:716-755`), which already pushes,
   rather than calling the widget from a new branch.
3. **Every overlay change is checked against M33's controlled
   `Select`** (`5161612`). Whether a menu is open is the application's
   state, and a dismissal must reach the owner through `tell_owner` or
   the next binding reopens it. `crates/lumen-static/tests/dismiss_click.rs`,
   `crates/lumen-static/tests/select.rs` and `handled_narrowing.rs`'s
   two-select step run in every overlay task's gate.
4. **Assert that the action landed**, not only that the result matches:
   `handled`, the node the probe hit, a count that moved. Two of the
   review's own probes would have passed vacuously without this (R3-F1's
   "two Lefts" round-trip; R18-F7's single-overlay "topmost").

## 1. What is wrong, at 1717808

### 1.1 The arrow-group rule takes the key before the widget (R3-F1, R10-F3, R3-F9)

`dispatch_at` moves focus within the focused node's parent on any
arrow key when that parent has more than one focusable child
(`pass.rs:521-558`, the return at `:547-553`). It runs before the
widget sees the key, and it never asks whether the widget uses arrows.
So a `TextInput`, `TextArea`, `NumberInput` or `Slider` with a
focusable sibling never receives an arrow: focus jumps, the caret stays,
typed text lands in the wrong place, a number input cannot step and a
slider cannot move (R3-F1, R10-F3; `docs/results-deep-review.md`). The
comments at `:524-526` and `:544-546` claim the opposite. The verifier
widened the scope: any arrow-driven widget is affected, not only text.
At `1717808` those are `TextInput`, `TextArea`, `NumberInput`,
`Slider`, `RangeSlider`, `SplitPane`, `PaneGrid` and `Pagination`.

**What the re-triage added.** With the review's prototype `11492f9`
applied (offer the key to the widget first, navigate on decline), the
`TextInput` case passes, but `Up` on a `TextArea`'s first line is
declined by the widget and still moves focus. So "ask first" does not
close R10-F3. And the prototype calls the widget without pushing
`Consumed` (§0 rule 1).

The group itself is collected into a 16-slot buffer
(`heapless_vec`, `pass.rs:981-1016`). Members past the sixteenth are
unreachable by arrows, and an arrow on one of them is a dead key: its
position in the group is `None`, so the branch does nothing and the key
falls through to a widget that ignores it (R3-F9).

### 1.2 A click is whatever the release lands on (R3-F3)

Activation runs on `PointerUp` at the release's hit
(`pass.rs:757-815`), and nothing compares that hit with the press.
`input.pressed` holds the right latch, but the `PointerUp` arm clears it
first (`:639-644`, and `:580-584` on the capture path). So dragging off
one button and releasing on another activates the second, and pressing
one row and releasing on another picks the second. When the list
changes between press and release, the handler receives the row index
now under the pointer, which is a different item (R3-F3). `EventCtx.row`'s
rustdoc ("can never go stale") is true of the index and false of the
identity. M33/T33.3's `swallow_release` (`:251-266`) is a different
case: a release after a press that dismissed an overlay.

### 1.3 Capture and touch: the pointer's lifecycle (R3-F4, R15-F4)

- **Capture outlives what it captured (R3-F4).** The capture block
  checks only that the index is inside the table (`pass.rs:574-577`).
  The C5 rule (`:480-492`), which drops focus from a node that is
  invisible or has no area, is applied to focus only. A slider hidden
  mid-drag keeps receiving moves and changing value until the release.
  Since T29.6 each such move also pushes the hidden node into
  `Consumed`, so it is marked and re-laid-out while invisible; that is
  more work but no new harm.
- **Touch reaches no widget (R15-F4).** `Event::Touch` is translated
  one to one from winit (`crates/lumen-static-winit/src/translate.rs:143-149`),
  and nothing in `pass.rs` or the shell promotes the primary touch to
  pointer events. No stock widget handles `Event::Touch`, so a tap on a
  Wayland touchscreen activates nothing (R15-F4). `docs/mobile-decision.md`
  defers packaging and devices, not touch-as-pointer.

### 1.4 Intake: the queue refuses silently, and every event sweeps (R3-F2, R3-F11, R3-F12)

- **Overflow (R3-F2).** `Host::post` is `self.queue.push(ev);` with the
  refusal discarded (`crates/lumen-static-shell/src/lib.rs:731-733`).
  Nothing outside `event.rs` reads `InputQueue::dropped`. The winit
  shell posts every `CursorMoved` without merging. So more than 64
  events between redraws loses the tail, and when the tail holds a
  `PointerUp`, capture is stranded: a later move with no button held
  still drags the slider. The agent's `input.type` and `input.fill`
  post one event per character into the same 64-slot queue and report
  every character as delivered (R3-F2). The re-triage re-ran both
  probes at `1717808` with the same results.
- **Stale comments (R3-F11).** `pass.rs:506-508` says Shift+Tab is not
  implemented, directly above its implementation. The agent's
  `input.type` comment (`crates/lumen-static-agent/src/lib.rs:724-726`)
  says the queue has no cap. `InputQueue`'s generic `Extend` impl
  (`crates/lumen-static/src/event.rs:970-972`) cites
  `InputQueue::<64>::CAP`. **This plan found a fourth at `1717808`:**
  the activation comment at `pass.rs:765-772` says Space does not
  activate because `Key` has no variant for it. `Key::Space` exists and
  activates (`:773-778`).
- **Sweeps (R3-F12).** Every dispatched event writes `focused` on every
  node (`pass.rs:494-499`). `set_hovered` clears every node on every
  move and press (`:972-979`), and a release clears every pressed bit.
  That is two writes per node per event, and moves are not merged
  (R3-F12). The claim at `pass.rs:568-572` ("moves cost one call") is
  about handler calls and is still true; the sweep is a cost it does not
  mention.

### 1.5 Rows the pass reaches wrongly (R3-F5, R3-F10, R15-F7)

- **Ring slot (R3-F5).** `Repeat`'s window is a ring: position `k` lives
  in slot `(first + k) % len` (`crates/lumen-static/src/repeat.rs:143`).
  `hit` and `paint` use the slot. `Handlers::handler`
  (`repeat.rs:651-656`) indexes the window by position, so once the
  ring has rotated, a bind that gives rows different handlers routes a
  click to another row's handler (R3-F5). The review's one-line
  prototype (`4c6bab6`) still applies.
- **Ceiling (R3-F10).** `Repeat::MAX_ADDRESSABLE_EXTENT`'s rustdoc says
  that above the ceiling `layout` debug-asserts and a release build
  scrolls with gaps (`repeat.rs:204-207`). No such assertion exists,
  and the failure is overlapping rows whose probe disagrees with paint,
  not gaps (R3-F10). The agent's audit lint is the only guard.
- **Cursor (R15-F7).** `Host::cursor` walks only the main table's
  hovered bits (`crates/lumen-static-shell/src/lib.rs:1378-1386`). A
  `Button` in a `Repeat` row reads as `Default`, which is the exact case
  `docs/plan-desktop.md` T14.6 names.

### 1.6 Overlays and the keyboard (R3-F7, R18-F7, R3-F6)

- **Every overlay traps Tab; Escape drops focus (R3-F7).** The Tab trap
  (`pass.rs:312-320`) applies to the topmost overlay of any kind. A
  tooltip has no focusables, so Tab while a tooltip shows sets focus to
  `None`, and stays dead while the pointer rests there. Escape clears
  focus (`:268-277`) or, on a modal whose panel closed, leaves it on a
  button that now has no area, which the next event's C5 check clears.
  Focus never returns to the trigger. M33/T33.3 added the passive-overlay
  idea (`Node::dismissal_consumes_press`, `crates/lumen-static/src/widget.rs:199-211`)
  for the press path only.
- **Stacking is untested (R18-F7).** Two mutants survive the whole
  suite at `1717808`: Escape closing the bottom overlay instead of the
  top (`pass.rs:269`), and the overlay hit loop running bottom-first
  (the `.rev()` at `:329`). The test named `escape_closes_the_topmost`
  (`crates/lumen-static/tests/overlay.rs:266`) builds one overlay. Two
  tests added since the review (`dismiss_click.rs`, `handled_narrowing.rs`)
  close two selects with one press, but their overlays do not overlap
  and nothing presses Escape with two open.
- **Keyboard users cannot choose a row (R3-F6, PARTIAL).** M33/T33.3
  made the `Select` trigger a `Button`, so Enter on a focused trigger
  now opens the menu. That half is fixed. **The re-triage's probe at
  `1717808`:** Tab focuses the trigger, Enter opens the menu, then Down,
  Down, Enter report handled `(false, false, true)` with nothing picked
  (the Enter re-ran the trigger's open handler), and Tab while the menu
  is open sets focus to `None` (§1.6's first bullet). Rows cannot be
  Tab stops, because focus is a main-table index and rows live in an
  arena. `Chooser`, `Picker`, `Tree` and `DataGrid` have no row
  navigation either.

## 2. Decisions

### 2.1 Who owns an arrow (T44.1)

- **A. Ask the widget first, navigate on decline** (the review's
  prototype, `11492f9`). About seven lines. But a declined arrow still
  leaves the field: Left at caret 0, and Up on a `TextArea`'s first
  line, move focus. The re-triage showed the second case red with the
  prototype applied.
- **B. The widget declares that it owns arrows.** A provided method on
  `Node`, `owns_arrows`, `false` by default and `true` for the eight
  arrow-driven widgets in §1.1. The group rule does not run when the
  focused node answers `true`, whether or not the widget then consumes
  the key. That is the platform convention: arrows never leave a text
  field.

**Recommended: B.** It also satisfies §0 rule 2 without effort: the key
simply skips the group branch and reaches Pass 1, which pushes
`Consumed` already. There is no new call site.

The method is asked of `node(i)` at the focused index, which resolves to
the widget that owns that index. That makes it a leaf's answer, like
`dismissal_consumes_press`, and a wrapper does not forward it. The one
trap is a composite whose `node_mut` returns itself for its inner
field's index. `NumberInput` does exactly that (`crates/lumen-static-widgets/src/field.rs:275`),
so `NumberInput` itself must answer `true`. T44.1's gate includes it
for that reason.

**One answer for all four arrows.** A slider in a column of sliders
then keeps Up and Down as well. ARIA's slider pattern does the same, and
a per-axis mask would be a second design for no case anyone has asked
for.

**What B breaks.** A focusable control that owns arrows can no longer be
left by an arrow, even from its edge. Tab and Shift+Tab are how a user
leaves it, as on every desktop platform.

### 2.2 What "the same item" means for a click (T44.2)

The re-triage proposed latching the press target together with the
row's `%k` key from M24/T24.4, and comparing keys on release, so that
removing a row mid-gesture cancels the click. **At `1717808` `%k` is not
an identity.** It is the agent selector for the *k*-th row
(`crates/lumen-static-agent/src/lib.rs:1633-1647`), a positional
index. `Repeat` has no item identity at all: `bind` is
`fn(&mut T, usize)` and never sees the application's data. So comparing
`%k` is comparing row indices, and the "item 105 becomes item 106" case
(the review's case (c)) is exactly the case in which the index is
unchanged.

- **A. Latch `(node, row)` only.** Fixes a drag between buttons and a
  drag between rows. Does not catch the list changing under a still
  pointer.
- **B. Latch `(node, row, generation)`**, where a `Repeat` reports a
  generation that it bumps in `layout` whenever its `count` differs from
  the last frame's. An insertion or removal between press and release
  cancels the click. A reorder or an in-place replacement with the same
  count does not.
- **C. B, plus an optional key function on `Repeat`**, a
  `fn(usize) -> u64` beside `bind`, supplied by an application that has
  real item identities. It is a function pointer for the same reload
  reason `bind` is one (`docs/events-decision.md`).

**Recommended: B now; C deferred** (§4) until an application needs
identity stronger than "the list did not change shape". B's generation
is asked of the hit's table node (the `Repeat` itself, `Hit::node`)
through a provided `Node` method whose default answers 0, so every
non-repeater keeps plain `(node, row)` semantics. **It is not a second
counter.** M41/T41.3 gives `Repeat` a data generation that a reported
data change bumps; T44.2 reads that one and adds a bump in `layout`
when the count changed, so an unreported insertion still cancels and a
reported in-place replacement now does too.

**The rule, written down in `docs/events-decision.md`:** a click is a
press and a release on the same logical item. A release anywhere else
clears the press shade and activates nothing.

**What it breaks.** A handler that relied on "release on a different
button" firing. None exists in the workspace; T44.2's commit greps for
drag-to-activate tests and lists what it found.

### 2.3 Which overlays trap Tab (T44.9)

The re-triage recommended a separate `is_modal` hook, true for `Veil`
and false for `Popup`, rather than reusing `dismissal_consumes_press`,
which answers a different question (whether the dismissing click is
eaten). **This plan agrees on the separate hook, but `Popup` cannot
answer `false` unconditionally.** At `1717808`
`crates/lumen-static/tests/overlay.rs`'s `focus_trap` module builds its
modal as a `Popup<Dialog>`, and `tab_cycles_inside_the_modal_and_never_leaves`
relies on the trap. `Popup` is the general overlay primitive (a menu, a
tooltip and that test's dialog are all `Popup`s).

So `Popup` gains a `modal` field beside `passive`, `false` by default,
and answers `is_modal` from it. `Veil` answers `true`. Tab then does
one of three things with the topmost overlay:

- **modal:** trapped, as today;
- **non-modal, not passive** (a menu): Tab dismisses it through the same
  path Escape takes, so `tell_owner` runs and M33's controlled `Select`
  clears its `open`, and then Tab moves on from where focus was;
- **passive** (a tooltip): Tab passes through untouched.

And the trap never sets focus to `None`: if a modal has no focusable
child, focus stays where it is.

**Focus restore.** `InputState` records the focus it had when an
overlay first appears in the overlay list, and Escape or a dismissal
restores it. If that node is gone (C5's test), focus goes to the
dismissed overlay's owner, which `tell_owner` already finds.

**What it breaks.** The `focus_trap` test must set `modal = true`, and
the commit says why. Any application using a `Popup` as a modal must do
the same. None in `examples/` does at `1717808`.

### 2.4 Cancelling a touch (T44.4)

A touch `Cancel` must release capture without activating anything. The
pass has no such path: `swallow_release` consumes a release but returns
before the capture block, so capture survives it.

- **A. A new event, `PointerCancel { x, y }`**, delivered to the capture
  holder like a release, activating nothing. `Event` stays 16 bytes,
  which `crates/lumen-static/tests/modifiers.rs` asserts.
- **B. The host resets capture and pressed state directly.** No new
  vocabulary, but a widget that keeps drag state of its own never learns
  the gesture ended.

**Recommended: A.** It is what a platform pointer-cancel is, and the
recording format then replays a cancelled drag as one. **What it
breaks:** every exhaustive `match` on `Event`, which the compiler lists,
and the agent's event names. The promotion itself lives in
`Host::pump`'s intake, not in `translate`, so an embedded touch panel
that feeds `Event::Touch` gets the same behaviour.

### 2.5 Overflow: merge and evict, not grow (T44.5)

A bigger queue moves the cliff, and the embedded budget pays for it
(`InputQueue` is `CAP * 16` bytes). Merging a `PointerMove` into a
trailing queued `PointerMove` removes the common cause. And when the
queue is full, a `PointerUp` or a key release evicts the newest queued
move instead of being refused, so capture cannot be stranded by
overflow. **Recommended**, as the re-triage said, with the agent's
`input.type` and `input.fill` chunking against `CAP` and pumping
between chunks, as `Host::paste` does, and reporting what arrived.

## 3. Tasks

### T44.0 — the gates first

Every finding becomes a check that is **red at `1717808`**, committed
marked expected-to-fail until its task lands, as M35/T35.0 did, with
its red output in the commit body. Where a finding cannot be red
(R18-F7, R3-F11) the commit says so and shows the injection instead.

1. `crates/lumen-static/tests/arrow_ownership.rs`. Through
   `pass::dispatch` and through `Host`, for an unsplit and a split
   application: `row { TextInput "abc", Button }`, End, **one** Left:
   focus unchanged and caret 2. `NumberInput` beside a `Button`: End,
   Left, Left, `9` gives `0.900`. `Slider` beside a `Button`: Right moves
   the value and keeps focus. `column { TextInput, TextArea, TextInput }`
   (R10-F3's shape): Left in the first field, and Up in the `TextArea`
   on line 0, keep focus. Each prints `(focus, caret)` beside the
   expected pair. **Red today** with focus on the sibling.
2. In `crates/lumen-static/tests/focus.rs`: a row of 20 `Button`s. Right
   ×20 visits 20 distinct members and returns to the start, and Right
   from member 18 moves to 19. Prints `visited N / 20`. **Red today:**
   16, and a dead key on member 18 (R3-F9).
3. `crates/lumen-static/tests/activation.rs` (R3-F3). Press `#save`,
   release on `#other`: nothing fires. Press row 1, release on row 7:
   nothing picked. Press row 5, remove row 4 through `App::apply`,
   relayout, release at the same point: nothing picked. A control press
   and release on one item activates exactly once. Each case asserts the
   press hit the intended node, and prints the latched
   `(node, row, generation)` beside the release's. **Red today** in all
   three cases.
4. `crates/lumen-static/tests/capture.rs` (R3-F4). `column { Either<Slider, Label> }`.
   The press captures, the switch hides the slider (its size is asserted
   0 as setup), and a move of +80 must leave the value unchanged and
   capture `None`. **Red today:** value 0.1 → 0.9.
5. `crates/lumen-static-shell/tests/touch_as_pointer.rs` (R15-F4), from
   the review's `r15_touch_only` fixture (probe `85ad947`): a `Button`,
   a `CheckBox` and a `Slider`, each tapped by touch Begin/End at its
   centre. Prints `touch actions handled: N / 3 (pointer: 3 / 3)`.
   **Red today:** 0 / 3.
6. `crates/lumen-static-shell/tests/input_overflow.rs` (R3-F2). Press a
   slider, post 200 moves and an Up without pumping, pump once: capture
   and pressed must be `None`, and a later unpressed move must not
   change the value. Prints queue length and dropped count beside `CAP`.
   And in `crates/lumen-static-agent/tests/input.rs`: `input.type` of
   100 characters leaves 100 in the field and the reply says 100;
   `input.fill` of 40 over 40 succeeds. **Red today:** capture stranded,
   and 64 of 100.
7. A write-counting test for R3-F12, in `crates/lumen-static/tests/hover.rs`:
   a test-only counter of `NodeState` writes per dispatched
   `PointerMove`, on a 1,685-node table, bounded at 4 and printed beside
   the node count. **Red today:** twice the node count.
8. In `crates/lumen-static/tests/events.rs`, beside `bind_row` (R3-F5):
   count 100, extent 20, scrolled so the ring has rotated (asserted as
   setup). The bind gives even and odd rows different handlers; ten
   clicks print `misrouted N of 10`. **Red today:** 5 of 10. The
   unrotated arm is green.
9. In `crates/lumen-static/tests/scroll_view.rs` (R3-F10): count 50
   million, extent 20. In debug, `#[should_panic]` naming the ceiling.
   In release, zero probe/paint disagreements at maximum scroll,
   printing the count and the worst offset beside 0. **Red today:** no
   panic, and 14 of 20 disagree.
10. `crates/lumen-static-shell/tests/cursor_rows.rs` (R15-F7), from probe
    `07eb009`: a table `Button` and a `Repeat` of rows holding a
    `Button`, plus a row holding a `Label`. Asserts the probe's hit
    (printed, with `inner.row`), then the cursor over each, printing
    `table=Pointer row=Pointer label-row=Default`. **Red today:** the
    row reads `Default`.
11. `crates/lumen-static-shell/tests/overlay_focus.rs` (R3-F7). A tooltip
    open (asserted as setup): Tab moves focus to the next control, the
    same as with no tooltip. A modal opened by Enter: Tab cycles inside
    it, and Escape returns focus to the trigger, printed beside the
    trigger's index. **Red today:** `None`, and the panel's button.
12. `crates/lumen-static/tests/overlay_stack.rs` (R18-F7). Two overlays
    with overlapping rects, opened A then B, with the overlap rect
    printed and asserted non-empty. Escape closes B only, and the
    consumed set is `{B}`. A press in the overlap reaches B's handler
    and not A's. `hit_test_overlaid` returns B at the overlap point.
    Prints `open [A, B]; after Escape [A]; overlap press -> B (A: 0, B: 1)`.
    **Green today, because the code is right.** Its injections are the
    two surviving mutants (Escape on `ov.at(0)`; the hit loop without
    `.rev()`) and the `.rev()` at `pass.rs:378`, each run in the commit
    and each red by name. **This closes R18-F7**, and it lands first on
    purpose: T44.9 edits the code these tests protect.
13. In `crates/lumen-static/tests/select.rs` (R3-F6): Tab, Enter (open,
    asserted), Down, Down, Enter picks row 1 and closes the menu; Tab
    while open keeps focus inside the menu. **Red today:** nothing
    picked, and focus `None`.
14. **R3-F11 has no red state.** Its gate is the reviewer's eye and
    T44.5's commit; this commit lists the four stale comments by
    `file:line` so T44.5 cannot miss one.

**Effort:** M. **Depends on:** M41 (every `Boundaries` leg), M42 (any
live drive leg).

### T44.1 — arrows belong to the widget that uses them; groups have no cap

**Closes:** R3-F1, R10-F3, R3-F9.

**Change.** Per §2.1 B: the provided `Node` method, answered `true` by
`TextInput`, `TextArea`, `NumberInput`, `Slider`, `RangeSlider`,
`SplitPane`, `PaneGrid` and `Pagination`. In `dispatch_at`
(`pass.rs:521-558`) the group branch is skipped when the focused node
answers `true`, and the key falls through to Pass 1. Replace
`heapless_vec` with a walk: from the focused index, scan the parent's
`subtree_range` forward (or backward) for the next focusable child of
the same parent, wrapping at the range's end. That is O(subtree), with
no allocation and no cap, and `heapless_vec` is deleted. Remove the
false sentences at `:524-526` and `:544-546`.

**Gate.** T44.0 items 1 and 2 go green. Item 1's split-application leg
runs under `Boundaries<256>`: the frame after the Left has the caret at
its new x, damage is non-empty, and the frame is byte-identical to
`Everything`'s. The same step is added to `handled_narrowing.rs`'s
session. `focus.rs`'s `arrows_move_within_a_group_and_wrap_inside_it`
and `an_arrow_in_a_lone_focusable_still_belongs_to_the_widget` stay
green.

**Injection.** (a) Put the group rule back ahead of the widget: item 1
fires with focus on the sibling after one Left, and with `0.009`. (b)
Build §2.1 A instead (the prototype's shape): the `TextArea` Up case
stays red. That is the evidence for B, and the commit records it. (c)
Build A with no `consumed.push` (the prototype exactly): the split
application's `Boundaries` leg fires on a stale caret while the
`pass::dispatch` leg is green, which shows the `Boundaries` leg is
needed. (d) Cap the new walk at 16 siblings: item 2 fires with 16 of
20. **Accepts good input:** the existing group-navigation test, a row
of three buttons, still wraps on Left and Right in the same session.

**Docs in the same commit.** `pass.rs`'s rustdoc for the rule and its
exemption, and for the walk (the "first sixteen" paragraph goes).
`widget.rs` rustdoc for the new method, cross-referencing
`dismissal_consumes_press` as the other hook answered by the node at an index rather than forwarded by a wrapper.
`docs/plan-capabilities.md` T15.6, which claims arrows keep editing
text. The book's `book/src/input-widgets.md` if it repeats that claim.

**Effort:** S. **Depends on:** T44.0.

### T44.2 — a click is a press and a release on the same item

**Closes:** R3-F3.

**Change.** Per §2.2 B. `InputState` gains the latched press target:
the hit's table node, its row, and the generation the hit's node
reports. It is set on `PointerDown` and cleared on every `PointerUp`,
including the capture path. On `PointerUp`, activation runs only when
the release's hit chain contains the latched node and, for a row, the
row and generation match. Otherwise the release clears the press shade,
returns `handled = false` and activates nothing. `Repeat` bumps its
generation (M41/T41.3's) in `layout` when its `count` changed since the
last frame. Keyboard activation (Enter, Space) is unchanged.

**Gate.** T44.0 item 3 goes green, with its setup assertions. A
`handled_narrowing.rs` step presses a button and releases it on another
(nothing activates, and the press shade clears under `Boundaries`,
byte-identical to `Everything`, which T25.0's pointer marks must
cover). `dismiss_click.rs` and `select.rs`'s
`a_press_on_a_row_reaches_the_application_with_its_index` stay green:
a controlled `Select`'s row press and release on one row must still
choose it and clear `open`.

**Injection.** Activate unconditionally on the release: cases (a) and
(c) fire. Latch `(node, row)` without the generation: only (c) fires,
which proves the generation is what catches it. **Accepts good input:**
the control click activates exactly once in the same session.

**Docs in the same commit.** `docs/events-decision.md` "Binding rules
going forward": the press/release rule, DECIDED. `EventCtx.row`'s
rustdoc in `event.rs` (a fresh index, with identity checked by the
latch). `pass.rs`'s activation comment. `book/src/events.md`.

**Effort:** M. **Depends on:** T44.0; M41/T41.3 (the generation it
reads).

### T44.3 — a hidden node loses capture

**Closes:** R3-F4.

**Change.** Factor C5's test (`!visible || size.w <= 0 || size.h <= 0`,
`pass.rs:484-487`) into one private function in `pass.rs`, and call it
at the top of the capture block (`:574`). When the captured node is
gone: clear capture, clear every pressed bit and `input.pressed`, clear
T44.2's latch, and fall through to ordinary dispatch, so the event is
not routed to the hidden node.

**Gate.** T44.0 item 4 goes green, printing the value before and after
beside the 0.1 start. `slider.rs`'s
`a_drag_keeps_tracking_after_the_pointer_leaves_the_control` stays
green: capture survives leaving a *visible* control. A
`handled_narrowing.rs` step hides a captured slider mid-drag and moves:
nothing is consumed, and the frame matches `Everything`.

**Injection.** Remove the new check: the value moves 0.1 → 0.9.
**Accepts good input:** the existing drag test, as above.

**Docs in the same commit.** `pass.rs`'s C5 comment (extended to
capture).

**Effort:** S. **Depends on:** T44.2 (it clears the latch T44.2 adds).

### T44.4 — the primary touch drives the pointer

**Closes:** R15-F4.

**Change.** Per §2.4 A. `Event` gains `PointerCancel`. In `Host::pump`'s
intake, before dispatch: the raw `Event::Touch` is dispatched first, as
today, so recognisers in `crates/lumen-static/src/gesture.rs` keep
their stream. Then the first touch id becomes the primary. Its `Begin`
synthesises a move and a left-button press at its point; its `Move`
synthesises a move; its `End` a release; its `Cancel` a
`PointerCancel`, which the pass delivers to the capture holder and
which releases capture, clears pressed state and T44.2's latch, and
activates nothing. Other touch ids never drive the pointer. On X11,
winit synthesises a `CursorMoved` for the first touch; the gate asserts
that this produces a move and no second press.

**Gate.** T44.0 item 5 goes green: `touch actions handled: 3 / 3
(pointer: 3 / 3)`, with the state change for each. Begin then Cancel on
the button: no press, capture released. A second concurrent touch id
does not click. A `handled_narrowing.rs` step taps a button by touch.

**Injection.** (a) Remove the promotion: 0 / 3. (b) Map `Cancel` to a
release: the cancel case fires with one press. (c) Promote every id: the
two-finger case fires twice. **Accepts good input:** the pointer arm of
the same fixture stays 3 / 3, and `crates/lumen-static/tests/gesture.rs`
still sees its raw `Touch` stream.

**Docs in the same commit.** `Event::Touch`'s and the new variant's
rustdoc. `book/src/gestures.md` (the primary touch also drives the
pointer). `docs/plan-capabilities.md` T15.5. `docs/mobile-decision.md`:
touch-as-pointer is done, not deferred. The agent protocol's event
names.

**Effort:** M. **Depends on:** T44.2, T44.3.

### T44.5 — the queue says what it dropped, and drops the right thing

**Closes:** R3-F2, R3-F11.

**Change.** Per §2.5. In `Host::post`: a `PointerMove` replaces a
trailing queued `PointerMove`; when the queue is full, a `PointerUp` or
key release evicts the newest queued move instead of being refused.
`InputQueue` gains the replace-last and evict-newest-move operations it
needs. `Host` publishes `InputQueue::dropped` through a new accessor,
`dropped_inputs`, and the agent reports it in `app.diagnostics`. The
agent's `input.type` and `input.fill` chunk against the queue's `CAP`,
pump between chunks, and report the count actually delivered. The four
stale comments of §1.4 are fixed in this commit.

**Gate.** T44.0 item 6 goes green, printing queue length and dropped
count beside `CAP`. A `handled_narrowing.rs` step types more than `CAP`
characters through the host in one frame.

**Injection.** Remove the eviction and the merge: capture stays
stranded, and the value moves on an unpressed move. Remove the agent's
chunking: 64 of 100. **Accepts good input:** 63 moves and an Up still
release, and a single move is delivered at its exact position.

**Docs in the same commit.** `InputQueue::push`'s rustdoc ("counted,
not silent", now true because something reads it). The agent's
`input.type` comment and the protocol's reply semantics for it.
`docs/plan-desktop.md`, whose paste-chunking rationale now covers the
agent too. `pass.rs:506-508`, `pass.rs:765-772`, `event.rs:970-972`.

**Effort:** M. **Depends on:** T44.0; M39 (the agent transport it
edits beside).

### T44.6 — an event writes the nodes it changed

**Closes:** R3-F12.

**Change.** Publish focus by clearing the previous index and setting the
new one, not by rewriting every node. `set_hovered` clears only the
previously hovered index, and a release clears only `input.pressed`.
Each previous index is bounds-checked, because a frame may have changed
the tree. The full sweep stays, as a single call, where the table is
re-seeded (a reload, or a structural change), which is the only case
`set_hovered`'s rustdoc gives for it.

**Gate.** T44.0 item 7 goes green: at most 4 writes per move, printed
beside the node count. `crates/lumen-static-shell/tests/invalidation_oracle.rs`'s
hover session stays green (no stale hover bit). The clock is a
**record, not a gate** (M35's rule 3): a bench arm at 2 and 1,685 nodes,
eight runs a side under the bench lock, recorded in a new section of
`docs/results-frame-cost.md`.

**Injection.** Restore the full sweep in `set_hovered`: the counter
reports twice the node count. Clear only the new index and forget the
old: the hover oracle fires with a stale bit. **Accepts good input:**
the oracle, as above.

**Docs in the same commit.** `pass.rs` rustdoc for `set_hovered` and
the focus publish; the "moves cost one call" comment can now include
the sweep. The results section.

**Effort:** M. **Depends on:** T44.5 (move merging is half the cost).

### T44.7 — the ring slot, and the ceiling enforced

**Closes:** R3-F5, R3-F10.

> **Fixed elsewhere since this plan was written (2026-09-25).** The
> R3-F5 half was fixed independently by M56's `af902b9` on
> `m56-showcase-design`, which was not merged when this plan was
> written. That commit maps `Handlers::handler` through `slot` and adds
> `crates/lumen-static/tests/repeat_handler.rs`: 40 rows whose handlers
> differ by parity, scrolled one row. Its recorded injection gave "10 of
> 11 rows answered by another row's handler". Once it is on `main`, this
> task's R3-F5 half is to check that the gate catches **this** finding's
> repro, T44.0 items 8 and 9 (the index's PARTIAL rule). It only extends
> the gate if it does not. The audit of the other window indexings below
> still stands.

**Change.** `repeat.rs:654`: `self.window.get(self.slot(k))`. Audit every
other place the window is indexed by position (the bind at `:424`
indexes by `row % len`, which is the same slot; `paint`, `hit` and the
observe walk use `slot`), and list the audit in the commit. For the
ceiling: in `Repeat`'s `layout`, a `debug_assert!` that `count` is at
most `max_addressable_rows()`, naming the ceiling, and in release the
effective count clamps to it, so every materialised row is reachable
and none overlaps. The rustdoc says "overlapping rows", and states the
clamp.

**Gate.** T44.0 items 8 and 9 go green: `misrouted 0 of 10` on the
rotated ring, and zero disagreements in release with the debug panic
named. Count 100 does not panic and disagrees nowhere.

**Injection.** Revert to `self.window.get(k)`: 5 of 10, while the
unrotated arm stays green, which shows the rotation is what the test
exercises. Remove the debug assertion: the `should_panic` test fails.
Remove the clamp: the release test reports 14 of 20.

**Docs in the same commit.** `Handlers::handler`'s rustdoc on `Repeat`
(the ring slot). `Repeat::MAX_ADDRESSABLE_EXTENT`'s rustdoc.
`docs/step3-repeaters.md` if it states the behaviour. The agent's
audit lint comment (`crates/lumen-static-agent/src/audit.rs`).

**Effort:** S. **Depends on:** T44.0 only. **The ring half may land
before Phase 3**: it is one line, it routes to a handler that already
exists, it changes nothing a `Boundaries` frame marks, and the review's
prototype showed it green both ways. T44.0 as a whole waits for M41
and M42, so an early ring fix carries its own gate, T44.0 item 8, which
is headless and needs neither; T44.0 then commits the other items.

### T44.8 — the cursor over a row

**Closes:** R15-F7.

**Change.** `Host` keeps the last hover `Hit`, which dispatch already
computes on every move, instead of re-probing inside `cursor`, which
the window calls per event. When the hit has an `inner` part, `cursor`
asks the hit's table node for the resolved cursor of that template
node, through a provided `Node` method that answers `None` by default
and that `Repeat` answers from its arena's style row. Otherwise it reads
the table style as today.

**Gate.** T44.0 item 10 goes green: `table=Pointer row=Pointer
label-row=Default`.

**Injection.** Ignore `inner`: `row=Default`. Return the `Repeat`'s own
cursor for any inner hit: the button row fails. Return `Pointer` for
every arena hit: the label row fails. **Accepts good input:** the table
button reads `Pointer` before and after.

**Docs in the same commit.** `Host::cursor`'s rustdoc.
`docs/plan-desktop.md` T14.6, which names this case: record that it is
now gated.

**Effort:** S. **Depends on:** T44.0.

### T44.9 — only a modal traps Tab, and focus comes back

**Closes:** R3-F7.

**Change.** Per §2.3. The provided `Node` method `is_modal`, `false` by
default. `Popup` gains a `modal` field and answers from it; `Veil`
answers `true`. The Tab branch (`pass.rs:312-320`) traps only for a
modal, never sets focus to `None`, dismisses a non-modal non-passive
overlay through the Escape path (pushing `Consumed` and calling
`tell_owner`), and passes through a passive one. `InputState` records
the focus held when an overlay first appears in the overlay list, and
Escape and dismissal restore it, falling back to the owner.
`overlay.rs`'s `focus_trap` fixture sets `modal = true`.

**Gate.** T44.0 item 11 goes green, printing focus beside the trigger's
index. T44.0 item 12 (`overlay_stack.rs`) stays green. `dismiss_click.rs`,
`select.rs` and `handled_narrowing.rs`'s two-select step stay green.
New `handled_narrowing.rs` step: Tab with a controlled `Select` open
closes it, the application's `open` is `false` afterwards, and the next
frame does not reopen it.

**Injection.** Make `Popup` answer `true`: the tooltip case fires with
`None`. Delete the restore: the modal case fires with the panel's
button. Skip `tell_owner` on the Tab dismissal: the controlled
`Select` reopens on the next frame, and the new step fires. **Accepts
good input:** `tab_cycles_inside_the_modal_and_never_leaves` stays
green, so the gate does not refuse trapping altogether.

**Docs in the same commit.** `pass.rs`'s Tab-trap comment (modal only).
`widget.rs` rustdoc for `is_modal`, cross-referencing
`dismissal_consumes_press`. `Popup`'s rustdoc for `modal`.
`book/src/overlays.md`. `docs/plan-widget-parity.md` T27.7 (`Veil`).

**Effort:** M. **Depends on:** T44.0 (item 12 first).

### T44.10 — rows a keyboard can choose

**Closes:** R3-F6.

**Change.** An active-row cursor, built once in `Repeat`, rather than
rows as Tab stops (which focus as a table index cannot express).

1. `Repeat` gains an active row, and answers `owns_arrows` (T44.1)
   `true` when it is keyboard-navigable. In its `on_event`, reached
   through Pass 1 as the focused node, Up, Down, Home, End, PageUp and
   PageDown move the active row, scroll it into view and mark it for an
   active-row highlight in `paint`.
2. Enter or Space on a focused node that reports an active row activates
   that row's handler: Pass 2 builds the `NodeSel` with the row's inner
   part and sets `ctx.row`, through a provided `Node` method,
   `active_row`, that answers `None` by default.
3. When an overlay opens from the keyboard, focus moves into it: to its
   list, with the active row set to the current selection. T44.9's
   restore returns it to the trigger on close.
4. `Chooser`, `Picker`, `Tree` and `DataGrid` adopt the cursor. `Tree`'s
   Left and Right fold and unfold.

**Gate.** T44.0 item 13 goes green: Tab, Enter, Down, Down, Enter picks
row 1 and closes the menu, and Tab while open stays inside it. A
`handled_narrowing.rs` step chooses a controlled `Select`'s row by
keyboard: the row's handler clears `open`, and the frame matches
`Everything`. Each of `Select`, `Chooser`, `Picker`, `Tree` and
`DataGrid` gets a keyboard-only action (open, move, choose) in
`scripts/widget_drive.sh`, driven under both strategies after M42, with
the chosen row printed beside the expected one.

**Injection.** Make `Repeat` ignore Down: nothing is picked. Make Enter
on the active row leave `ctx.row` unset: the test's row handler
panics by name on its `expect("a row index arrives from hit testing")`
(`crates/lumen-static/tests/select.rs:69`). **Accepts good input:**
`select.rs`'s pointer choose path stays green, and so does
`crates/lumen-static-widgets/tests/focus_overlay.rs`.

**Docs in the same commit.** `docs/plan-capabilities.md` T15.6
(keyboard scope). `docs/plan-widget-parity.md`'s action inventories,
with the new keyboard actions per widget. `Select` has no drive
inventory at 1717808 and M42 lists it as owed to this milestone
(M42 §4): this commit writes it, removes `Select` from M42's undriven
allow file, and adds the new actions to M42/T42.5's manifest.
`select.rs`'s module docs.
`book/src/input-widgets.md` and `book/src/overlays.md`.

**Also, assigned by the index (2026-09-25):** the `ScrollView` drive
inventory. M42 lists it as owed, and no milestone owned it. Its actions
are wheel, a drag on the bar, and the keyboard scroll T44.10 adds. Each
carries M42's three assertions, and M42's manifest is updated in the
same commit.

**Effort:** L. **Depends on:** T44.1, T44.9; M42 for the drive. **May be
split** into its own milestone if it outgrows one commit, keeping the
widget adapters (step 4) as separate tasks. It stays here because its
first three steps are pass and `Repeat` changes that T44.1 and T44.9
set up.

## 4. Boundaries of this plan

- **No item identity in `Repeat`** beyond the count generation of §2.2.
  The optional key function (§2.2 C) is deferred until an application
  needs a click to survive a reorder with the same count.
- **No marker on synthesised pointer events.** The re-triage suggested a
  flag so a widget can tell a touch from a mouse. No widget in the
  workspace needs to, and `Mods` is the wrong place for it. Add it with
  the first widget that does.
- **Not keyboard reach for widgets without rows.** T44.10 covers the
  five list-bearing widgets the review named. `just widget-drive`'s
  inventories after T44.10 are where a missing keyboard action would
  show up next.
- **Not a platform keyboard layer.** Word motion, multi-click selection
  and Home/End in text fields are M45's (R10-F14), and depend on T44.1.

## 5. Kill rules and risks

T44.6 is the only performance task, and it gates on a count, not a
clock (§3). **Its kill rule, written before it is built:** if bounding
the writes needs a re-seed hook that the reload path (M49's area) must
call and does not, the task stops at publishing focus by index and
records the rest, because a stale hover bit after a reload is worse than
the sweep.

**What would stop the milestone.** T44.1 is the keystone: T44.10 builds
on it, and M45's R10-F14 depends on it. If §2.1 B turns out to break a
widget that relies on the group rule to leave it, T44.1 falls back to A
plus an explicit "edge keeps the key" answer from the text widgets, and
records the case that forced it.

**Risks, and what would reveal each.**

- *A composite whose `node(i)` resolves to itself answers `false`.*
  Revealed by T44.0 item 1's `NumberInput` leg, which exists for this
  reason. A new composite that edits text needs a row there.
- *The latch cancels a legitimate click.* A widget that moves under the
  pointer between press and release (a pull-to-refresh, a list that
  scrolls on press) would lose its click. Revealed by `just
  widget-drive` after M42: every drive action asserts that its click
  landed.
- *Tab dismissing a menu fights the controlled `Select`.* Revealed by
  T44.9's reopen step in `handled_narrowing.rs`.
- *The narrowing leg passes because the frame did nothing.* Every
  `handled_narrowing.rs` step asserts that its action landed (§0 rule 4)
  before the oracle compares frames.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R3-F1 | S1 | OPEN | T44.1 |
| R10-F3 | S1 | OPEN | T44.1 |
| R3-F2 | S2 | OPEN | T44.5 |
| R3-F3 | S2 | OPEN | T44.2 |
| R3-F4 | S2 | OPEN | T44.3 |
| R3-F5 | S2 | OPEN | T44.7 |
| R3-F6 | S2 | PARTIAL | T44.10 |
| R3-F7 | S2 | OPEN | T44.9 |
| R15-F4 | S2 | OPEN | T44.4 |
| R3-F9 | S3 | OPEN | T44.1 |
| R3-F10 | S3 | OPEN | T44.7 |
| R3-F12 | S3 | OPEN | T44.6 |
| R15-F7 | S3 | OPEN | T44.8 |
| R18-F7 | S3 | OPEN | T44.0 |
| R3-F11 | S4 | OPEN | T44.5 |

## What building found (2026-09-26)

- **T44.0 lands in parts, not in one commit.** M45's T45.6 depends on
  T44.1's `owns_arrows`, and a second session was building M45 at the
  same time, so T44.0's items 1 and 2 (the arrow and group gates) landed
  first with T44.1 straight after them, and the other items landed as a
  second T44.0 commit. Every item is still committed red before its
  fix. Item 1's `Host` legs live in
  `crates/lumen-static-shell/tests/arrow_ownership_host.rs`, because
  core's tests cannot depend on the shell.
- **T44.1's injection (c) was green on the first gate.** A caret move
  repaints correctly with no mark at all, because `paint` reads the
  caret; and the host's stand-in measurer reports no clusters, so a
  caret is drawn at the run's end wherever it is. The `Host` leg now
  measures with clusters and walks the caret across the field's left
  edge, where only `layout` moves the scroll — and there the prototype
  without `consumed.push` fires at the nineteenth Left.
- **Focus is published one event late.** `dispatch_at` writes
  `NodeState::focused` before Tab or an arrow moves focus, so the frame
  after a focus move draws the ring on the node focus just left. T44.6,
  which rewrites the publish, fixes it.
- **T44.0 item 13's second half contradicted §2.3.** It asked that Tab
  while a `Select`'s menu is open "keeps focus inside the menu"; §2.3
  decides that Tab dismisses a non-modal, non-passive overlay (a menu)
  through the Escape path and moves on. §2.3 is the decision, so the
  gate asserts that: the menu closes, the dismissal reaches the
  application, and focus is not `None`. It is red until T44.9, not
  T44.10.
- **T44.0 item 7's counter is on `InputState`** (`state_writes`, a
  wrapping count that compares equal whatever it holds), because the
  core is `no_std` with no atomics guarantee on every target, and a
  sweep that rewrites a bit with its old value is invisible to any
  before/after diff. A sweep adds the table's length once.
- **Host's overlay list is private**, so item 11's setup asserts the
  tooltip's and the modal panel's area rather than the list.
- **T44.7 does not debug-assert the ceiling.** The plan asked for a
  `debug_assert!` in `layout` plus a release clamp. A repeater's count is
  the application's data, so a debug build would crash on a large file
  that a release build shows correctly — and the agent's `W1009`, which
  exists to report exactly this and runs in debug builds, could never be
  seen (its own test lays out a million-row list in debug). The clamp is
  in every build instead, and the gate runs in both: T44.0's debug
  `should_panic` test became `a_repeater_past_the_ceiling_disagrees_nowhere`.
- **T44.2 does not write `Repeat::generation`.** The plan said to bump
  M41's field in `layout` when the count changes. That field is the
  application's — `#[bind(ui.list.generation, version)]` writes it every
  frame — so a bump in `layout` would be overwritten by the next bind and
  re-marked the frame after. `Repeat` keeps a private count epoch instead
  and answers `row_generation` as `generation + epoch`, which is the same
  quantity the plan meant without two writers on one field. And the
  release rule is "the handler's node contains the press", as on the web,
  rather than "the release's chain contains the latched node": the two
  differ for a container whose handler spans two children, where the
  web's answer is the one users expect.
- **T44.10 is split, as §3 allows.** Its steps 1–3 (the active row in
  `Repeat`, key activation through `Node::active_row`, focus into a
  keyboard-opened overlay) landed with `Select` and `ScrollView`'s
  keyboard support as one commit; the drive inventories as a second;
  the adapters of step 4 as their own commits. `ScrollView` gained a
  **bar drag** there too: M42 listed "a drag on the bar" among its
  actions, and there was no such thing — `on_event` read only the wheel,
  and the bar was hit-tested as the content beneath it.
