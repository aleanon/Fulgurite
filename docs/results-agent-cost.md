# What the agent costs the frame it observes — measured

*2026-09-02, plan `docs/plan-review-fixes.md` F2. Harness:
`cargo run --release -p benches --bin agent`, batched (`benches/src/lib.rs`
`bench`, 21 blocks, floor printed). Machine: i9-13900KF, governor
`powersave`, turbo on, `tsc`, rustc 1.94.0.*

## The claim under test

`crates/fulgurite-agent/src/audit.rs`'s module doc adopts three scars
from Lumen's own post-mortem rather than re-earning them. The third:
*"Lumen's `lint()` was re-shaping every text node and deep-cloning the
tree per call. Here nothing allocates a snapshot at frame time."*

A review on 2026-09-02 measured `Agent::frame_done` at **117× the frame
it observes** at 10k nodes — 13.4 ms against a 115 µs frame, 17,344
allocations, 5.6 MB — and found the third scar re-earned in the file that
names it. All four causes are fixed; this records the numbers.

## Results

901 nodes (`benches::Ten10`). Two viewports, because the audit's cost
depends on how many findings a frame legitimately carries: a tall one
where almost nothing is flagged, and a 900 px one where 132 nodes are
genuinely below the fold.

| | before | after |
|---|---|---|
| frame (layout + paint) | 7.3 µs | 7.3 µs |
| `frame_done`, few findings | — | **45.7 µs** |
| `frame_done`, 132 findings | 133.5 µs | **53.7 µs** |
| allocations / `frame_done`, few findings | 3 collections | **0** |
| allocations / `frame_done`, 132 findings | 3 + ~264 | **0** |
| text measurements / idle frame, agent attached | every label | **0** |

The 132-finding case improved 2.5×, in three independent steps that were
measured separately:

| change | 132-finding `frame_done` |
|---|---|
| as found | 133.5 µs |
| + sorted-key presence diff (F2.3a) | 116.9 µs |
| + findings store numbers, format on report (F2.3b) | 55.6 µs |
| + findings buffer reused (F2.3b) | 53.7 µs |

## The four causes

1. **The intrinsic probe measured through `layout`** (F2.1), which writes
   `Label`'s memo — so the memo ended each frame keyed on the probe's
   constraints and missed on the next. Measured: 0 text measurements per
   idle frame with no agent, **400 of 400** with it, at 1k nodes. `Node`
   now has an observe-gated `probe` whose default *is* `layout`; `Label`
   overrides it to measure without touching the memo.

   This is the one with a consequence beyond cost. `BlockMetrics` is
   arithmetic, so the wasted work was cheap; under the real shaper M7
   adopts it is the application's entire shaping cost, three times per
   frame. **Any shaping measurement taken with an agent attached before
   this fix was wrong.**

2. **`measure()` allocated three collections per call** (F2.2) — two
   `Vec`s and a fresh `DisplayList`. Retained on the `Journal` and
   swapped with the baseline.

3. **A node was signed by calling `paint()` on it** (F2.2), which for a
   container repaints its whole subtree: O(N·depth), 2.1–2.8× N commands
   per frame. Leaves paint; containers *combine* their children's
   signatures. The table is DFS pre-order, so one reverse pass suffices.
   `child_count == 0` is the "paints its own content" test, deliberately
   true for the arena containers, whose window and clip belong to their
   own range.

4. **The presence diff was O(previous × found)** and every `Finding`
   eagerly built a `String` and a `serde_json::Value` (F2.3). 132
   findings meant ~17,000 comparisons and ~264 allocations per frame, to
   discover that nothing had changed. The diff goes through sorted key
   buffers; findings store numbers in a `Copy` enum and format only when
   `ui.lint` reports them or they cross an open/close transition.

## Verdict

**The audit is now inert with respect to the frame** —
`crates/fulgurite-agent/tests/audit_is_inert.rs` asserts an idle frame
measures nothing before and after `frame_done`, ten times over, and that
a real change still re-measures exactly one label. Those three tests fail
on the parent commit with 2 measurements where 0 is correct.

**`frame_done` is 6–7× the frame, not 117×.** That is still a real cost
and it is the right shape for a dev surface: it is off in release
(`observe`), and what remains is dominated by the intrinsic probe's two
measurements per leaf — which M2 could retire entirely, since `Shrink`
results *are* the intrinsic sizes. The reason it has not been is that the
truncation lint needs the *unclamped* natural size, which a laid-out
`Shrink` node no longer carries once its parent has squeezed it. That is
the next thing to attack here, and it is worth roughly the remaining
gap.

**Threshold for a regression:** zero allocations per `frame_done` in
steady state, either viewport. That is an integer, it is
machine-independent, and `benches --bin agent` prints it. The µs figures
are not portable and should not be gated on.

**It is asserted since M48/T48.8** (R14-F4), by
`crates/fulgurite-agent/tests/alloc_frame_done.rs` in `just test`,
and **the table's "0" above had stopped being true**: the bench printed
4.00 per call and nothing read it. `live_nodes` built an identity `Vec`
every call and the journal and the auditor each built their own `live`
mask. At T48.0 the test read `allocs/frame_done 4.04 / bound 0` at 911
nodes, with 0 and with 162 findings; now `0.00 / bound 0` both ways,
counted over 100 calls after 300 warm ones (past the journal's
256-frame ring, so the counted frames are its steady state). The mask
and the table's subtree sizes are scratch on `Agent`, the sizes computed
once per table.

## No endpoint, no journal — M48/T48.12

An agent build ran `frame_done` — the journal's diff and the audit, the
cost measured above — on **every frame of every window**, whether or not
an address was set (R14-F8); `Config::agent`'s `None` was inert only
without the feature. The window now journals only when an endpoint was
configured. Counted by `crates/fulgurite-winit/tests/agent_transport.rs`
over a spinning window's first three frames:

| | journaled / frames |
|---|---|
| no address (was 3 / 3 with the check dropped) | **0 / 3** |
| an address | 3 / 3 |

And a build without the feature links none of it: `just agent-symbols`
counts 0 `fulgurite_agent` and 0 `serde_json` symbols in a release
showcase, against 66 and 192 with `agent` (the positive control).
