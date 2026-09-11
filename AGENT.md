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

- **M22 — `docs/plan-liquid.md`** (T22.0–T22.4): the rest of the liquid
  material — dispersion, adaptive tint, press deformation, the
  smooth-minimum morph, and the GPU backend. The first four keep it
  panel-sized; **only the fifth changes what it can be used for**, and
  it is last because every task before it changes the shader the parity
  gate compares.

Unsequenced: `docs/plan-harness.md`.

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

**Two failure shapes account for every vacuous test found on this
repository so far, and neither looks wrong when you read it:**

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

**Three were caught by injecting a fault; three by printing the measured
number beside the bound and seeing the gap.** Neither method is
optional, and reading the code caught none of them — reading is what
wrote them.

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

Six checks in `just static` mechanise the part a human reliably misses:
a backticked identifier in a comment must exist in the code, a declared
derive attribute must have a user, a cited `docs/*.md` or bench binary
must exist, every `{{#include}}` in the book must name a live file and a
live anchor, the attribute table must say the same thing in the rustdoc
and in the book, and the core must contain no `unsafe`. They run before
any compile. The first three read `book/src/*.md` as well as `docs/*.md`,
from 2026-09-10.
