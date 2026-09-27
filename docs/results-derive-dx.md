# Results: M55 — the derive, the lints and the docs

`docs/plan-derive-dx.md` owns the plan; this file owns M55's numbers.
Most of M55 is refusals and prose, so there are two measurements.

## T55.5 — what the cascade's repair costs

§5's kill rule: "the sanitiser costs a measurable share of a cascade",
revealed by `benches --bin cascade`'s `cascade_deep` arm, eight runs a
side; if the delta is outside the arm's spread, the sanitiser moves out
of the per-node path.

**What shipped does no per-node work on a clean sheet, by construction.**
The cascade asks each rule once per cascade whether it carries a length
that cannot be laid out (`Patch::needs_repair`, inside the specificity
scan the cascade already made), and only when one does it runs a
separate repair pass over the nodes. The per-node loop is the loop that
ran before M55, token for token. The added executed work on a clean
sheet is one predicate per rule per cascade: 27 on the showcase-shaped
sheet, against 901 nodes × up to four passes.

**The clock could not see that, in either direction.** Every row below
is eight interleaved runs a side under the bench lock, quiet unless
marked, median (min–max) ns on the showcase-shaped sheet, where the
disagreement was largest; "before" is the parent commit's `style.rs`,
agent and shell restored and rebuilt.

| build compared | this build | the other | delta |
|---|---:|---:|---|
| repair inside `apply_to` (first design) vs repair short-circuited | 45 029 (44 743–45 174) | 44 224 (43 955–44 362) | +1.8%, outside |
| per-sheet scan, `sanitized()` copy per application vs before | 42 950 (41 968–43 882) | 42 565 (41 392–42 797) | +0.9%, inside |
| the same, vs itself with the scan deleted (strictly less work) | 43 178 (42 562–43 607) | 48 167 (47 644–48 545) | **−10%, outside** |
| per-node repair against kind defaults vs before | 44 775 (44 704–44 850) | 42 557 (42 463–42 756) | +5.2%, outside |
| a separate pass repairing against the kind's defaults vs before | 46 908 (46 338–48 514) | 41 112 (40 631–42 567) | +14%, outside |
| the same, **both built with functions and blocks aligned** (`-C llvm-args=-align-all-functions=6 -align-all-nofallthru-blocks=5`) | 41 802 (39 827–42 105) | 46 056 (44 906–47 133) | **−9%, outside** |
| **shipped** (a cold pass against type defaults) vs before | 42 305 (42 028–73 903) | 42 633 (42 342–42 771) | −0.8%, inside |
| **shipped** vs before, both aligned | 43 458 (42 140–43 765) | 47 015 (45 134–47 565) | −7.6%, outside |

On the whole-table arm the shipped build measured 13 665 (13 076–25 317)
against 13 134 (12 786–13 683) plain, inside the spread, and 12 636
(12 241–13 297) against 13 377 (12 157–16 312) aligned.

The spreads are tight within a session (the shipped plain row's maximum
is one outlier run) and the medians swing ±10% between builds, **including a build with strictly less work measuring
10% slower** and the same pair reversing sign once alignment is pinned.
That is the code-alignment effect M26's Phase D recorded (finding 9's two
call sites differed by 20×), and it is larger than anything the repair
could cost: a delta outside the spread here is a fact about the build,
not about the sanitiser.

**Verdict: the kill rule does not fire.** It was written to catch a
sanitiser on the per-node path; the first design put it there (+1.8%,
outside the spread) and it moved. What shipped has none, and the one
comparison that controls for layout measures it faster, not slower. The
firmware pays for the code, not for the calls: `docs/results-embedded.md`,
M55/T55.5.

## T55.6 — does anything still escape a parent that does not clip?

§2.3 decided W1003 by probe, after T55.5 removed negative padding, over
the three candidate constructs, each in a 200 × 100 window
(2026-09-27, a scratch test in `crates/lumen-static-agent`, not kept):

| construct | escapes its parent? |
|---|---|
| a `Fixed` of 1000 × 1000 in a column | no — clamped to the 200 × 100 it was offered |
| a 1000 × 1000 `Fixed` in a `stack` | no — clamped the same way (and a stack's children are W1004's) |
| a `wrap` of two 500 px `Fixed`s | no |
| **a row given `width: Fixed(50)`, holding two labels of `width: Fixed(300)`** | **yes** — each label is laid out against the 200 px the row was offered, not its own 50 px: `[0, 100, 200, 100]` in `[0, 100, 50, 100]`, and the second at x = 200 |

**Verdict: W1003 ships.** The escaping construct is a container sized
smaller than its limits — the plan's "a minimum larger than the parent's
maximum", from the parent's side — and its children are painted outside
it and unreachable by the pointer, which is what W1003 was reserved to
report. `tests/lint.rs` keeps it as `w1003_fires_on_escape_without_clip`
(two findings, one per label, at 50 px against 80 px labels), beside
`w1003_quiet_inside_scrollview` (content 64 px tall in a 40 px scroller,
0 findings). The layout behaviour itself is not changed here.
