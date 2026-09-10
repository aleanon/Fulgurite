# M20 — the book

> **Status: planned 2026-09-10. Scaffolding landed the same day
> (`3d26e32`): `book/`, three gates, and four chapters written. T20.0
> and T20.1 landed 2026-09-10. Unsequenced — it blocks nothing and
> nothing blocks it, so it is written between milestones rather than
> instead of one.**

*Each chapter is one commit. Task ids go in commit subjects. The rules
the scaffolding established are in `AGENT.md`; the reasoning for the
book being in this repository at all, and the seven injections its gates
were verified with, are in `docs/book-decision.md`.*

---

## 1. What the book is for, and where it stops

There are now four documentation surfaces and they are not
interchangeable:

| surface | answers | shape |
|---|---|---|
| `README.md` | what is this | sixty seconds |
| **the book** | **how do I build something** | **teaching order** |
| rustdoc | what does this call do | reference, per item |
| `docs/` | why is it this way, and what was measured | argument and evidence |

rustdoc is already strong — `crates/lumen-static/src/lib.rs` carries
`Node` versus `Widget`, all seven attributes in one table, the two
`kind_name` conventions and where state lives. The book must not
reproduce it.

**The test for admitting a chapter: does it change what the reader
types?** A chapter that only explains why something is so belongs in
`docs/` and is cited from here. Part 0 is the single, deliberate
exception, and §2 states its terms.

## 2. The four decisions

| | decided | why |
|---|---|---|
| **Scope** | Guide, internals **and** the design argument — all seven parts. | A reader who does not know why the tree is a type writes against the framework rather than with it. The argument is short and it is load-bearing. |
| **Where the argument goes** | Part II, after the tutorial, carrying **no figures** and citing `docs/00-handoff-static-tree.md` for every claim about a decision. | The spec owns that argument, with DECIDED markers that move. A restatement here would be a second copy with no marker and no update path. |
| **Example code** | Grow the three existing examples with anchors. No new crate. | `examples/counter-window` is already the kitchen sink — `app.rs` has a `with =` projection and the two-spacer centring trick, `form.rs` has `TextInput`, `CheckBox`, `Switch`, `Slider`, `ScrollView` and a tab strip built from `Either`. §4 says what it must still grow. |
| **Recording** | This document, with T20 ids. | A place to record why a chapter was cut, which `SUMMARY.md` has no room for. |

## 3. The chapters

Twenty-nine numbered, in seven parts, behind an unnumbered
introduction (mdBook's prefix chapter). ✅ marks what `3d26e32` already
contains.

**The design argument sits second, not first.** Part 0 was drafted ahead
of the tutorial and moved: a reader who has not yet written a tree has
no use for the retained-mode baseline, and a book that opens by arguing
loses the reader who came to build something. Chapter 7 — *who should
not use this* — is the one piece an evaluator wants early, so the
introduction carries a short version of it and chapter 7 carries the
honest one.

### Introduction ✅ — unnumbered

The four surfaces, the cites-don't-restate rule, and two paragraphs of
"is this for you". Includes `minimal:app`.

### Part I — Build something

| # | chapter | teaches | code from |
|---|---|---|---|
| 1 | Your first tree ✅ | tree, state, `#[bind]`, handlers, a stylesheet | `counter` ×4 |
| 2 | State, bindings and handlers ✅ | `#[tree]`; `#[bind]` with a format string, with extra arguments, and with `with =` for a projection; `#[app(after_update)]`; why a handler is a bare `fn` and not a message | `crates/lumen-static/tests/derive_app.rs` — **not** an example. It is the only place all four `#[bind]` forms appear in one struct, and each one is asserted there, so the chapter's snippet is a test `just test` already runs. |
| 3 | Layout | row and column, the three `Length` cases, padding, spacing, cross-axis `Align`, one axis per container; main-axis centring with two `Fill` spacers, since there is no `justify-content` | `counter-window/app.rs`, the `Page` struct |
| 4 | Styling | `Rule`, `Selector`, `Patch`; `#[widget_id]` versus `#[widget_class]`; **the lowercase-leaf / PascalCase-container split in `kind_name`** | `counter:style` |

Chapter 4 earns its place on the naming split alone: `Selector::Kind("label")`
and `Selector::Kind("Ui")` are both correct and nothing in the type
system says which one a given node needs.

### Part II — Why a static tree

| # | chapter | teaches | cites |
|---|---|---|---|
| 5 | The retained-mode baseline | what a pointer-graph tree costs and what it forecloses | spec §2 |
| 6 | Topology in the type ✅ | the idea, and its two constants made operational | spec §3, §5 |
| 7 | What it costs, and who should not use this | monomorphization, the four escape hatches as a *consequence*, and an honest "use something else if" | spec §4, §11 |

This part carries **no figures at all**. The spec owns the argument,
with DECIDED markers that move when the code does; a restatement here
would be a second copy with no marker and no update path.

### Part III — The model

| # | chapter | teaches |
|---|---|---|
| 8 | What a frame does | input, bindings, layout, paint — and why the bindings sit in the middle, which is the reason a handler cannot leave a stale value on screen |
| 9 | The const node table | the flat table, index addressing, and what is keyed on it: the agent surface, the change journal, the ambient audit |
| 10 | Where state lives | sibling fields own the truth, widget fields own the presentation copy, and the measurement cache is why |
| 11 | The component boundary | the unit of reuse, retention, damage and reload, all at once |

### Part IV — The widget set

| # | chapter | code from |
|---|---|---|
| 12 | Leaves: `Label`, `Button`, `Fixed`, `Spacer`, `Image` | `counter`, plus §4's image growth |
| 13 | Taking input: `TextInput`, `TextArea`, `Slider`, `CheckBox`, `Radio`, `Switch` | `counter-window/form.rs` — all present today |
| 14 | Containers and escape hatches: `ScrollView`, `Either`, `Dyn`, `List`, `Repeat`; the `Arena`, and why per-instance styling does not exist | `form.rs` + §4 |
| 15 | Overlays: `Popup`, dropdown, tooltip, modal, context menu — a paint-and-hit *phase*, not a layout mode | §4's overlay tab |
| 16 | Writing your own widget: `Node` versus `Widget`, the object-safe half, `defaults()`, **and the wrapper rule** | §4's custom-widget growth |

Chapter 16's wrapper rule is the most useful page in Part IV and it is
a shipped-defect story: `Deferring` overrode seven of `Painter`'s eleven
methods and inherited the rest, so four commands went in and two came
out. **A provided trait method is right for a leaf and wrong for a
wrapper, always** — `AGENT.md` states it; this chapter is where someone
extending the framework actually reads it.

### Part V — Making it feel right

| # | chapter |
|---|---|
| 17 | Events, focus and keyboard navigation — one input queue, capture, focus ring, tab order |
| 18 | Gestures, touch, drag and drop — `LongPress`, `Pinch`, `Touches`, and why the drag payload is a type parameter |
| 19 | Animation — `Animated`, `Easing`, and why the tick is not a per-node pass |
| 20 | Text — fonts and baking, complex scripts in their own crate, `i18n`, and what shaping costs |

### Part VI — Shipping

| # | chapter |
|---|---|
| 21 | Backends — `Host` and `Canvas` headless, `run()` under winit, `--features gpu`, `LUMEN_BACKEND=software` to force the fallback |
| 22 | Windows, dialogs, clipboard, cursors |
| 23 | Embedded and `no_std` — three additive tiers, why there is deliberately no `no_std` feature, `just thumb` |
| 24 | Hot reload — `#[reload(keep\|fresh\|reset)]`, the swap sequence, enum-migration fallback |
| 25 | The agent protocol — the method set, selectors with teaching errors, `ui.explain`, `ui.waitSettled`, and driving your own app in tests |

Chapter 25 is the differentiator. A framework whose test harness, whose
accessibility tree and whose agent surface are the same object is not a
common shape, and the book is where that becomes usable rather than
merely true.

### Part VII — Under the hood

| # | chapter |
|---|---|
| 26 | Damage and invalidation — why damage is a set of regions and not a bounding box |
| 27 | The rasteriser — the command vocabulary, band-parallel work, and the clip that was the real bottleneck |
| 28 | Performance — the budgets, `LUMEN_PROFILE`, the `just profile*` recipes, and **how to add a measurement that can say no** |
| 29 | What this does not do — the gap list, per framework, and which gaps are decisions |

### Appendices

`Where the decisions live` ✅ · `Attribute reference` (the seven, each
linked to its rustdoc) · `Glossary`.

### Cut, and why

* **A separate "component boundary" chapter** stays folded into 11; it
  is one idea, not two.
* **"Migrating from retained-mode thinking"** is cut. It would be a
  rant with no code in it, and chapter 7 covers the honest version.
* **A tutorial that builds one app across ten chapters** is cut. It
  reads well and it rots as one unit: a change to any early chapter
  invalidates every later one, which is the opposite of what the include
  gate buys.

## 4. What the examples must grow

The decision is to grow the three that exist, not to add a fourth. Six
chapters have no anchor today. `counter-window` already has a tab strip,
so the natural growth is **one new tab per capability**, which keeps the
example an application rather than a gallery of disconnected fragments.

| chapter | needs | where |
|---|---|---|
| 12 Leaves | an `Image` on screen | a tab, or the about panel |
| 14 Containers | a `Repeat` over real rows, and a `Dyn` | `form.rs`, beside the existing scroll body |
| 15 Overlays | a dropdown and a tooltip | a new tab |
| 16 Custom widget | one small widget implementing both halves | a module in `counter-window` |
| 18 Gestures | a drag target | the overlay tab |
| 19 Animation | one animated property | the existing tab transition |

Every addition is gated the way `form.rs` already is: it lands with its
own test, not only as something to point a chapter at.

## 5. Phasing

**v0.1 — a book you can hand someone.** The introduction and chapters
1–4, 8, 21, 25. Build an app, understand the frame, put it in a window,
test it. Three are written; five to go.

**v0.2 — the argument and the vocabulary.** Part II, the rest of Part
III, Part IV, and chapter 29.

**v0.3 — depth.** Parts V and VII, and the appendices.

## 6. Tasks

v0.1 is specified per chapter because it is what gets scheduled. Later
phases are grouped, and each will be split into per-chapter commits when
it is picked up.

**Anchors land with the chapter that consumes them**, never ahead of it.
T20.0 was originally written as an anchor harvest with no prose, and its
own gate contradicted it: an anchor harvested before its chapter is an
orphan by definition. `check_derive_attrs.sh` already requires every
declared derive attribute to have a user; T20.0 is that relation for
anchors, and it makes each chapter's commit carry its own includes —
which is the doc-currency rule applied to example code.

| id | task | done when |
|---|---|---|
| **T20.0** ✅ | The orphan-anchor gate: `scripts/check_book_includes.sh` gains the reverse direction, so an `// ANCHOR:` no chapter includes fails `just static`. | An anchor added to an example with no consumer names the file and the anchor; with it consumed, the gate reports both counts. |
| **T20.1** ✅ | Ch 2 — state, bindings and handlers. | A snippet binds with a format string, with extra arguments and with `with =`, and `mdbook test` runs it. Done: five anchors in `derive_app.rs`, thirteen includes in the book, and **no retyped code** — the three `rust,ignore` blocks the first draft had were replaced by includes or by inline prose. |
| **T20.2** | Ch 3 — layout. | The two-spacer centring trick is shown from `Page`, and the chapter states the `Fixed`-wider-than-parent trap by name. |
| **T20.3** | Ch 4 — styling. | Both `kind_name` conventions appear in one working stylesheet. |
| **T20.4** | Ch 8 — what a frame does. | A snippet demonstrates that a handler cannot leave a stale value on screen — by asserting it, not asserting it in prose. |
| **T20.5** | Ch 21 — backends. | The same tree renders headless and under winit in the chapter, from the two examples that already do it. |
| **T20.6** | Ch 25 — the agent protocol. | A snippet drives a widget by selector and asserts the effect, included from `counter`'s existing `center_of` path. |
| **T20.7** | v0.2 — Part II, chapters 5 and 7. | Every claim about a decision carries its spec section; the chapters contain no figures at all. |
| **T20.8** | v0.2 — Part III remainder (9–11). | — |
| **T20.9** | v0.2 — Part IV (12–16) with §4's example growth. | Each growth lands with a test, in the same commit as the chapter that needs it. |
| **T20.10** | v0.2 — ch 29, the gap list. | It cites the survey rather than restating it, and the survey is current on the day it is cited (§9). |
| **T20.11** | v0.3 — Part V (17–20). | — |
| **T20.12** | v0.3 — Part VII (26–28). | — |
| **T20.13** | v0.3 — appendices, and the attribute reference cross-linked to rustdoc. | The seven attributes appear in one table here and one table in `lib.rs`, and a check fails if they diverge. |

**Sizing.** v0.1's five remaining chapters are a day each including the
anchors. T20.9 is the largest single item because it carries example
growth. T20.13's divergence check is an hour and is worth more than the
appendix it guards.

## 7. What must be shown to fail

Per `AGENT.md`: a chapter's gates are gates, and a gate that has never
refused anything has not been tested.

| gate | injection | proves |
|---|---|---|
| T20.0 forward | rename an anchor the chapter includes | the gate names the chapter and the anchor — the end-anchored version, since the substring version did not |
| T20.0 reverse | add an anchor to an example that no chapter includes | the gate names the file and the anchor: `// ANCHOR: orphan` in `examples/counter`, exit 1 |
| any chapter's snippet | flip an asserted constant | `mdbook test` fails at *runtime*, so the snippet is executed rather than merely compiled |
| T20.4 | remove the binding from the state struct in the snippet | the stale-value assertion fires; if it does not, the chapter is asserting nothing |
| any chapter's links | link to a chapter not yet written | the gate names the file and the href — caught for real on ch 2's forward reference to `frame.md` |
| T20.10 | cite a results doc that does not exist | `scripts/check_doc_paths.sh` names the chapter |
| T20.13 | add an eighth attribute to `lib.rs` and not to the appendix | the divergence check fails |

## 8. The vacuous chapters this will otherwise produce

`AGENT.md`'s two shapes, in the form they take in prose. All three were
observed on this book during the scaffolding commit, not imagined.

**A display-only include is an untested include.** If a chapter includes
an anchor inside a fenced block but nothing in that block *references*
what it included, the include can expand to the empty string and the
snippet still compiles. Measured: renaming one anchor shrank the
introduction's example from 26 rendered lines to 3 while `mdbook test`
exited 0 (`docs/book-decision.md`). **Every chapter's snippet must use
what it includes** — construct the type, call the function, assert the
constant. Where a fragment genuinely is display-only, the include gate
is the only thing standing behind it, which is why that gate runs on
every commit and not pre-push.

**A snippet that compiles is not a snippet that ran.** A block with no
assertion passes `mdbook test` by existing. A chapter teaching a
behaviour asserts that behaviour, or it teaches nothing a reader can
check.

**A retyped snippet is an untested snippet, and it looks exactly like a
tested one.** Chapter 2's first draft had three `rust,ignore` blocks
showing a projection, an attribute and a handler that the anchors above
them already contained. `ignore` compiles nothing, so all three were
free to drift from the code they duplicated — and `AGENT.md`'s rule is
that book code is *included, never retyped*. Where a fragment is too
small to compile on its own, the answer is a finer anchor plus hidden
setup lines, or inline prose. Not a fence that opts out.

**A figure without a citation is a figure that will drift.** Stated once
in `AGENT.md`, restated once here, and enforced by nothing — no gate can
tell a cited number from an invented one. It is the rule most likely to
be broken by a chapter written in a hurry, and the reason Part II is
forbidden figures outright rather than trusted with them.

## 9. One thing to fix before chapter 29 cites it

`docs/competitive-feature-gaps.md` is marked **live** in `docs/README.md`
and states a draw vocabulary of six commands and no image support. Both
were true when it was surveyed on 2026-09-06 and neither was true a day
later: T15.2 and T15.3 landed images, lines and gradients on 2026-09-07.
`Cmd` has nine variants today. The survey needs a dated note or a
re-survey before a chapter cites it, or the book will inherit a number
that the code disagrees with — which is the exact failure the
cites-don't-restate rule exists to prevent, arriving through the
citation instead of around it.
