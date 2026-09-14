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
//!    re-shaping every text node and deep-cloning the tree per call.
//!
//!    This scar was re-earned here and is now closed, which is worth
//!    recording rather than quietly fixing. The intrinsic probe measured
//!    every leaf twice per frame *through `layout`*, which writes
//!    `Label`'s memo — so the memo ended each frame keyed on the probe's
//!    constraints and missed on the next one: 400 of 400 labels
//!    re-measured per frame at 1k nodes, in the file whose own doc says
//!    not to. The probe now goes through [`Node::probe`], which does not
//!    touch the memo, and every buffer here is retained, so a frame
//!    boundary allocates nothing whatever the finding count.
//!    `docs/results-agent-cost.md`, `docs/plan-review-fixes.md` F2.
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

/// How many of a container's children `W1004` compares pairwise.
///
/// The check is O(n²) in the children of **one** container, and a
/// container with hundreds of direct children is a `List` or a `Repeat`
/// — which place sequentially and cannot overlap anyway. Bounded rather
/// than unbounded so a pathological tree cannot make the audit the most
/// expensive thing in the frame; a container wider than this reports on
/// its first 32, which is where a layout bug will be visible if it is
/// visible at all.
const MAX_CHECKED_SIBLINGS: usize = 32;

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
    /// Two direct children of the same container overlap.
    ///
    /// **Shipped 2026-09-10 with `#[widget(stack)]`** (M11/T11.6), which
    /// is the layout mode this code was reserved against: it was
    /// "structurally impossible while containers place children
    /// sequentially by measured size", and a stack is the first mode
    /// that does not.
    ///
    /// A stack's children overlap *on purpose*, so it is exempt — by
    /// declaration, through [`lumen_static::widget::Node::overlaps_children`],
    /// not by inspecting geometry, which cannot tell an intended overlap
    /// from an accidental one. What is left is what the code was always
    /// for: a row or a column whose children collide, which means a
    /// child took more room than its slot.
    pub const OVERLAP: &str = "W1004";
    /// Intrinsic content allotted less room than it needs.
    pub const TRUNCATED: &str = "W1005";
    /// Nothing with area and content anywhere in the frame.
    pub const BLANK: &str = "W1006";
    /// A collection with items that occupies no space.
    pub const COLLAPSED_COLLECTION: &str = "W1007";
    /// A virtualizing collection nested inside a scroll region.
    ///
    /// A7 of `docs/text-and-scrolling-decisions.md`. `Repeat` derives
    /// its window from the constraint it is given; a scroller hands its
    /// content an unbounded main axis, so the repeater's window becomes
    /// "all of it" and a million-row list materializes every row —
    /// silently, and it is the first composition anyone writes. A
    /// `Repeat` *is* a scroller. `ScrollView::layout` debug-asserts
    /// against it; this is what a release build reports.
    pub const NESTED_VIRTUALIZER: &str = "W1008";
    /// A collection with more rows than its scroll offset can address.
    ///
    /// `Repeat` picks the first visible row with `scroll / item_extent`
    /// in `f32`, which represents consecutive integers exactly only to
    /// 2^24. Past `count * item_extent = 16,777,216` one ulp of the
    /// offset is wider than a row, so some rows have no offset that
    /// selects them — scrolling skips them, silently and worse the
    /// further down you are. `Repeat::MAX_ADDRESSABLE_EXTENT`.
    pub const UNADDRESSABLE_ROWS: &str = "W1009";
}

/// A condition that is true of the current frame.
pub(crate) struct Finding {
    pub code: &'static str,
    pub idx: u32,
    pub what: What,
}

/// What a finding found, as numbers.
///
/// Findings are built for every node every frame and *most are thrown
/// away*: a 900-node tree in a short viewport legitimately carries ~130
/// below-the-fold nodes, and building a `String` and a `serde_json::Value`
/// for each, every frame, to discover that nothing changed, was the
/// audit's largest single cost after the probe
/// (`docs/plan-review-fixes.md` F2.3). This is `Copy` and allocates
/// nothing; the text is written in `message()`/`detail()`, which run
/// only when a finding is reported by `ui.lint` or crosses an
/// open/close transition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum What {
    Invisible { natural: Size, size: Size },
    Truncated { natural: Size, size: Size },
    Collapsed { items: u64, shown: u32, size: Size },
    Offscreen { bounds: [f32; 4], viewport: [f32; 4] },
    Blank { nodes: usize },
    NestedVirtualizer { scroller: u32, items: u64 },
    UnaddressableRows { rows: u64, addressable: u64, extent: f32 },
    Overlap { a: u32, b: u32, over: [f32; 4] },
}

impl Finding {
    /// The identity used for the presence diff: structural, never text.
    fn key(&self) -> (&'static str, u32) {
        (self.code, self.idx)
    }

    pub fn message(&self) -> String {
        match self.what {
            What::Invisible { natural, size } => format!(
                "content wants {:.0}x{:.0} but was laid out {:.0}x{:.0} — \
                 present in the tree, absent on screen",
                natural.w, natural.h, size.w, size.h
            ),
            What::Truncated { natural, size } => format!(
                "content wants {:.0}x{:.0} but only got {:.0}x{:.0}",
                natural.w, natural.h, size.w, size.h
            ),
            What::Collapsed { items, shown, .. } => format!(
                "collection has {items} items but shows {shown} — \
                 check the item extent and the space it was given"
            ),
            What::Offscreen { bounds: r, viewport: v } => format!(
                "laid out at [{:.0},{:.0},{:.0},{:.0}], entirely outside the \
                 root's [{:.0},{:.0},{:.0},{:.0}]",
                r[0], r[1], r[2], r[3], v[0], v[1], v[2], v[3]
            ),
            What::Overlap { a, b, over } => format!(
                "children {a} and {b} overlap over [{:.0},{:.0},{:.0},{:.0}] — \
                 a sequential container's children should not collide",
                over[0], over[1], over[2], over[3]
            ),
            What::Blank { nodes } => format!(
                "nothing with both area and content in {nodes} nodes — the frame is blank"
            ),
            What::UnaddressableRows { rows, addressable, extent } => format!(
                "{rows} rows at {extent} px: only the first {addressable} have a scroll \
                 offset that selects them, because f32 holds consecutive integers to 2^24. \
                 The rest are unreachable by scrolling"
            ),
            What::NestedVirtualizer { scroller, items } => format!(
                "a virtualizing collection of {items} rows is inside the scroll region at \
                 node {scroller}: it will materialize every row. A Repeat is already a \
                 scroller — drop the ScrollView, or put a fixed-size subtree in it"
            ),
        }
    }

    pub fn detail(&self) -> Value {
        match self.what {
            What::Invisible { natural, size } => json!({
                "natural": [natural.w, natural.h],
                "size": [size.w, size.h],
            }),
            What::Truncated { natural, size } => json!({
                "natural": [natural.w, natural.h],
                "size": [size.w, size.h],
                "missing": [
                    (natural.w - size.w).max(0.0),
                    (natural.h - size.h).max(0.0),
                ],
            }),
            What::Overlap { a, b, over } => json!({
                "a": a,
                "b": b,
                "overlap": over,
                "area": ((over[2] - over[0]) * (over[3] - over[1])).max(0.0),
            }),
            What::Collapsed { items, shown, size } => json!({
                "items": items,
                "materialized": shown,
                "size": [size.w, size.h],
            }),
            What::Offscreen { bounds, viewport } => json!({
                "bounds": bounds, "viewport": viewport,
            }),
            What::Blank { nodes } => json!({ "nodes": nodes }),
            What::NestedVirtualizer { scroller, items } => {
                json!({ "scroller": scroller, "items": items })
            }
            What::UnaddressableRows { rows, addressable, extent } => {
                json!({ "rows": rows, "addressable": addressable, "item_extent": extent })
            }
        }
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

pub(crate) struct Auditor {
    /// Findings true as of the last recorded frame; `ui.lint` reads this,
    /// so queries cost nothing and the work happens once per frame.
    current: Vec<Finding>,
    /// What the intrinsic probe measures text with. The block stand-in
    /// unless the host installs its own — see `natural`.
    pub(crate) measure: Box<dyn lumen_static::TextMeasure>,
    /// Scratch state array for the intrinsic probe. Kept across frames so
    /// it stops allocating after the first.
    scratch: Vec<NodeState>,
    log: VecDeque<LogEntry>,
    dropped: u64,
    seeded: bool,
    /// Sorted finding keys for this frame and the previous one, retained
    /// so the presence diff allocates nothing. See `record`.
    now_keys: Vec<(&'static str, u32)>,
    was_keys: Vec<(&'static str, u32)>,
    /// The findings buffer not currently held by `current`.
    spare: Vec<Finding>,
}

impl Default for Auditor {
    fn default() -> Self {
        Auditor {
            current: Vec::new(),
            measure: Box::new(lumen_static::BlockMetrics),
            scratch: Vec::new(),
            log: VecDeque::new(),
            dropped: 0,
            seeded: false,
            now_keys: Vec::new(),
            was_keys: Vec::new(),
            spare: Vec::new(),
        }
    }
}

/// Rows a virtualizer holds that no scroll offset can select.
///
/// Asks the node for its row count and extent through the same
/// `NodeStats`/`Node` surface everything else here uses, so a widget
/// that starts virtualizing is covered without touching this.
fn unaddressable(tree: &TreeRef, i: usize) -> Option<(u64, u64, f32)> {
    let node = tree.root.node(i);
    if !node.virtualizes() {
        return None;
    }
    let mut s = lumen_static::desc::NodeStats::default();
    node.stats(&mut s);
    let rows = s.items?;
    let extent = s.item_extent?;
    if !(extent.is_finite() && extent > 0.0) {
        return None;
    }
    let addressable =
        (lumen_static::repeat::MAX_ADDRESSABLE_EXTENT / extent) as u64;
    (rows > addressable).then_some((rows, addressable, extent))
}

/// The nearest ancestor of `i` that is a scroll region, if any.
///
/// By kind name rather than by a flag, because that is what the
/// selector vocabulary already uses and what a finding can name back to
/// the author.
fn scrolling_ancestor(
    tree: &TreeRef,
    table: &[lumen_static::desc::NodeDesc],
    i: usize,
) -> Option<usize> {
    let mut p = table[i].parent;
    while p != lumen_static::desc::NO_PARENT {
        let k = p as usize;
        if tree.root.node(k).kind_name() == "scrollview" {
            return Some(k);
        }
        p = table[k].parent;
    }
    None
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
        // `found` is the buffer the previous frame's findings vacated:
        // the two alternate, so a frame with findings allocates nothing
        // once both have grown.
        let mut found = std::mem::take(&mut self.spare);
        self.collect(tree, &mut found);

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
        //
        // Membership goes through sorted key buffers rather than a
        // nested `any`: the old form was O(previous x found), and a
        // 1k-node tree in a short viewport legitimately carries ~200
        // below-the-fold findings, so it ran 40,000 comparisons every
        // frame to discover that nothing had changed
        // (`docs/plan-review-fixes.md` F2.3). The buffers are retained,
        // so the diff allocates nothing in steady state.
        let previously = std::mem::take(&mut self.current);
        let (mut now_keys, mut was_keys) =
            (std::mem::take(&mut self.now_keys), std::mem::take(&mut self.was_keys));
        now_keys.clear();
        now_keys.extend(found.iter().map(Finding::key));
        now_keys.sort_unstable();
        was_keys.clear();
        was_keys.extend(previously.iter().map(Finding::key));
        was_keys.sort_unstable();

        for old in &previously {
            if now_keys.binary_search(&old.key()).is_err() {
                self.push_log(LogEntry {
                    frame,
                    event: "closed",
                    code: old.code,
                    idx: old.idx,
                    message: old.message(),
                });
            }
        }
        for f in &found {
            if was_keys.binary_search(&f.key()).is_err() {
                self.push_log(LogEntry {
                    frame,
                    event: "opened",
                    code: f.code,
                    idx: f.idx,
                    message: f.message(),
                });
            }
        }

        self.now_keys = now_keys;
        self.was_keys = was_keys;
        self.spare = previously;
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
        measure: &mut dyn lumen_static::TextMeasure,
        i: usize,
    ) -> Natural {
        // Measured with the host's own text stack (`Agent::
        // set_text_measure`), because the question is what *this*
        // frame's layout would have wanted. Measuring with the block
        // stand-in while the window laid out with a real face reported
        // every button of the live tally as truncated — the block's
        // 0.5 em advance is wider than any real digit.
        let mut cx = lumen_static::LayoutCx::new(measure);
        let a = root.node_mut(i).probe(&mut cx, PROBE_A, styles, scratch, i);
        let b = root.node_mut(i).probe(&mut cx, PROBE_B, styles, scratch, i);
        Natural { intrinsic: a == b, size: a }
    }

    fn stats(tree: &TreeRef, i: usize) -> NodeStats {
        let mut out = NodeStats::default();
        tree.root.node(i).stats(&mut out);
        out
    }

    fn collect(&mut self, tree: &mut TreeMut, out: &mut Vec<Finding>) {
        out.clear();
        let st = tree.states;

        self.scratch.clear();
        self.scratch.extend_from_slice(st);

        let viewport = rect(&st[0]);
        let viewport_ok = area(&st[0]) > 0.0;
        let mut anything_visible = false;
        // Content the audit cannot see into. A node that owns an arena
        // keeps its children off the table, so `collect`'s walk never
        // examines them — and asserting "the frame is blank" on the
        // strength of a walk that could not look is exactly the false
        // positive this module's own docs call "the first thing an agent
        // ever sees". `List` escaped it only by accident, because it
        // implements `stats()`; `Either` and `Dyn` did not
        // (`docs/plan-review-fixes.md` F4.4).
        let mut opaque_content = false;

        let table = tree.table;
        let mut live = Vec::new();
        crate::live_nodes(tree.as_ref().root, table, &mut live);
        for (i, d) in table.iter().enumerate() {
            let leaf = d.child_count == 0 && !d.is_container();
            let a = area(&st[i]);

            if a > 0.0 && tree.as_ref().root.node(i).arena_count() > 0 {
                opaque_content = true;
            }

            // -- content-bearing checks, leaves only (see module docs) --
            // Only leaves `node_mut(i)` actually answers for. For an
            // index inside a branch that is not laid out it answers with
            // the enclosing container, and probing that at `i` lays its
            // children out from `i` and off the end of `scratch` — see
            // `crate::live_nodes`. Note this is *not* "has area": a leaf
            // with content and no room is `W1001`, which must still
            // fire, and gating on area turned that lint off.
            if leaf && live[i] {
                let nat =
                    Self::natural(tree.root, tree.styles, &mut self.scratch, &mut *self.measure, i);
                let has_content = nat.intrinsic && nat.size.w > 0.0 && nat.size.h > 0.0;

                if has_content && a > 0.0 {
                    anything_visible = true;
                }

                if has_content && a <= 0.0 {
                    out.push(Finding {
                        code: codes::INVISIBLE,
                        idx: i as u32,
                        what: What::Invisible { natural: nat.size, size: st[i].size },
                    });
                } else if has_content
                    && (st[i].size.w + EPS < nat.size.w || st[i].size.h + EPS < nat.size.h)
                {
                    out.push(Finding {
                        code: codes::TRUNCATED,
                        idx: i as u32,
                        what: What::Truncated { natural: nat.size, size: st[i].size },
                    });
                }
            }

            // -- the f32 scroll ceiling (`Repeat::MAX_ADDRESSABLE_EXTENT`) --
            if let Some((rows, addressable, extent)) = unaddressable(&tree.as_ref(), i) {
                out.push(Finding {
                    code: codes::UNADDRESSABLE_ROWS,
                    idx: i as u32,
                    what: What::UnaddressableRows { rows, addressable, extent },
                });
            }

            // -- W1004: two children of a sequential container collide --
            //
            // Shipped with `#[widget(stack)]` (M11/T11.6), the layout
            // mode this code was reserved against. A stack is exempt by
            // **declaration**, not by geometry: overlapping children are
            // what it is for, and a code that fires on every stack is
            // noise rather than a diagnostic.
            //
            // Direct children only. A grandchild escaping its parent is
            // W1003's business and is still unreachable; this is about
            // siblings that were each given a slot and did not stay in
            // it.
            if d.child_count > 1 && !tree.as_ref().root.node(i).overlaps_children() {
                let mut kids = [0u32; MAX_CHECKED_SIBLINGS];
                let mut k = 0usize;
                let mut c = d.first_child as usize;
                while k < kids.len() && c != 0 && c < table.len() {
                    if area(&st[c]) > 0.0 {
                        kids[k] = c as u32;
                        k += 1;
                    }
                    c = table[c].next_sibling as usize;
                }
                'pairs: for x in 0..k {
                    for y in (x + 1)..k {
                        let (ra, rb) = (rect(&st[kids[x] as usize]), rect(&st[kids[y] as usize]));
                        let over = [
                            ra[0].max(rb[0]),
                            ra[1].max(rb[1]),
                            ra[2].min(rb[2]),
                            ra[3].min(rb[3]),
                        ];
                        // Strictly greater: sequential placement puts a
                        // child's edge exactly on its neighbour's, and a
                        // shared edge is not an overlap. `EPS` because
                        // those edges are accumulated sums of f32.
                        if over[2] - over[0] > EPS && over[3] - over[1] > EPS {
                            out.push(Finding {
                                code: codes::OVERLAP,
                                idx: i as u32,
                                what: What::Overlap { a: kids[x], b: kids[y], over },
                            });
                            // One finding per container: the pair is
                            // enough to send someone to the right node,
                            // and N children that all collide would
                            // otherwise report N*(N-1)/2 times.
                            break 'pairs;
                        }
                    }
                }
            }

            // -- A7: a virtualizer inside a scroll region --
            if tree.as_ref().root.node(i).virtualizes() {
                if let Some(scroller) = scrolling_ancestor(&tree.as_ref(), table, i) {
                    let mut s = lumen_static::desc::NodeStats::default();
                    tree.as_ref().root.node(i).stats(&mut s);
                    out.push(Finding {
                        code: codes::NESTED_VIRTUALIZER,
                        idx: i as u32,
                        what: What::NestedVirtualizer {
                            scroller: scroller as u32,
                            items: s.items.unwrap_or(0),
                        },
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
                        what: What::Collapsed { items, shown, size: st[i].size },
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
                        what: What::Offscreen { bounds: r, viewport },
                    });
                }
            }

        }

        if !anything_visible && !opaque_content {
            out.push(Finding {
                code: codes::BLANK,
                idx: 0,
                what: What::Blank { nodes: tree.table.len() },
            });
        }
    }
}
