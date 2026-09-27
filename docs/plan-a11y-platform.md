# M54 — a screen reader can see a turnkey application, and the platform claims say what was built

> **Written 2026-09-24, not started.** Closes R15-F1, R15-F2, R15-F3,
> R15-F5, R15-F8, R15-F9, R16-F8, R16-F9 and R12-F6 from M38's deep
> review (`docs/results-deep-review.md`), as re-triaged against `main`
> at 1717808 on 2026-09-24. Where the review and the re-triage differ,
> the re-triage is authoritative, because the code moved after the
> review was written.
>
> **Phase 5** of the fix queue. It comes after M43 (the local CI runner
> and the reworked static scripts), M48 (the agent's JSON carries
> `toggled`, numeric value and `expanded`, R15-F10), M40 (tree bounds
> are in physical pixels at any scale), M41 (a new event kind must mark
> correctly under `Boundaries`) and M44 (focus and keyboard reach). §3
> names the two tasks that can land earlier, and why.
>
> The handoff says one AccessKit tree "serves both screen readers and
> agents" (`docs/00-handoff-static-tree.md`, Agent observability and
> Step 5). **At 1717808 it serves agents only.** The one application a
> screen reader can see is `examples/counter-window` built with its own
> `a11y` feature, through a hand-written loop. Every application started
> through `run` or `Program` — the front door since M32 — is absent from
> the platform's accessibility bus. Inside the tree, only `Button` is a
> named control, and no action other than a centre click exists anywhere.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **A screen reader is a user, so it gets a live gate.** A unit test
   of the tree we *would* send proves the builder. Only a walk of the
   platform's accessibility bus proves that the window sent it. Both are
   required (T54.2).
2. **The tree is checked per widget, from the snapshot.** Roles, names,
   states and actions are read off the emitted `accesskit::Node`, never
   off the widget type. A check that reads the type cannot see a widget
   that forgot to publish (R15-F3's injection is exactly that).
3. **Every action a node publishes must be handled.** A node that
   announces `Increment` and ignores it is worse than one that announces
   nothing: the screen reader says the control can be adjusted, and it
   cannot. The table gate invokes every published action.
4. **A capability is DONE when an application can reach it.** R15-F5 is
   three capabilities marked DONE with zero callers. This milestone
   un-marks them rather than leaving the claim to stand (§2.5).
5. **Contrast is measured on what is painted.** The display list, after
   layout, with the real palette. Hand-written (text role, ground role)
   pairs are the "bound computed from the wrong quantity" shape: they
   check pairs nobody paints and miss pairs somebody does.

## 1. What is wrong, at 1717808

### 1.1 No turnkey application reaches a screen reader (R15-F1)

`accesskit` is optional in `crates/lumen-static/Cargo.toml` and is
enabled only by `observe` (`:35`), the dev surface that also stamps
`NodeDesc.src` and so grows `NodeDesc` from 48 B to 72 B
(`crates/lumen-static/src/desc.rs:27`). A release application therefore
cannot have a screen-reader tree without paying for the agent's
feature. `accesskit_winit` appears only in
`examples/counter-window/Cargo.toml:24`. `crates/lumen-static-winit`
has no `a11y` feature, yet its manifest comment at `:35` says
"Clipboard is default-ON, unlike the a11y feature", about a feature it
does not have. `Win::resumed_impl` (`crates/lumen-static-winit/src/lib.rs:669`)
creates the window visible, with no adapter. M32's `Program`
(`crates/lumen-static-shell/src/program.rs:152`) has no accessibility
setting, so the gap now covers the recommended path as well. The
crate's module doc still points a11y users at counter-window's
hand-rolled loop (`lib.rs:14-15`). The live evidence is the review's
AT-SPI walk (R15-F1; `docs/results-deep-review.md`).

### 1.2 The tree names one control, and nothing is actionable (R15-F2, R15-F3)

At 1717808, `set_label` has two real call sites
(`crates/lumen-static-widgets/src/ops.rs:473` for `Button`, `split.rs:576`
for `Collapsible`) and `add_action` has two (`ops.rs:474`,
`toggle.rs:275`). `set_selected`, `set_disabled`, `set_description` and
`labelled_by` have none. `Toggle` publishes a role and `toggled` but no
name (`toggle.rs:267-275`). `Slider` and `TextInput` publish no name
(`slider.rs:243`, `input.rs:687`). `select.rs`, `image.rs`, the
progress indicator, toast, card, tooltip, popup, glass and scroll have
no `observe` at all, so they report `GenericContainer`. The agent's
`input.invokeAction` is literally `input.click`
(`crates/lumen-static-agent/src/lib.rs:478`), and counter-window's
adapter turns every `ActionRequested` into a click at the node's centre
(`examples/counter-window/src/window.rs:171-181`).

**Nothing would notice any of it getting worse** (R15-F3). The review's
injection deleted `Toggle::observe`'s `set_role` and the whole suite
stayed green. The one test added since, T35.1's
`crates/lumen-static-widgets/tests/observe_walk.rs`, compares two walks
that both call the same `observe`, so it proves the walks agree and
says nothing about what either reports.

### 1.3 Colour is never checked where it is painted (R16-F8, R16-F9)

`nav.rs:306` chooses a selected strip label's colour
(`let col = if on && KIND != TABS { s.bg } else { s.fg };`), and its
guard test checks `type_defaults`, not the painted command. The review
made the label the pill's own colour and nothing went red, unit or live
(R16-F8). The paint-level contrast checks that exist are
`scripts/ui_check.py` (counter-window's buttons and labels only), one
pagination geometry check in `crates/lumen-static-widgets/tests/data.rs`,
and, new since the review, T36.2's `contrast` helper in
`crates/lumen-static-shell/tests/failure_screen.rs:59-70`, which covers
the panic screen and nothing else.

The showcase's accent `0x2f6bff` (`examples/showcase/src/main.rs:67`)
sits below WCAG AA on three grounds (R16-F9;
`docs/results-deep-review.md`). **The review's suggested replacement
does not fix it**: the re-triage recomputed `#2a62f0` at 4.44:1 on the
Ghost button's ground (`GHOST`, `main.rs:96`), still under 4.5, and
found `#265ce6` to be the first value that passes all three grounds.
**The framework's own palette has the same defect** (re-triage, new at
1717808): `Palette::LIGHT`'s accent (53, 99, 239) is 4.07:1 on its own
`surface` (226, 232, 240) (`crates/lumen-static/src/style.rs:505-514`).
This plan's own computation over `Palette::LIGHT` and `Palette::DARK`
found two more role pairs under 4.5 — LIGHT `muted` on `surface` and
DARK `accent` on `surface` — but whether any widget *paints* those pairs
is unknown, which is rule 5's point. T54.0's harness decides it.

### 1.4 Three platform capabilities are DONE and were never assembled (R15-F5)

`docs/plan-capabilities.md` marks T15.7 (drag and drop, `:504`), T15.10
(multi-window) and T15.11 (platform dialogs) DONE 2026-09-07. At
1717808, `translate::dropped` and `Dropped` have no caller outside
`translate.rs`. `Panes` and `Dialogs` are only re-exported
(`crates/lumen-static-winit/src/lib.rs:124`, `:127`), with no user in
any crate, example or fixture. `crates/lumen-static-winit/tests` holds
`gpu_regions.rs` and `translate.rs` and nothing that opens two windows,
drops a file or shows a dialog. `window_event_impl` sends every event
through `translate::window_event` (`lib.rs:764`), which has no arm for
`DroppedFile` or `HoveredFile`, so **a `dnd` build of `run` discards
drops**. `Dropped::Files` wraps a `Vec` that always holds exactly one
path (`translate.rs:221`, `:237`), because winit emits one
`DroppedFile` per file. `book/src/platform.md` presents multi-window and
dialogs as available.

### 1.5 Stated sizes and features that do not exist (R15-F8, R15-F9, R12-F6)

- `crates/lumen-static/src/drag.rs:18-20` and `:44-45` say `Drag<()>`
  "is one byte". It is 24 B (R15-F8; `docs/results-deep-review.md`).
  The test named for the claim,
  `a_tree_that_drags_nothing_carries_one_byte`
  (`crates/lumen-static/tests/drag.rs:88`), asserts
  `size_of::<Option<()>>() == 1`, a language fact. Its relative
  assertion does guard the mechanism its comment describes (no
  `Box<dyn Any>` payload), which is why the re-triage lowered it to S3:
  the number is wrong, the mechanism is guarded.
- `crates/lumen-static-widgets/src/image.rs:10-12` says decoding "lives
  behind" the shell's default-off `image` feature. The shell has no such
  feature (R15-F9).
- `docs/mobile-decision.md:27-31` says a bare `cargo check` of the core
  for Android "cannot complete". `cargo check` does not link. The
  re-triage ran it at 1717808: core, widgets and shell check cleanly for
  `aarch64-linux-android`; the windowing crate fails only on its
  default-on `clipboard` (arboard has no Android backend), and checks
  with `--no-default-features --features winit/android-native-activity`,
  including with the `reload`, `hot` and `agent` features M36 and M37
  added (R12-F6).

## 2. Decisions

### 2.1 Where the screen-reader tree comes from: an `a11y` feature split from `observe`

- **(a) Turn `observe` on in release.** It works today. It costs every
  application 24 B per `NodeDesc`, `serde_json`, and the agent's audit
  hooks, for a tree that needs none of them.
- **(b) Correct the handoff to "agent tree only".** Cheap and honest. It
  leaves the stated goal false and every `Program` application
  invisible to assistive technology.
- **(c) Split `a11y` out of `observe`. Recommended.** In `lumen-static`,
  `a11y = ["std", "dep:accesskit"]` gates `Node::observe`,
  `Node::observe_walk`, `observe::emit_root`, `observe::snapshot` and
  `observe::index_of_node`. `observe = ["a11y", "dep:serde",
  "dep:serde_json"]` keeps the JSON dump, `probe`, `stats` and the
  `NodeDesc.src` stamp. The same split in `lumen-static-widgets`
  (`a11y = ["dep:accesskit", "lumen-static/a11y"]`), and
  `__observe_items!` becomes two macros so the derive's generated walk
  follows `a11y` and its `src` stamp follows `observe`.

**What (c) breaks.** About fifty `cfg(feature = "observe")` sites move
to `a11y`, and each one is a place M30's feature-seam defect can come
back: a build that turns on `lumen-static/a11y` but forgets
`lumen-static-widgets/a11y` reports every widget as `GenericContainer`,
and no unit test in the core would notice. T54.1's gate builds the
widgets crate with `a11y` alone and reads a role back. The
`just observe-inlining` symbol gate (M35) must also run under `a11y`
alone, because `observe_walk` moves with it.

### 2.2 The adapter: `accesskit_winit` in `lumen-static-winit`, reached through `Program`

`accesskit_winit` already covers AT-SPI, UI Automation and
NSAccessibility, and counter-window proves the pattern with 0.34. A
hand-written platform bridge would be three bridges. So:
`Win::resumed_impl` creates the window invisible, builds the adapter
with the loop's proxy, then shows the window (the order `accesskit_winit`
requires). `UserEvent` gains an accessibility variant. On
`InitialTreeRequested` the session sends a full snapshot. After a frame
the session reports as changed, it sends a full `TreeUpdate`;
incremental updates from the journal stay deferred, as the handoff
already records (§4). On `ActionRequested` it routes through §2.4.
The `Session` trait gains the two calls under the feature, so the hot
session (M37) carries them too. `Program` gains `.accessible(bool)`,
defaulting to the feature's value, so an application can turn the
adapter off at run time without a rebuild. Counter-window's hand-written
adapter is deleted and the example uses the shared one, so there is one
implementation.

### 2.3 Does `a11y` default on for desktop? **Yes, subject to a kill rule written now.**

The manifest's own argument for `clipboard` is "a text field without
copy and paste is not a working text field". The same argument is
stronger here: an application its user cannot perceive is not a working
application, and a screen-reader user cannot turn a Cargo feature on.
The cost the manifest records is 62 crates, against `clipboard`'s 11,
measured 2026-09-05 (`crates/lumen-static-winit/Cargo.toml:35-40`). The
runtime cost should be nothing until an assistive technology connects,
because the adapter asks for the first tree only then.

**Kill rule, written before T54.2 is built.** `a11y` ships default-on in
`lumen-static-winit` unless T54.2 measures **either** of these:

1. the showcase's idle frame with no assistive technology attached moves
   outside the eight-run spread of the same build without the feature, or
2. the stripped release showcase binary grows by more than 3 MB.

If either fires, the feature ships default-off, the manifest comment
records the number that decided it, and the handoff says in the same
commit that a turnkey application is accessible only when it asks. The
dependency cost to `just test` (every example that links the windowing
crate builds a D-Bus stack) is recorded, not gated: it is the price the
decision pays on purpose.

### 2.4 Actions: an event, delivered like every other event — so the hook is **provided**

The re-triage proposed a new `Node` method, `a11y_action`, provided with
a default of `false`, and said wrappers such as `Glass`, `Popup`, `Pad`
and `Repeat` must forward it. `AGENT.md` warns that a provided method is
right for a leaf and wrong for a wrapper. The question is whether a
wrapper is ever on this path. **At 1717808 it is not.** Dispatch
resolves the target by table index through `node_mut`, and a wrapper's
`node_mut` hands out its content for every index but its own
(`crates/lumen-static-widgets/src/glass.rs:296-301`). `on_event`,
`compose` and `dismissal_consumes_press` are delivered this way
(`crates/lumen-static/src/pass.rs:739`, `crates/lumen-static-shell/src/lib.rs:1362`),
which is why `dismissal_consumes_press`'s rustdoc calls itself "a leaf's
answer and never a wrapper's to forward". An action addressed to a
slider inside a `Glass` reaches the slider without `Glass` being asked.
The re-triage's injection — "make `Glass`'s `a11y_action` provided,
not forwarding" — cannot be built, because `Glass` would never receive
the call.

So the choice is between a new method and no new method:

- **(a) A required `Node` method.** Every hand-written impl and the
  derive write it, and nearly all of them write `false`. It buys a
  compiler-produced list of impls, but the list contains no wrapper that
  needs anything.
- **(b) A provided `Node` method, default `false`.** Correct, for the
  reason above. It adds a second per-node entry point that has to agree
  with `on_event` about capture, focus and marking.
- **(c) An `Event::Action` variant, handled in `on_event`.
  Recommended.** `on_event` is already provided, already
  index-delivered, already bubbles deepest-first so a `Default` action on
  a label inside a button reaches the button, and already runs before the
  application's activation handler, so a `Default` action activates
  exactly as a click does without synthesising pointer coordinates. The
  action enum lives in core, `no_std`, with no `accesskit` type in it:
  `Default`, `Increment`, `Decrement`, `SetValue(f32)`, `Expand`,
  `Collapse`, `Focus`. The value is `f32`, not accesskit's `f64`, for
  two reasons: `Event` is 16 B and `crates/lumen-static/tests/modifiers.rs`
  asserts it (M44 §2.4 keeps it there), and M52/T52.2 bounds soft-`f64`
  symbols in the interactive firmware at 0, which an `f64` in an `Event`
  arm would break. The adapter maps `accesskit::Action` onto it and
  narrows the value, on the desktop side. A new `Host::action(node,
  action)` starts dispatch at `node`, the same loop a key uses.

**The answer to "required or provided" is provided**, in either (b) or
(c), and the reason is the delivery path, not the default's value: a
wrapper is never the receiver, so the default can never act on a wrapper
in place of what it wraps. The risk that remains is a different one — a
node that *publishes* an action it does not handle — and rule 3's gate
exists for it.

**Arena rows.** A `Repeat` row is not in the table, and `Repeat`'s
`on_event` handles only `Scroll` (`crates/lumen-static/src/repeat.rs:583`).
An action on a row's node resolves to the repeater's index plus the row,
exactly as a click does, and only `Default` means anything there: it
reaches the application's row handler. Row widgets hold no per-row state
to increment. §4 records this.

**What (c) breaks.** Every exhaustive `match` on `Event` gains an arm,
and the compiler lists them. T54.4's gate asserts `Event` is still
16 B. Under `Boundaries`, a consumed action must
mark like any consumed event; M41's oracle must see an action frame
(T54.4's gate drives one through it).

### 2.5 R15-F5: **un-mark all three**, and fix the one type that misleads

Either assembling or un-marking is honest. This plan un-marks, for three
reasons. **No application in the workspace asks for any of them**, and
M27's lesson is that assembly is driven by using the thing: a capability
built ahead of its caller is how T15.10 came to be DONE without ever
opening a second window. **Their gates cannot run where M43's runner
runs**: a real drop needs a platform drag source, two windows need a
display, and `rfd` has no mock backend, so the re-triage itself proposed
keeping T15.11 "not gated". And **this milestone's large item is the
accessibility tree**, which the handoff claims as a goal. Spending it
on three unrequested capabilities would leave that claim false for
longer.

Un-marking is not the same as leaving things alone. `Dropped::Files`
becomes `Dropped::File(PathBuf)`, one per event, as winit delivers it,
so an application that calls `translate::dropped` itself is not told a
`Vec` might hold several. The `dnd` feature's manifest comment and
`Dropped`'s rustdoc say that `run` and `Program` do not route drops.
§4 names what would reopen each one.

### 2.6 R16-F9: change the accent values; do not add a role

The re-triage recommended an `accent_text` palette role, so fills can
stay vivid while text passes. Rejected: `Palette` is a public struct
that applications build as literals (the showcase builds two,
`main.rs:59` and `:72`), so a new field breaks every one of them, and it
doubles the choices a stylesheet author has for one colour. Instead, the
accent value changes wherever T54.0's harness finds accent-coloured
text painted below 4.5:1: the showcase's to `#265ce6` (the re-triage's
search), `Palette::LIGHT`'s to the first value the same search finds
passing every ground the harness reports, printed with its minimum
ratio. Any other role pair the harness reports gets the same treatment.
**What it breaks:** golden images whose pixels include the accent are
re-blessed in the same commit, with the diff count in the body.

## 3. Tasks

Two tasks can land before Phase 5. **T54.7** is documentation plus a
type rename with no dependency, and a DONE mark that is false misleads
every plan written against it in the meantime. **T54.9**'s two doc
fixes can land at any time; its static rule waits for M43, which is
rewriting `scripts/check_doc_identifiers.sh` (R17-F15).

### T54.0 — the gates first

Each finding becomes a check that is red at 1717808, committed with its
red state in the commit body, and marked expected-to-fail until its task
lands (the T35.0 and T36.0 precedent).

1. **The role table**, `crates/lumen-static-widgets/tests/a11y_roles.rs`,
   under `observe` until T54.1 creates `a11y`. One row per `Node` impl
   in the widgets crate, built in a `Host`, snapshotted, and checked for
   role, name presence, state fields and published actions. It prints
   one line per row (`Slider role=Slider label=None actions=[]`), then
   `rows checked: n / Node impls: m` against a count of impls taken from
   the source, so a new widget without a row fails. Toggles are built
   in both states, so the `toggled` column reads live state. **Red
   today** on every unnamed control, on the `GenericContainer` count,
   and on the sliders' empty action set.
2. **The AT-SPI gate**, `just a11y-drive` (`scripts/a11y_drive.py`,
   Linux, `pyatspi`, launched through M42's launcher; with no AT-SPI
   bus it prints the reason and exits 77, which M43's `just live`
   counts as not run, never as a pass, M43 §2.2). It
   launches showcase, waits for its window, walks the desktop and
   asserts that an application with the showcase's title is present and
   that its push-button count equals the agent's `Button` count, both
   printed. **Red today** ("showcase not in AT-SPI app list"). *Good
   input:* counter-window built with `--features a11y` passes the same
   walk today, which proves the walker sees what is there.
3. **The contrast harness**, `crates/lumen-static-widgets/tests/text_contrast.rs`
   (§2.6, T54.5). **Green at 1717808 against 3:1**, because the defect
   R16-F8 describes is a missing gate, not a painted defect. It goes red
   under the review's injection `fd31a02` (the selected rail label drawn
   in the pill's colour: ratio 1.0), and that red run is what this
   commit records.
4. **The palette bound**, the same harness at 4.5:1 over the showcase's
   `SHOW_LIGHT` and the framework's `Palette::LIGHT`. **Red today**: the
   showcase's accent runs and `Palette::LIGHT`'s accent on `surface`.
   The count of runs below the bound and the total measured are both
   printed, so an empty measurement cannot pass.
5. **The drag size claim as a test**: assert the rustdoc's number, one
   byte. **Red today** at 24.
6. **The mobile matrix**, `just mobile-check`: `cargo check` for
   `aarch64-linux-android` of core, widgets and shell, and of the
   windowing crate with `--no-default-features --features
   winit/android-native-activity`, one line per crate with its exit
   status. Manual, like the GPU recipes. **Green at 1717808, and that is
   the finding**: the doc says it cannot complete.

The feature-name rule for R15-F9 is not in this commit: it needs M43's
rewrite of the identifier check first (T54.9).

**Effort:** M. **Depends on:** M48 (R15-F10) for the widget-drive role
column only; the unit table does not need it.

### T54.1 — split `a11y` out of `observe`

**Closes:** none by itself; it is R15-F1's prerequisite (§2.1).

**Change.** `crates/lumen-static/Cargo.toml` and
`crates/lumen-static-widgets/Cargo.toml` gain `a11y`; `observe` implies
it. The `cfg` sites in `widget.rs`, `observe.rs`, `ops.rs`, `hatch.rs`,
`repeat.rs` and the widgets move per §2.1, with `NodeDesc.src` staying
behind `observe`. `__observe_items!` is split so the derive's walk
follows `a11y`. `crates/lumen-static-agent/Cargo.toml` keeps enabling
`observe`, which now implies `a11y`. M48/T48.9's compile-time seam
assertion (the widgets crate refuses to build when the core has
`observe` and it does not) is extended to `a11y` in the same commit, so
this split does not reopen the seam M48 closed.

**Gate.**
- A size test in `crates/lumen-static/tests/desc_size.rs`: under `a11y`
  without `observe`, `size_of::<NodeDesc>()` is 48; under `observe`, 72.
  It prints `NodeDesc = n B under <features> (bound 48)`. No test
  asserts either number today; `desc.rs:27` states them.
- The role table from T54.0, run under `lumen-static-widgets/a11y`
  alone, reads `CheckBox` back from a toggle.
- `just observe-inlining` runs under `a11y` alone as well as `observe`.

**Injection.** Move the `src` stamp under `a11y`: the size gate fails at
72 against 48. Drop `lumen-static/a11y` from the widgets crate's `a11y`
feature list: the table reads `GenericContainer` for the toggle, and the
build does not fail — which is the M30 seam this gate exists for.
*Good input:* the `observe` build still reports 72 and passes, so the
test measures the feature set it names.

**Docs in the same commit.** `desc.rs:27` (the size under each feature,
citing the test); both manifests' feature comments;
`crates/lumen-static-agent/Cargo.toml:10-25` (the observe unification
comment); `docs/step5-observability.md` (the gating paragraph).

**Effort:** M. **Depends on:** T54.0.

### T54.2 — the adapter in the window, through `Program`

**Closes:** R15-F1.

**Change.** §2.2 and §2.3. `crates/lumen-static-winit/Cargo.toml` gains
`a11y = ["dep:accesskit_winit", "dep:accesskit",
"lumen-static-shell/a11y"]`; the shell's `Session` gains the tree and
action calls under it; `Win::resumed_impl` builds the adapter before
showing the window; `UserEvent` gains the variant; `Program` gains
`.accessible`. Counter-window drops its own adapter. Measure §2.3's kill
rule and decide the default in this commit.

**Gate.**
- `just a11y-drive` from T54.0 goes green on showcase, a `Program`
  application: application present, push-button count equal to the
  agent's (both printed), and the button named `Primary` has the push
  button role.
- `crates/lumen-static-winit/tests/a11y_tree.rs`: the `TreeUpdate` the
  session would send for a small `Program` has as many nodes as the
  table has visible nodes (both printed), and at scale 2.0 a button's
  bounds are twice its logical bounds (printed). The second assertion is
  why this task waits for M40: tree bounds are in the platform's
  physical pixels.
- The kill-rule measurement, eight runs a side, printed with both
  spreads and the binary delta.

**Injection.** Remove the adapter construction from `resumed_impl` with
the feature on: the AT-SPI gate fails with the application missing from
the list (its length printed). Send a root-only tree on
`InitialTreeRequested`: the button count fails at 0 against n. Pass
logical bounds unscaled: the scale-2 assertion fails. *Good input:*
counter-window, now on the shared adapter, still passes the AT-SPI walk.

**Docs in the same commit.** Handoff Agent observability and Step 5
paragraphs: which feature gives the screen-reader tree, and the adapter
flipped from implied to DECIDED with a pointer to this task.
`docs/step5-observability.md:95` ("Platform adapter wiring") moves to
done. The `lumen-static-winit` manifest comment at `:35` becomes true;
`lib.rs:14-15` stops pointing at counter-window. `docs/plan-desktop.md`
T14.0. The book: `component-boundary.md:29` already names screen
readers; `agent.md` or `backends.md` gains `.accessible`, included by
anchor from an example.

**Effort:** L. **Depends on:** T54.1, M40, M43 (the runner the live gate
is registered with).

### T54.3 — names, roles and states for every widget that means something

**Closes:** R15-F3.

**Change.** A `label` field, default empty and published with
`set_label` when non-empty, on `Toggle`, `Slider`, `RangeSlider`,
`TextInput`, `NumberInput`, `SearchField`, and the chooser and picker
triggers. A field rather than a `labelled_by` relation to a sibling
`Label`: it matches `Button` and `Collapsible`, which already have one,
and it works inside a `Repeat` row, where a sibling relation would need
arena ids. `observe` impls for `Select` (combo box, expanded, value =
selected option's text), the progress indicator (numeric value, min,
max), `Toast` (status, polite live region), `Image` (image, with a new
`alt` field as its name), `Tooltip` (described-by on its anchor) and
`Card` (group, when titled). `Fixed`, `Spacer` and `Pad` keep
`GenericContainer`, set explicitly so the choice is visible.

**Gate.** T54.0's role table goes green. It prints
`GenericContainer kinds: k (bound b)`, with `b` the explicit list above,
and every control kind in its "labelled" list must have a non-empty name
when built with one. With M48's JSON fields, the widget-drive action
inventories (`scripts/drive/widgets/`) gain a role column that asserts
`ui.getTree`'s role for the driven node.

**Injection.** The review's `d85e851` (delete `Toggle::observe`'s
`set_role`): the CheckBox, Switch and Radio rows fail by name. Drop
`set_toggled`: the toggled column fails in the on-state row. Remove
`Select`'s new `observe`: the `GenericContainer` count exceeds its
bound. *Good input:* a control built with an empty label is reported
unnamed rather than passing, which shows the name check reads the node.

**Docs in the same commit.** `toggle.rs:272-274` cites the test.
`docs/plan-widgets.md` M11 gate; `docs/plan-widget-parity.md`'s
inventory format (the role column). The book's agent chapter gains a
roles table, generated from or checked against the test, never retyped.

**Effort:** L. **Depends on:** T54.1; M48 (R15-F10) for the widget-drive
column.

### T54.4 — actions

**Closes:** R15-F2.

**Change.** §2.4 (c). The core action enum and `Event::Action`;
`Host::action`; `Increment`, `Decrement` and SetValue on `Slider`,
`RangeSlider` and `NumberInput`; `Expand` and `Collapse` on the
choosers, pickers and `Collapsible`; `Focus` on the inputs, through
M44's focus path; `set_disabled` where a widget has a disabled state.
Each widget publishes exactly the actions its `on_event` handles. The
adapter maps `ActionRequested` onto `Host::action`. The agent's
`input.invokeAction` takes an `action` parameter and calls the same
`Host::action` instead of clicking.

**Gate.**
- The role table invokes every published action on every row and
  asserts `handled`. It prints `published: p, handled: h` per row, with
  the bound `h == p`.
- `crates/lumen-static-agent/tests/actions.rs`: `invokeAction`
  `increment` on a slider moves its value by exactly one step (before
  and after printed) and answers `handled: true`. The same on a slider
  inside a `Glass`, which proves index delivery reaches past a wrapper.
- One action frame is driven through M41's shadow oracle, which runs in
  a `Boundaries` build and judges it against an `Everything` layout of
  the same frame (M41/T41.2); the oracle is not compiled into an
  `Everything` build (M42/T42.4 refuses that leg).
- `modifiers.rs`'s 16 B assertion on `Event` stays green with the new
  variant, and M52/T52.2's soft-`f64` count on the interactive firmware
  stays 0.

**Injection.** Publish `Increment` on `Slider` and remove its
`on_event` arm: the row fails at `handled: 0 / published: 3`. Make
`Host::action` deliver to the table's root instead of resolving the
target: the `Glass` case answers `handled: false` and the value does not
move. *Good input:* a `Default` action on a button's label bubbles to
the button and runs its handler once (count printed), exactly as a
click does.

**Docs in the same commit.** Handoff Step 5 ("Deferred: action
synthesis" becomes done, with a pointer). `Event`'s rustdoc for the new
variant. The agent protocol's rustdoc for `invokeAction`. The book's
events chapter.

**Effort:** M. **Depends on:** T54.3, M41, M44, M39 (the agent's
transport, which `invokeAction` travels on).

### T54.5 — painted contrast, measured

**Closes:** R16-F8.

**Change.** A shared `text_contrast` function in `lumen-static-shell`
(headless and `std`, so widget, example and agent tests can all reach
it): for each text command in a `DisplayList`, find the topmost opaque
fill preceding it whose rectangle contains the run's centre, respecting
clips, and compute the WCAG ratio. Translucent grounds are counted as
"unmeasured" and printed, never silently skipped. T36.2's `contrast`
helper moves into it, so there is one WCAG implementation. The unit
harness runs every widget in the catalogue, selected and unselected,
under `Palette::LIGHT` and `Palette::DARK`, against 3:1 (the non-text
minimum; §2.6 owns 4.5). `just widget-drive` runs it after every action
on the driven widget's subtree.

**Gate.** `text_contrast.rs` prints
`min contrast: r at <widget>/<state> (bound 3.0), measured: n, unmeasured: u`.
widget-drive prints the same per action.

**Injection.** The review's `fd31a02` (the rail's selected label in the
pill's colour): fails at 1.0 on the navigation rail and the bottom bar.
Paint `Pagination`'s current digit outside its pill (the T27.10 defect):
fails, because the digit is then measured against the page. *Good
input:* the unmodified catalogue passes with a measured count above
zero, so "no text found" cannot pass.

**Docs in the same commit.** `nav.rs:300-305` (the guard is now the
painted colour, not the defaults). `docs/plan-widget-parity.md`: a
fourth per-action assertion. The rail-label row of `AGENT.md`'s
vacuous-test table now has a gate — the commit body says so; `AGENT.md`
is edited only if the project records closures there.

**Effort:** M. **Depends on:** T54.0, M42 (widget-drive reports what it
drove).

### T54.6 — palettes that pass

**Closes:** R16-F9.

**Change.** §2.6. The showcase's `SHOW_LIGHT` accent becomes `#265ce6`
(and its `focus`, which shares the value, is checked at 3:1 as a
non-text colour). `Palette::LIGHT` and any other role the harness
reports below 4.5 change value. `SHOW_DARK` and `Palette::DARK` are
measured the same way and changed only if the harness reports them.

**Gate.** T54.0's palette bound goes green: `runs below 4.5: 0 / N`,
with `N` printed. A headless test in the showcase binary's test module
(`examples/showcase/src/main.rs`, beside its existing tests) runs the
harness over the whole catalogue with the real stylesheet.

**Injection.** Restore `0x2f6bff`: the showcase test fails with the
count of runs below the bound printed. Restore `Palette::LIGHT`'s
(53, 99, 239): the framework row fails on its `surface`. *Good input:*
the fixed palettes pass with `N` printed beside the catalogue's count of
text commands, and the test fails if `N` is under half of it, so a
harness that measured nothing cannot pass.

**Docs in the same commit.** `docs/results-appearance.md` owns the
palette values and their ratios, stated once there. `Palette`'s
rustdoc cites it. The book's styling chapter cites the results doc and
restates no ratio. Re-blessed goldens are listed in the commit body with
their pixel counts.

**Effort:** S. **Depends on:** T54.5.

### T54.7 — un-mark T15.7's platform half, T15.10 and T15.11

**Closes:** R15-F5.

**Change.** §2.5. `docs/plan-capabilities.md`'s three status lines
become "types shipped, never assembled" with the date and a pointer to
this plan. `Dropped::Files` becomes one path per event. The rustdoc of
`translate::dropped`, `Dropped`, `Panes` and `Dialogs` says that no
turnkey path calls them. The `dnd` feature's manifest comment says
`run` and `Program` do not route drops.

**Gate.** `crates/lumen-static-winit/tests/translate.rs` gains a case:
a `DroppedFile` event yields exactly one `Dropped::File` carrying its
path (printed), and a `CursorMoved` yields none. The un-marking itself
has no behavioural gate; the honest record is the doc, and §4 names
what reopens each capability.

**Injection.** Map `DroppedFile` to `Dropped::Hovering`: the case fails
with the path missing. *Good input:* the `CursorMoved` row yields 0
drops.

**Docs in the same commit.** `docs/plan-capabilities.md`;
the handoff's capability table; `book/src/platform.md`'s multi-window
and dialogs sections and its table at `:64-66`.

**Effort:** S. **Depends on:** nothing. May land before Phase 5 (§3
preamble).

### T54.8 — mobile: the check completes, and the matrix is recorded

**Closes:** R12-F6.

**Change.** Rewrite `docs/mobile-decision.md:27-31`: `cargo check` does
not link; the portable crates check cleanly for `aarch64-linux-android`;
the windowing crate's defaults are desktop-only (`clipboard` fails on
arboard, `dialogs` on `rfd`), `winit/android-native-activity` must come
from the application, and `reload`, `hot` and `agent` check. `arboard`
moves under a target-gated dependency table so the default build checks
for Android. T54.2's `a11y` is added to the matrix; if
`accesskit_winit` does not check for Android, it gets the same
target gate, and §2.3's default is desktop-only by construction.

**Gate.** `just mobile-check` from T54.0, extended with the default
feature set and with `a11y`, prints one line per configuration.

**Injection.** Put `arboard` back as an ungated dependency: the
default-features line goes red on arboard. *Good input:* removing it
again turns the line green in the same session.

**Docs in the same commit.** `docs/mobile-decision.md`;
`docs/competitive-feature-gaps.md` if it repeats the claim (grep first).

**Effort:** S. **Depends on:** T54.2.

### T54.9 — two stated facts, and a rule for the second kind

**Closes:** R15-F8, R15-F9.

**Change.** Rename `a_tree_that_drags_nothing_carries_one_byte` to what
its comment protects (the payload is in the type, not boxed), and give
it an absolute assertion with the size printed against a bound of 24.
`drag.rs:18-20` and `:44-45` say 24 B (the phase, two points and a
threshold) and that no tree carries one unless the application adds it.
`image.rs:10-12` says decoding is planned (T15.2, not built) and that
an application supplies decoded pixels through `ImageSource` today.
`scripts/check_doc_identifiers.sh` gains a feature-name rule: a
backticked name followed by "feature" in a comment must be a key in
some `[features]` table.

**Gate.** The renamed drag test prints `Drag<()> = 24 B (bound 24)`
and the size of the 64-byte-payload variant. The static rule prints
`feature refs checked: n, missing: m (bound 0)`.

**Injection.** Add a 1,000-byte field to `Drag` (the review's
`45fa10a`): the absolute bound fails at 1,024. Box the payload: the
relative assertion fails. Revert `image.rs`: the rule reports
`image.rs:11` `image` missing. *Good input:* the rule's checked count
is above zero, so it saw the `observe` and `dnd` mentions that do
resolve.

**Docs in the same commit.** The two rustdocs; `book/src/gestures.md`
if it repeats "one byte" (grep first); the deep-review results row.

**Effort:** S. **Depends on:** M43 (R17-F15) for the rule only; the
drag and image fixes may land at any time.

## 4. Boundaries of this plan

- **Incremental `TreeUpdate`s** stay deferred, as the handoff records.
  T54.2 sends a full snapshot per changed frame. Reopen when a
  measurement shows the full snapshot costing a frame that matters.
- **Per-row actions other than `Default`.** Row widgets in a `Repeat`
  hold no per-row state, so there is nothing to increment (§2.4).
- **Text SetValue.** A text field's value is replaced through the
  input method path (M45), not through an accessibility action that
  would bypass its selection and preedit state.
- **A contrast lint in `ui.lint`** (the re-triage's W1013). Not built:
  the harness covers the framework's widgets, and the per-frame audit's
  cost belongs to M48. Reopen for applications' own colours.
- **An unnamed-control lint for applications.** The role table covers
  the framework; an application's unlabelled toggle is not caught. A
  natural follow-up once T54.3 lands.
- **Drag and drop, multi-window and dialogs** (§2.5). Each reopens when
  an application asks for it, and its gate is then the one the
  re-triage wrote: a hit-tested drop event delivered to the node under
  the cursor; a two-window example with one agent port per window and a
  close that leaves the other answering; a showcase button behind
  `dialogs` with a portal-less fallback.
- **Touch input** belongs to M44 (R15-F4); **a second `resumed`
  dropping images** to M47 (R15-F6). Both are mobile lifecycle issues
  the assignment placed elsewhere.
- **iOS.** The target is not installed on the development machine, so
  nothing here is claimed for it.

## 5. Kill rules and risks

- **§2.3's kill rule** decides the default, and it is written above,
  before the build.
- **The feature seam** (§2.1). *Revealed by* T54.1's `a11y`-alone build
  of the role table.
- **The adapter's first tree arriving before the first layout.** An
  assistive technology that connects before the first frame gets a tree
  with zero-area nodes, all hidden. *Revealed by* the AT-SPI gate
  asserting a non-zero button count on its first walk, with the time
  since launch printed.
- **Hot reload and the adapter.** The adapter holds node ids from one
  library's table; a swap can change the table. *Revealed by* adding an
  AT-SPI walk after `scripts/window_reload_demo.sh`'s swap; if it
  fails, the session sends a full tree on swap.
- **Action delivery under `Boundaries`.** *Revealed by* T54.4's oracle
  frame.
- **`d85e851`, `45fa10a` and `fd31a02` are review probe commits** on a
  scratch branch. If they are gone when this runs, each injection is
  described well enough above to be re-made by hand.

## What building found

*Dated notes, added as each task was built; the sections above are the
plan as written and are left as they were.*

**T54.0 (2026-09-27).** Every finding re-checked at `12c5df1`: all nine
still stand where the plan put them, with line numbers moved.
- **AT-SPI names an application after its executable**, not its window
  title, so "an application with the showcase's title" is not a test
  anyone can write. The walker (`scripts/a11y_drive.py`) finds the
  application **by pid** (`Accessible.get_process_id`), which the live
  launcher already knows, so a second copy of the same binary from a
  sibling worktree cannot answer for this one.
- **The expected-to-fail leg is an `XFAIL`/`XPASS` pair**, not an
  `#[ignore]`: the showcase leg passes while the application is absent
  and fails the moment it appears, so the expectation cannot outlive
  T54.2. That is what lets the script sit in `scripts/live.sh`'s list
  from the first commit.
- **The measuring function is built here, not in T54.5.** A gate needs
  a measurement, so `lumen_static_shell::contrast` lands with T54.0;
  T54.5 moves T36.2's helper onto it and wires it into the drive.
- **The widgets' own defaults are a third accent.** The painted accent
  is `#2f6bff` — the showcase's value — not `Palette::LIGHT`'s
  (53, 99, 239), so §1.3's "the framework's palette has the same
  defect" is true of a palette no default paints. The harness reports
  six runs under 4.5:1 in the catalogue, all in `#2f6bff` (T54.6 owns
  the values), and 40 of 446 in the showcase.
- **`scripts/check_toolchain_targets.sh` could not see an Android
  triple**: its pattern knew `-unknown-`, `-apple-`, `-pc-windows-` and
  `-none-eabi*`, so `just mobile-check` would have passed undeclared.

**T54.7 (2026-09-27).** As planned. The `dnd` test is `cfg`-gated in
`tests/translate.rs`, so `just test` runs that binary once more with the
feature (`dev-shape`); without that line it would never run.

**T54.9 (2026-09-27).** Two corrections to the plan's injections.
- **"Box the payload" cannot be built as written**: `Drag` is `Copy`, and
  a `Box` is not. The injection that ran drops `Copy`, boxes the payload
  and deletes the test's `assert_copy::<Drag<u32>>()` line with it; the
  relative assertion then fires (32 B against 32 B). The relative
  assertion now runs before the absolute one, so each injection is
  reported by the assertion named for it.
- **The feature rule needs a negation.** `lumen-static`'s manifest and
  `lib.rs` both say there is deliberately *no* `no_std` feature; "no
  `x` feature", with the "no" on the same or the previous comment line,
  is skipped. The rule lives in `scripts/check_doc_identifiers.sh`, as
  the plan said, with a selftest plant
  (`scripts/selftest/plants/static/ident-feature-ghost.sh`).

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R12-F6 | S4 | OPEN | T54.8 |
| R15-F1 | S2 | OPEN | T54.2 |
| R15-F2 | S2 | OPEN | T54.4 |
| R15-F3 | S2 | OPEN | T54.3 |
| R15-F5 | S2 | OPEN | T54.7 |
| R15-F8 | S3 | OPEN | T54.9 |
| R15-F9 | S4 | OPEN | T54.9 |
| R16-F8 | S2 | OPEN | T54.5 |
| R16-F9 | S3 | OPEN | T54.6 |
