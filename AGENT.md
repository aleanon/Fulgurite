## Work queue

The current plan is `docs/plan-architecture-v2.md`; **the review-fix
milestones in `docs/plan-review-fixes.md` (F0–F7) run before its M7.**
Task ids (`T0.6`, `F1.3`) go in commit subjects. Decision records live beside it in `docs/`; the
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
2. any measurement or decision record in `docs/` the change invalidates.

Decisions backed by measurement get their own `docs/results-*.md` or
`docs/*-decision.md` file, Lumen-style: the number, the threshold it was
judged against, and the verdict — so the gate can say no and the next
reader can see why it did.

Rationale (inherited from Lumen's 2026-07 audit): specs drift ~30% from
code when docs are updated in separate passes. Docs describe reality, not
intent.
