## Work queue

The current plan is `docs/plan-invalidation.md` (M16, T16.0–T16.14) —
the queue from the 2026-09-08 performance review. **All of it landed on
2026-09-08**; its header records what was built, what was decided
against, and what it measured.

Shipped and kept for their reasoning: `docs/plan-book.md` (M20 — the
book, all twenty-nine chapters, done 2026-09-10),
`docs/plan-architecture-v2.md`
(M0–M12), `docs/plan-review-fixes.md` (F0–F7),
`docs/plan-embedded.md` (M13), `docs/plan-desktop.md` (M14),
`docs/plan-capabilities.md` (M15), `docs/plan-invalidation.md` (M16).
Not started, in dependency order. **M19 has two prerequisites and they
are independent of each other**, so either may go first:

- ~~**`docs/plan-widgets.md` T11.6 — the `stack` axis.**~~ **DONE
  2026-09-10**, with `W1004` shipped in the same commit as T11.5
  pre-registered. T11.5's three widgets (dropdown, tooltip, modal) are
  now unblocked too.
- ~~**M18 — `Damage` as a set of regions.**~~ **DONE 2026-09-10.** No
  plan doc; the measurement is in the commit and in
  `docs/damage-amplification-decision.md`, which it partly supersedes.
- ~~**M19 — `docs/plan-glass.md`**~~ **DONE 2026-09-10** (T19.0–T19.6):
  `backdrop-filter: blur()` as `Glass<W>`, software only, with
  `docs/results-glass.md` for what it costs. Two of its seven tasks were
  decided *against* on measurement, which is where the doc is worth
  reading.

- **M24 — `docs/plan-select.md`** (T24.0–T24.3): the cascade's seed,
  and the `Select` it unblocks. Written from a reading of
  `examples/showcase`: the dropdown is hand-wired because three
  framework pieces that should have carried it could not be reached —
  `Popup`, the enum variant switch, and T11.5's never-built `Select`.
**M24 is done**: T24.0 and T24.1
  landed 2026-09-13, T24.2 and T24.3 on 2026-09-14, and **T24.4 —
  per-row addressing — on 2026-09-19**, as `%k` plus a descendant step
  that can reach an arena at all (it filtered on the table's parent
  chain, and a repeater's rows are not in the table). §1 has the measurement and the
  injection table, including the one that came back green; T24.1's
  entry has the 2.4× the cascade got faster for free.

- ~~**M22 — `docs/plan-liquid.md`** (T22.0–T22.6)~~ — **DONE
  2026-09-12**, all seven tasks; the queue entry above was stale until
  2026-09-19, when the code was checked against it field by field.
  Fresnel, inner shadow, saturation, sub-pixel sampling, dispersion,
  adaptive tint, press deformation, the smooth-minimum morph, and the
  GPU backend. `docs/results-glass.md` §0b–§0h own the numbers; the
  whole material costs **+18%** over M21's bevel at either panel size,
  and the GPU filter is 148 ms → **1.44 ms** at 1080p. Read §12 before
  §2: a four-agent review found five of the first draft's seven
  injections would have passed under their own injection, and the plan
  keeps the corrections rather than quietly replacing itself. **The
  §10/§11 rewrite earned itself twice over** — T22.0's first Fresnel
  test sampled the whole 13 px rim and measured a lift of 0.3 against a
  threshold of 12, which is the "bound computed from the wrong
  quantity" shape; and T22.1's half-pixel regression was caught by
  `glass_blur.rs`'s odd/even test, written before it.

- **M25 — `docs/plan-appearance.md`** (T25.0–T25.11): **DONE
  2026-09-15 — stopped at T25.5 on its own rule, and finished by its
  fallback.** T25.0 fixed stale hover and press shading under
  `Boundaries`; T25.2 shipped the sheet lint (`W1010`); T25.3 the palette
  and `app.setPalette`. The `Toggle` pilot — appearance as a look function
  on the widget, iced's shape — tripped two thresholds written before it
  was built and is in history as measured, not shipped. **T25.10 and
  T25.11 shipped instead**: colour fields on the widgets that need them,
  and palette roles in stylesheets that `Host::set_palette` re-resolves.
  T25.6–T25.9 are not built. `docs/results-appearance.md` T25.5 is the
  decision's evidence. **T25.12 measured the pilot again with its look
  cached (2026-09-16).** The cache works, the frame still trips, and the
  stop stands; the code is in history at `0df160b`. Its probe also
  overturned the attribution written before the build: the cost left is
  the cache check and the look-shaped paint, not the widget's size.

- **M26 — `docs/plan-frame-and-types.md`** (T26.0–T26.17): **Phases A
  and B are done** — A on 2026-09-17 (T26.0–T26.5), **B on 2026-09-19**
  (T26.6–T26.13). **Phase C landed the same day**
  (T26.14–T26.16): `wid_index!` resolves a `#[widget_id]` at compile
  time, so a typo is a build error rather than a frame that never
  redraws — and a **duplicated** id cannot be addressed at all, while
  styling still matches every node carrying it, which is why the refusal
  lives on the addressing path and not in the derive. `Seed::new` now
  **requires** its field mask: the `Fields::ALL` default was the
  `W1010` lint's off switch, and it was what a new widget got by doing
  nothing. The derive asserts both array lengths, and the injections
  showed the plan's "unfalsifiable" claim true of exactly one of the
  four cases. Only **Phase D** — the measurement round that decides
  which of the review's 18 plausible findings are worth building — is
  left. Phase B's headline figures, each on the
  arm its task named and eight runs a side:
  `TextInput` layout **47×** and `TextArea` **108×** (they measured what
  shaping had already returned); `Badge`/`Chip` **55×** (no memo at all);
  the cascade **1.56×**; a popup's damage **37.5×** on the synthetic arm
  and 100% → 84% of the window on the showcase's real dropdown; the GPU
  handed 5,000 px² where it used to get 452,400; and the raster path from
  1.00 + 3.00-per-panel allocations to **zero**. **Two tasks reported
  *no* win and say so**: T26.9's frame delta sits inside its own arm's
  spread, and T26.10's is below what its arm resolves — both kept for
  reasons other than the number. **T26.7's named arms could not move at
  all**, because neither lays out a text widget; the arm was added where
  it belongs. T26.1 fixed the enum derive zeroing its active variant under
  `Boundaries`, T26.2 the agent reporting a hidden node's container kind,
  T26.3 the text memo serving a stale box after a font-size or weight
  change, T26.4 an `Assembled` slab seeding hidden nodes from a live
  probe, T26.5 `Boundaries<N>` wrapping its length at 256 marks — after
  which every subtree answered clean and the frame skipped the whole
  tree. **Four of the five were invisible under the default
  `Everything`** or latent for want of a fixture, which is what Phase A
  was for. T26.0 added the six
  arms the review could not price — `place`, `dispatch`, the sheet lint,
  an `Either` frame, `Badge`/`Chip` against a real face, and a
  popup-open damage ratio — proved each moves under its own injection,
  and recorded the baselines. Two arms were vacuous on the first attempt
  and were rebuilt, and two of the plan's own proposed injections could
  not fire and were replaced. Written 2026-09-17 from a 14-agent review
  of the frame path
  and the type-level guarantees (42 findings; 20 confirmed, 18 plausible,
  4 refuted). **Phase A is five defects, not optimizations** — the enum
  derive zeroes the *active* variant before laying it out, so a clean enum
  subtree collapses under `Boundaries` and is correct under `Everything`,
  which is M25/T25.0's shape again. Phase B is the confirmed
  optimizations, each with its bench arm named first; Phase C the
  compile-time guarantees; **Phase D is a measurement round that decides
  which of the 18 plausible findings are worth building at all**, with a
  kill rule written before the probes. §6 records what the verifiers
  corrected, including four findings the review refuted and nine numbers
  cited for the wrong quantity.
  **All four phases are done, 2026-09-19.** Phase D probed its fourteen
  live findings out: **six closed on a measured ceiling** (1, 4, 5, 6, 8,
  11), every one of them *true as written* and worth nothing measured;
  **one was a defect** (12 — the text memo does not key on the display
  scale, which the plan predicted needed "an event ordering nobody has
  produced" and is in fact the ordinary monitor-to-monitor drag, since
  `ScaleFactorChanged` carries a new scale *and* a new physical size);
  **two shipped** (13, `FontStack::prepare` scanning the chosen face once
  instead of twice, −5.9% on a text-heavy layout; and 10,
  `lint_sheet_deep` asking for arenas once instead of once per rule,
  **4.04×**); and **four are promoted with their numbers and not built**
  (3, 9, 14, 15). The largest unbuilt one is 9 — the per-node arena ask
  costs 5.9 ns/node and −40% of `cascade_deep`, but removing it needs a
  const forwarded through 54 hand-written `Widget` impls, where a wrapper
  that forwards it wrongly silently stops styling the arena beneath it.
  Two Phase D lessons are general: **four runs a side could not separate
  a real effect twice** (findings 3 and 13), and eight could; and **two
  arms that disagree about one quantity are a fact about the arms** —
  finding 9's two call sites differed by 20×, and the cause was code
  alignment shifted by adding a bench arm.

- **M27 — `docs/plan-widget-parity.md`** (T27.0–T27.12): the thirty-three
  widgets left between here and Lumen's set — Stepper and FindReplaceBar
  dropped by the ask, a number input added — and, the larger half of the
  document, **the gate each one goes through**: an action inventory
  written before the widget and stored as data so coverage is an
  assertion rather than a review question; a showcase entry seeded to
  reach every state it names; a per-widget drive script run against the
  live window under **both** invalidation strategies, three assertions
  per action (it landed / the specific consequence, with the quantity
  printed beside its bound / the same input just outside does nothing);
  and an adversarial agent that must produce injections, not opinions.
  **The adversary is itself tested** once per tranche against a planted
  weakness. §2 names three prerequisites already owed elsewhere — per-row
  addressing (`docs/plan-select.md` T24.4), the stroked rect `Style` and
  `Cmd` still lack, and three taken names. **T27.3 retrofits the harness
  to the eighteen widgets that already exist and does not close if it
  finds nothing in them.** M23's §4 task list is superseded; its
  groupings are kept because M27's tranches inherit them.
  **T27.0 landing 1 shipped 2026-09-18**: the `~text` selector matches a
  label as well as a value, and the pointer path takes its centre from an
  entry's bounds, so a repeater row is clickable by the text it shows.
  `scripts/agent_drive.sh` picks the Switch by name instead of
  multiplying a row height. **T27.1 shipped 2026-09-18**: `Cmd::StrokedRect`
  and `border_width`/`border_color` on `Style`, inset so a bordered control
  still lines up with an unbordered one. `Painter::stroked_rect` is
  **required**, not provided — an injection reproduces the `Deferring`
  bug exactly by making it provided. It cost 16 B per node and +676 B of
  `.text` (`docs/results-embedded.md`), re-budgeted rather than edited.
  **T27.2** settled the three taken names (`SelectMenu`; Canvas and Tree
  keep theirs). **T27.3's harness shipped 2026-09-19** —
  `just widget-drive` drives every action a widget accepts against the
  live window under *both* invalidation strategies, with three assertions
  per action and coverage as an assertion rather than a review question.
  Its prediction held: **six defects while retrofitting the first six
  widgets**, five of them in the framework — duplicate author ids (now
  `W1011`), `after_update` unable to report, `ui.lastDamage` answering
  with a bounding box instead of M18's region set, the journal folding
  children into containers, a container's own paint never signed at all,
  and `Cmd::hash_into` folding a text run's buffer offset. Damage per
  interaction went from 170×–1 200× the widget's own box to **1.00×**
  (`docs/damage-amplification-decision.md`). Twelve widgets are left to retrofit; the harness drives 18
  actions across six today, every one at 1.00×. *(Corrected
  2026-09-27: the 1.00× is the agent journal's damage in logical units,
  over repainting actions. It never measured what the surface presents;
  M48/T48.4 prints that beside it, `docs/plan-agent-truth.md`. Every
  "green both ways" below was re-run at its own SHA by M42/T42.8, on the
  binary each leg launched: all legs pass, `docs/results-drive-identity.md`.)*
  **T27.4 shipped
  2026-09-19** — `Card`, `Toast`, `Tooltip` in `lumen-static-widgets`,
  and three more defects the drive found: a `Repeat` handled no wheel at
  all (so **a `Select` with more options than fit could not be scrolled
  by anyone**), it measured its viewport from the limits it was offered
  rather than its own resolved box (M24/T24.2 fixed that asymmetry for
  the width and left the height), and a tooltip drawn over its own
  control took every click on it, because the overlay pass hit-tests
  before `Node::hit`. Nine widgets, 24 actions, green under both
  strategies. **T27.5 shipped 2026-09-19** — `NumberInput` (the ask's
  addition, built against the plan's own eleven-row inventory),
  `RangeSlider`, `SearchField`; plus two more findings: no widget in
  `lumen-static-widgets` published anything to the agent (the crate had
  no `observe` feature), and **a wrapper must loosen a child's limits to
  let it shrink** — the maximum is inherited, the minimum is not.
  Twelve widgets, 33 actions, green both ways.
  **T27.6 shipped 2026-09-19** — `Tabs`/`BottomNav`/`NavigationRail` as
  one `Strip<KIND>`, plus `AppBar` and `PullToRefresh`, which are *not*
  strips and the tranche table's grouping was wrong about. A render
  caught the rail's selected label drawn in a transparent background —
  invisible on its own fill, §3b's ghost button for the third time in
  M27. Seventeen widgets, 42 actions, green both ways.
  **T27.7 shipped 2026-09-19** — `Modal`/`Sheet`/`Drawer` as one
  `Veil<W, KIND>` that defers *itself* so scrim and panel land together.
  `W1011` fired a third time (one body type shared by three veils), and
  the harness now refuses to drive any tree carrying one: **an author id
  lives on a type's field, so a composite used twice is unaddressable by
  construction.** Twenty widgets, 51 actions, green both ways.
  **T27.8 shipped 2026-09-19** — `wrap` and `grid` as **derive axes**
  (one lowering: a grid is a wrap whose break is a count, not a width),
  plus `SplitPane` and `Collapsible`. There is deliberately no
  `Accordion`: it is a column of `Collapsible`s, and a type per section
  count is what owning the one-open rule would cost. Twenty-four widgets,
  59 actions, green both ways. **T27.9 shipped 2026-09-19** —
  `PickList`/`Combobox`/`Menu` as one `Chooser<KIND>`, which do *not*
  use `Repeat` (a bind `fn` cannot capture the widget owning the
  options). It forced **T27.0's third landing**: a descendant step,
  `#outer > #inner`, after `W1011` fired four times on the same cause —
  a part id lives on a type's table, so a composite used twice is
  unaddressable by construction. `W1011` is scoped to match. Twenty-seven
  widgets, 64 actions, green both ways.
  **T27.10 shipped 2026-09-19** — `Pagination`, `Tree`, `DataGrid`, and
  a defect only the live assembly could find: the agent's per-frame
  audit measures every leaf twice under 1e6/2e6 px boxes, and
  `Repeat::layout` clamped its *stored* scroll offset against the box it
  was handed — so **a grid could not be scrolled at all** while a unit
  test, a headless `Host` and `tests/repeat_scroll.rs` all agreed it
  could. Layout derives and no longer decides; the viewport comes from
  `place`; `Repeat` overrides `Node::probe`, the hook that exists for
  exactly this. **Its adversarial review (§5, run for real) found
  fifteen more** — a pagination whose current page number was painted
  *outside* its own pill in the pill's contrast colour (an empty blue
  lozenge, shipped), a tree painting 116 px outside its box, and
  `Event::Hover`: a `PointerMove` reaches a widget only under pointer
  capture, so two widgets' hover highlights were unreachable code. The
  review's planted weakness was caught, so the adversary passes its own
  gate. `AgentHost::pump` now answers `(frame, handled)` — the only
  direct answer to "did that input land", and the only way to test an
  action whose correct outcome is that *nothing happens*.
  **T27.11 shipped 2026-09-19** — `ColorPicker`, `DatePicker`,
  `TimePicker`, `FilePicker` as one `Picker<KIND>`: a trigger and a
  popup of *cells*, because a swatch grid, a month, a clock and a file
  list are the same geometry. Three of its defects were caught by
  looking at the render — a six-column palette that cut across its own
  hue families, a month with no weekday strip, a clock with no seam
  between hours and minutes. Thirty-four widgets, 88 actions, green both
  ways.
  **T27.12 shipped 2026-09-19 and closes M27** — `RichText`, `Markdown`,
  `RichTextEditor`, `PaintBox`. A span is a **word**: the measure hook
  answers how wide a string is, not where it would break, so a span
  never wraps inside itself and the breaker only decides whether the
  next one fits. Markdown has **no italic** and says why — `Painter`'s
  only typographic axis is weight, and markup that rendered identically
  to plain text would claim to have done something. **The render found
  a defect sixteen green tests did not**: `**bold**,` — emphasis
  followed by punctuation, the ordinary case — does not end with `**`,
  so an end-strip never closed it and every word after stayed bold. The
  parser scans *inside* the word now and a marker toggles.
  **And `PaneGrid`, which §1 listed and T27.8 never built** — caught by
  checking the count mechanically instead of restating it, one `grep`
  per name because "thirty-three are built" was about to be written
  down. Four panes and three dividers, a shared vertical and one per
  column; iced's is a *tree* the user re-arranges, which is topology as
  data, so what is missing is named in the rustdoc rather than faked.
  It also forced `~=`, an exact-text selector: one widget's name can be
  a prefix of another's, and `~RichText` matched `RichTextEditor` too.
  **M27 is closed: thirty-three widgets, checked name by name.** Thirty-
  nine drivable entries, 94 actions, green both ways.

- ~~**M28 — `docs/plan-tasks.md`** (T28.0–T28.12)~~ — **DONE
  2026-09-20**, all thirteen tasks. iced's `Task` and `Subscription`
  shapes in this architecture's terms, with **chains as data**: a
  continuation is a *step index*, not a closure across an await, which
  is the same move that let positional identity replace captures in
  handlers. `docs/results-tasks.md` owns every number; its threshold
  table was written before any of it was built and is unchanged.

  Written 2026-09-18 and **revised the same day after a seven-lens
  review: 42 findings, 13 blocking.** The first draft could not be
  built — nothing could *spawn* a task, no message type reached the
  host, and a reload bumping the epoch made the cross-swap gate
  unpassable. §5 records all of it.

  **T28.7 is the milestone's centre and the one to read**: a chain
  started against v1 completes into v2's step function across a real
  dylib swap, with the pending descriptor held by the loader process as
  three `u64`s. Its injection — holding the continuation as a captured
  function pointer — makes the demo answer with v1's value, which is
  `docs/events-decision.md` rule 3's defect **demonstrated rather than
  argued** for the first time since that rule was written. *(Corrected
  2026-09-27: that injection lives on the fixture side, and
  `fixtures/hot-host` holds only the descriptor, so T28.7 proves the
  descriptor survives, not that the framework carries a chain. A window
  swap re-issuing work in flight is M49/T49.10's,
  `docs/plan-reload-hardening.md`.)*

  **Three plan claims were falsified by building them**, and all three
  are recorded where they were wrong rather than quietly fixed: §2.6
  said the hand-written `App` impls would be untouched (associated type
  *defaults* are unstable, so `type Msg` is required and 19 impls
  changed — the count was 19, not 13); the first draft had a reload bump
  the epoch (T28.8 now tests the opposite, with the first draft's own
  specification as the injection); and `fixtures/hot-app`/`hot-host` did
  **not** "already drive a swap with state in flight", so T28.7 built
  what the gate needed.

  **Bounding the inbox made it faster**, which is worth knowing because
  the opposite is the usual trade: 19.4 → 14.7 ns per result, since an
  unbounded channel allocates a node per send and frees it per receive.
  M28's whole per-frame cost is inside the idle arm's own spread
  (351.19 µs against 349.97–351.47 before it), and the embedded profile
  links **zero** task symbols.

  **T28.12 is why the milestone needed a "use the thing" task.** Every
  piece had a passing test and the live window still could not run a
  task: `run()` installed neither the waker nor a spawner, and
  `about_to_wait` never set `WaitUntil` from the host's deadline, so a
  timer on an idle window was never checked. Both are defects no
  headless test can exhibit.

- **M29 — `docs/plan-damage-precision.md`** (T29.0–T29.4): **T29.0 and
  T29.1 done 2026-09-21 — the probe's kill rule did not fire, so the
  rest is justified.** M26's Phase D measured `#[bind]`'s blast
  radius — one binding writing one label calls `mark_all`, and `mark_all`
  under `Boundaries` costs exactly what `Everything` costs: **6.22 µs,
  9.7×**. **The prescription it recorded is wrong and §5 says how**: the
  derive sees `ui.card.value` as *tokens*, not types, so it cannot resolve
  a path to a node index; `with =` and `after_update` can never be
  narrowed; and the 9.7× is layout-only on 901 synthetic nodes. So the
  milestone **builds a spreadsheet fixture first** — thousands of table
  nodes, a virtualized body, and one bound label on the table — and
  T29.1 is a probe with a kill rule that can end the milestone. The fix,
  if it survives, is per-field offsets as consts, which associated types
  make possible where a proc macro cannot.
  **It survived — and a seven-lens review then found the fixture
  measuring something other than what it said.** `benches --bin grid`
  (`just grid`) is a 2,227-node spreadsheet; one edit costs about a third
  of the frame (`docs/results-damage-precision.md` T29.1), and the kill rule wanted anything above the arm's spread, so
  T29.2–T29.5 are justified. `docs/results-damage-precision.md` owns
  every figure. **The review's central finding is the milestone's
  premise, not its details**: `pump_with` calls `mark_all()` for a
  consumed event eighty lines before it asks `App::update` anything, and
  every `#[bind]` in this workspace is written by a click handler — so
  the saving is **zero on the frames applications actually produce**,
  and the probe now measures that frame beside the others (§2.5). Two of
  §2.3's mechanism and T29.3's injection did not survive either: an
  associated-type chain cannot be named past the first hop, and the
  plan had specified an injection it predicted would leave the gate
  green. **The fixture's best output was a defect it was not built to
  find**, and **T29.5 then found that finding's own attribution wrong**:
  the 12.2 µs was real, but 10.8 of it was the *fixture's* row bind
  calling `format!` — one heap allocation per row per frame — and not
  `Repeat` re-laying a clean window, which costs ~1.5 µs and was decided
  against. **Nothing caught it because `alloc_gate.rs`'s two repeater
  arms both bound with `noop_bind`**, so the zero it proved was a zero
  for a list whose rows are never filled in; the only thing that
  allocates on a repeater frame is a bind that *writes*. T29.5 shipped
  the missing arm (its injection fires at 19 allocs/frame on a 19-row
  window), the rustdoc on `Repeat::bind` that says a bind is
  steady-state code, and the fixture's own fix. **A real number
  attributed to the nearest plausible mechanism is the failure mode
  here** — the measurement was never wrong, the story was, and only
  splitting the quantity four ways separated them.
  **And the lesson that generalises**: T29.0 had injected its faults and
  watched them go red, and four of its seven defects survived that,
  because each was a question nobody had asked the fixture — how many
  rows did you materialize, did anything shape, which passes are in that
  column, who wrote the value. Asking what a fixture *contains* is a
  distinct method from asking whether its assertions can *fail*.
  **DONE 2026-09-24 (T29.2–T29.4, T29.6):** a binding marks its own node
  (found by address, cached per site), and a click on a split application
  narrows too — its frame lands on the narrowed row
  (`docs/results-damage-precision.md` T29.6).
  **Amended 2026-09-23:** T29.2–T29.4 wait for M33 (below). Narrowing
  the `handled` path, which §4 had deferred, is now **T29.6**, because
  M33 makes it possible. Paint culling stays shelved on §4's condition.

- ~~**M30 — one crate owns the widgets.**~~ **DONE 2026-09-21.** Every
  user-placeable widget moved from `lumen-static` to
  `lumen-static-widgets`: `Label`, `Button`, `Fixed`, `Spacer`, the
  three toggles, `Slider`, `TextInput`, `TextArea`, `Image`,
  `ScrollView`, `Select`, `Popup`, `Glass`. No plan doc; the reasoning
  is in the commit and in `docs/results-widget-crate.md`.

  **The number the old split was argued from does not support it.** The
  widgets crate's manifest argued from the embedded budget; the leaf
  `no_std` profile links `.text` **27,024 B** (at M30; the current figure
  is the headline in `docs/results-embedded.md`) with every widget in the
  optional crate against 27,036 B with the first tier in core —
  *twelve bytes smaller*. The gate was never the crate: it is
  `#[cfg(feature)]` and the linker, and both drop what a tree does not
  name whichever crate it lives in.

  **What stayed is not widgets**: `Either`, `List`, `Dyn` and `Repeat`
  are what the *derive* builds, and `lumen_static::ops` keeps the layout
  helpers derive-generated code calls. Splitting `ops` in two closes
  `docs/plan-review-fixes.md` **F6.1**, which that module's own rustdoc
  had named as "the honest end state" and deferred for churn.

  Three things worth carrying forward. **The precondition was a latent
  defect**: `lumen-static-widgets` advertised an `alloc` tier and could
  never build `no_std`, because its dependency line omitted
  `default-features = false` — and the failure named *core* ("std is
  required by lumen_static"), which is why an advertised tier went
  unexercised. **The derive needed no change at all**, because it had
  never named a concrete widget type. And **a feature seam only a live
  gate could find**: a widget's `observe` impl is gated on the widgets
  crate's feature, not core's, so an agent build that forgets
  `lumen-static-widgets/observe` reports every label as
  `GenericContainer` with no value — caught by `counter-window --drive2`,
  not by any unit test.

- ~~**M31 — `docs/plan-application.md`** (T31.0–T31.3)~~ — **DONE
  2026-09-23**, all four tasks. An application had **two construction
  paths and nothing checked they agreed**: its author wrote
  `Counter::new()`, and the framework rebuilt it with
  `Default::default()` because `Reloadable::fresh()` derived to exactly
  that. `Application::build` is the one path now, `fresh()` routes to it,
  and `Application::style` is re-asked on every swap so an edited
  stylesheet reaches the frame. **The derive needs no new attribute** —
  an application is the thing with a `#[tree]` field, and a field
  attribute is visible to every derive on the struct.
  **Two of the four tasks were rewritten by building them.** T31.1 said
  25 types and the right number was four: the derive already *forces*
  the trait on every reloadable application, which is exactly the set
  where two paths can exist, and most `#[tree]` types are test fixtures
  that never swap. It also found what the trait cannot express —
  `build(sigma)`, `build(k)`, `build(entry, open)` are **parameterised
  constructors**, so `examples/glass`, `glasswall` and `showcase` keep
  `Host::new`, which is why that constructor stays. T31.3 specified a
  pointer-range check against a host's sheet, and `fixtures/hot-host`
  has **no `Host`** — a loader cannot name the application type across a
  dylib boundary, and a `Host` inside the dylib dies with the swap, so
  that arrangement cannot exhibit the defect at all. The scenario that
  can is a host outliving the application it holds, which is what
  `Host::adopt` serves. So the dylib test proves the **lifetime** fact
  (v1's selector still reads `panel-v1` at its old address after v2
  loads — the loader never `dlclose`s, so a kept sheet styles by last
  week's rules in silence) and `tests/reload_sheet.rs` proves the
  **behaviour**; neither proves both.
  T31.2's refusal is a **coherence conflict**, because Rust has no
  negative bounds: a blanket impl that overlaps only when `Self: Default`,
  so rustc answers E0119 and *prints the trait's name* — which is why the
  name is a sentence. Both directions checked in one session, since a
  blanket written wrong refuses every application instead of none.

**Queued 2026-09-23, from the handoff's six decisions, and revised the
same day after a seven-lens review of the plans** (62 findings: 58
confirmed, 4 plausible, 2 blocking). Each plan keeps a "What the review
found" section rather than silently replacing itself.

**Dependency order:**

- M32 first.
- M33 after M32. T33.4's gate uses the `--shot` paths T32.1 adds.
- M29's T29.2–T29.4 and T29.6 after M33. T29.6's *Measure* also waits
  for M34/T34.2.
- M34 alongside M33.
- M36 after M32, and M37 after M36.
- M35 is independent of all the rest.

- ~~**M32 — `docs/plan-program.md`** (T32.0–T32.2)~~ **DONE 2026-09-24**,
  62 of 62 migrated frames byte-identical; its migration found tasks-demo's
  sheet had never matched anything (§6).
  `application(boot)`, iced-shaped, with constant-or-function
  `title`/`style` and a boot that may capture and may return initial
  jobs. **The program description is the only source** of construction
  and styling, for the window and for every headless host
  (`Program::host()`). `Config::stylesheet` and `Application::style`
  go, and boot supersedes `Application::build`. The first draft's
  "reload through the builder's `fn` pointer" was **rule 3's defect**
  and was removed.
- ~~**M33 — `docs/plan-state-split.md`** (T33.0–T33.4)~~ **DONE 2026-09-24**;
  it found and fixed a dismissal defect on the way (`f72fb05`). Ship
  `proto/state-split`. `App::State` puts the tree out of a handler's
  reach by type. `#[bind]` gains a source path and a value form.
  **`Select` becomes a controlled widget**, following `Toggle`'s
  precedent, which is what the showcase redesign needs. The prototype's
  "no handler writes the tree" was false: showcase, tasks-demo and
  glass all do.
- ~~**M34 — `docs/plan-boundaries-default.md`** (T34.0–T34.2)~~ **DONE
  2026-09-24**: `Boundaries<256>` is the default for `Host` and the window;
  the inventory found a tick that wrote the tree unmarked. the
  inventory (`agent-drive` turned out to be `Everything`-only). A
  shadow oracle that re-lays out into scratch state **inside one host**
  and reports through the agent rather than panicking. Then both
  defaults flip, the winit alias and `Host`'s type parameter, with
  `everything` kept as a diagnostic feature.
- ~~**M35 — `docs/plan-observe-cost.md`** (T35.0–T35.2)~~ **DONE 2026-09-24**:
  the frame arm's `observe` cost is gone; the grid never paid it. the cause is
  measured (`docs/results-competitive.md` §7, "Attributed"). The fix
  is a **required**, table-driven, statically dispatched observe walk,
  which the agent moves onto as well, with an equivalence gate against
  the `&dyn` walk. The symbol table is the gate, and the clock is the
  record.
- ~~**M36 — `docs/plan-panic-recovery.md`**~~ — **DONE 2026-09-24**,
  all six tasks. `just panic-drive` drives seven panic sites in a live
  window under both strategies, and `just hot-demo` catches a library's
  panic inside the library — the loader's own hook, installed
  throughout, is asserted to see **zero**, which is the `cdylib` fact.
  (T36.0–T36.5): a screen with
  the message, `file:line:col` and the nearest application frame. It is
  compiled in for debug builds, and for **release builds with
  `lumen-static-winit/reload`**. One guarded entry covers every path,
  the agent's `Live::pump` included. Input is swallowed while the screen
  is up, and recovery is an explicit retry: R, or `app.retry`.
- ~~**M37 — `docs/plan-window-reload.md`**~~ — **DONE 2026-09-24**,
  all six tasks. The window drives a `Session`, and a hot library
  exports one through `export_program!`. The boundary is borrowed
  pointers behind an ABI stamp, **by measurement**: a real byte encoding
  exceeded the frame arm's spread on three of four entries. `just
  hot-window-drive` edits a copy of a live window's source and swaps in
  0.46–0.64 s (`docs/step4-hot-reload.md`). Driving it found two
  defects no headless gate could: a fixed-back build left its error up
  forever, and the error bar **outlived its fix**, because its pixels
  were the window's and in no session's damage. **A stamp's sizes alone
  did not catch an `observe` build**: no listed type changed size while
  every vtable did, so the feature field is the load-bearing one.
  (T37.0–T37.5): hot reload inside a window, modelled on `hot_ice`. The **`Host` lives in the
  library**, because the tree holds code, which iced's does not. A swap
  re-runs the new build's `program()`/boot and grafts the
  `#[reload(keep)]` state onto it. The build and watcher are built in,
  failed builds are shown over the running application, and a
  successful swap clears M36's screen. v1 never unloads a library.

- **M38 — `docs/plan-deep-review.md`** (R1–R18): **DONE 2026-09-24.**
  A deep review of the four goals. It reviewed and built nothing.
  `docs/results-deep-review.md` owns every figure. 182 findings survived
  adversarial verification (2 S0, 16 S1, 77 S2, 87 S3/S4). None was
  refuted, but verifiers lowered 19 severities.
  **Verdicts:** performance, configurability and embedded are *partly
  met*; the agent, hot reload and mobile are *not met*.
  **The two S0s:**
  - **R14-F1:** any web page can drive a running dev app through the
    agent endpoint. The line transport skips HTTP header lines as parse
    errors, then executes the body. It was reproduced three times,
    including from a real Firefox.
  - **R1-F1:** damage is logical but presented to a physical surface, so
    every scale other than 1.0 leaves stale pixels in the default
    configuration.
  **The S1s cluster where the queue is about to lean:**
  - Boundaries misses compose, tick, `Repeat` data and the audit's probe
    (R1-F2..F5), and M34/T34.1's shadow oracle, as planned, cannot see
    three of those four (R1-F6).
  - An idle window never fires its timers (R7-F1), and the two gates
    cited for it stay green with the wiring removed (R7-F2).
  - "Raster allocates nothing" holds only on stand-in fonts (R8-F1).
  - No window can hot reload while the docs say one can (R4-F1).
  **74 tests or gates were found proving nothing**, the table AGENT.md
  keeps extended; each is listed with the injection that exposed it.
  **Seven kill rules or injections in M29, M32–M34 and M37 cannot fire as
  written** (§7).
  **Proposed follow-ups, M39–M55:** M39 (agent transport), M40 (physical
  damage), M42 (drive-harness identity) and M43 (static gates and CI) go
  first, and M41 (Boundaries completeness) is a prerequisite of M34's
  flip. **Now planned: see the M39–M55 entry below.**

- ~~**M39–M55 — `docs/plan-m38-fixes.md`**~~ — **DONE 2026-09-27** on
  branch `m39-m55-fixes`, **`just ci-verify 29a2fe7` green**: 28 steps,
  0 failed, 0 skipped, a clean tree. That covers every test tier, the
  live gates, the firmware, the GPU, miri, the book and three self-test
  tiers. Every task is done except **T49.12**, a live latency gate that
  needs a library target in the showcase M56 is redesigning. It waits for
  M56.
  - **The three S0s are closed.** Damage is physical, so a scale other
    than 1.0 leaves no stale pixels (M40). The agent transport closes on
    anything but a JSON object and binds loopback only (M39). An IME
    commit marks what it changed (M41/T41.1).
  - **M34's evidence was gathered again** with an oracle that inspects
    without repairing (M41/T41.7), on a harness that names the binary it
    drove (M42). Every leg passed, and the first sweep found a real
    divergence in the shipped showcase, which T41.3 fixed.
  - **Merge-time integration found about a dozen defects that no branch
    could see alone.** Each is fixed in its own commit, with red and
    green evidence. Examples: a `harness=false` test counting as zero
    tests; a launcher flag that read as a cargo target; allow entries
    and a firmware headline that one branch made stale for another;
    M48's doc link and M55's `hot` build, which only `check-extra`
    builds; and M55's W1003 firing on every open popup. **Run `just ci`,
    not `just test`, before calling a milestone done.** That is now the
    rule in this queue's preamble.
  - **Decided against on measurement.** Each is in its plan's "What
    building found":
    - T50.9's bounded glass capture (unsound for every material);
    - T50.4's pool, where the exact budget ships instead;
    - `a11y` on by default (its kill rule fired: +4.12 MiB), so the
      showcase opts in;
    - M53's arena-owner list, replaced by a walk over marked ranges.
  - **Open, recorded rather than fixed:**
    - dark palettes fail contrast, because widget defaults are
      light-theme values (M54 §6; shared with M56);
    - a 2 px seam under every card shadow (M51 §6), which changes every
      golden when fixed;
    - `ui_check.py`'s p50 line cannot fail (M48 §6);
    - the drive harness could judge lint findings by key rather than by
      index (M55 §6);
    - intermittent segfaults of unrelated programs on the reference
      machine (python3, LLVM `opt`, rustc; `dmesg` shows ip 0), which
      look like hardware or system instability rather than any code.

- **M60 — `docs/plan-tally-slim.md`** (T60.0–T60.5), branch
  `tally-slim`: `examples/counter-window` onto `Program::run`. About 540
  of its 1,383 lines re-implement the window crate's loop, and only two
  things needed that loop — app-level keys and a sheet keyed on the
  count's sign — so the milestone is two framework hooks
  (`Program::on_key`, `Program::style_by`, both through
  `Host::pump_with`) and then the migration. **T60.0–T60.5 DONE
  2026-09-28, `just ci-verify 8d496c1` green** (28 steps, 0 failed, 0
  skipped): the tally runs on `Program::run`, 1,383 → 514 lines, its
  window loop 538 → 0. The migration found two framework gaps every
  `Program` window had: no input latency in `app.perf`, and a `ui.lint`
  audit measuring with the block stand-in instead of the session's face.

- **M61 — `docs/plan-lss.md`** (T61.0–T61.7): `.lss` stylesheets,
  **parsed at compile time**. **T61.0–T61.7 DONE 2026-09-28, `just
  ci-verify 0793dd5` green** (28 steps, 0 failed, 0 skipped; live gates
  on a private Xephyr display). All three kill rules hold
  (`docs/results-lss.md`): K1 zero bytes in four firmwares, K2 +9 ms
  (tally) / +44 ms (showcase) of warm build, K3 0 reader symbols in a
  release window. The tally's sheet is `app.lss`, its frames
  byte-identical at −3, 0, 7. Found: the first tree checks spent 88 of
  K2's 100 ms (one table pass now); `cfg(debug_assertions)` is false in
  the shell's dev build, so the window decides when the file is read
  live (`Program::live_lss`); paths resolve from the invoking file.
  - **What it is:** `include_lss!(Tree, "app.lss")` emits the same
    `const` rules a person writes by hand. Its claim is zero bytes and
    zero instructions over a hand-written sheet (kill rule K1: the
    firmware's `.text` and `.rodata` byte-identical).
  - **Errors at build time:** an unknown property, a bad value or an
    `#id`/`.class` the tree lacks is a compile error at `file:line:col`.
    A kind selector stays with `W1010` at run time, because `NodeDesc`
    carries no kind.
  - **v1's grammar:** what our cascade can apply, plus `@tokens`,
    palette roles as `$tokens`, and `@sheet` blocks for `style_by`.
    Everything else in Lumen's `.lss` is a named compile error.
  - **Dev builds:** the same parser reads the file at run time, so a
    colour change needs no rebuild.
  - **Deferred:** a compile-time cascade (§4).

- **M80–M90 — `docs/plan-iced-roadmap.md`** and its eleven milestone
  plans: what iced's roadmap to 1.0 has that this framework does not,
  on branch `iced-roadmap`. **Queued 2026-10-04; T80.0 starts it.**
  - **Closed so far:** **M80a DONE**, `just ci-verify 1cd123e` green
    (28 steps, 0 failed, 0 skipped; the first run's `selftest-build`
    red was a `zerocopy` miscompile in the selftest copy that did not
    reproduce, the known machine instability). **M80b and M81a DONE**,
    `just ci-verify 123e138` green (28 steps, 0 failed, 0 skipped).
    M80b's first run, at `d4859b4`, failed the full-catalogue live
    drive: T80.5's IME check demanded an area from `PickList`'s
    read-only trigger, which no single-widget drive had met; fixed in
    `123e138` (a read-only field publishes `readOnly`, and the check
    runs both ways). K81a fired at T81.3 (Complex 2.17x / 3.63x Basic)
    and its written response holds it (0.97x / 0.99x). **M81b DONE**,
    `just ci-verify b873ab0` green (28 steps, 0 failed, 0 skipped).
    **M82a, M82b and M83a DONE**, `just ci-verify a4080ea` green (28
    steps, 0 failed, 0 skipped): one record for three closes, since
    M82a's close (`a77cc52`) and M82b's (`5b6da2b`) are ancestors of
    M83a's. M82a's own run failed four steps on a full disk ("No space
    left on device"), not on code; a reboot had also left the CI
    copies' incremental caches half-written (an incremental-compilation
    ICE), cleared before the re-run. Rulings recorded in the plans:
    K82c is read one-sided as a stop (a faster median is not a firing);
    K83a is measured on the reference GPU (llvmpipe's 111 us/shader is
    recorded, not a firing). **M83b, M84a and M84b DONE**, `just
    ci-verify 2fcb000` green (28 steps, 0 failed, 0 skipped). M83b's own
    runs at `1024cba` failed: first on a full disk, then
    deterministically on the live tiers, because four drive scripts
    read the showcase's entry list from `catalog.rs`'s text and T83.6's
    `#[cfg]`-gated `ShaderView` entry skewed every later index by one
    (fixed in `553ac3c`: the drives ask the binary, `--entries`); then
    `selftest` refused `check_default_crates.sh` in a parentless
    one-commit copy (fixed in `11c7aa2`). One run was restarted after a
    rustc hung 79 minutes on `x11_dl` (the machine's instability). K84b
    fired on its third condition (GStreamer dev libraries absent), so
    `lumen-static-video` is deferred; it reopens when the owner installs
    `libgstreamer1.0-dev` and `libgstreamer-plugins-base1.0-dev`.
    **M85 and M86 DONE**, `just ci-verify 262643a` green (28 steps, 0
    failed, 0 skipped). M85's run at `73f174c` failed `stack`: T85.1's
    out-of-line `Round::limits` put three temporaries on every derived
    level's stack (+72 B a level); fixed in `1148444` by recovering the
    bytes, not re-budgeting -- and changes to emitted layout code now
    check `just stack` beside `just firmware-delta`. K85c fired
    (`scope_end` costs +11.6% on every frame), so container culling is
    decided against and T85.8 does not ship; K85e decided the capture
    phase against (one real case of two). **M86 stopped at T86.1 on
    K86b**, re-measured on a quiet machine after the first reading was
    taken beside a CI run; T86.1 (the level, the blend behind
    `transitions`, `transition-duration`) stands, T86.2-T86.4 are not
    built. A defect found on the way is fixed in `c22d6a0`: a popup
    inside a `List`/`Repeat`/`Dyn` drew nothing and was not hit.
    **M87 DONE** (T87.0-T87.7), `just ci-verify 6984df0` green (28
    steps, 0 failed, 0 skipped). K87 fired on measure, so widths are
    reused by word content (a 64 KB append 3.23 -> 0.57 ms) rather than
    re-parsing incrementally; K87c fired too; K87a and K87b were not
    reached (`docs/plan-markdown.md`). A CI run must take its driver
    from the commit it tests (`git show <sha>:scripts/ci.sh`): the
    first run at 6984df0 died when a later commit edited `ci.sh`
    under it.
    **M88, M89a and M89b DONE**, `just ci-verify 1faf792` green (29
    steps, 0 failed, 0 skipped). M88 (T88.0-T88.5) and M89a
    (T89.0-T89.7) shipped as planned; **M89b closed with K89b fired**:
    T89.9's resume leg and per-window RSS ship, T89.10's shared GPU
    context does not and is kept on `t89.10-shared-gpu-context`
    (`9cc2e22`) pending the owner's call on judging K89b by the GPU
    share (`docs/results-windows.md`, Summary). The runs at `a796eb7`,
    `6cd1e0d` and `cad8edc` failed on a compiler SIGILL, two script
    defects (`cce0f65`) and a Toast drive race (`f3deb9a`).
    **M90a DONE** (T90.0-T90.5), `just ci-verify ac98b18` green (29
    steps, 0 failed, 0 skipped): context menus, the drawn menu bar (D2)
    and accelerators. **M90b is open**: T90.6's tray seam is in
    (`8747678`); **T90.7 stopped on K90c** (`ksni` +3.29 MB against
    3 MB) because its written fallback, `tray-icon` on a GTK thread,
    reverses D2 -- the owner's call (`docs/results-desktop-integration.md`,
    Summary).
  Written 2026-09-28 from iced's board, then reviewed twice by eight
  lenses with adversarial verification (202, then 189 findings; 19,
  then 9 blocking), redrafted per milestone, and checked three times
  for cross-file consistency; every file keeps both review lists.
  Renumbered from M62–M72 because `m62-m64-plans` claimed those first.
  - **Order (index §5):** M80 text fit → M81 shaped paint →
    M82 paint vocabulary → M83 GPU extension → M84 images in motion →
    M85 layout and input → M86 transitions → M87 Markdown →
    M88 platform requests → M89 windows → M90 desktop integration.
    Most are split into a/b halves, each closing on its own
    `just ci-verify`.
  - **Owner decisions, binding (index header):** D1 no new crate, four
    moves; D2 a drawn menu bar on Linux, native `muda` deferred;
    D3 `shader` gated at every use site (`cfg(lumen_shader)` outside
    the core); D4 `Style` 84 / `NodeState` 88 at most; D5 the firmware
    ceiling is **80 KB** (raised from 64 KB; 19,998 B left on
    `firmware_interactive`); D6 `text_spaced` and `fills` are accepted
    firmware growth; D7 `xclip` is installed on the reference machine.
  - **Per-task firmware cost** is `just firmware-delta <parent>`
    (K4: 1,000 B per task on `firmware_interactive`).
  - **Not planned, with reasons (index §1.3):** font hinting,
    time-travel debugging, the Comet debugger, arbitrary z-order, a
    separate test emulator, iced's own renderers and dependencies.


- ~~**M56 — `docs/plan-showcase-design.md`**~~ — **DONE 2026-09-25**
  on branch `m56-showcase-design`, all six tasks. It built what was
  asked, and building it found **three framework defects no test had
  seen**, each fixed with its own gate:
  - a `Fixed` container laid its content out across its *offer* (two
    halves of 250 px inside a 100 px box);
  - a scrolled `Repeat` asked the wrong row for its handler (10 of 11
    rows);
  - `ui.lastDamage` counted nodes that draw nothing (7.47x on a 1.00x
    frame).
  All three were latent until a layout changed what containers were
  offered. **Two tests had been passing on the chrome**: a "draws only
  text" check credited RichText and PullToRefresh with the header's
  hairline. The contrast gate's first run found three AA failures that
  looked fine by eye, and the visual review found four demos illegible
  in dark mode **before** M56 too. One attribution was corrected before
  it left the machine: a sidebar click was being eaten by an open
  popup's dismissal, not by the `Repeat` defect found while chasing it.
  (T56.0–T56.5), queued
  2026-09-24. The showcase gets a component-gallery layout:
  a grouped sidebar replaces the dropdown picker, the demo sits on a
  canvas card, the readout becomes a status chip, and the chrome follows
  4/8 px spacing and a 12/14/16/24 type scale. **The explanatory note
  under each widget is removed**, and any claim a note held that no
  rustdoc does is moved there first. The one-line blurb stays. A
  contrast gate covers both palettes. The drive scripts follow the new
  chrome and must stay green under both strategies.

- ~~**M57 — `docs/plan-widget-fixes.md`**~~ — **DONE 2026-09-27** on
  branch `m56-showcase-design`: every A, B, C and D item. It fixed what
  `docs/widget-audit.md` found across the showcase's 52 widgets (12
  broken, 35 with flaws), **framework defects in the framework, never
  worked around in the example**. The audit's two tables now carry *fixed
  in* and *checked by* columns computed by `scripts/audit_fixed_in.py`
  from the log and the drives, not written from memory.

  **Phase D is the part worth carrying forward**, seven standing gates:
  every drive action declares whether its consequence shows in pixels
  (D1, `ui.regionHash`); a direct `on_event` test goes through the pass
  or says why (D2, in `just static`); a keyboard-only pass reaches every
  focusable demo by Tab (D3); contrast across every demo in both
  palettes (D4); the live window with the agent is byte-identical to a
  headless render without it (D5); a third pass at scale 2 compares every
  damaged frame with a full repaint (D6); and every leaf speaks or is
  hidden (D7). **D1 found a vacuous drive action on its first run** --
  the Switch's track check passed on a hover shade -- and D7 found layout
  leaves publishing themselves as nameless controls.

  The plan's "What building it found" section records where it was wrong
  (B15's end caret was already right live; B9's "500 px" never existed;
  B26's gate found four hand-written wrappers laying out across their
  offer, M56's derive defect again) and five drive actions that asserted
  the defect they were meant to catch. Not built, with reasons: `Radio<T>`,
  `Slider::on_change`, a rounded clip. Three defects reported from the
  showcase after closing are fixed and recorded in the plan ("Reported
  after closing"): a filtered combobox choosing by unfiltered position,
  a search field clipping its text, and a DataGrid that read as a list
  -- now a sheet the keyboard reaches, which also closes D3's finding.


- ~~**Merge of `m56-showcase-design` into `main`**~~ — **2026-09-28.** Two
  programs had run in parallel from `1717808`: M38's review and M39–M55
  on `main`, M56 and M57 on the branch. 45 files conflicted, and **both
  sides had fixed several of the same defects independently**: physical
  damage (M40/T40.1 and M57/A2), the ring-indexed repeater handler
  (M44/T44.7 and M56), arena rows against the main table's dirty set
  (M41/T41.3 and M57/A5), pixel snapping (M40/T40.3 and M57/A15), the
  press latch (M44/T44.2 and M57/A9), toggles showing their flag
  (M48/T48.4 and M57/C2), a stuck timer on an idle window (M47/T47.1 and
  M57/B40), and keyboards for the choosers, pickers, tree and grid
  (M44/T44.10 and M57/B20–B23, B50, B51). **Where one side's mechanism
  came from a dedicated repair program with its own oracle, it was kept
  and the other side's tests were made to pass against it**; where the
  branch's was the one asked for (the sheet, the combobox's reopening),
  it was kept and `main`'s tests were ported to it with their claims
  intact. The merge commit's body lists every decision. Two of `main`'s
  findings closed at the merge: its dark-palette catalogue test (T54.5)
  reads 0 of 1672 runs under 3:1 now that M57/A8's roles exist, and its
  W1003 fixture depended on the layout bug M56 fixed, so it was rebuilt.
  **`just ci` on the merge commit then failed 15 of 28 steps, and five
  of the causes were defects neither side had alone** — each side's fix
  correct, the pair wrong: the journal's part-signing branch (M48/T48.4)
  skipped the `paints` flag M56 added, so a chosen date damaged nothing;
  the shadow oracle still copied the focus-ring zeroing M57/A6 had
  removed, and reported every pressed control as diverged; a toast's
  countdown (M57/B34) is motion that changes no pixel, and frames that
  present nothing (M53/T53.4) cost nothing, so the window spun at 2,000
  frames a second until an unpresented animating frame waited a display
  interval; the tree's keyboard (M44) walked from the row the *pointer*
  lit, which M57/D1's pre-hover exposed; and `W1007` ignored the
  `content_absent` its leaf checks honoured. The rest was `main`'s
  scripts addressing the old chrome and the branch's drives meeting
  `main`'s damage check for the first time.
  **M61 landed on `main` while that ran**, and merged without a textual
  conflict and three semantic ones: `.lss` could not spell M57/A8's seven
  new palette roles or M57/A16's `disabled` (both refused by
  `scripts/check_lss_setters.sh`), and the showcase's `.lss` mirror named
  `#hr` and `.pair-label`, which M56's redesign removed — so it was
  regenerated from the merged `stylesheet()`, 111 rules held equal by
  value.
Unsequenced: ~~`docs/plan-harness.md`.~~ — **M12 DONE 2026-09-20.**
T12.3 and T12.4 landed 2026-09-04; T12.1 (a locator and expectation API,
`lumen_static_agent::harness::TestApp`) and T12.2 (golden images with a
diff artifact) on 2026-09-20. `docs/results-harness.md` owns the
numbers. The premise held — its substrate was already finished, so it
stayed cheap: what was missing was ergonomics and artifacts, and that is
all that was built. *(The golden gates ran in no recipe until M43/T43.9:
feature unification compiled them to zero tests. `dev-shape` now runs
them per package, `docs/plan-gate-hygiene.md`.)*

**A milestone is not done until `just ci-verify` passes for the commit
that closed it, and its queue entry cites that SHA** (`docs/plan-m38-fixes.md`
§5; M43/T43.13).

Task ids (`T0.6`, `F1.3`, `T16.7`) go in commit subjects. Decision records live beside it in `docs/`; the
design spec is `docs/00-handoff-static-tree.md`, whose DECIDED/SUPERSEDED
markers must move with the code.

## Committing

commit with a clear explanation of what has been done for every task or subtask completed

## Tests must be shown to fail (binding — 2026-09-08)

**A test that has never failed has not been tested.** Before a new test
or gate is committed, break the thing it protects and watch it go red.
Then restore, and watch it go green again. Put what the injection did —
and by how much — in the commit body, because that number is the only
evidence the test works, and nobody can re-derive it from the passing
test later.

Three things the injection has to actually be:

1. **It must break the mechanism, not merely change a value.** Delete
   the guard, invert the condition, remove the clip, stop pushing the
   scope. "The output differs" is not the claim; "the assertion fires,
   by name" is.
2. **An injection that leaves the test green is a finding about the
   test.** Do not weaken the injection until it fails — find why the
   test could not see it, and fix the test. When this happened on the
   GPU damage gate, the test was checking a real property in a situation
   that could not exhibit it: a half-cleared boundary pixel under
   *opaque* content is overwritten by the next command, so the artefact
   never survived to be compared. Rewriting it to damage a region under
   *translucent* content made the same injection produce 23 differing
   pixels.
3. **A gate that refuses bad input must also accept good input.** Check
   both directions in the same session, or the next tightening turns it
   into a gate that refuses everything.

**Three failure shapes account for every vacuous test found on this
repository so far, and none of them looks wrong when you read it:**

- **The setup silently did not happen**, so both sides agree about a
  frame in which nothing occurred. A `Fixed(220)` card in a 300 px
  window put a button at x=256, outside its own parent, so every click
  in an oracle test hit the root and the handler never ran — the oracle
  passed, comparing two identical no-ops (M16/T16.7). An overlay hit
  test declared the popup *before* what it covered, so the ordinary walk
  found it and the overlay path was never exercised (M15/T15.0). Assert
  that the action landed — `handled`, a changed count, a hit on the node
  you meant — not only that the result matches.
- **The bound was computed from the wrong quantity**, so it cannot be
  exceeded. A damage check bounded height at `st[1].size.h` — which is
  the *list's* 2200 px total, not a row's 22 — making the limit 4400 px
  and the assertion unreachable (M16/T16.2). A parity bound of 6% sat
  over a measured 0.01% (M15/T15.12). Print the quantity next to the
  bound so a vacuous one is visible in the output.
- **The gate never runs**, and its silence reads as a pass. It was
  compiled out by feature unification, skipped with exit 0 (no display,
  no adapter, no font), or run against a process it did not launch, such
  as a leftover window on the port. M38 found this shape more often than
  either of the others (`docs/results-deep-review.md` §4). The answer is
  M43's and M42's: a skip exits 77 and fails unless allowed, a test
  binary that lists zero tests must say why, and every live leg checks
  the pid, strategy and features of the process it drove.

**And the injection itself can be wrong.** A `#[no_mangle] pub static`
added to prove the `.bss` gate fires was dropped by the linker, so the
test proved linker GC and nothing else (M15). An awk locale probe
written as `1.5 + 0 != 1.5` never fires, because awk lexes its own
source in the C locale whatever the environment says — only converting a
*string* tests what input parsing does (M16/T16.3). When an injection
passes, suspect it before you trust it.

Rationale: over M15 and M16, **six** tests and gates were found passing
while proving nothing:

| what | why it proved nothing |
|---|---|
| overlay hit test (T15.0) | popup declared before what it covered |
| GPU partial-redraw (T15.12) | passed with `snap_out` deleted |
| backend parity bound (T15.12) | 6% bound over a 0.01% measurement |
| `.bss` ceiling (M15) | 2 KB bound over a measured 0 |
| invalidation oracle (T16.7) | clicks never landed |
| damage height bound (T16.2) | limit 200× the quantity it bounded |
| banded byte-identity (T17.4) | 6 of 10 cases never split |
| the whole suite vs. `Repeat`'s box (M24/T24.2) | no test laid a repeater out inside a narrower parent |
| the whole suite vs. `Repeat`'s fill (M24/T24.2) | no test read a repeater's own background |
| `tests/overlay.rs` dismissal (M24/T24.2) | asserted the popup *stayed open*, never that anything inside it fired |
| every gate vs. a hidden subtree (M24/T24.3) | no tree had ever put an inactive enum variant near the end of the table |
| invalidation oracle vs. hover and press (M25/T25.0) | its session never hovered, so `Boundaries` painted a stale shade on every real frame and the oracle agreed with it |
| a repeater vs. the audit's probe (M27/T27.10) | the fixture pinned the list at `Fixed`, so no probe could change the height the clamp was computed from |
| a tree's "pressing a leaf does nothing" (M27/T27.11) | a leaf has no children, so folding its bit changes nothing anyone can see — neither the row count nor the damage nor the reported changes could move under an injection that let a leaf toggle |
| `a_grid_taller_than_its_box_can_scroll` (M27/T27.11) | its doc named the limits-versus-box distinction; it asserted content > viewport, which is true under both |
| `a_press_hits_the_visible_row_not_the_raw_index` (M27/T27.11) | it pressed the *second* visible row, which is the second raw row whether or not anything below it is collapsed |
| the first `Event::Hover` test (M27/T27.11) | it called `on_event` with a `Hover` in hand, so deleting the synthesis in `dispatch` left it green |
| every test vs. per-widget damage (M27/T27.3) | nothing had ever asked what one interaction costs in pixels, so a click reported the whole window at **1 200×** the widget's own box and 151 test binaries were green |
| every test vs. a container's own background (M27/T27.3) | the journal folded a container's signature from its *children's*, so every container looked busy whenever anything under it moved — **one defect hid the other**, and no test could have seen the second while the first stood |
| `shaping.rs`'s substitution test (M26/T26.17 #13) | it names substitution in its title and never triggers one: every system face has U+2212 and U+2014, so the fall-through it claims to cover is dead. Found by an injection that disabled substitution and left it green |
| M38's review and its re-triage (2026-09-23/24) | 79 rows: 74, plus a dated addendum of 5 -- see `docs/results-deep-review.md` §4, which owns the tally and names each one's injection |

**Three were caught by injecting a fault; three by printing the measured
number beside the bound and seeing the gap.** Neither method is
optional, and reading the code caught none of them — reading is what
wrote them.

**The two added on 2026-09-19 were caught by the third method again, and
they are the strongest case for it yet.** `just widget-drive` asked, for
the first time, what a single interaction costs in pixels — and got the
whole window, on a framework with two milestones of invalidation work
behind it and 151 green test binaries. One of the two was *invisible
while the other stood*: a container's own paint was never signed, and
nothing could notice because the fold made every container report
anyway. **A defect that hides a second defect is the argument for
end-to-end assembly in one sentence** — reading finds neither, and a
unit test for the hidden one cannot be written by someone who does not
know it is there.

**The four added on 2026-09-14 were caught by a third method: using the
thing.** M24/T24.2 built a dropdown on machinery that had shipped in
M15 and been tested pass-by-pass ever since. Three of its four gaps were
visible in the first rendered frame — rows laid out three times too
wide, a panel with no background, a stage that stopped being centred —
and the fourth was a **release-build panic** that appeared the moment a
menu opened over an enum switch, with `just check` green and 134 test
binaries green.

The pattern is not that the tests were bad. Each one tested its pass
directly and correctly. What nothing did was **assemble them** — and an
end-to-end assembly is the only thing that can find a capability which
is present, correct in isolation, and unreachable. `just gpu-parity` and
`just agent-drive` exist because of this, and `just ci` runs every one
of them, with `just live`, against the commit in a clean worktree; a new
capability wants one before it is called done.

**The seventh was caught by a review agent reading the plan for a
*different* feature** (2026-09-10, M18's spec review). `tests/banded.rs`
said its five sizes were "large enough to cross the splitter's threshold
at every one of them"; six of its ten cases used one band, so the
cross-build checksum compared a single-band render with itself and would
have agreed however badly the splitter was broken. The comment was wrong
by a factor of two — `workers_for`'s 250 000 px is the per-*worker*
quantum and splitting needs two workers — which is the "bound computed
from the wrong quantity" shape again, in prose rather than in an
assertion. **A threshold named in a comment is not a threshold that was
checked.** `Canvas::bands_for` now exists so the test asserts its own
setup instead of describing it.

The same review found a *shipped* defect the same way: `Deferring`
overrode seven of `Painter`'s eleven methods and inherited the provided
bodies for the other four, so every `Line` and `Image` painted under an
overlay pass was dropped and every gradient flattened — four commands
in, two out. **A provided method is right for a leaf and wrong for a
wrapper, always**, because a wrapper can always forward and the default
acts on the wrapper instead. When a trait gains a method, every wrapper
of it is a call site the compiler will not show you.

## The book (`book/`, added 2026-09-10)

`book/` is an mdBook: the reader-facing narrative, in teaching order,
planned chapter by chapter in `docs/plan-book.md`. It is in this
repository and not beside it because the doc-currency rule below
requires prose to move in the **same commit** as the behaviour, and a
second repository has no such commit — `docs/book-decision.md` records
the reasoning, the two tooling traps, and the seven injections the gates
were verified with.

Three rules, all of them the existing ones applied to a new surface:

1. **It cites; it does not restate.** A figure in a chapter names the
   `docs/results-*.md` that owns it, in the same sentence. The book is
   the surface most tempted to repeat a number in friendlier prose, and
   that is how a corrected table and its uncorrected paragraph happen.
2. **Code that exists is included; code that only teaches is executed.**
   If the repository already contains the code, the chapter includes it
   by *named anchor* — never retypes it, because a copy drifts. If the
   code exists only to make a point, it lives in the chapter and must
   **run**: a ```rust block with an assertion in it. **```rust,ignore is
   forbidden** and gated: it compiles nothing while rendering exactly
   like a tested block, and it is what a writer reaches for when a
   fragment will not compile alone — where the honest answers are a
   finer anchor plus hidden setup lines, or prose. The first version of
   this rule said "included, never retyped" full stop; it was written
   for a four-chapter book sourced entirely from `examples/`, and no
   example demonstrates gestures, i18n or animation in teachable form.
   A renamed anchor expands to the empty string and `mdbook test` still
   exits 0, and an intra-book link to an unwritten chapter renders as a
   dead link — both silent, both why `scripts/check_book_includes.sh`
   runs in `just static`. Line-number includes are refused, not checked.
3. **A `SUMMARY.md` entry that points nowhere is an error**
   (`create-missing = false`). Unwritten chapters are draft entries.

`just book` builds it and compiles and runs every snippet against the
real rlibs; `just book-serve` reads it locally. Both need
`cargo binstall mdbook`, which is why only the pure-bash include check
gates every commit and the compile gates pre-push.

## Doc currency (binding — adapted from Lumen's D0.7 rule, 2026-08-30)

Any commit that changes public behavior or settles a design question must
update, **in the same commit**:
1. the affected section in `docs/00-handoff-static-tree.md` — the design
   spec for this project (flip an **OPEN** marker to **DECIDED** with a
   one-line pointer to the measurement or decision doc that settled it;
   mark deferrals);
2. any measurement or decision record in `docs/` the change invalidates;
3. **the doc comments on the code you changed.** Added 2026-09-02: of
   twenty drifted claims found by review, *thirteen were in rustdoc* —
   which this rule did not mention, and so did not catch. A comment
   describing machinery in another file is the fragile kind; every
   comment about its own file's behaviour survived M0 intact.

**A rule a commit establishes goes in a doc or a comment; the commit body
keeps the story.** Ten pieces of load-bearing rationale currently exist
only in `git log` — a medium with no index, no search and no update path.
The clearest case: one commit stated the correct mechanism for
`Style.visible` in its body, used it to overturn a design decision, and
left the wrong claim in `style.rs`, where it stayed until a review found
it.

**Numbers are stated once.** In the results doc; cited everywhere else. A
doc comment that must carry a figure carries the citation in the same
sentence. The alternative is what happened to `NodeDesc`'s size and to
the flash argument in `results-safe-prototype.md`: a table corrected and
the prose eight lines below it missed, in the same careful pass.

Decisions backed by measurement get their own `docs/results-*.md` or
`docs/*-decision.md` file, Lumen-style: the number, the threshold it was
judged against, and the verdict — so the gate can say no and the next
reader can see why it did.

Rationale (inherited from Lumen's 2026-07 audit): specs drift ~30% from
code when docs are updated in separate passes. Docs describe reality, not
intent.

Measured on this repo, 2026-09-02: **29 commits touched `crates/*/src`;
13 of them updated no docs — and the split was not random.** Every
`step*`, `agent` and `events` commit complied (16 of 16); every
milestone-sized `feat(T*)` commit did not (0 of 11). The rule holds when
docs and code are written in one sitting and collapses under a large
commit, after which the debt gets paid in a retro-pass — which is the
separate pass the rule exists to prevent. If a commit is big enough that
the docs feel like a second task, that is the signal to split the commit,
not to defer the docs.

The checks in `just static` (the recipe is the list; since M43 each has a
planted-violation self-test in `scripts/selftest/`) mechanise the part a
human reliably misses:
a backticked identifier in a comment must exist in the code, a declared
derive attribute must have a user, a cited `docs/*.md` or bench binary
must exist, every `{{#include}}` in the book must name a live file and a
live anchor, the attribute table must say the same thing in the rustdoc
and in the book, no crate but the reload FFI crate may contain
`unsafe` (`#![forbid(unsafe_code)]` since M43/T43.1), every widget must
implement `observe` or be listed with the reason it has nothing to say
(since M57/A17), and a widget test that hands `on_event` a press or a key
and never pumps must say why (`// on_event only:`, since M57/D2 — that
shape hid A4, A7, A13 and the first `Hover` test). They run before
any compile. The first three read `book/src/*.md` as well as `docs/*.md`,
from 2026-09-10.
