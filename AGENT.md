## Work queue

The current plan is `docs/plan-invalidation.md` (M16, T16.0–T16.14) —
the queue from the 2026-09-08 performance review. **All of it landed on
2026-09-08**; its header records what was built, what was decided
against, and what it measured.

Shipped and kept for their reasoning: `docs/plan-architecture-v2.md`
(M0–M12), `docs/plan-review-fixes.md` (F0–F7),
`docs/plan-embedded.md` (M13), `docs/plan-desktop.md` (M14),
`docs/plan-capabilities.md` (M15), `docs/plan-invalidation.md` (M16).
Not started:
`docs/plan-widgets.md` T11.5, `docs/plan-harness.md`.

Task ids (`T0.6`, `F1.3`, `T16.7`) go in commit subjects. Decision records live beside it in `docs/`; the
design spec is `docs/00-handoff-static-tree.md`, whose DECIDED/SUPERSEDED
markers must move with the code.

## Committing

commit with a clear explanation of what has been done for every task or subtask completed

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

Four checks in `just static` mechanise the part a human reliably misses:
a backticked identifier in a comment must exist in the code, a declared
derive attribute must have a user, a cited `docs/*.md` or bench binary
must exist, and the core must contain no `unsafe`. They run before any
compile.
