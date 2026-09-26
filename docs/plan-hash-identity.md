# M46 — what a hash, an id and a table count claim to identify

> **T46.3 DONE 2026-09-26 (early landing).** `concat_table` refuses
> more than 65,535 instances by name and debug-asserts
> `table_is_well_formed`; at `0091266` 65,536 instances gave a root
> child count of 0 and a malformed table. `expand_enum`'s rustdoc says
> `1 + Σ variants`, and its "like `Either`" was stale too.
>
> **Written 2026-09-24.** Closes R2-F2, R2-F3, R2-F4,
> R2-F7, R2-F8 and R2-F9 from M38's deep review
> (`docs/results-deep-review.md`), as re-triaged against `main` at
> `1717808` on 2026-09-24. Where the review and the re-triage differ,
> the re-triage is what this plan follows.
>
> **Phase 3** of the fix queue: after M41 (Boundaries repaired, and a
> shadow oracle that can fire) and M42 (a drive harness that can say
> which binary it drove). Two reasons it waits. T46.1 changes the hash
> every retained frame is diffed on, so the only evidence that it did
> not break retention is `crates/lumen-static-shell/tests/damage_oracle.rs`,
> and the evidence that it did not break the agent journal is the
> drive's damage ratios. Those ratios read the journal that
> `DisplayList::hash_from` signs, not retention (M40 §2.1), so they
> speak for the journal half only. M41 and M42 are what make both
> trustworthy. And T46.2 changes what `Host::invalidate` marks, which
> is Boundaries machinery M41 is still repairing. T46.3 is the
> exception, and says why.
>
> The three defects share one shape. **Something is compared as if it
> were an identity, and it is not one.** A linear fold of the bytes
> stands in for the text. The first node carrying an id stands in for
> all of them. A `u16` stands in for a child count. Each is fine on the
> inputs the tests happened to use, and wrong on an input nobody built.

*Each task is one commit and ends with `just test` green. Task ids go in
commit subjects. Every gate names the injection that must turn it red,
and prints the measured quantity beside its bound.*

---

## 0. The rules for this milestone

1. **The hash is on every frame's path, so it is priced before it is
   built.** `display::fold` runs for every byte of every text command
   the retention stream sees, every frame. §5 has the kill rule, written
   before T46.1, with eight runs a side under the bench lock. If it
   fires, T46.1 ships the corrected claim and not the new hash.
2. **A hash is described by what it guarantees, not by a probability
   that assumes random input.** The 10⁻¹³ figure in `retain.rs` and
   `docs/results-retention.md` is true for text nobody chose and false
   for text somebody did. After this milestone the docs say which.
3. **The fixtures that collide today stay in the tests as fixtures.**
   They are solved pairs for the current fold, so they are the one input
   guaranteed to exercise the defect. A gate that only uses them would
   pass for any other fold, though, so each gate also has a pair the new
   function must separate for a structural reason (a length prefix, a
   NUL prefix).

## 1. What is wrong, at 1717808

### 1.1 Text identity is a linear fold (R2-F2, R2-F3, R2-F4)

`display::fold` is `h * K + v` (`crates/lumen-static/src/display.rs:52`).
It is used for text in three places: `Recording::text_weighted` folds
each byte into the command's hash (`crates/lumen-static/src/retain.rs:860-867`),
`DisplayList::hash_from` does the same for the agent journal's node
signatures (`display.rs:965-975`), and `FoldedText::fold_of` folds a
label's bytes into the memo key for `Button`, `Badge` and `Chip`
(`crates/lumen-static/src/text.rs:224-225`). Because the fold is linear,
two strings of the same length collide whenever a small integer
combination of their byte differences vanishes mod 2⁶⁴. The review
solved for such pairs, and a prefix does not help: the difference stays
linear whatever was folded before it.

- **Retention (R2-F2).** Under the default `Host<_, Incremental>`, a
  label changed to a colliding string reports no damage and leaves the
  old pixels on screen (R2-F2; `docs/results-deep-review.md`). The
  re-triage re-ran the review's probe unchanged at `1717808` and got
  the same result: the control gives a damage region and no stale
  pixels, the colliding string gives no damage and a stale label.
  `damage_oracle.rs` cannot see it, because its inputs are integer
  counts that never collide.
- **Measurement (R2-F3).** `FoldedText` starts from 0 and folds no
  length, so `"x"` and `"\0x"` have the same key, as do the solved
  same-length pairs. `Button`, `Badge` and `Chip` then serve a stale
  width, and paint centres on that width (R2-F3; same doc).
- **The candidate fix and its price (R2-F4).** The plan's own candidate,
  folding in the length and finalising, does not separate same-length
  pairs. A word-at-a-time nonlinear mix does, and the review measured
  it as faster than today's per-byte fold at 60 B, at a small cost per
  `FoldedText` hit (R2-F4; same doc; the numbers were taken under load).
  The verifier's caveat stands: an unseeded mix of that shape separates
  the solved pair, and that is all it proves. The rustdoc on
  `FoldedText` (`text.rs:202`) still prices the memo against
  `docs/bindings-decision.md` T7.3's 4.8 ns, which is a
  `Label::layout` hit through `Measured` with no fold at all.

The probability claim is at `retain.rs:45-49`, `docs/results-retention.md`
("What it costs"), and historically `docs/plan-embedded.md` §T13.8.

### 1.2 An id that names two nodes acts on one (R2-F7)

`Host::invalidate` and `Host::restyle_subtree`
(`crates/lumen-static-shell/src/lib.rs:629` and `:691`) resolve the id
through `index_of_wid`, which returns the first match, and then return
`true`. Their `#[must_use]` strings speak only of "matched no node".
`count_wid`'s own rustdoc (`crates/lumen-static/src/style.rs:1191-1202`)
names the consequence, "a `Host::invalidate` that marks one of two,
forever", and nothing acts on it.

**Its reach grew with M34.** With `Boundaries<256>` the default, a
one-of-two mark is a stale second instance in the default
configuration, not a no-op. The re-triage re-ran the review's probe at
`1717808` and found one new fact: in a debug build, `restyle_subtree`
on a duplicated id panics in `cascade_subtree`'s witness
(`style.rs:1368-1373`), and the message blames the sheet ("the sheet
changed a rule that matches outside the subtree") rather than the
duplicate id. `W1011` warns about duplicated ids in dev and agent
builds only.

### 1.3 Table-shape claims that nothing checks (R2-F8, R2-F9)

- **R2-F9.** `assemble::concat_table` writes `count as u16` into the
  root's child count (`crates/lumen-static/src/assemble.rs:29`), with
  no bound and no call to `table_is_well_formed`. Above 65,535
  instances the count wraps. At 65,536 it is 0 on a node marked
  `CONTAINER`, and the observe walk then lists no children. The
  re-triage re-ran the probe: at 70,000 the root's child count is 4,464
  and the table is not well formed. The rustdoc that says runtime
  assembly "calls the same fn" is at `crates/lumen-static/src/widget.rs:379`.
- **R2-F8.** `expand_enum`'s rustdoc
  (`crates/lumen-static-macros/src/lib.rs:1341-1344`) says an enum
  contributes **one** table node and keeps variants off-table, so
  `NODE_COUNT` does not depend on the variants. The same file at
  `:1393` says every variant gets its own index range, `1 + sum`
  (F4.3), which is what the derive emits. The paragraph also says "like
  `Either`"; T46.3 checks whether that half is still true too.

## 2. Decisions

### 2.1 What the new text hash guarantees (T46.1)

The fix is one function in `display.rs` that hashes a byte string: it
seeds with the length, mixes a word at a time with a nonlinear step,
and finalises. `Recording::text_weighted`, `DisplayList::hash_from` and
`FoldedText::fold_of` all call it. The scalar fields (rect, size,
weight, colour) keep `fold`: they are not attacker-chosen, and changing
them would move every hash for no gain.

The real choice is whether that function is **keyed**.

- **A. Unkeyed mix with a length seed and a finaliser.** It removes the
  two collisions that exist today by construction: the linear solve and
  the NUL prefix. A determined attacker with the source can still
  search for a collision. The review's numbers say it is cheap.
- **B. Keyed, a seed chosen per process.** The re-triage recommended
  this "since it costs one load". **At `1717808` it is not one load.**
  The core is `no_std` with no entropy source. The embedded target,
  `thumbv7em-none-eabihf`, has no 64-bit atomics, so a global seed
  cannot be set once and read everywhere. And `FoldedText::fold_of` is
  a free function called from a widget's `layout`, with no context to
  carry a seed. A seed would have to be plumbed from the host through
  `LayoutCx` and into `Recording`. And a seed on a non-cryptographic
  multiply-rotate mix is not a security property on its own: it raises
  the cost of a search without bounding it.
- **C. A keyed PRF (SipHash-1-3) with a host-provided key.** The only
  option under which "a collision needs luck" holds for crafted text.
  It costs the plumbing of B plus a slower function.

**Recommended: A, with C measured beside it in the same bench.** T46.0
writes the bench with all three arms. If C passes §5's kill rule it is
taken instead of A, and the plumbing becomes part of T46.1. Either way,
the doc sentence changes. With A it reads: a collision needs
deliberately constructed text, the retention stream is not a security
boundary, and an application that displays hostile text can only keep a
stale label on screen, which is R2-F2's verified reach. The 10⁻¹³
figure is kept, labelled as the probability for text nobody chose.

**What A breaks.** Every hash value changes, so any hash persisted
across a dylib swap compares unequal once. The hashes live in the host's
retention state and in `FoldedText` memos. Unequal means damage or a
re-measure, never a skipped frame, so the failure direction is safe.
T46.1's commit states this.

**Rejected: comparing the bytes.** Exact, but a `FoldedText` would then
own a copy of its text, which is a heap allocation per text change. The
M26/T26.8 budget (`docs/results-frame-and-types.md`) exists to keep that
out of the frame.

### 2.2 An id that matches twice (T46.2)

- **A. Act on every match.** `invalidate` marks every subtree carrying
  the id; `restyle_subtree` re-cascades each, then runs the debug
  witness once over the union. This is what styling already does:
  `examples/glasswall` styles four intentional duplicates by selector.
- **B. Refuse.** Return a result naming the count, mirroring the
  compile-time refusal `wid_index!` gives a const id (M26 Phase C).
  Consistent, but it breaks every caller that relies on "an id selects
  all of them", which is how styling reads an id today.

**Recommended: A.** The `#[must_use]` string then says what `true`
means: at least one node matched, and every match was acted on.
Separately, and under either option, the debug witness in
`cascade_subtree` checks `count_wid` before it blames the sheet, and
names the duplicate id when there is one. **What A breaks:** nothing
that works today. A caller that relied on the first match alone was
relying on the defect.

## 3. Tasks

### T46.0 — the gates first

Each finding becomes a check that is **red at `1717808`**, committed
marked expected-to-fail until its task lands, with its red output in
the commit body, as M35/T35.0 and M36/T36.0 did.

1. `crates/lumen-static-shell/tests/text_hash_collision.rs`, promoted
   from the review's `r2_collision.rs` (probe `ae9125b`). Through
   `Host<_, Incremental>` with a bound `Label<String>`. The control
   string (same length, not colliding) must give a damage region and
   zero stale pixels. The solved colliding string must give a damage
   region too. **Red today:** no damage, and a stale label, printed as
   `stale_pixels N / bound 0`. The setup asserts that the label holds
   the new text and that the list is non-empty, so a frame in which
   nothing happened cannot pass.
2. `crates/lumen-static-widgets/tests/folded_text_identity.rs`, promoted
   from `r2_folded.rs` (probe `7cadfa6`). For `Button`, `Badge` and
   `Chip`, under `BlockMetrics` and under a proportional measurer: the
   width served after a text change must equal a fresh widget's, for
   `"x"`→`"\0x"`, `"x"`→`"\0\0\0x"` and the solved same-length pair.
   Printed as `served w / correct w`. **Red today** on all three
   widgets. The control row (a string that does not collide) is equal
   both ways.
3. **The price, written down before the change (R2-F4).** A bench
   binary in `benches/src/bin`, promoted from the review's `r2-hash`
   bench (probe `b546755`), with arms for today's fold, length plus a
   finaliser, the unkeyed word mix (§2.1 A) and a keyed PRF (§2.1 C),
   at 9 B and 60 B. It prints min/median/max per arm and the load
   average, and runs **eight runs a side**, pinned, under the bench
   lock. It also asserts, per arm, whether the solved pair and the
   NUL-prefix pair collide. **Red today** for the arm that stands for
   the current code and for length plus a finaliser (`collision
   survives=true`). §5's thresholds are printed beside the numbers.
   `--bin retain` gains a text-heavy arm (labels of realistic lengths)
   for the retention half of the kill rule.
4. `crates/lumen-static-shell/tests/duplicate_id_host.rs`, promoted from
   `r2_dup.rs` (probe `9823e46`). An `Inner { #[widget_id("leaf")] Label }`
   used twice under the default `Host`. `invalidate("leaf")` must leave
   both instances dirty; `restyle_subtree` must give the new font size
   at both, and must not panic in debug. It prints each instance's
   state. **Red today:** `instance 4 clean=true`, and in debug the
   witness panic that blames the sheet.
5. `crates/lumen-static/tests/assemble_bounds.rs`. `concat_table(65_535, …)`
   is well formed with child count 65,535. `concat_table(65_536, …)`
   must panic with a message naming the limit (`#[should_panic]`). A
   second test, without `should_panic`, asserts child count == count at
   65,536, so that the gate cannot be satisfied by panicking for the
   wrong reason. It prints the child count beside the count. **Red
   today:** no panic, and a child count of 0.
6. **R2-F8 has no red state**, and the commit says so. The derive is
   right and its rustdoc is wrong, so a test of the derive passes today.
   `crates/lumen-static/tests/table_invariants.rs` gains an assertion
   that a three-variant enum's `NODE_COUNT` is `1 +` the sum of its
   variants' counts. **Injection:** make the derive emit one node for
   an enum, and the assertion fails. That is the only direction it can
   show.

**Effort:** M. **Depends on:** M41 (the oracle redesign), because item 1
is a damage test and item 4 exercises `Boundaries` marks. The bench
lock is the review's convention (`docs/plan-deep-review.md` §3), not
something M43 builds; no plan in M39–M55 adds a recipe for it.

### T46.1 — one text hash, priced

**Closes:** R2-F2, R2-F3, R2-F4.

**Change.** One byte-string hash in `crates/lumen-static/src/display.rs`,
per §2.1's decision. `Recording::text_weighted` (`retain.rs:860-867`),
`DisplayList::hash_from` (`display.rs:965-975`) and `FoldedText::fold_of`
(`text.rs:224`) call it in place of their per-byte loops.
`FoldedText::fold_of` stays public, so a test keys on the same
function rather than on a second copy of the arithmetic. Correct the
`text.rs:202` citation so it prices the memo against the `Badge`/`Chip`
"layout, unchanged" arm of `--bin bindings`, not T7.3's
`Label`/`Measured` figure.

**Gate.** T46.0 items 1–3 go green. Item 1 prints a damage region and
`stale_pixels 0 / bound 0` for the colliding string. Item 2 prints
`served == correct` on every row. Item 3's bench shows the chosen arm
with `collision survives=false` for both pairs, and its median inside
§5's thresholds. A core unit test asserts that the new function
separates the solved pairs and the length-prefix pair (`"x"` against
`"\0x"`). `damage_oracle.rs` stays green, and so does
`crates/lumen-static/tests/text_memo.rs`'s "a still frame measures
nothing", which shows the memo still hits.

**Injection.** Put the per-byte loop back in
`Recording::text_weighted` only: item 1 fires with no damage and a
stale label, and item 2 stays green, which shows each gate covers its
own call site. Put it back in `FoldedText::fold_of` only: item 2 fires
on `Button`, and item 1 stays green. Swap in length plus a finaliser as
"the fix": the NUL-prefix rows of item 2 go green and the same-length
rows stay red, which is R2-F4's claim demonstrated in the gate.
**Accepts good input:** the control rows in items 1 and 2 are equal
before and after, and `text_memo.rs` stays green.

**Docs in the same commit.** `retain.rs:45-49` and
`docs/results-retention.md` "What it costs": the probability restated
per §2.1, with the new section's numbers (the bench's table, the load
average, the kill rule and its verdict). `docs/plan-embedded.md`
§T13.8: a one-line pointer, since it is a record of what was decided
then. The rustdoc of `fold` and of the new function, and `FoldedText`'s
"the key carries a fold of the bytes", which must say what identity the
fold gives. `docs/bindings-decision.md` T7.3: a note that 4.82 ns is
`Label` through `Measured`. `docs/00-handoff-static-tree.md` §11's
performance calibration only if it cites the retention figure (it does
not at `1717808`; check).

**Effort:** M (S of code, the rest measurement). **Depends on:** T46.0;
M41; M43/T43.13's runner, for the gates (the bench lock is T46.0's
note, not M43's).

### T46.2 — an id acts on every node it names

**Closes:** R2-F7.

**Change.** Per §2.2 A. In `Host::invalidate` and
`Host::restyle_subtree`, walk every index whose `wid` matches, the way
`count_wid` already walks them. `invalidate` marks each match's
`subtree_range`. `restyle_subtree` runs the scoped cascade for every
root first, and only then the debug witness, over the union of the
ranges. Checking after each root would compare the first root's result
against a full cascade that has already restyled the second, and would
fire for a correct call. `cascade_subtree`'s witness asks `count_wid`
before it blames the sheet: when the root's id is duplicated, the
message names the id and the count. The `#[must_use]` strings say what
`true` means.

**Gate.** `duplicate_id_host.rs` (T46.0 item 4) is green in debug and
release. Both instances are dirty after `invalidate`, both have the new
font size after `restyle_subtree`, and nothing panics. It prints
`[2] dirty, [4] dirty` beside `bound: both`. A frame is pumped after
the invalidate under `Boundaries<256>`, and the second instance's
painted text is compared with a fresh host's, so the stale-frame half
of the defect is checked and not only the mark.

**Injection.** Resolve through `index_of_wid` only, as today: the gate
fires with `instance 4 clean=true`. Restore the per-root witness check:
the debug leg panics on a correct call, which shows the union is load-
bearing. **Accepts good input:** a unique id marks exactly one subtree
and returns `true`, and a missing id returns `false`, both unchanged.

**Docs in the same commit.** The rustdoc and `#[must_use]` strings of
`Host::invalidate` and `Host::restyle_subtree`. `count_wid`'s rustdoc
(`style.rs:1191-1202`), which will no longer describe "marks one of
two, forever" as current. The `W1011` explanation in the agent's audit
rustdoc, if it says a duplicated id is unaddressable at runtime: it is
now addressable as a set.

**Effort:** S. **Depends on:** T46.0; M41 (the marks it adds are
`Boundaries` marks, and M41 is what makes them trustworthy).

### T46.3 — table counts that are checked

**Closes:** R2-F9, R2-F8.

**Change.** In `assemble::concat_table`, assert `count <= u16::MAX`
with a message naming the limit and the count, and
`debug_assert!(table_is_well_formed(&table))` on the result. Once M41's
R2-F5 lands its two new clauses in `table_is_well_formed`, the same
debug assertion also covers pre-order and sibling termination, with no
further change here. Rewrite `expand_enum`'s rustdoc
(`crates/lumen-static-macros/src/lib.rs:1341-1344`) to match F4.3: the
enum contributes one node plus every variant's subtable, each variant
has its own index range, and inactive variants are hidden, not
off-table. Check the "like `Either`" half of the sentence against
`Either`'s own table while there, and fix it too if it is also stale.

**Gate.** `assemble_bounds.rs` (T46.0 item 5) goes green: 65,535 is
well formed, 65,536 panics with the message, and child count equals
count wherever it does not panic. `table_invariants.rs`'s enum
assertion (T46.0 item 6) stays green.

**Injection.** Remove the assert: the `should_panic` test fails, and
the child-count assertion fails with 0. For R2-F8, as T46.0 item 6.
**Accepts good input:** every runtime-assembled table in the workspace
still passes the new debug assertion (`crates/lumen-static/tests/assembled_seeds.rs`
and the agent's hatch tests run in debug).

**Docs in the same commit.** The `assemble.rs` rustdoc (the instance
limit). `widget.rs:379` ("runtime assembly … calls the same fn"): say it
is now also checked. The book's enum section in `book/src/containers.md`
or `book/src/topology.md`, if either says an enum is one node.

*What building found (2026-09-26).*

- **T46.0 item 5 contradicts itself.** "`concat_table(65_536)` must
  panic" and "a second test, without `should_panic`, asserts child
  count == count at 65,536" cannot both pass. What the second test is
  for — that the gate cannot be satisfied by panicking for the wrong
  reason — is carried by `should_panic(expected = "65535")` instead,
  and the accept test checks child count == count at 0, 1, 2, 1,000
  and 65,535. The injection that removes the bound shows why the
  expected string matters: the new debug assertion then panics first,
  on a malformed table, and the test reports "panic did not contain
  expected string".
- **The "like `Either`" half was stale too.** `Either` is
  `1 + A + B` on the table since its own rework, not one node with an
  arena. The rustdoc now says both.
- **R2-F8's injection cannot reach the test.** Making the derive
  declare an enum `NODE_COUNT = 1` is refused at compile time by the
  derive's own length asserts ("the spliced table's length is not this
  tree's NODE_COUNT", E0080, from M26/T26.16). The runtime assertion
  in `table_invariants.rs` is kept as the readable statement of the
  count; the compile-time assert is what fires first.
- `crates/lumen-static/tests/table_invariants.rs` is also edited by M41
  (T41.6); this task appends one test at its end.

**Effort:** S. **Depends on:** nothing it cannot land without. **This
task may land before Phase 3**, because it touches no retention, no
marks and no live gate: an assertion in a constructor and a rustdoc.
M41's R2-F5 improves what its debug assertion catches but is not needed
for it to be correct. T46.0 as a whole waits for M41, so an early T46.3
carries its own gates, T46.0 items 5 and 6, which are table tests and
need neither M41 nor M42.

## 4. Boundaries of this plan

- **Not the non-text folds.** The agent journal's `fingerprint` and its
  style signature (`crates/lumen-static-agent/src/journal.rs:118-125`,
  `:195-203`) repeat `fold`'s constant for table shapes and style
  floats. Those inputs are not chosen by whoever supplies displayed
  text, so a linear collision there needs an adversary the milestone
  does not model. They stay as they are, and T46.1's doc says the new
  function covers text only.
- **Not a security boundary.** Under §2.1 A, the retention stream is
  described as resisting accidental and algebraic collisions, not
  adversarial search. If a later milestone puts displayed text from an
  untrusted source under a claim stronger than "can keep a stale label
  on screen", it takes §2.1 C and its plumbing.
- **Not a refusal of duplicated ids.** §2.2 keeps duplicates legal and
  addressable as a set. `W1011` stays a dev-build warning.
- **R2-F5** (`table_is_well_formed`'s missing clauses) is M41's, and
  T46.3 only benefits from it.

## 5. Kill rules and risks

**Kill rule for T46.1, written before it is built.** The bench is
T46.0 item 3 plus `--bin retain`'s text-heavy arm and the `Badge` and
`Chip` "layout, unchanged" arms of `--bin bindings`. Eight runs a side,
alternating builds, pinned to one core, under the bench lock, with the
load average printed before and after. The statistic is the difference
of medians, printed beside both arms' min and max.

- **K1.** A `FoldedText` hit may cost at most **+0.5 ns** at 9 B and at
  60 B, against today's fold.
- **K2.** `--bin retain`'s per-command cost under `Incremental`, on the
  text-heavy arm, may not rise by more than the noise floor. The floor
  is measured the same way, today's build against itself, before T46.1
  is built, and printed in the commit.
- **K3.** The leaf `no_std` profile's `.text` may not grow past the
  headroom `docs/results-embedded.md` states. The delta is recorded
  either way.

**If K1, K2 or K3 fires** for the chosen arm and for every other arm
that separates both pairs, T46.1 does not ship a new hash. It ships the
restated claim (the 10⁻¹³ figure labelled as holding for text nobody
chose), the corrected `text.rs:202` citation, and the gates from T46.0
items 1 and 2 marked as known failures with the numbers beside them.
The results doc records the verdict. The review's numbers predict the
unkeyed mix passes all three, since it is faster than today's fold at
60 B. That is a prediction, and the rule is what decides.

**Risks, and what would reveal each.**

- *The bench measures a function the frame does not call.* Revealed by
  checking the symbol: the bench must call `FoldedText::fold_of` and a
  `Recording` through the crate's public path, not a copy of the
  function pasted into the bench. The review's bench was a copy.
- *A load average that makes eight runs meaningless.* Revealed by the
  printed load average. AGENT.md's Phase D lesson is that four runs
  could not separate a real effect twice and eight could. A run on a
  machine loaded beyond its core count is re-run, not reported.
- *Mixed hash versions across a reload.* A host built with one hash
  compares against a dylib built with another, and every text command
  compares unequal once. Revealed by `just hot-window-drive` or M37's reload
  drive showing one full-damage frame after a swap and no more. It is
  the safe direction, and T46.1's commit says so.
- *A caller relying on `invalidate` marking only the first match.*
  Revealed by `just widget-drive` damage ratios moving above 1.00× on a
  tree with duplicated ids. None is known at `1717808`: the harness
  refuses to drive a tree carrying `W1011`.

## Appendix: finding → task

| ID | sev now | status | task |
|---|---|---|---|
| R2-F2 | S2 | OPEN | T46.1 |
| R2-F3 | S2 | OPEN | T46.1 |
| R2-F4 | S3 | OPEN (read only) | T46.1 |
| R2-F7 | S3 | OPEN | T46.2 |
| R2-F8 | S4 | OPEN (read only) | T46.3 |
| R2-F9 | S4 | OPEN | T46.3 |
