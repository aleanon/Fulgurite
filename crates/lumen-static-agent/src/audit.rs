//! Ambient per-frame audit — parity plan P3.
//!
//! Lumen's own 2026-08 review names the gap this closes: its agent
//! surface is *interrogative* — you must already suspect a node and name
//! it — while human perception is **ambient** (the whole frame at once),
//! **push** (the anomaly announces itself), and **hypothesis-free**. So
//! the audit runs at every frame boundary and volunteers what it finds.
//!
//! Two mechanisms, split on principle (Lumen's organizing idea):
//!
//! | | `ui.lint` findings | `app.logs` entries |
//! |---|---|---|
//! | shape | conditions **currently true** | **transitions** (opened/closed) |
//! | access | pull, cached from the last frame | push, paged with `since` |
//!
//! Three scars from Lumen's rev2 plan, adopted rather than re-earned:
//!
//! 1. **Findings are keyed `(code, node)` structurally** — never by
//!    message text. Lumen shipped `.with_node()` with zero call sites, so
//!    its dedup key degenerated to `(code, None)` and swallowed every
//!    finding after the first per code.
//! 2. **Dedup is a per-frame presence diff**, not a monotonic seen-set: a
//!    finding that is fixed and then re-broken must report again.
//! 3. **The audit must not repeat expensive work.** Lumen's `lint()` was
//!    re-shaping every text node and deep-cloning the tree per call. Here
//!    nothing allocates a snapshot at frame time; the probe below reuses
//!    the existing `up` op and a scratch buffer that stops reallocating
//!    after the first frame.
//!
//! ## The intrinsic-size probe
//!
//! Several findings need to know what a node *wants* to be, which the
//! laid-out state cannot show. Rather than add vocabulary to every
//! widget, the audit asks the existing measure protocol twice, under two
//! different generous constraints:
//!
//! - equal answers ⇒ the node is **intrinsic** (a label sized by its
//!   text): the answer is its natural size;
//! - growing answers ⇒ the node is **filler** (a spacer): it has no
//!   natural size, and findings about "content that doesn't fit" do not
//!   apply to it.
//!
//! That distinction is what keeps a legitimately collapsed spacer from
//! being reported as invisible content — the class of false positive
//! Lumen's review warned would otherwise be the first thing an agent ever
//! sees.
//!
//! Probing is restricted to leaves: a container's `up` reads its
//! children's already-computed sizes out of the state array, so
//! re-measuring one under different constraints would not mean anything.

use std::collections::VecDeque;

use lumen_static::desc::{NodeState, NodeStats};
use lumen_static::widget::Node;
use lumen_static::{Constraints, Size};
use serde_json::{json, Value};

use crate::{TreeMut, TreeRef};

/// Two generous, *different* constraint sets: a node whose measurement is
/// the same under both is sized by its content, not by its container.
const PROBE_A: Constraints =
    Constraints { min_w: 0.0, max_w: 1.0e6, min_h: 0.0, max_h: 1.0e6 };
const PROBE_B: Constraints =
    Constraints { min_w: 0.0, max_w: 2.0e6, min_h: 0.0, max_h: 2.0e6 };

/// Slack before a size or overflow difference counts. Layout arithmetic
/// is f32; a finding that fires on rounding noise is worse than none.
const EPS: f32 = 0.5;

/// Frames of transitions kept for `app.logs`.
const LOG_CAPACITY: usize = 256;

/// Diagnostic codes. Reserved entries are documented rather than shipped
/// unreachable — Lumen's review found finished-but-uncallable analysis
/// sitting in its tree for months.
pub mod codes {
    /// Intrinsic content laid out with zero area: in the tree, absent on
    /// screen.
    pub const INVISIBLE: &str = "W1001";
    /// Non-empty node entirely outside the root's bounds.
    pub const OFFSCREEN: &str = "W1002";
    /// *Reserved:* a child's bounds escape its parent's. Unreachable in
    /// the current layout model and therefore not shipped: constraints
    /// flow down and every node clamps to them, so a child is never
    /// allotted more space than its slot (a container's own size clamps
    /// to `max`, and its children were already divided out of that same
    /// `max`). In this model "does not fit" surfaces as
    /// [`TRUNCATED`] instead — which is what the audit reports, and what
    /// `overflow_is_impossible_...` in `tests/lint.rs` pins.
    ///
    /// Ships with the first construct that can exceed its constraints: a
    /// scrollable content region, absolute positioning, or a minimum size
    /// larger than the parent's maximum.
    pub const _OVERFLOW_RESERVED: &str = "W1003";
    /// *Reserved:* sibling overlap. Structurally impossible while
    /// containers place children sequentially by measured size; ships
    /// with the first layout mode that can produce it (absolute/stack).
    pub const _OVERLAP_RESERVED: &str = "W1004";
    /// Intrinsic content allotted less room than it needs.
    pub const TRUNCATED: &str = "W1005";
    /// Nothing with area and content anywhere in the frame.
    pub const BLANK: &str = "W1006";
    /// A collection with items that occupies no space.
    pub const COLLAPSED_COLLECTION: &str = "W1007";
}

/// A condition that is true of the current frame.
pub(crate) struct Finding {
    pub code: &'static str,
    pub idx: u32,
    pub message: String,
    pub detail: Value,
}

impl Finding {
    /// The identity used for the presence diff: structural, never text.
    fn key(&self) -> (&'static str, u32) {
        (self.code, self.idx)
    }
}

/// A transition — the push half of the surface.
pub(crate) struct LogEntry {
    pub frame: u64,
    pub event: &'static str,
    pub code: &'static str,
    pub idx: u32,
    pub message: String,
}

#[derive(Default)]
pub(crate) struct Auditor {
    /// Findings true as of the last recorded frame; `ui.lint` reads this,
    /// so queries cost nothing and the work happens once per frame.
    current: Vec<Finding>,
    /// Scratch state array for the intrinsic probe. Kept across frames so
    /// it stops allocating after the first.
    scratch: Vec<NodeState>,
    log: VecDeque<LogEntry>,
    dropped: u64,
    seeded: bool,
}

/// What a leaf measured under the two probes.
struct Natural {
    intrinsic: bool,
    size: Size,
}

fn rect(st: &NodeState) -> [f32; 4] {
    [st.pos.x, st.pos.y, st.pos.x + st.size.w, st.pos.y + st.size.h]
}

fn area(st: &NodeState) -> f32 {
    st.size.w.max(0.0) * st.size.h.max(0.0)
}

impl Auditor {
    /// Run the audit for one frame and record transitions.
    ///
    /// `structural_reset` comes from the journal's fingerprint: after a
    /// hot reload the previous frame's node indices address different
    /// nodes, so the old finding set is discarded and everything true now
    /// re-announces itself. An agent that just swapped a dylib wants the
    /// current state of the world, not a diff against a vanished one.
    pub fn record(&mut self, frame: u64, tree: &mut TreeMut, structural_reset: bool) {
        let found = self.collect(tree);

        if structural_reset {
            self.current.clear();
            self.push_log(LogEntry {
                frame,
                event: "reset",
                code: "",
                idx: 0,
                message: "tree structure changed; findings re-evaluated".into(),
            });
        }

        // Presence diff (never a monotonic seen-set): closed first, so a
        // fix-then-re-break cycle reads in causal order.
        let previously = std::mem::take(&mut self.current);
        for old in &previously {
            if !found.iter().any(|f| f.key() == old.key()) {
                self.push_log(LogEntry {
                    frame,
                    event: "closed",
                    code: old.code,
                    idx: old.idx,
                    message: old.message.clone(),
                });
            }
        }
        for f in &found {
            if !previously.iter().any(|o| o.key() == f.key()) {
                self.push_log(LogEntry {
                    frame,
                    event: "opened",
                    code: f.code,
                    idx: f.idx,
                    message: f.message.clone(),
                });
            }
        }

        self.current = found;
        self.seeded = true;
    }

    pub fn current(&self) -> &[Finding] {
        &self.current
    }

    pub fn log_since(&self, since: u64) -> (Vec<&LogEntry>, bool) {
        let entries: Vec<&LogEntry> = self.log.iter().filter(|e| e.frame > since).collect();
        // The ring dropped entries the caller may still be owed.
        let truncated = self.dropped > 0
            && self.log.front().is_some_and(|e| since.saturating_add(1) < e.frame);
        (entries, truncated)
    }

    fn push_log(&mut self, e: LogEntry) {
        self.log.push_back(e);
        while self.log.len() > LOG_CAPACITY {
            self.log.pop_front();
            self.dropped += 1;
        }
    }

    /// Measure a leaf under both probes: same answer twice means the node
    /// is sized by its own content.
    ///
    /// Goes through [`Node::probe`], not `layout`: probing through
    /// `layout` left every `Label`'s memo keyed on `PROBE_B` and so
    /// forced a re-measure on every following frame — the scar this
    /// module's own docs claim to have learned from, re-earned
    /// (`docs/plan-review-fixes.md` F2.1).
    ///
    /// M2 could retire this entirely: `Shrink` results *are* the
    /// intrinsic sizes. Kept for now because the truncation lint needs
    /// the *unclamped* natural size, which a laid-out `Shrink` node no
    /// longer carries once its parent has squeezed it.
    fn natural(
        root: &mut dyn Node,
        styles: &[lumen_static::Style],
        scratch: &mut [NodeState],
        i: usize,
    ) -> Natural {
        // The probe measures with the built-in metrics; a host with a
        // real shaping stack gets slightly different natural sizes, which
        // is one more reason M2's `Shrink` results should replace this.
        let mut m = lumen_static::BlockMetrics;
        let mut cx = lumen_static::LayoutCx::new(&mut m);
        let a = root.node_mut(i).probe(&mut cx, PROBE_A, styles, scratch, i);
        let b = root.node_mut(i).probe(&mut cx, PROBE_B, styles, scratch, i);
        Natural { intrinsic: a == b, size: a }
    }

    fn stats(tree: &TreeRef, i: usize) -> NodeStats {
        let mut out = NodeStats::default();
        tree.root.node(i).stats(&mut out);
        out
    }

    fn collect(&mut self, tree: &mut TreeMut) -> Vec<Finding> {
        let mut out: Vec<Finding> = Vec::new();
        let st = tree.states;

        self.scratch.clear();
        self.scratch.extend_from_slice(st);

        let viewport = rect(&st[0]);
        let viewport_ok = area(&st[0]) > 0.0;
        let mut anything_visible = false;

        let table = tree.table;
        for (i, d) in table.iter().enumerate() {
            let leaf = d.child_count == 0 && !d.is_container();
            let a = area(&st[i]);

            // -- content-bearing checks, leaves only (see module docs) --
            if leaf {
                let nat = Self::natural(tree.root, tree.styles, &mut self.scratch, i);
                let has_content = nat.intrinsic && nat.size.w > 0.0 && nat.size.h > 0.0;

                if has_content && a > 0.0 {
                    anything_visible = true;
                }

                if has_content && a <= 0.0 {
                    out.push(Finding {
                        code: codes::INVISIBLE,
                        idx: i as u32,
                        message: format!(
                            "content wants {:.0}x{:.0} but was laid out {:.0}x{:.0} — \
                             present in the tree, absent on screen",
                            nat.size.w, nat.size.h, st[i].size.w, st[i].size.h
                        ),
                        detail: json!({
                            "natural": [nat.size.w, nat.size.h],
                            "size": [st[i].size.w, st[i].size.h],
                        }),
                    });
                } else if has_content
                    && (st[i].size.w + EPS < nat.size.w || st[i].size.h + EPS < nat.size.h)
                {
                    out.push(Finding {
                        code: codes::TRUNCATED,
                        idx: i as u32,
                        message: format!(
                            "content wants {:.0}x{:.0} but only got {:.0}x{:.0}",
                            nat.size.w, nat.size.h, st[i].size.w, st[i].size.h
                        ),
                        detail: json!({
                            "natural": [nat.size.w, nat.size.h],
                            "size": [st[i].size.w, st[i].size.h],
                            "missing": [
                                (nat.size.w - st[i].size.w).max(0.0),
                                (nat.size.h - st[i].size.h).max(0.0),
                            ],
                        }),
                    });
                }
            }

            // -- collections: geometry alone cannot tell an empty list
            //    from a broken one, so ask the widget (NodeStats) --
            let stats = Self::stats(&tree.as_ref(), i);
            if let Some(items) = stats.items {
                let shown = stats.materialized.unwrap_or(0);
                // Under the `Length` model a list *fills* its scrollport,
                // so zero height is no longer the signal — a list with
                // rows that materializes nothing is.
                if items > 0 && (shown == 0 || a <= 0.0) {
                    out.push(Finding {
                        code: codes::COLLAPSED_COLLECTION,
                        idx: i as u32,
                        message: format!(
                            "collection has {items} items but shows {shown} — \
                             check the item extent and the space it was given"
                        ),
                        detail: json!({
                            "items": items,
                            "materialized": shown,
                            "size": [st[i].size.w, st[i].size.h],
                        }),
                    });
                } else if items > 0 && a > 0.0 {
                    anything_visible = true;
                }
            }

            if i == 0 {
                continue;
            }

            // -- offscreen: skip when the root itself is degenerate, or
            //    W1006 would drag every node along with it --
            if viewport_ok && a > 0.0 {
                let r = rect(&st[i]);
                if r[2] <= viewport[0] || r[0] >= viewport[2] || r[3] <= viewport[1] || r[1] >= viewport[3]
                {
                    out.push(Finding {
                        code: codes::OFFSCREEN,
                        idx: i as u32,
                        message: format!(
                            "laid out at [{:.0},{:.0},{:.0},{:.0}], entirely outside the \
                             root's [{:.0},{:.0},{:.0},{:.0}]",
                            r[0], r[1], r[2], r[3],
                            viewport[0], viewport[1], viewport[2], viewport[3]
                        ),
                        detail: json!({ "bounds": r, "viewport": viewport }),
                    });
                }
            }

        }

        if !anything_visible {
            out.push(Finding {
                code: codes::BLANK,
                idx: 0,
                message: format!(
                    "nothing with both area and content in {} nodes — the frame is blank",
                    tree.table.len()
                ),
                detail: json!({ "nodes": tree.table.len() }),
            });
        }

        out
    }
}
